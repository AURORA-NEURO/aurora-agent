"""Crash-safe checkpointing for the reviewed generic evidence execution controller."""

from __future__ import annotations

from dataclasses import dataclass
import json
import threading
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_execution import (
    AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA,
    AutonomousEvidenceExecutionController,
    AutonomousEvidenceExecutionPlan,
    AutonomousEvidenceExecutionResult,
)
from .autonomous_evidence_runtime import AutonomousEvidenceRuntimeJournal
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA = "bioprism-python-autonomous-evidence-execution-checkpoint/0.1"
AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-execution-resumable-result/0.1"
MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES = 128_000
MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS = 128

_RETENTION = "metadata_only;requests_readiness_and_source_values_caller_owned"
_RESULT_RETENTION = "metadata_only;source_values_and_runtime_payloads_caller_owned"
_SECRET_MATERIAL = "never_returned"
_STATUSES = frozenset({
    "approval_required", "blocked", "dispatch_pending", "awaiting_evaluation", "partial",
    "failed", "reconciliation_required", "completed",
})
_RUNTIME_STATUSES = frozenset({"completed", "partial", "awaiting_evaluation", "failed", "reconciliation_required"})


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 256 or "\x00" in value or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-" for character in value):
        raise ArgumentError(f"{name} is outside its bounded identifier contract")
    return value


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        raise ArgumentError(f"{name} must be an integer in [{minimum}, {maximum}]")
    return value


def _request_projection(request: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(request, Mapping):
        raise ArgumentError("evidence execution checkpoint request is malformed")
    requirement_id = _identifier("evidence execution checkpoint requirement_id", request.get("requirement_id"))
    source_id = _identifier("evidence execution checkpoint source_id", request.get("source_id"))
    source_digest = _digest("evidence execution checkpoint source_digest", request.get("source_digest"), allow_none=True)
    request_id = None if request.get("request_id") is None else _identifier("evidence execution checkpoint request_id", request.get("request_id"))
    metadata = request.get("metadata", {})
    if not isinstance(metadata, Mapping):
        raise ArgumentError("evidence execution checkpoint request metadata is malformed")
    # Runtime-level metadata validation is deliberately repeated here: the checkpoint digest
    # must never become a covert persistence channel for credentials or provider payloads.
    from .autonomous_evidence_runtime import _assert_metadata, _json_bytes

    metadata_value = dict(metadata)
    _assert_metadata(metadata_value, "evidence execution checkpoint request metadata")
    _json_bytes(metadata_value, "evidence execution checkpoint request metadata")
    return {
        "requirement_id": requirement_id,
        "source_id": source_id,
        "source_digest": source_digest,
        "request_id": request_id,
        "metadata_digest": content_digest(metadata_value),
    }


def evidence_execution_requests_digest(requests: Sequence[Mapping[str, Any]]) -> str:
    if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)) or not 1 <= len(requests) <= MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS:
        raise ArgumentError("evidence execution checkpoint requests are outside their bound")
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
        "requests": [_request_projection(request) for request in requests],
    })


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionCheckpoint:
    job_id: str
    evidence_plan_digest: str
    execution_plan_digest: str
    request_digest: str
    readiness_report_digest: str
    status: str
    runtime_status: str | None
    runtime_result_digest: str | None
    completed_request_count: int
    pending_request_count: int
    accepted_request_count: int
    checkpoint_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
            "job_id": self.job_id,
            "evidence_plan_digest": self.evidence_plan_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "request_digest": self.request_digest,
            "readiness_report_digest": self.readiness_report_digest,
            "status": self.status,
            "runtime_status": self.runtime_status,
            "runtime_result_digest": self.runtime_result_digest,
            "completed_request_count": self.completed_request_count,
            "pending_request_count": self.pending_request_count,
            "accepted_request_count": self.accepted_request_count,
            "checkpoint_digest": self.checkpoint_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _checkpoint_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value[key] for key in (
        "schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest",
        "readiness_report_digest", "status", "runtime_status", "runtime_result_digest",
        "completed_request_count", "pending_request_count", "accepted_request_count",
    )}


def validate_autonomous_evidence_execution_checkpoint(value: Mapping[str, Any] | AutonomousEvidenceExecutionCheckpoint) -> AutonomousEvidenceExecutionCheckpoint:
    raw = value.to_dict() if isinstance(value, AutonomousEvidenceExecutionCheckpoint) else value
    if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA:
        raise ArgumentError("evidence execution checkpoint schema is invalid")
    allowed = {
        "schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest",
        "readiness_report_digest", "status", "runtime_status", "runtime_result_digest",
        "completed_request_count", "pending_request_count", "accepted_request_count",
        "checkpoint_digest", "retention", "secret_material",
    }
    if set(raw) != allowed:
        raise ArgumentError("evidence execution checkpoint contains unsupported fields")
    if raw.get("status") not in _STATUSES:
        raise ArgumentError("evidence execution checkpoint status is invalid")
    runtime_status = raw.get("runtime_status")
    if runtime_status is not None and runtime_status not in _RUNTIME_STATUSES:
        raise ArgumentError("evidence execution checkpoint runtime status is invalid")
    runtime_digest = _digest("evidence execution checkpoint runtime_result_digest", raw.get("runtime_result_digest"), allow_none=True)
    normalized = AutonomousEvidenceExecutionCheckpoint(
        job_id=_identifier("evidence execution checkpoint job_id", raw.get("job_id")),
        evidence_plan_digest=_digest("evidence execution checkpoint evidence_plan_digest", raw.get("evidence_plan_digest")),  # type: ignore[arg-type]
        execution_plan_digest=_digest("evidence execution checkpoint execution_plan_digest", raw.get("execution_plan_digest")),  # type: ignore[arg-type]
        request_digest=_digest("evidence execution checkpoint request_digest", raw.get("request_digest")),  # type: ignore[arg-type]
        readiness_report_digest=_digest("evidence execution checkpoint readiness_report_digest", raw.get("readiness_report_digest")),  # type: ignore[arg-type]
        status=raw.get("status"),
        runtime_status=runtime_status,
        runtime_result_digest=runtime_digest,
        completed_request_count=_integer("evidence execution checkpoint completed_request_count", raw.get("completed_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS),
        pending_request_count=_integer("evidence execution checkpoint pending_request_count", raw.get("pending_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS * 2),
        accepted_request_count=_integer("evidence execution checkpoint accepted_request_count", raw.get("accepted_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS),
        checkpoint_digest=_digest("evidence execution checkpoint checkpoint_digest", raw.get("checkpoint_digest")),  # type: ignore[arg-type]
    )
    has_runtime = normalized.runtime_status is not None or normalized.runtime_result_digest is not None or normalized.completed_request_count > 0 or normalized.pending_request_count > 0 or normalized.accepted_request_count > 0
    if normalized.status in {"approval_required", "blocked", "dispatch_pending"} and has_runtime:
        raise ArgumentError("pre-dispatch evidence execution checkpoint cannot contain runtime state")
    if normalized.status == "completed" and (normalized.runtime_status != "completed" or normalized.runtime_result_digest is None):
        raise ArgumentError("completed evidence execution checkpoint requires a completed runtime digest")
    if normalized.status not in {"completed", "approval_required", "blocked", "dispatch_pending"} and normalized.runtime_result_digest is None:
        raise ArgumentError("post-dispatch evidence execution checkpoint requires a runtime digest")
    if raw.get("retention") != _RETENTION or raw.get("secret_material") != _SECRET_MATERIAL:
        raise ArgumentError("evidence execution checkpoint retention contract is invalid")
    if content_digest(_checkpoint_payload(raw)) != normalized.checkpoint_digest:
        raise ArgumentError("evidence execution checkpoint digest is invalid")
    if len(canonical_json(raw).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES:
        raise ArgumentError("evidence execution checkpoint exceeds its bound")
    return normalized


class AutonomousEvidenceExecutionCheckpointStore(Protocol):
    def read(self) -> AutonomousEvidenceExecutionCheckpoint | Mapping[str, Any] | None: ...
    def write(self, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> None: ...


class TransactionalAutonomousEvidenceExecutionCheckpointStore(AutonomousEvidenceExecutionCheckpointStore, Protocol):
    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> bool: ...


class InMemoryAutonomousEvidenceExecutionCheckpointStore:
    def __init__(self) -> None:
        self._checkpoint: AutonomousEvidenceExecutionCheckpoint | None = None
        self._lock = threading.RLock()

    def read(self) -> AutonomousEvidenceExecutionCheckpoint | None:
        with self._lock:
            return self._checkpoint

    def write(self, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> None:
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        with self._lock:
            self._checkpoint = validated

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> bool:
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        with self._lock:
            current = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
            if current != expected_checkpoint_digest:
                return False
            self._checkpoint = validated
            return True


class AutonomousEvidenceExecutionCheckpointTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvidenceExecutionCheckpointTextStore(AutonomousEvidenceExecutionCheckpointTextStore, Protocol):
    def write_if_unchanged(self, expected_checkpoint_digest: str | None, value: str) -> bool: ...


class JsonAutonomousEvidenceExecutionCheckpointPersistence:
    def __init__(self, store: AutonomousEvidenceExecutionCheckpointTextStore) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            raise ArgumentError("evidence execution checkpoint JSON store is malformed")
        self.store = store

    def read(self) -> AutonomousEvidenceExecutionCheckpoint | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES:
            raise ArgumentError("evidence execution checkpoint JSON exceeds its bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("evidence execution checkpoint JSON is invalid") from error
        if not isinstance(value, Mapping) or canonical_json(value) != encoded:
            raise ArgumentError("evidence execution checkpoint JSON is not canonical")
        return validate_autonomous_evidence_execution_checkpoint(value)

    def write(self, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> None:
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        self.store.write(canonical_json(validated.to_dict()))


class TransactionalJsonAutonomousEvidenceExecutionCheckpointPersistence(JsonAutonomousEvidenceExecutionCheckpointPersistence):
    def __init__(self, store: TransactionalAutonomousEvidenceExecutionCheckpointTextStore) -> None:
        super().__init__(store)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional evidence execution checkpoint store is malformed")
        self.store = store

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> bool:
        if expected_checkpoint_digest is not None:
            _digest("evidence execution expected checkpoint digest", expected_checkpoint_digest)
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        return self.store.write_if_unchanged(expected_checkpoint_digest, canonical_json(validated.to_dict()))


def _checkpoint_for(job_id: str, execution_plan: AutonomousEvidenceExecutionPlan, request_digest: str, status: str, result: AutonomousEvidenceExecutionResult | None = None) -> AutonomousEvidenceExecutionCheckpoint:
    runtime = None if result is None else result.runtime
    payload = {
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
        "job_id": job_id,
        "evidence_plan_digest": execution_plan.evidence_plan_digest,
        "execution_plan_digest": execution_plan.plan_digest,
        "request_digest": request_digest,
        "readiness_report_digest": execution_plan.readiness.report_digest,
        "status": status,
        "runtime_status": None if runtime is None else runtime.status,
        "runtime_result_digest": None if runtime is None else runtime.result_digest,
        "completed_request_count": 0 if runtime is None else len(runtime.completed_requirement_ids),
        "pending_request_count": 0 if runtime is None else len(runtime.pending_evaluation_requirement_ids) + len(runtime.missing_requirement_ids),
        "accepted_request_count": 0 if runtime is None else sum(item.verdict == "accepted" for item in runtime.assessments),
    }
    return AutonomousEvidenceExecutionCheckpoint(
        job_id=job_id,
        evidence_plan_digest=execution_plan.evidence_plan_digest,
        execution_plan_digest=execution_plan.plan_digest,
        request_digest=request_digest,
        readiness_report_digest=execution_plan.readiness.report_digest,
        status=status,
        runtime_status=payload["runtime_status"],
        runtime_result_digest=payload["runtime_result_digest"],
        completed_request_count=payload["completed_request_count"],
        pending_request_count=payload["pending_request_count"],
        accepted_request_count=payload["accepted_request_count"],
        checkpoint_digest=content_digest(payload),
    )


def _status_for_result(result: AutonomousEvidenceExecutionResult) -> str:
    return result.status if result.status in {"completed", "awaiting_evaluation", "partial", "failed", "reconciliation_required"} else "reconciliation_required"


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionResumableRun:
    job_id: str
    status: str
    checkpoint: AutonomousEvidenceExecutionCheckpoint
    result: AutonomousEvidenceExecutionResult | None
    replayed: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA,
            "job_id": self.job_id,
            "status": self.status,
            "checkpoint_digest": self.checkpoint.checkpoint_digest,
            "execution_plan_digest": self.checkpoint.execution_plan_digest,
            "evidence_result_digest": None if self.result is None else self.result.result_digest,
            "replayed": self.replayed,
            "retention": _RESULT_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


class AutonomousEvidenceExecutionResumableController:
    """Serialize checkpoint transitions and fence writers with optional compare-and-swap."""

    def __init__(self, controller: AutonomousEvidenceExecutionController, persistence: AutonomousEvidenceExecutionCheckpointStore, job_id: str) -> None:
        if not isinstance(controller, AutonomousEvidenceExecutionController):
            raise ArgumentError("evidence execution resumable controller requires a typed execution controller")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            raise ArgumentError("evidence execution resumable persistence is malformed")
        self.controller = controller
        self.persistence = persistence
        self.job_id = _identifier("evidence execution resumable job_id", job_id)
        self._checkpoint: AutonomousEvidenceExecutionCheckpoint | None = None
        self._expected_checkpoint_digest: str | None = None
        self._restored = False
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any]:
        with self._lock:
            return self._restore_locked()

    def _restore_locked(self) -> dict[str, Any]:
        if self._restored:
            return {"status": "restored" if self._checkpoint is not None else "empty", "checkpoint_digest": self._expected_checkpoint_digest}
        raw = self.persistence.read()
        self._checkpoint = None if raw is None else validate_autonomous_evidence_execution_checkpoint(raw)
        if self._checkpoint is not None and self._checkpoint.job_id != self.job_id:
            raise ArgumentError("evidence execution checkpoint belongs to a different job")
        self._expected_checkpoint_digest = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
        self._restored = True
        return {"status": "restored" if self._checkpoint is not None else "empty", "checkpoint_digest": self._expected_checkpoint_digest}

    def _projection(self, checkpoint: AutonomousEvidenceExecutionCheckpoint, result: AutonomousEvidenceExecutionResult | None = None, replayed: bool = False) -> AutonomousEvidenceExecutionResumableRun:
        return AutonomousEvidenceExecutionResumableRun(self.job_id, checkpoint.status, checkpoint, result, replayed)

    def _commit(self, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> None:
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self._expected_checkpoint_digest, validated):
                raise ArgumentError("evidence execution checkpoint is stale; another worker committed after restore")
        else:
            self.persistence.write(validated)
        self._checkpoint = validated
        self._expected_checkpoint_digest = validated.checkpoint_digest

    def run(
        self,
        execution_plan: AutonomousEvidenceExecutionPlan,
        evidence_plan: AutonomousEvidencePlan,
        requests: Sequence[Mapping[str, Any]],
        *,
        approve_source_dispatch: bool = False,
        resume_after_reconciliation: bool = False,
        **execute_options: Any,
    ) -> AutonomousEvidenceExecutionResumableRun:
        with self._lock:
            self._restore_locked()
            if not isinstance(execution_plan, AutonomousEvidenceExecutionPlan) or not isinstance(evidence_plan, AutonomousEvidencePlan):
                raise ArgumentError("evidence execution resumable run requires typed plans")
            request_digest = evidence_execution_requests_digest(requests)
            current = self._checkpoint
            if current is not None and (current.evidence_plan_digest != execution_plan.evidence_plan_digest or current.execution_plan_digest != execution_plan.plan_digest or current.request_digest != request_digest or current.readiness_report_digest != execution_plan.readiness.report_digest):
                raise ArgumentError("evidence execution checkpoint is bound to a different plan, request set, or readiness report")
            journal = execute_options.get("journal")
            if current is not None and current.status in {"completed", "awaiting_evaluation", "partial", "failed"} and journal is None:
                return self._projection(current)
            if current is not None and current.status in {"dispatch_pending", "reconciliation_required"} and not resume_after_reconciliation:
                return self._projection(current)
            if approve_source_dispatch is not True:
                status = "approval_required" if execution_plan.status == "ready_for_review" else "blocked"
                gated = _checkpoint_for(self.job_id, execution_plan, request_digest, status)
                self._commit(gated)
                return self._projection(gated)
            if execution_plan.status != "ready_for_review":
                blocked = _checkpoint_for(self.job_id, execution_plan, request_digest, "blocked")
                self._commit(blocked)
                return self._projection(blocked)
            pending = _checkpoint_for(self.job_id, execution_plan, request_digest, "dispatch_pending")
            self._commit(pending)
            try:
                result = self.controller.execute(execution_plan, evidence_plan, requests, approve_source_dispatch=True, **execute_options)
                settled = _checkpoint_for(self.job_id, execution_plan, request_digest, _status_for_result(result), result)
                self._commit(settled)
                runtime_receipts = result.runtime.receipts
                return self._projection(settled, result, any(receipt.replay == "replayed" for receipt in runtime_receipts))
            except Exception:
                reconciliation = _checkpoint_for(self.job_id, execution_plan, request_digest, "reconciliation_required")
                self._commit(reconciliation)
                raise


__all__ = [
    "AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA",
    "MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES",
    "MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS",
    "AutonomousEvidenceExecutionCheckpoint",
    "AutonomousEvidenceExecutionCheckpointStore",
    "TransactionalAutonomousEvidenceExecutionCheckpointStore",
    "InMemoryAutonomousEvidenceExecutionCheckpointStore",
    "AutonomousEvidenceExecutionCheckpointTextStore",
    "TransactionalAutonomousEvidenceExecutionCheckpointTextStore",
    "JsonAutonomousEvidenceExecutionCheckpointPersistence",
    "TransactionalJsonAutonomousEvidenceExecutionCheckpointPersistence",
    "AutonomousEvidenceExecutionResumableRun",
    "AutonomousEvidenceExecutionResumableController",
    "evidence_execution_requests_digest",
    "validate_autonomous_evidence_execution_checkpoint",
]
