"""Metadata-only, hash-chained traces for autonomous brain runs.

The autonomous brain has several deliberately separate persistence systems: execution journals
enforce restart-safe policy, learning ledgers retain bounded reward evidence, and workflow
checkpoints retain resumable stage state.  This module is the observability boundary that ties
those decisions together without turning a trace into a prompt or result archive.

Only stable metadata is accepted:

* task, route, plan, selection, and provider outcome digests;
* domain, phase, status, attempt/turn, token, tool, latency, and failure counters; and
* a hash chain that makes append order and tampering detectable.

Callers retain the task, provider response, credentials, connector payloads, and learning state.
The high-level agent helpers return the live result alongside a trace summary, while every
``to_dict`` method in this module remains payload-free.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from pathlib import Path
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_RUN_TRACE_SCHEMA = "bioprism-python-autonomous-run-trace/0.1"
AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA = "bioprism-python-autonomous-run-trace-event/0.1"
AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-run-trace-snapshot/0.1"
AUTONOMOUS_RUN_TRACE_PHASES = (
    "started",
    "plan_compiled",
    "connector_started",
    "connector_finished",
    "provider_invocation_started",
    "provider_invocation_finished",
    "evaluation_settled",
    "learning_prepared",
    "completed",
    "paused",
    "refused",
    "failed",
)
AUTONOMOUS_RUN_TRACE_STATUSES = (
    "running",
    "completed",
    "partial",
    "paused",
    "refused",
    "failed",
    "unknown",
)
MAX_AUTONOMOUS_RUN_TRACE_EVENTS = 100_000
MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES = 16_000
MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES = 50_000_000
MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT = 10_000
AUTONOMOUS_RUN_TRACE_RETENTION = "metadata_only_no_prompts_responses_or_tool_payloads"
AUTONOMOUS_RUN_TRACE_SNAPSHOT_RETENTION = "metadata_only_hash_chained_no_prompts_responses_or_tool_payloads"
AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL = "never_returned"


def _validate_trace_limits(max_events: Any, max_bytes: Any) -> tuple[int, int]:
    if isinstance(max_events, bool) or not isinstance(max_events, int) or not 1 <= max_events <= MAX_AUTONOMOUS_RUN_TRACE_EVENTS:
        raise ArgumentError("autonomous run trace max_events is outside its bounds")
    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES <= max_bytes <= MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES:
        raise ArgumentError("autonomous run trace max_bytes is outside its bounds")
    return max_events, max_bytes


def _bounded_text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value or len(value) > maximum or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    text = _bounded_text(name, value)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in text):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return text


def _digest(name: str, value: Any, *, allow_none: bool = True) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _nonnegative_integer(name: str, value: Any, *, allow_none: bool = True) -> int | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _nullable_text(name: str, value: Any) -> str | None:
    return None if value is None else _bounded_text(name, value)


def _status_code(value: Any) -> int | None:
    if value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool) or not 100 <= value <= 599:
        raise ArgumentError("autonomous run trace status_code is invalid")
    return value


def _latency(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0 <= float(value) <= 86_400_000:
        raise ArgumentError("autonomous run trace latency_ms is invalid")
    return float(value)


def _domains(value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or not 1 <= len(value) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError(f"autonomous run trace domains must contain 1..={len(AUTONOMOUS_DOMAIN_NAMES)} entries")
    result = tuple(value)
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError("autonomous run trace contains an unsupported domain")
    if len(set(result)) != len(result):
        raise ArgumentError("autonomous run trace domains must be unique")
    return result


def _event_mapping(value: Mapping[str, Any] | "AutonomousRunTraceEvent") -> Mapping[str, Any]:
    if isinstance(value, AutonomousRunTraceEvent):
        return value.to_dict()
    if not isinstance(value, Mapping):
        raise ArgumentError("autonomous run trace event must be a mapping")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceEvent:
    schema: str
    sequence: int
    run_id: str
    task_digest: str
    domains: tuple[str, ...]
    phase: str
    status: str
    route_digest: str | None
    plan_digest: str | None
    selection_digest: str | None
    provider: str | None
    model: str | None
    attempt: int | None
    turn: int | None
    latency_ms: float | None
    input_tokens: int | None
    output_tokens: int | None
    tool_count: int | None
    status_code: int | None
    failure_class: str | None
    failure_code: str | None
    retryable: bool | None
    detail_digest: str | None
    recorded_at: int
    previous_digest: str
    event_digest: str
    retention: str = AUTONOMOUS_RUN_TRACE_RETENTION
    secret_material: str = AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL

    def to_dict(self, *, include_digest: bool = True) -> dict[str, Any]:
        result = {
            "schema": self.schema,
            "sequence": self.sequence,
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "domains": list(self.domains),
            "phase": self.phase,
            "status": self.status,
            "route_digest": self.route_digest,
            "plan_digest": self.plan_digest,
            "selection_digest": self.selection_digest,
            "provider": self.provider,
            "model": self.model,
            "attempt": self.attempt,
            "turn": self.turn,
            "latency_ms": self.latency_ms,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "tool_count": self.tool_count,
            "status_code": self.status_code,
            "failure_class": self.failure_class,
            "failure_code": self.failure_code,
            "retryable": self.retryable,
            "detail_digest": self.detail_digest,
            "recorded_at": self.recorded_at,
            "previous_digest": self.previous_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        if include_digest:
            result["event_digest"] = self.event_digest
        return result

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "AutonomousRunTraceEvent":
        if not isinstance(raw, Mapping):
            raise ArgumentError("autonomous run trace event must be a mapping")
        allowed = {
            "schema", "sequence", "run_id", "task_digest", "domains", "phase", "status",
            "route_digest", "plan_digest", "selection_digest", "provider", "model", "attempt",
            "turn", "latency_ms", "input_tokens", "output_tokens", "tool_count", "status_code",
            "failure_class", "failure_code", "retryable", "detail_digest", "recorded_at",
            "previous_digest", "event_digest", "retention", "secret_material",
        }
        if set(raw).difference(allowed):
            raise ArgumentError("autonomous run trace event contains unsupported fields")
        normalized = _normalize_event(raw, raw.get("sequence"), raw.get("previous_digest"), raw.get("recorded_at"))
        supplied = raw.get("event_digest")
        if not isinstance(supplied, str) or _event_digest(normalized) != supplied:
            raise ArgumentError("autonomous run trace event digest is invalid")
        return cls(**normalized, event_digest=supplied)


def _normalize_event(value: Mapping[str, Any], sequence: Any, previous_digest: Any, recorded_at: Any) -> dict[str, Any]:
    if not isinstance(sequence, int) or isinstance(sequence, bool) or sequence < 1:
        raise ArgumentError("autonomous run trace sequence is invalid")
    if not isinstance(recorded_at, int) or isinstance(recorded_at, bool) or recorded_at < 0:
        raise ArgumentError("autonomous run trace recorded_at is invalid")
    phase = value.get("phase")
    status = value.get("status")
    if phase not in AUTONOMOUS_RUN_TRACE_PHASES:
        raise ArgumentError("autonomous run trace phase is invalid")
    if status not in AUTONOMOUS_RUN_TRACE_STATUSES:
        raise ArgumentError("autonomous run trace status is invalid")
    retryable = value.get("retryable")
    if retryable is not None and not isinstance(retryable, bool):
        raise ArgumentError("autonomous run trace retryable must be boolean or null")
    previous = previous_digest
    if not isinstance(previous, str) or (previous and _digest("previous_digest", previous) is None):
        raise ArgumentError("autonomous run trace previous_digest is invalid")
    result = {
        "schema": AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA,
        "sequence": sequence,
        "run_id": _identifier("autonomous run trace run_id", value.get("run_id")),
        "task_digest": _digest("autonomous run trace task_digest", value.get("task_digest"), allow_none=False),
        "domains": list(_domains(value.get("domains"))),
        "phase": phase,
        "status": status,
        "route_digest": _digest("autonomous run trace route_digest", value.get("route_digest")),
        "plan_digest": _digest("autonomous run trace plan_digest", value.get("plan_digest")),
        "selection_digest": _digest("autonomous run trace selection_digest", value.get("selection_digest")),
        "provider": _nullable_text("autonomous run trace provider", value.get("provider")),
        "model": _nullable_text("autonomous run trace model", value.get("model")),
        "attempt": _nonnegative_integer("autonomous run trace attempt", value.get("attempt")),
        "turn": _nonnegative_integer("autonomous run trace turn", value.get("turn")),
        "latency_ms": _latency(value.get("latency_ms")),
        "input_tokens": _nonnegative_integer("autonomous run trace input_tokens", value.get("input_tokens")),
        "output_tokens": _nonnegative_integer("autonomous run trace output_tokens", value.get("output_tokens")),
        "tool_count": _nonnegative_integer("autonomous run trace tool_count", value.get("tool_count")),
        "status_code": _status_code(value.get("status_code")),
        "failure_class": _nullable_text("autonomous run trace failure_class", value.get("failure_class")),
        "failure_code": _nullable_text("autonomous run trace failure_code", value.get("failure_code")),
        "retryable": retryable,
        "detail_digest": _digest("autonomous run trace detail_digest", value.get("detail_digest")),
        "recorded_at": recorded_at,
        "previous_digest": previous,
        "retention": AUTONOMOUS_RUN_TRACE_RETENTION,
        "secret_material": AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL,
    }
    if value.get("schema") not in (None, AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA):
        raise ArgumentError("autonomous run trace event schema is unsupported")
    if value.get("retention") not in (None, AUTONOMOUS_RUN_TRACE_RETENTION):
        raise ArgumentError("autonomous run trace event retention is invalid")
    if value.get("secret_material") not in (None, AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL):
        raise ArgumentError("autonomous run trace event secret material marker is invalid")
    return result


def _event_digest(value: Mapping[str, Any]) -> str:
    return content_digest(dict(value))


def _event_from_normalized(value: Mapping[str, Any]) -> AutonomousRunTraceEvent:
    body = dict(value)
    return AutonomousRunTraceEvent(**body, event_digest=_event_digest(body))


def _verify_chain(events: Sequence[AutonomousRunTraceEvent], maximum: int) -> dict[str, Any]:
    if not isinstance(events, Sequence) or len(events) > maximum:
        raise ArgumentError("autonomous run trace event capacity is exceeded")
    previous = ""
    for index, event in enumerate(events, start=1):
        if not isinstance(event, AutonomousRunTraceEvent) or event.sequence != index or event.previous_digest != previous:
            raise ArgumentError(f"autonomous run trace hash chain breaks at sequence {index}")
        if _event_digest(event.to_dict(include_digest=False)) != event.event_digest:
            raise ArgumentError(f"autonomous run trace event digest mismatch at sequence {index}")
        previous = event.event_digest
    return {"verified": True, "events": len(events), "head_digest": previous}


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceSnapshot:
    schema: str
    sequence: int
    head_digest: str
    events: tuple[AutonomousRunTraceEvent, ...]
    snapshot_digest: str
    retention: str = AUTONOMOUS_RUN_TRACE_SNAPSHOT_RETENTION
    secret_material: str = AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL

    def to_dict(self, *, include_digest: bool = True) -> dict[str, Any]:
        body = {
            "schema": self.schema,
            "sequence": self.sequence,
            "head_digest": self.head_digest,
            "events": [event.to_dict() for event in self.events],
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        if include_digest:
            body["snapshot_digest"] = self.snapshot_digest
        return body

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any], *, max_events: int = MAX_AUTONOMOUS_RUN_TRACE_EVENTS, max_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) -> "AutonomousRunTraceSnapshot":
        max_events, max_bytes = _validate_trace_limits(max_events, max_bytes)
        if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA:
            raise ArgumentError("autonomous run trace snapshot is malformed")
        if raw.get("retention") != AUTONOMOUS_RUN_TRACE_SNAPSHOT_RETENTION or raw.get("secret_material") != AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL:
            raise ArgumentError("autonomous run trace snapshot retention is invalid")
        raw_events = raw.get("events")
        if not isinstance(raw_events, Sequence) or isinstance(raw_events, (str, bytes)):
            raise ArgumentError("autonomous run trace snapshot events are malformed")
        events = tuple(AutonomousRunTraceEvent.from_dict(event) for event in raw_events)
        _verify_chain(events, max_events)
        expected_head = events[-1].event_digest if events else ""
        if raw.get("sequence") != len(events) or raw.get("head_digest") != expected_head:
            raise ArgumentError("autonomous run trace snapshot head is inconsistent")
        body = {
            "schema": AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
            "sequence": len(events),
            "head_digest": expected_head,
            "events": [event.to_dict() for event in events],
            "retention": AUTONOMOUS_RUN_TRACE_SNAPSHOT_RETENTION,
            "secret_material": AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL,
        }
        supplied = raw.get("snapshot_digest")
        if not isinstance(supplied, str) or content_digest(body) != supplied:
            raise ArgumentError("autonomous run trace snapshot digest is invalid")
        snapshot = cls(
            schema=AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
            sequence=len(events),
            head_digest=expected_head,
            events=events,
            snapshot_digest=supplied,
        )
        if len(canonical_json(snapshot.to_dict()).encode("utf-8")) > max_bytes:
            raise ArgumentError("autonomous run trace snapshot exceeds its byte capacity")
        return snapshot


def validate_autonomous_run_trace_snapshot(raw: Mapping[str, Any] | AutonomousRunTraceSnapshot, *, max_events: int = MAX_AUTONOMOUS_RUN_TRACE_EVENTS, max_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) -> AutonomousRunTraceSnapshot:
    if isinstance(raw, AutonomousRunTraceSnapshot):
        raw = raw.to_dict()
    max_events, max_bytes = _validate_trace_limits(max_events, max_bytes)
    return AutonomousRunTraceSnapshot.from_dict(raw, max_events=max_events, max_bytes=max_bytes)


class AutonomousRunTraceStore(Protocol):
    def append(self, event: Mapping[str, Any]) -> AutonomousRunTraceEvent: ...
    def events(self, query: Mapping[str, Any] | None = None) -> tuple[AutonomousRunTraceEvent, ...]: ...
    def snapshot(self) -> AutonomousRunTraceSnapshot: ...
    def restore(self, snapshot: Mapping[str, Any] | AutonomousRunTraceSnapshot) -> None: ...
    def verify_integrity(self) -> dict[str, Any]: ...


class AutonomousRunTraceTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousRunTraceTransactionalTextStore(AutonomousRunTraceTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class InMemoryAutonomousRunTraceStore:
    """Bounded append-only trace store for local processes and tests."""

    def __init__(self, *, max_events: int = MAX_AUTONOMOUS_RUN_TRACE_EVENTS, max_event_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES, max_snapshot_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES, clock: Callable[[], int] | None = None) -> None:
        if isinstance(max_events, bool) or not isinstance(max_events, int) or not 1 <= max_events <= MAX_AUTONOMOUS_RUN_TRACE_EVENTS:
            raise ArgumentError("autonomous run trace max_events is outside its bounds")
        if isinstance(max_event_bytes, bool) or not isinstance(max_event_bytes, int) or not 512 <= max_event_bytes <= MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES:
            raise ArgumentError("autonomous run trace max_event_bytes is outside its bounds")
        if isinstance(max_snapshot_bytes, bool) or not isinstance(max_snapshot_bytes, int) or not max_event_bytes <= max_snapshot_bytes <= MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES:
            raise ArgumentError("autonomous run trace max_snapshot_bytes is outside its bounds")
        self.max_events = max_events
        self.max_event_bytes = max_event_bytes
        self.max_snapshot_bytes = max_snapshot_bytes
        self.clock = time.time_ns if clock is None else clock
        self._events: list[AutonomousRunTraceEvent] = []
        self._lock = threading.RLock()

    def append(self, event: Mapping[str, Any]) -> AutonomousRunTraceEvent:
        with self._lock:
            if len(self._events) >= self.max_events:
                raise ArgumentError("autonomous run trace event capacity is exhausted")
            normalized_input = dict(event)
            normalized = _normalize_event(
                normalized_input,
                len(self._events) + 1,
                self._events[-1].event_digest if self._events else "",
                self.clock(),
            )
            result = _event_from_normalized(normalized)
            if len(canonical_json(result.to_dict()).encode("utf-8")) > self.max_event_bytes:
                raise ArgumentError("autonomous run trace event exceeds its byte capacity")
            self._events.append(result)
            return result

    def events(self, query: Mapping[str, Any] | None = None) -> tuple[AutonomousRunTraceEvent, ...]:
        query = {} if query is None else query
        if not isinstance(query, Mapping):
            raise ArgumentError("autonomous run trace query must be a mapping")
        after = _nonnegative_integer("autonomous run trace after_sequence", query.get("after_sequence", 0), allow_none=False)
        limit = query.get("limit", MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT)
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT:
            raise ArgumentError("autonomous run trace query limit is outside its bounds")
        if query.get("run_id") is not None:
            _identifier("autonomous run trace query run_id", query["run_id"])
        for name in ("provider", "model"):
            if query.get(name) is not None:
                _bounded_text(f"autonomous run trace query {name}", query[name])
        for name, values in (("domain", AUTONOMOUS_DOMAIN_NAMES), ("phase", AUTONOMOUS_RUN_TRACE_PHASES), ("status", AUTONOMOUS_RUN_TRACE_STATUSES)):
            if query.get(name) is not None and query[name] not in values:
                raise ArgumentError(f"autonomous run trace query {name} is unsupported")
        with self._lock:
            selected = [event for event in self._events if event.sequence > after]
            selected = [event for event in selected if query.get("run_id") is None or event.run_id == query["run_id"]]
            selected = [event for event in selected if query.get("domain") is None or query["domain"] in event.domains]
            selected = [event for event in selected if query.get("phase") is None or event.phase == query["phase"]]
            selected = [event for event in selected if query.get("status") is None or event.status == query["status"]]
            selected = [event for event in selected if query.get("provider") is None or event.provider == query["provider"]]
            selected = [event for event in selected if query.get("model") is None or event.model == query["model"]]
            return tuple(selected[:limit])

    def snapshot(self) -> AutonomousRunTraceSnapshot:
        with self._lock:
            _verify_chain(self._events, self.max_events)
            body = {
                "schema": AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
                "sequence": len(self._events),
                "head_digest": self._events[-1].event_digest if self._events else "",
                "events": [event.to_dict() for event in self._events],
                "retention": AUTONOMOUS_RUN_TRACE_SNAPSHOT_RETENTION,
                "secret_material": AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL,
            }
            snapshot = AutonomousRunTraceSnapshot(
                schema=AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
                sequence=len(self._events),
                head_digest=body["head_digest"],
                events=tuple(self._events),
                snapshot_digest=content_digest(body),
            )
            if len(canonical_json(snapshot.to_dict()).encode("utf-8")) > self.max_snapshot_bytes:
                raise ArgumentError("autonomous run trace snapshot exceeds its byte capacity")
            return snapshot

    def restore(self, snapshot: Mapping[str, Any] | AutonomousRunTraceSnapshot) -> None:
        verified = validate_autonomous_run_trace_snapshot(snapshot, max_events=self.max_events, max_bytes=self.max_snapshot_bytes)
        with self._lock:
            self._events = list(verified.events)

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            return _verify_chain(self._events, self.max_events)


class JsonAutonomousRunTracePersistence:
    """Canonical JSON adapter over an application-owned text store."""

    def __init__(self, store: AutonomousRunTraceTextStore, *, max_events: int = MAX_AUTONOMOUS_RUN_TRACE_EVENTS, max_bytes: int = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous run trace JSON persistence requires a text store")
        max_events, max_bytes = _validate_trace_limits(max_events, max_bytes)
        self.store = store
        self.max_events = max_events
        self.max_bytes = max_bytes

    def read(self) -> AutonomousRunTraceSnapshot | None:
        text = self.store.read()
        if text is None:
            return None
        if not isinstance(text, str) or len(text.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("autonomous run trace JSON exceeds its byte bound")
        try:
            raw = json.loads(text)
        except (TypeError, json.JSONDecodeError) as error:
            raise ArgumentError("autonomous run trace JSON is invalid") from error
        if canonical_json(raw) != text:
            raise ArgumentError("autonomous run trace JSON is not canonical")
        return validate_autonomous_run_trace_snapshot(raw, max_events=self.max_events, max_bytes=self.max_bytes)

    def write(self, snapshot: Mapping[str, Any] | AutonomousRunTraceSnapshot) -> None:
        verified = validate_autonomous_run_trace_snapshot(snapshot, max_events=self.max_events, max_bytes=self.max_bytes)
        self.store.write(canonical_json(verified.to_dict()))


class TransactionalJsonAutonomousRunTracePersistence(JsonAutonomousRunTracePersistence):
    def __init__(self, store: AutonomousRunTraceTransactionalTextStore, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("autonomous run trace transactional persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any] | AutonomousRunTraceSnapshot) -> bool:
        if expected_snapshot_digest is not None:
            _digest("autonomous run trace expected snapshot digest", expected_snapshot_digest, allow_none=False)
        verified = validate_autonomous_run_trace_snapshot(snapshot, max_events=self.max_events, max_bytes=self.max_bytes)
        return self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(verified.to_dict()))


class InMemoryAutonomousRunTraceTextStore:
    """Thread-safe text store with an atomic digest fence for integration tests."""

    def __init__(self) -> None:
        self._value: str | None = None
        self._lock = threading.RLock()

    def read(self) -> str | None:
        with self._lock:
            return self._value

    def write(self, value: str) -> None:
        if not isinstance(value, str):
            raise ArgumentError("autonomous run trace text value must be a string")
        with self._lock:
            self._value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        if not isinstance(value, str):
            raise ArgumentError("autonomous run trace text value must be a string")
        with self._lock:
            current_digest = None
            if self._value is not None:
                try:
                    current_digest = json.loads(self._value).get("snapshot_digest")
                except (TypeError, json.JSONDecodeError):
                    return False
            if current_digest != expected_snapshot_digest:
                return False
            self._value = value
            return True


class FileAutonomousRunTraceTextStore:
    """Small caller-owned filesystem adapter; writes use replace for process-local atomicity."""

    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)
        if self.path.name in {"", ".", ".."}:
            raise ArgumentError("autonomous run trace file path is invalid")
        self._lock = threading.RLock()

    def read(self) -> str | None:
        with self._lock:
            if not self.path.exists():
                return None
            return self.path.read_text(encoding="utf-8")

    def write(self, value: str) -> None:
        if not isinstance(value, str):
            raise ArgumentError("autonomous run trace text value must be a string")
        with self._lock:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary = self.path.with_name(f".{self.path.name}.tmp")
            temporary.write_text(value, encoding="utf-8")
            temporary.replace(self.path)


class AutonomousRunTracePersistenceCoordinator:
    """Restore/flush coordinator with stale-writer fencing when the adapter supports CAS."""

    def __init__(self, store: AutonomousRunTraceStore, persistence: Any) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("append", "events", "snapshot", "restore")):
            raise ArgumentError("autonomous run trace persistence requires a complete trace store")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous run trace persistence adapter is malformed")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> AutonomousRunTraceSnapshot | None:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
                return None
            self.store.restore(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot

    def flush(self) -> AutonomousRunTraceSnapshot:
        with self._lock:
            snapshot = self.store.snapshot()
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("autonomous run trace persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceSummary:
    schema: str
    run_id: str
    task_digest: str
    domains: tuple[str, ...]
    status: str
    first_sequence: int | None
    last_sequence: int | None
    event_count: int
    provider_invocations: int
    provider_failures: int
    input_tokens: int
    output_tokens: int
    tool_calls: int
    route_digest: str | None
    plan_digest: str | None
    selection_digests: tuple[str, ...]
    failure_codes: tuple[str, ...]
    trace_digest: str
    retention: str = AUTONOMOUS_RUN_TRACE_RETENTION
    secret_material: str = AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": self.schema,
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "domains": list(self.domains),
            "status": self.status,
            "first_sequence": self.first_sequence,
            "last_sequence": self.last_sequence,
            "event_count": self.event_count,
            "provider_invocations": self.provider_invocations,
            "provider_failures": self.provider_failures,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "tool_calls": self.tool_calls,
            "route_digest": self.route_digest,
            "plan_digest": self.plan_digest,
            "selection_digests": list(self.selection_digests),
            "failure_codes": list(self.failure_codes),
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        body["trace_digest"] = self.trace_digest
        return body


def _terminal_phase(status: str) -> str:
    return "completed" if status in {"completed", "partial"} else status if status in {"paused", "refused", "failed"} else "failed"


class AutonomousRunTraceSession:
    """Lifecycle helper for one run; it never accepts provider payloads."""

    def __init__(self, store: AutonomousRunTraceStore, *, run_id: str, task_digest: str, domains: Sequence[str]) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("append", "events")):
            raise ArgumentError("autonomous run trace session requires a trace store")
        self.store = store
        self.run_id = _identifier("autonomous run trace run_id", run_id)
        self.task_digest = _digest("autonomous run trace task_digest", task_digest, allow_none=False)
        self.domains = _domains(domains)
        self._terminal = False

    def _events(self) -> tuple[AutonomousRunTraceEvent, ...]:
        """Read this run in bounded pages so summaries remain correct past one query page."""

        collected: list[AutonomousRunTraceEvent] = []
        after = 0
        while True:
            page = self.store.events({
                "run_id": self.run_id,
                "after_sequence": after,
                "limit": MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT,
            })
            collected.extend(page)
            if len(page) < MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT:
                return tuple(collected)
            after = page[-1].sequence

    def started(self, *, detail_digest: str | None = None) -> AutonomousRunTraceEvent:
        if self._events():
            raise ArgumentError("autonomous run trace run_id already has events")
        return self.record(phase="started", status="running", detail_digest=detail_digest)

    def record(self, *, phase: str, status: str, domains: Sequence[str] | None = None, **metadata: Any) -> AutonomousRunTraceEvent:
        if self._terminal:
            raise ArgumentError("autonomous run trace session is already terminal")
        event = {
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "domains": self.domains if domains is None else domains,
            "phase": phase,
            "status": status,
            **metadata,
        }
        return self.store.append(event)

    def provider_observer(self) -> Any:
        """Return a value-only observer compatible with ``LLMRuntime`` hooks."""

        session = self

        class Observer:
            def __init__(self) -> None:
                self._pending_turns: list[int] = []

            def before(self, metadata: Any) -> None:
                turn = len(self._pending_turns)
                self._pending_turns.append(turn)
                session.record(
                    phase="provider_invocation_started",
                    status="running",
                    provider=getattr(metadata, "provider", None),
                    model=getattr(metadata, "model", None),
                    attempt=0,
                    turn=turn,
                    input_tokens=getattr(metadata, "input_tokens", None),
                    tool_count=getattr(metadata, "tool_count", None),
                )

            def after(self, metadata: Any, response: Any, error: BaseException | None, latency_ms: float) -> None:
                turn = self._pending_turns.pop() if self._pending_turns else 0
                usage = getattr(response, "usage", {}) if response is not None else {}
                input_tokens = usage.get("input_tokens", usage.get("prompt_tokens", getattr(metadata, "input_tokens", 0))) if isinstance(usage, Mapping) else getattr(metadata, "input_tokens", 0)
                output_tokens = usage.get("output_tokens", usage.get("completion_tokens", 0)) if isinstance(usage, Mapping) else 0
                status_code = getattr(error, "status_code", None) if error is not None else None
                failure_class = type(error).__name__ if error is not None else None
                session.record(
                    phase="provider_invocation_finished",
                    status="running",
                    provider=getattr(metadata, "provider", None),
                    model=getattr(metadata, "model", None),
                    turn=turn,
                    latency_ms=latency_ms,
                    input_tokens=input_tokens,
                    output_tokens=output_tokens,
                    tool_count=getattr(metadata, "tool_count", None),
                    status_code=status_code,
                    failure_class=failure_class,
                    failure_code="provider_error" if error is not None else None,
                    retryable=getattr(error, "retryable", None) if error is not None else None,
                )

        return Observer()

    def record_provider_receipts(self, receipts: Sequence[Mapping[str, Any]]) -> None:
        """Project existing redacted provider receipts into the trace, omitting all payloads."""

        if not isinstance(receipts, Sequence) or isinstance(receipts, (str, bytes)):
            raise ArgumentError("autonomous run trace provider receipts must be a sequence")
        for receipt in receipts:
            if not isinstance(receipt, Mapping):
                raise ArgumentError("autonomous run trace provider receipt must be a mapping")
            provider = _bounded_text("autonomous run trace receipt provider", receipt.get("provider"))
            model = _bounded_text("autonomous run trace receipt model", receipt.get("model"))
            base = {
                "provider": provider,
                "model": model,
                "attempt": receipt.get("attempt"),
                "turn": receipt.get("turn"),
                "input_tokens": receipt.get("input_tokens"),
                "output_tokens": receipt.get("output_tokens"),
                "tool_count": receipt.get("tool_count", 0),
                "status_code": receipt.get("status_code"),
                "failure_class": receipt.get("failure_class"),
                "failure_code": "provider_error" if receipt.get("failure_class") else None,
                "selection_digest": receipt.get("selection_digest"),
                "detail_digest": receipt.get("outcome_digest"),
                "latency_ms": receipt.get("latency_ms"),
                "retryable": receipt.get("retryable"),
            }
            self.record(phase="provider_invocation_started", status="running", **{key: value for key, value in base.items() if key not in {"output_tokens", "latency_ms", "status_code", "failure_class", "failure_code", "detail_digest", "retryable"}})
            self.record(phase="provider_invocation_finished", status="running", **base)

    def complete(self, *, status: str, route_digest: str | None = None, plan_digest: str | None = None, selection_digest: str | None = None, domains: Sequence[str] | None = None, detail_digest: str | None = None, failure_class: str | None = None, failure_code: str | None = None) -> AutonomousRunTraceEvent:
        if self._terminal:
            raise ArgumentError("autonomous run trace session is already terminal")
        if status not in AUTONOMOUS_RUN_TRACE_STATUSES:
            raise ArgumentError("autonomous run trace completion status is invalid")
        self._terminal = True
        return self.store.append({
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "domains": self.domains if domains is None else domains,
            "phase": _terminal_phase(status),
            "status": status,
            "route_digest": route_digest,
            "plan_digest": plan_digest,
            "selection_digest": selection_digest,
            "detail_digest": detail_digest,
            "failure_class": failure_class,
            "failure_code": failure_code,
        })

    def fail(self, *, failure_class: str | None = None, failure_code: str | None = None, detail_digest: str | None = None) -> AutonomousRunTraceEvent:
        return self.complete(status="failed", failure_class=failure_class, failure_code=failure_code, detail_digest=detail_digest)

    def summary(self) -> AutonomousRunTraceSummary:
        events = self._events()
        if not events:
            raise ArgumentError("autonomous run trace has no events")
        last = events[-1]
        domains = tuple(sorted({domain for event in events for domain in event.domains}))
        selection_digests = tuple(sorted({event.selection_digest for event in events if event.selection_digest is not None}))
        failure_codes = tuple(sorted({event.failure_code for event in events if event.failure_code is not None}))
        completed = tuple(event for event in events if event.phase == "provider_invocation_finished")
        body = {
            "schema": AUTONOMOUS_RUN_TRACE_SCHEMA,
            "run_id": self.run_id,
            "task_digest": self.task_digest,
            "domains": list(domains),
            "status": last.status,
            "first_sequence": events[0].sequence,
            "last_sequence": last.sequence,
            "event_count": len(events),
            "provider_invocations": len(completed),
            "provider_failures": sum(event.failure_code is not None for event in completed),
            "input_tokens": sum(event.input_tokens or 0 for event in completed),
            "output_tokens": sum(event.output_tokens or 0 for event in completed),
            "tool_calls": sum(event.tool_count or 0 for event in completed),
            "route_digest": next((event.route_digest for event in reversed(events) if event.route_digest is not None), None),
            "plan_digest": next((event.plan_digest for event in reversed(events) if event.plan_digest is not None), None),
            "selection_digests": list(selection_digests),
            "failure_codes": list(failure_codes),
            "retention": AUTONOMOUS_RUN_TRACE_RETENTION,
            "secret_material": AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL,
        }
        return AutonomousRunTraceSummary(
            schema=AUTONOMOUS_RUN_TRACE_SCHEMA,
            run_id=self.run_id,
            task_digest=self.task_digest,
            domains=domains,
            status=last.status,
            first_sequence=events[0].sequence,
            last_sequence=last.sequence,
            event_count=len(events),
            provider_invocations=len(completed),
            provider_failures=sum(event.failure_code is not None for event in completed),
            input_tokens=sum(event.input_tokens or 0 for event in completed),
            output_tokens=sum(event.output_tokens or 0 for event in completed),
            tool_calls=sum(event.tool_count or 0 for event in completed),
            route_digest=body["route_digest"],
            plan_digest=body["plan_digest"],
            selection_digests=selection_digests,
            failure_codes=failure_codes,
            trace_digest=content_digest(body),
        )


def autonomous_run_trace_status(status: str) -> str:
    if status in {"completed", "completed_provider_call", "completed_tool_loop", "completed_mission", "completed_workflow"}:
        return "completed"
    if status in {"cross_domain_partial", "children_partial", "children_completed", "completed_without_replan", "replan_limit_reached"}:
        return "partial"
    if status in {"route_review_required", "approval_required", "reconciliation_required", "turn_limit_reached", "plan_review_required", "connector_blocked", "paused", "stage_blocked", "stage_proposed", "stage_not_attempted"}:
        return "paused"
    if status in {"abstained", "provider_abstained", "provider_invalid", "provider_disagreement"}:
        return "refused"
    if status in {"child_failed", "execution_failed", "stage_failed", "provider_failed"}:
        return "failed"
    return "unknown"


@dataclass(frozen=True, slots=True)
class AutonomousTracedRunResult:
    """Live caller-owned result plus a payload-free trace summary."""

    result: Any
    trace: AutonomousRunTraceSummary

    @property
    def status(self) -> str:
        return self.trace.status

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-python-autonomous-traced-result/0.1",
            "status": self.status,
            "trace": self.trace.to_dict(),
            "result": "caller_owned_live_result_not_serialized",
            "retention": AUTONOMOUS_RUN_TRACE_RETENTION,
            "secret_material": AUTONOMOUS_RUN_TRACE_SECRET_MATERIAL,
        }


__all__ = [
    "AUTONOMOUS_RUN_TRACE_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_EVENT_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_PHASES",
    "AUTONOMOUS_RUN_TRACE_STATUSES",
    "MAX_AUTONOMOUS_RUN_TRACE_EVENTS",
    "MAX_AUTONOMOUS_RUN_TRACE_EVENT_BYTES",
    "MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_RUN_TRACE_QUERY_LIMIT",
    "AutonomousRunTraceEvent",
    "AutonomousRunTraceSnapshot",
    "AutonomousRunTraceSummary",
    "AutonomousRunTraceStore",
    "AutonomousRunTraceTextStore",
    "AutonomousRunTraceTransactionalTextStore",
    "InMemoryAutonomousRunTraceStore",
    "JsonAutonomousRunTracePersistence",
    "TransactionalJsonAutonomousRunTracePersistence",
    "InMemoryAutonomousRunTraceTextStore",
    "FileAutonomousRunTraceTextStore",
    "AutonomousRunTracePersistenceCoordinator",
    "AutonomousRunTraceSession",
    "AutonomousTracedRunResult",
    "validate_autonomous_run_trace_snapshot",
    "autonomous_run_trace_status",
]
