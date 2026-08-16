"""Typed projections for the bounded factory lifecycle simulator.

The Rust ``JobStore`` remains the authority for lifecycle semantics.  This module deliberately
does not reimplement queue behavior in Python.  It validates the transport envelope, preserves
every ordered action and refusal, and exposes the safety boundaries that are otherwise easy to
lose when a lifecycle trace is treated as untyped JSON: one active lease, explicit expiry
recovery, staged-versus-committed output, compensation, human quarantine release, cancellation,
and the absence of external effects.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


FACTORY_LIFECYCLE_MAX_INPUT_BYTES = 20_000_000
FACTORY_LIFECYCLE_MAX_JOBS = 256
FACTORY_LIFECYCLE_MAX_WORKERS = 256
FACTORY_LIFECYCLE_MAX_ACTIONS = 2_000
FACTORY_ACTIONS = frozenset(
    {
        "enqueue",
        "lease",
        "heartbeat",
        "stage",
        "commit",
        "fail",
        "recover_expired",
        "compensate",
        "release_quarantine",
        "cancel",
    }
)
FACTORY_RESOURCE_CLASSES = frozenset({"compile", "ingest", "sandbox", "evaluate", "mutate", "index"})
FACTORY_IDEMPOTENCY_CLASSES = frozenset({"idempotent", "non_idempotent", "compensable"})
FACTORY_JOB_STATES = frozenset(
    {"queued", "leased", "staged", "succeeded", "failed", "quarantined", "dead_lettered", "cancelled"}
)
FACTORY_RECOVERY_OUTCOMES = frozenset(
    {"requeued", "quarantined", "awaiting_compensation", "dead_lettered"}
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mappings(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Find a direct, MCP nested, or HTTP tool projection without accepting an unrelated object."""

    raw = _route_mapping("factory lifecycle response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"factory lifecycle response text is not JSON: {error}") from error
                        if isinstance(decoded, Mapping):
                            candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if isinstance(candidate.get("ok"), bool) and isinstance(candidate.get("action_count"), int) and isinstance(candidate.get("trace"), list) and isinstance(candidate.get("jobs"), list):
            return dict(candidate)
    raise ArgumentError("response does not contain a factory lifecycle projection")


@dataclass(frozen=True)
class FactoryLifecycleSimulateArgs:
    """Bounded serialized inputs accepted by ``factory_lifecycle_simulate``.

    Job and worker semantics intentionally stay serialized here: the Rust factory crate owns the
    canonical Job and WorkerCapability schemas.  The SDK still checks that the envelopes are
    JSON objects, that worker identity is unambiguous, and that the replay cannot exceed the
    server's resource bounds.
    """

    jobs: tuple[dict[str, Any], ...]
    workers: tuple[dict[str, Any], ...]
    actions: tuple[dict[str, Any], ...]

    def __init__(self, jobs: Sequence[Mapping[str, Any]], workers: Sequence[Mapping[str, Any]], actions: Sequence[Mapping[str, Any]]) -> None:
        normalized_jobs = _mappings("factory jobs", jobs)
        normalized_workers = _mappings("factory workers", workers)
        normalized_actions = _mappings("factory actions", actions)
        if not 1 <= len(normalized_jobs) <= FACTORY_LIFECYCLE_MAX_JOBS:
            raise ArgumentError("factory jobs must contain between 1 and 256 entries")
        if not 1 <= len(normalized_workers) <= FACTORY_LIFECYCLE_MAX_WORKERS:
            raise ArgumentError("factory workers must contain between 1 and 256 entries")
        if len(normalized_actions) > FACTORY_LIFECYCLE_MAX_ACTIONS:
            raise ArgumentError("factory actions are bounded at 2000 entries")
        worker_ids: list[str] = []
        for index, worker in enumerate(normalized_workers):
            worker_id = _route_text(f"factory workers[{index}].worker_id", worker.get("worker_id"))
            worker_ids.append(worker_id)
        if len(worker_ids) != len(set(worker_ids)):
            raise ArgumentError("factory workers must have unique worker_id values")
        for index, action in enumerate(normalized_actions):
            kind = action.get("kind")
            if kind is not None and not isinstance(kind, str):
                raise ArgumentError(f"factory actions[{index}].kind must be a string when supplied")
        arguments = {"jobs": list(normalized_jobs), "workers": list(normalized_workers), "actions": list(normalized_actions)}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"factory lifecycle arguments are not JSON serializable: {error}") from error
        if len(encoded) > FACTORY_LIFECYCLE_MAX_INPUT_BYTES:
            raise ArgumentError("factory lifecycle input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "jobs", normalized_jobs)
        object.__setattr__(self, "workers", normalized_workers)
        object.__setattr__(self, "actions", normalized_actions)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FactoryLifecycleSimulateArgs":
        raw = _route_mapping("factory lifecycle arguments", value)
        return cls(raw.get("jobs"), raw.get("workers"), raw.get("actions"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"jobs": [dict(item) for item in self.jobs], "workers": [dict(item) for item in self.workers], "actions": [dict(item) for item in self.actions]}


@dataclass(frozen=True)
class FactoryRecoveryReport:
    raw: dict[str, Any]
    outcome: str
    job_id: str
    attempt: int | None
    attempts: int | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FactoryRecoveryReport":
        raw = _route_mapping("factory recovery", value)
        outcome = _route_text("factory recovery outcome", raw.get("outcome"))
        if outcome not in FACTORY_RECOVERY_OUTCOMES:
            raise ArgumentError(f"unknown factory recovery outcome {outcome!r}")
        job_id = _route_text("factory recovery job_id", raw.get("job_id"))
        attempt = raw.get("attempt")
        attempts = raw.get("attempts")
        if attempt is not None:
            attempt = _route_count("factory recovery attempt", attempt)
        if attempts is not None:
            attempts = _route_count("factory recovery attempts", attempts)
        reason = _optional_text("factory recovery reason", raw.get("reason"))
        return cls(raw, outcome, job_id, attempt, attempts, reason)


@dataclass(frozen=True)
class FactoryLeaseReport:
    raw: dict[str, Any] | None
    present: bool
    job_id: str | None
    worker_id: str | None
    attempt: int | None
    granted_at: Any
    expires_at: Any
    last_heartbeat: Any

    @classmethod
    def from_wire(cls, value: Any) -> "FactoryLeaseReport":
        if value is None:
            return cls(None, False, None, None, None, None, None, None)
        raw = _route_mapping("factory lease", value)
        attempt = _route_count("factory lease attempt", raw.get("attempt"))
        return cls(raw, True, _route_text("factory lease job_id", raw.get("job_id")), _route_text("factory lease worker_id", raw.get("worker_id")), attempt, raw.get("granted_at"), raw.get("expires_at"), raw.get("last_heartbeat"))


@dataclass(frozen=True)
class FactoryJobSnapshotReport:
    raw: dict[str, Any]
    job: dict[str, Any]
    id: str
    resource_class: str | None
    idempotency: str | None
    state: str | None
    attempts: int | None
    reason: str | None
    committed_result: Any

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FactoryJobSnapshotReport":
        raw = _route_mapping("factory job snapshot", value)
        job_value = raw.get("job", raw)
        job = _route_mapping("factory job snapshot.job", job_value)
        identifier = _route_text("factory job snapshot id", raw.get("id", job.get("id")))
        if job.get("id") is not None and job.get("id") != identifier:
            raise ArgumentError("factory job snapshot id does not match job.id")
        resource_class = _optional_text("factory job resource_class", job.get("resource_class"))
        if resource_class is not None and resource_class not in FACTORY_RESOURCE_CLASSES:
            raise ArgumentError(f"unknown factory resource class {resource_class!r}")
        idempotency = _optional_text("factory job idempotency", job.get("idempotency"))
        if idempotency is not None and idempotency not in FACTORY_IDEMPOTENCY_CLASSES:
            raise ArgumentError(f"unknown factory idempotency class {idempotency!r}")
        state = _optional_text("factory job state", job.get("state"))
        if state is not None and state not in FACTORY_JOB_STATES:
            raise ArgumentError(f"unknown factory job state {state!r}")
        attempts = job.get("attempts")
        if attempts is not None:
            attempts = _route_count("factory job attempts", attempts)
        reason = _optional_text("factory job reason", job.get("reason"))
        return cls(raw, job, identifier, resource_class, idempotency, state, attempts, reason, raw.get("committed_result"))

    @property
    def committed(self) -> bool:
        return self.state == "succeeded"

    @property
    def terminal(self) -> bool:
        return self.state in {"succeeded", "dead_lettered", "cancelled"}


@dataclass(frozen=True)
class FactoryActionTraceReport:
    raw: dict[str, Any]
    index: int
    kind: str
    ok: bool
    result: Any
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FactoryActionTraceReport":
        raw = _route_mapping("factory lifecycle trace row", value)
        index = _route_count("factory lifecycle trace index", raw.get("index"))
        kind = _route_text("factory lifecycle trace kind", raw.get("kind"))
        ok = _bool("factory lifecycle trace ok", raw.get("ok"))
        refusal = _optional_text("factory lifecycle trace refusal", raw.get("refusal"))
        fail_closed = _bool("factory lifecycle trace fail_closed", raw.get("fail_closed", False))
        if not ok and refusal is None:
            raise ArgumentError("failed factory lifecycle trace rows require a refusal")
        if not ok and not fail_closed:
            raise ArgumentError("failed factory lifecycle trace rows must be fail_closed")
        return cls(raw, index, kind, ok, raw.get("result"), refusal, fail_closed)

    @property
    def lease(self) -> FactoryLeaseReport | None:
        return None if self.kind != "lease" or not self.ok else FactoryLeaseReport.from_wire(self.result)

    @property
    def recovery(self) -> FactoryRecoveryReport | None:
        if self.kind != "fail" or not self.ok:
            return None
        return FactoryRecoveryReport.from_wire(self.result)

    @property
    def recoveries(self) -> tuple[FactoryRecoveryReport, ...]:
        if self.kind != "recover_expired" or not self.ok:
            return ()
        return tuple(FactoryRecoveryReport.from_wire(item) for item in _sequence("factory recoveries", self.result))

    @property
    def staged_output_is_hidden(self) -> bool:
        return self.kind == "stage" and self.ok and isinstance(self.result, Mapping) and self.result.get("visible_before_commit") is False


@dataclass(frozen=True)
class FactoryLifecycleReport:
    raw: dict[str, Any]
    ok: bool
    action_count: int
    action_failures: int
    trace: tuple[FactoryActionTraceReport, ...]
    jobs: tuple[FactoryJobSnapshotReport, ...]
    quarantined: tuple[FactoryJobSnapshotReport, ...]
    dead_lettered: tuple[FactoryJobSnapshotReport, ...]
    counts_by_class: dict[str, int]
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FactoryLifecycleReport":
        raw = _payload(value)
        ok = _bool("factory lifecycle ok", raw.get("ok"))
        action_count = _route_count("factory lifecycle action_count", raw.get("action_count"))
        action_failures = _route_count("factory lifecycle action_failures", raw.get("action_failures"))
        trace = tuple(FactoryActionTraceReport.from_wire(item) for item in _sequence("factory lifecycle trace", raw.get("trace")))
        if len(trace) != action_count:
            raise ArgumentError("factory lifecycle action_count does not match trace length")
        if action_failures != sum(not item.ok for item in trace):
            raise ArgumentError("factory lifecycle action_failures does not match failed trace rows")
        if ok != (action_failures == 0):
            raise ArgumentError("factory lifecycle ok does not match action failures")
        jobs = tuple(FactoryJobSnapshotReport.from_wire(item) for item in _sequence("factory lifecycle jobs", raw.get("jobs")))
        quarantined = tuple(FactoryJobSnapshotReport.from_wire(item) for item in _sequence("factory lifecycle quarantined", raw.get("quarantined", [])))
        dead_lettered = tuple(FactoryJobSnapshotReport.from_wire(item) for item in _sequence("factory lifecycle dead_lettered", raw.get("dead_lettered", [])))
        counts_raw = _route_mapping("factory lifecycle counts_by_class", raw.get("counts_by_class"))
        counts: dict[str, int] = {}
        for resource_class, count in counts_raw.items():
            key = _route_text("factory lifecycle resource class", resource_class)
            if key not in FACTORY_RESOURCE_CLASSES:
                raise ArgumentError(f"unknown factory resource class {key!r}")
            counts[key] = _route_count(f"factory lifecycle count for {key}", count)
        guarantees = _route_strings("factory lifecycle guarantees", raw.get("guarantees", []))
        return cls(raw, ok, action_count, action_failures, trace, jobs, quarantined, dead_lettered, counts, guarantees)

    @property
    def complete(self) -> bool:
        return self.ok

    @property
    def successful_action_count(self) -> int:
        return sum(item.ok for item in self.trace)

    @property
    def fail_closed_refusal_count(self) -> int:
        return sum(item.fail_closed for item in self.trace if not item.ok)

    @property
    def committed_job_ids(self) -> tuple[str, ...]:
        return tuple(item.id for item in self.jobs if item.committed)

    @property
    def quarantined_job_ids(self) -> tuple[str, ...]:
        return tuple(item.id for item in self.quarantined)

    @property
    def dead_lettered_job_ids(self) -> tuple[str, ...]:
        return tuple(item.id for item in self.dead_lettered)

    @property
    def state_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for item in self.jobs:
            if item.state is not None:
                counts[item.state] = counts.get(item.state, 0) + 1
        return counts

    @property
    def recovery_outcomes(self) -> tuple[str, ...]:
        outcomes: list[str] = []
        for item in self.trace:
            if item.recovery is not None:
                outcomes.append(item.recovery.outcome)
            outcomes.extend(recovery.outcome for recovery in item.recoveries)
        return tuple(outcomes)

    @property
    def staged_visibility_is_explicit(self) -> bool:
        return any(item.staged_output_is_hidden for item in self.trace)

    @property
    def no_external_effects_claimed(self) -> bool:
        return any("no worker process, queue, clock, filesystem, network, or external side effect" in item for item in self.guarantees)

    @property
    def lifecycle_invariants_are_claimed(self) -> bool:
        return any("every lifecycle transition" in item and "typed in-memory JobStore" in item for item in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def factory_lifecycle_report(value: Mapping[str, Any]) -> FactoryLifecycleReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return FactoryLifecycleReport.from_wire(value)


__all__ = [
    "FACTORY_LIFECYCLE_MAX_INPUT_BYTES",
    "FACTORY_LIFECYCLE_MAX_JOBS",
    "FACTORY_LIFECYCLE_MAX_WORKERS",
    "FACTORY_LIFECYCLE_MAX_ACTIONS",
    "FACTORY_ACTIONS",
    "FACTORY_RESOURCE_CLASSES",
    "FACTORY_IDEMPOTENCY_CLASSES",
    "FACTORY_JOB_STATES",
    "FACTORY_RECOVERY_OUTCOMES",
    "FactoryLifecycleSimulateArgs",
    "FactoryRecoveryReport",
    "FactoryLeaseReport",
    "FactoryJobSnapshotReport",
    "FactoryActionTraceReport",
    "FactoryLifecycleReport",
    "factory_lifecycle_report",
]
