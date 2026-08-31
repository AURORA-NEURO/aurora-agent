"""Bounded sync and async monitoring for the metadata-only brain job control plane.

The lower-level :mod:`prism_sdk.brain_api` client validates requests and transports typed job
operations.  This module adds the operator-facing supervision layer that is useful once a job
has been admitted: bounded status fan-out, hash-chain event cursors, explicit approval calls,
and restart-safe polling.  It deliberately accepts only the value-free projections returned by
the control plane.  Task text, prompts, credentials, provider responses, tool arguments, and
raw effect values are rejected before they can reach a monitor result.
"""

from __future__ import annotations

import asyncio
from concurrent.futures import ThreadPoolExecutor
import math
import re
import time
from typing import Any, Awaitable, Callable, Mapping, Sequence

from .brain_api import (
    BrainApprovalCommand,
    BrainControlError,
    BrainControlRefusal,
    BrainEventPageRequest,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES


AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA = (
    "bioprism-python-autonomous-brain-control-plane-monitor/0.1"
)
MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS = 60_000
MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS = 300_000
MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS = 256
MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS = 256

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_TERMINAL_STATES = frozenset(
    {"succeeded", "failed", "dead_lettered", "cancelled", "reconciliation_required"}
)
_SECRET_KEYS = frozenset(
    {
        "apikey",
        "bearer",
        "body",
        "content",
        "credential",
        "credentials",
        "headers",
        "messages",
        "password",
        "prompt",
        "request",
        "response",
        "secret",
        "task",
        "token",
    }
)
_BOUNDARIES = frozenset({"not_started", "preflight", "dispatched", "unknown"})


def _error(name: str, message: str) -> BrainControlError:
    return BrainControlError(f"{name} {message}")


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise _error(name, "must be a non-empty NUL-free string")
    if len(value.encode("utf-8")) > maximum:
        raise _error(name, "exceeds its bounded size")
    return value


def _identifier(name: str, value: Any) -> str:
    return _text(name, value, 256)


def _digest(name: str, value: Any, *, allow_empty: bool = False) -> str:
    if allow_empty and value == "":
        return ""
    value = _text(name, value, 64)
    if not _DIGEST.fullmatch(value):
        raise _error(name, "must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        raise _error(name, f"must be an integer within [{minimum}, {maximum}]")
    return value


def _positive(name: str, value: Any, maximum: int) -> int:
    return _integer(name, value, 1, maximum)


def _secret_free(value: Any, *, depth: int = 0) -> None:
    """Reject transient/secret-shaped metadata before returning a remote projection."""

    if depth > 8:
        raise _error("control-plane metadata", "nesting exceeds its bound")
    if isinstance(value, Mapping):
        if len(value) > 256:
            raise _error("control-plane metadata", "mapping exceeds its bound")
        for key, child in value.items():
            if not isinstance(key, str):
                raise _error("control-plane metadata", "contains a non-string key")
            normalized = re.sub(r"[^a-z0-9]", "", key.lower())
            if normalized in _SECRET_KEYS:
                raise _error("control-plane metadata", "contains transient or secret-shaped fields")
            _secret_free(child, depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 256:
            raise _error("control-plane metadata", "array exceeds its bound")
        for child in value:
            _secret_free(child, depth=depth + 1)
        return
    if isinstance(value, (bytes, bytearray)):
        raise _error("control-plane metadata", "contains unsupported binary material")
    if isinstance(value, float) and not math.isfinite(value):
        raise _error("control-plane metadata", "contains a non-finite number")
    if isinstance(value, str) and len(value.encode("utf-8")) > 4_096:
        raise _error("control-plane metadata", "contains an oversized text value")


def _projection(operation: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise _error(operation, "returned a non-object projection")
    result = dict(value)
    _secret_free(result)
    if result.get("ok") is False:
        raise BrainControlRefusal(operation, result)
    if result.get("ok") is not True:
        raise _error(operation, "returned no successful control-plane marker")
    return result


def _validate_job(value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise _error("control-plane job", "projection is malformed")
    job = dict(value)
    _identifier("control-plane job_id", job.get("job_id"))
    _digest("control-plane spec_digest", job.get("spec_digest"))
    if job.get("domain") not in AUTONOMOUS_DOMAIN_NAMES:
        raise _error("control-plane job domain", "is unsupported")
    _identifier("control-plane capability", job.get("capability"))
    _identifier("control-plane risk_class", job.get("risk_class"))
    state = _text("control-plane job state", job.get("state"), 128)
    attempts = _integer("control-plane job attempts", job.get("attempts"), 0, 8)
    max_attempts = _positive("control-plane job max_attempts", job.get("max_attempts"), 8)
    if attempts > max_attempts:
        raise _error("control-plane job attempts", "exceed its max_attempts")
    if job.get("side_effect_boundary") not in _BOUNDARIES:
        raise _error("control-plane job side_effect_boundary", "is invalid")
    if not isinstance(job.get("recovered_after_restart"), bool):
        raise _error("control-plane job recovered_after_restart", "must be boolean")
    for key in (
        "idempotency_key_digest",
        "checkpoint_digest",
        "reason_digest",
        "result_digest",
        "reconciliation_digest",
        "record_digest",
    ):
        if job.get(key) is not None:
            _digest(f"control-plane job {key}", job[key])
    for key in ("lease_expires_ns", "created_sequence", "updated_sequence"):
        if job.get(key) is not None:
            _integer(f"control-plane job {key}", job[key], 0, 2**63 - 1)
    return job


def _validate_status(value: Any) -> dict[str, Any]:
    result = _projection("brain_job_status", value)
    result["job"] = _validate_job(result.get("job"))
    _digest("control-plane head_digest", result.get("head_digest"), allow_empty=True)
    return result


def _validate_event_page(value: Any, *, job_id: str | None, after: int, limit: int) -> dict[str, Any]:
    result = _projection("brain_job_events", value)
    events = result.get("events")
    if isinstance(events, (str, bytes)) or not isinstance(events, Sequence) or len(events) > limit:
        raise _error("control-plane events", "projection is outside its page bound")
    if events and not isinstance(events[-1], Mapping):
        raise _error("control-plane event", "row is malformed")
    if _integer("control-plane event after", result.get("after"), 0, 2**63 - 1) != after:
        raise _error("control-plane event after", "cursor was not honored")
    next_after = _integer("control-plane event next_after", result.get("next_after"), 0, 2**63 - 1)
    expected_next_after = after if not events else events[-1].get("sequence")
    if next_after != expected_next_after or next_after < after:
        raise _error("control-plane event next_after", "cursor advanced inconsistently")
    _digest("control-plane event head_digest", result.get("head_digest"), allow_empty=True)
    previous_sequence = after
    previous_digest = ""
    normalized_events: list[dict[str, Any]] = []
    for raw in events:
        if not isinstance(raw, Mapping):
            raise _error("control-plane event", "row is malformed")
        event = dict(raw)
        sequence = _integer("control-plane event sequence", event.get("sequence"), 1, 2**63 - 1)
        if sequence <= previous_sequence:
            raise _error("control-plane event sequence", "is not strictly increasing")
        event_job_id = _identifier("control-plane event job_id", event.get("job_id"))
        if job_id is not None and event_job_id != job_id:
            raise _error("control-plane event job_id", "does not match the requested job")
        _text("control-plane event event_type", event.get("event_type"), 128)
        if not isinstance(event.get("payload"), Mapping):
            raise _error("control-plane event payload", "must be an object")
        _digest("control-plane event event_digest", event.get("event_digest"))
        event_previous = _digest(
            "control-plane event previous_digest", event.get("previous_digest"), allow_empty=True
        )
        if sequence == previous_sequence + 1 and event_previous != previous_digest:
            raise _error("control-plane event previous_digest", "does not match its predecessor")
        if event.get("head_digest") is not None:
            head = _digest("control-plane event head_digest", event["head_digest"])
            if head != event["event_digest"]:
                raise _error("control-plane event head_digest", "does not match event_digest")
        if event.get("payload_digest") is not None:
            _digest("control-plane event payload_digest", event["payload_digest"])
        _integer("control-plane event created_ns", event.get("created_ns"), 0, 2**63 - 1)
        normalized_events.append(event)
        previous_sequence = sequence
        previous_digest = event["event_digest"]
    result["events"] = normalized_events
    return result


def _validate_approval(value: Any) -> dict[str, Any]:
    result = _projection("brain_job_approval", value)
    result["job"] = _validate_job(result.get("job"))
    event = result.get("event")
    if event is not None and not isinstance(event, Mapping):
        raise _error("control-plane approval event", "projection is malformed")
    return result


def _normalize_job_ids(job_ids: Sequence[str]) -> tuple[str, ...]:
    if isinstance(job_ids, (str, bytes)) or not isinstance(job_ids, Sequence):
        raise _error("control-plane status_all job_ids", "must be a sequence")
    if not 1 <= len(job_ids) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise _error("control-plane status_all job_ids", "must contain between one and twelve ids")
    normalized = tuple(_identifier("control-plane status_all job_id", job_id) for job_id in job_ids)
    if len(set(normalized)) != len(normalized):
        raise _error("control-plane status_all job_ids", "must be unique")
    return normalized


def _normalize_until(until: Sequence[str] | None) -> frozenset[str]:
    if until is None:
        return frozenset()
    if isinstance(until, (str, bytes)) or not isinstance(until, Sequence) or len(until) > 32:
        raise _error("control-plane wait until", "must be a bounded sequence")
    return frozenset(_text("control-plane wait target state", item, 128) for item in until)


def _result(
    **values: Any,
) -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA,
        **values,
        "retention": "metadata_only_control_plane_projection",
        "secret_material": "never_returned",
    }


class AutonomousBrainControlPlaneMonitor:
    """Observe and approve remote brain jobs without becoming a provider worker."""

    def __init__(
        self,
        client: Any,
        *,
        clock: Callable[[], float] | None = None,
        sleep: Callable[[int], None] | None = None,
    ) -> None:
        if client is None or not all(
            callable(getattr(client, name, None)) for name in ("job_status", "job_events", "approval")
        ):
            raise BrainControlError(
                "brain control-plane monitor requires job_status, job_events, and approval methods"
            )
        if clock is not None and not callable(clock):
            raise BrainControlError("brain control-plane clock must be callable")
        if sleep is not None and not callable(sleep):
            raise BrainControlError("brain control-plane sleep must be callable")
        self.client = client
        self.clock = clock or (lambda: time.monotonic() * 1_000)
        self.sleep = sleep or (lambda milliseconds: time.sleep(milliseconds / 1_000.0))

    def status(self, job_id: str) -> dict[str, Any]:
        normalized = _identifier("brain control-plane job_id", job_id)
        return _result(status=_validate_status(self.client.job_status(normalized)))

    def events(self, job_id: str | None = None, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        normalized = None if job_id is None else _identifier("brain control-plane job_id", job_id)
        bounded_after = _integer("brain control-plane after", after, 0, 2**63 - 1)
        bounded_limit = _positive("brain control-plane event limit", limit, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS)
        request = BrainEventPageRequest(job_id=normalized, after=bounded_after, limit=bounded_limit)
        page = _validate_event_page(
            self.client.job_events(request), job_id=normalized, after=bounded_after, limit=bounded_limit
        )
        return _result(events=page)

    def approval(
        self,
        job_id: str,
        action: str,
        *,
        reason: str | None = None,
        authorization_digest: str | None = None,
    ) -> dict[str, Any]:
        request = BrainApprovalCommand(
            job_id=_identifier("brain control-plane job_id", job_id),
            action=action,
            reason=reason,
            authorization_digest=authorization_digest,
        )
        return _result(approval=_validate_approval(self.client.approval(request)))

    def status_all(
        self, job_ids: Sequence[str], *, max_parallel: int = 4
    ) -> dict[str, Any]:
        normalized = _normalize_job_ids(job_ids)
        bounded_parallel = _positive(
            "brain control-plane max_parallel", max_parallel, len(AUTONOMOUS_DOMAIN_NAMES)
        )
        with ThreadPoolExecutor(max_workers=bounded_parallel) as executor:
            statuses = list(executor.map(self.status, normalized))
        return _result(
            status="completed",
            jobs=[item["status"] for item in statuses],
            domains=[item["status"]["job"]["domain"] for item in statuses],
            max_parallel=bounded_parallel,
        )

    def wait(
        self,
        job_id: str,
        *,
        until: Sequence[str] | None = None,
        timeout_ms: int = MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS,
        poll_ms: int = 1_000,
        max_polls: int | None = None,
        event_limit: int = 100,
        after_event: int = 0,
    ) -> dict[str, Any]:
        normalized = _identifier("brain control-plane job_id", job_id)
        timeout = _positive("brain control-plane timeout_ms", timeout_ms, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS)
        poll = _positive("brain control-plane poll_ms", poll_ms, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS)
        polls_limit = (
            min(MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS, math.ceil(timeout / poll) + 1)
            if max_polls is None
            else _positive("brain control-plane max_polls", max_polls, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS)
        )
        bounded_event_limit = _positive(
            "brain control-plane event_limit", event_limit, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS
        )
        cursor = _integer("brain control-plane after_event", after_event, 0, 2**63 - 1)
        targets = _normalize_until(until)
        started = self.clock()
        collected: dict[int, dict[str, Any]] = {}
        latest: dict[str, Any] | None = None
        polls = 0
        for _ in range(polls_limit):
            polls += 1
            latest = self.status(normalized)["status"]["job"]
            page = self.events(normalized, after=cursor, limit=bounded_event_limit)["events"]
            cursor = page["next_after"]
            for event in page["events"]:
                collected[event["sequence"]] = event
            if latest["state"] in targets or (not targets and latest["state"] in _TERMINAL_STATES):
                return _result(
                    status="reached",
                    job_id=normalized,
                    terminal_state=latest["state"],
                    job=latest,
                    events=[collected[key] for key in sorted(collected)],
                    event_cursor=cursor,
                    polls=polls,
                    elapsed_ms=max(0, int(self.clock() - started)),
                )
            if self.clock() - started >= timeout:
                break
            self.sleep(poll)
        if latest is None:
            raise BrainControlError("brain control-plane wait ended without a status projection")
        return _result(
            status="timed_out",
            job_id=normalized,
            terminal_state=latest["state"],
            job=latest,
            events=[collected[key] for key in sorted(collected)],
            event_cursor=cursor,
            polls=polls,
            elapsed_ms=max(0, int(self.clock() - started)),
        )


class AsyncAutonomousBrainControlPlaneMonitor:
    """Async counterpart for :class:`AutonomousBrainControlPlaneMonitor`."""

    def __init__(
        self,
        client: Any,
        *,
        clock: Callable[[], float] | None = None,
        sleep: Callable[[int], Awaitable[None]] | None = None,
    ) -> None:
        if client is None or not all(
            callable(getattr(client, name, None)) for name in ("job_status", "job_events", "approval")
        ):
            raise BrainControlError(
                "async brain control-plane monitor requires job_status, job_events, and approval methods"
            )
        if clock is not None and not callable(clock):
            raise BrainControlError("async brain control-plane clock must be callable")
        if sleep is not None and not callable(sleep):
            raise BrainControlError("async brain control-plane sleep must be callable")
        self.client = client
        self.clock = clock or (lambda: time.monotonic() * 1_000)
        self.sleep = sleep or (lambda milliseconds: asyncio.sleep(milliseconds / 1_000.0))

    async def status(self, job_id: str) -> dict[str, Any]:
        normalized = _identifier("brain control-plane job_id", job_id)
        return _result(status=_validate_status(await self.client.job_status(normalized)))

    async def events(self, job_id: str | None = None, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        normalized = None if job_id is None else _identifier("brain control-plane job_id", job_id)
        bounded_after = _integer("brain control-plane after", after, 0, 2**63 - 1)
        bounded_limit = _positive("brain control-plane event limit", limit, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS)
        request = BrainEventPageRequest(job_id=normalized, after=bounded_after, limit=bounded_limit)
        page = _validate_event_page(
            await self.client.job_events(request), job_id=normalized, after=bounded_after, limit=bounded_limit
        )
        return _result(events=page)

    async def approval(
        self,
        job_id: str,
        action: str,
        *,
        reason: str | None = None,
        authorization_digest: str | None = None,
    ) -> dict[str, Any]:
        request = BrainApprovalCommand(
            job_id=_identifier("brain control-plane job_id", job_id),
            action=action,
            reason=reason,
            authorization_digest=authorization_digest,
        )
        return _result(approval=_validate_approval(await self.client.approval(request)))

    async def status_all(
        self, job_ids: Sequence[str], *, max_parallel: int = 4
    ) -> dict[str, Any]:
        normalized = _normalize_job_ids(job_ids)
        bounded_parallel = _positive(
            "brain control-plane max_parallel", max_parallel, len(AUTONOMOUS_DOMAIN_NAMES)
        )
        semaphore = asyncio.Semaphore(bounded_parallel)

        async def read(job_id: str) -> dict[str, Any]:
            async with semaphore:
                return await self.status(job_id)

        statuses = list(await asyncio.gather(*(read(job_id) for job_id in normalized)))
        return _result(
            status="completed",
            jobs=[item["status"] for item in statuses],
            domains=[item["status"]["job"]["domain"] for item in statuses],
            max_parallel=bounded_parallel,
        )

    async def wait(
        self,
        job_id: str,
        *,
        until: Sequence[str] | None = None,
        timeout_ms: int = MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS,
        poll_ms: int = 1_000,
        max_polls: int | None = None,
        event_limit: int = 100,
        after_event: int = 0,
    ) -> dict[str, Any]:
        normalized = _identifier("brain control-plane job_id", job_id)
        timeout = _positive("brain control-plane timeout_ms", timeout_ms, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS)
        poll = _positive("brain control-plane poll_ms", poll_ms, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS)
        polls_limit = (
            min(MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS, math.ceil(timeout / poll) + 1)
            if max_polls is None
            else _positive("brain control-plane max_polls", max_polls, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS)
        )
        bounded_event_limit = _positive(
            "brain control-plane event_limit", event_limit, MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS
        )
        cursor = _integer("brain control-plane after_event", after_event, 0, 2**63 - 1)
        targets = _normalize_until(until)
        started = self.clock()
        collected: dict[int, dict[str, Any]] = {}
        latest: dict[str, Any] | None = None
        polls = 0
        for _ in range(polls_limit):
            polls += 1
            latest = (await self.status(normalized))["status"]["job"]
            page = (await self.events(normalized, after=cursor, limit=bounded_event_limit))["events"]
            cursor = page["next_after"]
            for event in page["events"]:
                collected[event["sequence"]] = event
            if latest["state"] in targets or (not targets and latest["state"] in _TERMINAL_STATES):
                return _result(
                    status="reached",
                    job_id=normalized,
                    terminal_state=latest["state"],
                    job=latest,
                    events=[collected[key] for key in sorted(collected)],
                    event_cursor=cursor,
                    polls=polls,
                    elapsed_ms=max(0, int(self.clock() - started)),
                )
            if self.clock() - started >= timeout:
                break
            await self.sleep(poll)
        if latest is None:
            raise BrainControlError("async brain control-plane wait ended without a status projection")
        return _result(
            status="timed_out",
            job_id=normalized,
            terminal_state=latest["state"],
            job=latest,
            events=[collected[key] for key in sorted(collected)],
            event_cursor=cursor,
            polls=polls,
            elapsed_ms=max(0, int(self.clock() - started)),
        )


__all__ = [
    "AUTONOMOUS_BRAIN_CONTROL_PLANE_MONITOR_SCHEMA",
    "MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLL_MS",
    "MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_TIMEOUT_MS",
    "MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_POLLS",
    "MAX_AUTONOMOUS_BRAIN_CONTROL_PLANE_EVENTS",
    "AutonomousBrainControlPlaneMonitor",
    "AsyncAutonomousBrainControlPlaneMonitor",
]
