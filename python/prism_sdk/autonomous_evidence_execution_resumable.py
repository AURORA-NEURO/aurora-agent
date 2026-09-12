"""Crash-safe checkpointing for the reviewed generic evidence execution controller."""

from __future__ import annotations

from dataclasses import dataclass
import json
import threading
import types
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_execution import (
    AutonomousEvidenceExecutionController,
    AutonomousEvidenceExecutionPlan,
    AutonomousEvidenceExecutionResult,
)
from .autonomous_evidence_runtime import (
    AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
    MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeJournalEntry,
    validate_autonomous_evidence_runtime_snapshot,
)
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA = "bioprism-python-autonomous-evidence-execution-checkpoint/0.2"
AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA = "bioprism-python-autonomous-evidence-execution-resumable-result/0.1"
AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_SCHEMA = "bioprism-python-autonomous-evidence-execution-reconciliation/0.1"
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
_RECONCILIATION_OUTCOMES = frozenset({"not_executed", "succeeded", "unknown"})
_RECONCILIATION_RETENTION = "metadata_only;reconciliation_evidence_and_source_values_caller_owned"
_EXECUTION_POLICY_IDENTITY_ROLES = (
    "journal",
    "projector",
    "evaluator",
    "value_rehydrator",
    "classifier",
    "failover_observer",
    "attempt_observer",
    "clock",
    "sleeper",
    "source_boundary",
    "authorization_context",
)


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
    projections = [_request_projection(request) for request in requests]
    projection_digests = [content_digest(projection) for projection in projections]
    if len(set(projection_digests)) != len(projection_digests):
        raise ArgumentError("evidence execution checkpoint requests contain duplicates")
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
        "requests": projections,
    })


def evidence_execution_reconciliation_request_digest(evidence_plan: AutonomousEvidencePlan, request: Mapping[str, Any]) -> str:
    """Return the exact runtime request identity used by journal reconciliation."""
    if not isinstance(evidence_plan, AutonomousEvidencePlan):
        raise ArgumentError("evidence execution reconciliation requires a typed evidence plan")
    projection = _request_projection(request)
    metadata = request.get("metadata", {})
    return content_digest({
        "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
        "plan_digest": evidence_plan.plan_digest,
        "requirement_id": projection["requirement_id"],
        "source_id": projection["source_id"],
        "source_digest": projection["source_digest"],
        "request_id": projection["request_id"],
        "metadata": dict(metadata),
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
    required_requirement_count: int
    execution_policy_digest: str
    reconciliation_authority_id: str | None
    reconciliation_authority_version: str | None
    reconciliation_authority_config_digest: str | None
    reconciliation_receipt_digest: str | None
    checkpoint_generation: int
    previous_checkpoint_digest: str | None
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
            "required_requirement_count": self.required_requirement_count,
            "execution_policy_digest": self.execution_policy_digest,
            "reconciliation_authority_id": self.reconciliation_authority_id,
            "reconciliation_authority_version": self.reconciliation_authority_version,
            "reconciliation_authority_config_digest": self.reconciliation_authority_config_digest,
            "reconciliation_receipt_digest": self.reconciliation_receipt_digest,
            "checkpoint_generation": self.checkpoint_generation,
            "previous_checkpoint_digest": self.previous_checkpoint_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _checkpoint_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value[key] for key in (
        "schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest",
        "readiness_report_digest", "status", "runtime_status", "runtime_result_digest",
        "completed_request_count", "pending_request_count", "accepted_request_count",
        "required_requirement_count", "execution_policy_digest", "reconciliation_authority_id",
        "reconciliation_authority_version", "reconciliation_authority_config_digest",
        "reconciliation_receipt_digest",
        "checkpoint_generation", "previous_checkpoint_digest",
    )}


def validate_autonomous_evidence_execution_checkpoint(value: Mapping[str, Any] | AutonomousEvidenceExecutionCheckpoint) -> AutonomousEvidenceExecutionCheckpoint:
    raw = value.to_dict() if isinstance(value, AutonomousEvidenceExecutionCheckpoint) else value
    if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA:
        raise ArgumentError("evidence execution checkpoint schema is invalid")
    allowed = {
        "schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest",
        "readiness_report_digest", "status", "runtime_status", "runtime_result_digest",
        "completed_request_count", "pending_request_count", "accepted_request_count",
        "required_requirement_count", "execution_policy_digest", "reconciliation_authority_id",
        "reconciliation_authority_version", "reconciliation_authority_config_digest",
        "reconciliation_receipt_digest",
        "checkpoint_generation", "previous_checkpoint_digest", "checkpoint_digest",
        "retention", "secret_material",
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
        status=str(raw["status"]),
        runtime_status=runtime_status,
        runtime_result_digest=runtime_digest,
        completed_request_count=_integer("evidence execution checkpoint completed_request_count", raw.get("completed_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS),
        pending_request_count=_integer("evidence execution checkpoint pending_request_count", raw.get("pending_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS * 2),
        accepted_request_count=_integer("evidence execution checkpoint accepted_request_count", raw.get("accepted_request_count"), 0, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS),
        required_requirement_count=_integer("evidence execution checkpoint required_requirement_count", raw.get("required_requirement_count"), 1, MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS),
        execution_policy_digest=_digest("evidence execution checkpoint execution_policy_digest", raw.get("execution_policy_digest")),  # type: ignore[arg-type]
        reconciliation_authority_id=None if raw.get("reconciliation_authority_id") is None else _identifier("evidence execution checkpoint reconciliation_authority_id", raw.get("reconciliation_authority_id")),
        reconciliation_authority_version=None if raw.get("reconciliation_authority_version") is None else _identifier("evidence execution checkpoint reconciliation_authority_version", raw.get("reconciliation_authority_version")),
        reconciliation_authority_config_digest=_digest("evidence execution checkpoint reconciliation_authority_config_digest", raw.get("reconciliation_authority_config_digest"), allow_none=True),
        reconciliation_receipt_digest=_digest("evidence execution checkpoint reconciliation_receipt_digest", raw.get("reconciliation_receipt_digest"), allow_none=True),
        checkpoint_generation=_integer("evidence execution checkpoint checkpoint_generation", raw.get("checkpoint_generation"), 1, 9_007_199_254_740_991),
        previous_checkpoint_digest=_digest("evidence execution checkpoint previous_checkpoint_digest", raw.get("previous_checkpoint_digest"), allow_none=True),
        checkpoint_digest=_digest("evidence execution checkpoint checkpoint_digest", raw.get("checkpoint_digest")),  # type: ignore[arg-type]
    )
    if (normalized.reconciliation_authority_id is None) != (normalized.reconciliation_authority_version is None):
        raise ArgumentError("evidence execution checkpoint reconciliation authority identity is incomplete")
    if normalized.reconciliation_authority_id is None and normalized.reconciliation_authority_config_digest is not None:
        raise ArgumentError("evidence execution checkpoint reconciliation authority config has no identity")
    if (normalized.checkpoint_generation == 1) != (normalized.previous_checkpoint_digest is None):
        raise ArgumentError("evidence execution checkpoint lineage is malformed")
    has_runtime = normalized.runtime_status is not None or normalized.runtime_result_digest is not None or normalized.completed_request_count > 0 or normalized.pending_request_count > 0 or normalized.accepted_request_count > 0
    if normalized.status in {"approval_required", "blocked", "dispatch_pending"} and has_runtime:
        raise ArgumentError("pre-dispatch evidence execution checkpoint cannot contain runtime state")
    if normalized.accepted_request_count < normalized.completed_request_count:
        raise ArgumentError("evidence execution checkpoint has fewer accepted receipts than completed requirements")
    if normalized.completed_request_count > normalized.required_requirement_count:
        raise ArgumentError("evidence execution checkpoint completed count exceeds its plan")
    post_dispatch = {"completed", "awaiting_evaluation", "partial", "failed"}
    if normalized.status in post_dispatch and (
        normalized.runtime_status != normalized.status or normalized.runtime_result_digest is None
    ):
        raise ArgumentError("post-dispatch evidence execution checkpoint status does not match its runtime")
    if normalized.status == "completed" and (
        normalized.completed_request_count != normalized.required_requirement_count
        or normalized.pending_request_count != 0
    ):
        raise ArgumentError("completed evidence execution checkpoint has incomplete request counts")
    if normalized.status in {"awaiting_evaluation", "partial"} and normalized.pending_request_count == 0:
        raise ArgumentError("incomplete evidence execution checkpoint requires pending requests")
    if normalized.status == "failed" and normalized.completed_request_count != 0:
        raise ArgumentError("failed evidence execution checkpoint cannot contain completed requests")
    if normalized.status in post_dispatch and (
        normalized.completed_request_count + normalized.pending_request_count
        != normalized.required_requirement_count
    ):
        raise ArgumentError("post-dispatch evidence execution checkpoint counts do not cover its plan")
    if normalized.status == "reconciliation_required" and has_runtime and (
        normalized.runtime_status != "reconciliation_required" or normalized.runtime_result_digest is None
    ):
        raise ArgumentError("evidence execution reconciliation checkpoint runtime state is inconsistent")
    if normalized.status == "reconciliation_required" and has_runtime and (
        normalized.completed_request_count + normalized.pending_request_count
        != normalized.required_requirement_count
    ):
        raise ArgumentError("evidence execution reconciliation checkpoint counts do not cover its plan")
    if normalized.reconciliation_receipt_digest is not None and normalized.status in {"approval_required", "blocked"}:
        raise ArgumentError("pre-dispatch approval checkpoints cannot carry a reconciliation receipt")
    if raw.get("retention") != _RETENTION or raw.get("secret_material") != _SECRET_MATERIAL:
        raise ArgumentError("evidence execution checkpoint retention contract is invalid")
    if content_digest(_checkpoint_payload(raw)) != normalized.checkpoint_digest:
        raise ArgumentError("evidence execution checkpoint digest is invalid")
    if len(canonical_json(raw).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES:
        raise ArgumentError("evidence execution checkpoint exceeds its bound")
    return normalized


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionReconciliationOutcome:
    request_digest: str
    outcome: str
    evidence_digest: str
    evidence_kind: str
    effect_absent: bool
    runtime_receipt_digest: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "request_digest": self.request_digest,
            "outcome": self.outcome,
            "evidence_digest": self.evidence_digest,
            "evidence_kind": self.evidence_kind,
            "effect_absent": self.effect_absent,
            "runtime_receipt_digest": self.runtime_receipt_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceExecutionReconciliationReceipt:
    job_id: str
    checkpoint_digest: str
    evidence_plan_digest: str
    execution_plan_digest: str
    request_set_digest: str
    authority_id: str
    authority_version: str
    outcomes: tuple[AutonomousEvidenceExecutionReconciliationOutcome, ...]
    receipt_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_SCHEMA,
            "job_id": self.job_id,
            "checkpoint_digest": self.checkpoint_digest,
            "evidence_plan_digest": self.evidence_plan_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "request_set_digest": self.request_set_digest,
            "authority_id": self.authority_id,
            "authority_version": self.authority_version,
            "outcomes": [item.to_dict() for item in self.outcomes],
            "receipt_digest": self.receipt_digest,
            "retention": _RECONCILIATION_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


def _reconciliation_outcome(value: Mapping[str, Any]) -> AutonomousEvidenceExecutionReconciliationOutcome:
    if not isinstance(value, Mapping) or set(value) != {
        "request_digest", "outcome", "evidence_digest", "evidence_kind", "effect_absent", "runtime_receipt_digest",
    }:
        raise ArgumentError("evidence execution reconciliation outcome is malformed")
    outcome = value.get("outcome")
    if outcome not in _RECONCILIATION_OUTCOMES:
        raise ArgumentError("evidence execution reconciliation outcome is invalid")
    effect_absent = value.get("effect_absent")
    if not isinstance(effect_absent, bool):
        raise ArgumentError("evidence execution reconciliation effect_absent must be boolean")
    if (outcome == "not_executed") != effect_absent:
        raise ArgumentError("evidence execution reconciliation outcome contradicts effect_absent")
    runtime_receipt_digest = _digest(
        "evidence execution reconciliation runtime_receipt_digest",
        value.get("runtime_receipt_digest"),
        allow_none=True,
    )
    if outcome == "succeeded" and runtime_receipt_digest is None:
        raise ArgumentError("succeeded evidence execution reconciliation requires a runtime receipt digest")
    if outcome == "not_executed" and runtime_receipt_digest is not None:
        raise ArgumentError("not_executed evidence execution reconciliation cannot carry a runtime receipt digest")
    return AutonomousEvidenceExecutionReconciliationOutcome(
        request_digest=_digest("evidence execution reconciliation request_digest", value.get("request_digest")),  # type: ignore[arg-type]
        outcome=outcome,
        evidence_digest=_digest("evidence execution reconciliation evidence_digest", value.get("evidence_digest")),  # type: ignore[arg-type]
        evidence_kind=_identifier("evidence execution reconciliation evidence_kind", value.get("evidence_kind")),
        effect_absent=effect_absent,
        runtime_receipt_digest=runtime_receipt_digest,
    )


def _reconciliation_receipt_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value[key] for key in (
        "schema", "job_id", "checkpoint_digest", "evidence_plan_digest", "execution_plan_digest",
        "request_set_digest", "authority_id", "authority_version", "outcomes", "retention", "secret_material",
    )}


def validate_autonomous_evidence_execution_reconciliation_receipt(
    value: Mapping[str, Any] | AutonomousEvidenceExecutionReconciliationReceipt,
) -> AutonomousEvidenceExecutionReconciliationReceipt:
    raw = value.to_dict() if isinstance(value, AutonomousEvidenceExecutionReconciliationReceipt) else value
    allowed = {
        "schema", "job_id", "checkpoint_digest", "evidence_plan_digest", "execution_plan_digest",
        "request_set_digest", "authority_id", "authority_version", "outcomes", "receipt_digest",
        "retention", "secret_material",
    }
    if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_SCHEMA or set(raw) != allowed:
        raise ArgumentError("evidence execution reconciliation receipt schema is invalid")
    raw_outcomes = raw.get("outcomes")
    if not isinstance(raw_outcomes, Sequence) or isinstance(raw_outcomes, (str, bytes, bytearray)) or not 1 <= len(raw_outcomes) <= MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS:
        raise ArgumentError("evidence execution reconciliation outcomes are outside their bound")
    outcomes = tuple(_reconciliation_outcome(item) for item in raw_outcomes)
    request_digests = [item.request_digest for item in outcomes]
    if request_digests != sorted(request_digests) or len(set(request_digests)) != len(request_digests):
        raise ArgumentError("evidence execution reconciliation outcomes must be unique and sorted")
    normalized = AutonomousEvidenceExecutionReconciliationReceipt(
        job_id=_identifier("evidence execution reconciliation job_id", raw.get("job_id")),
        checkpoint_digest=_digest("evidence execution reconciliation checkpoint_digest", raw.get("checkpoint_digest")),  # type: ignore[arg-type]
        evidence_plan_digest=_digest("evidence execution reconciliation evidence_plan_digest", raw.get("evidence_plan_digest")),  # type: ignore[arg-type]
        execution_plan_digest=_digest("evidence execution reconciliation execution_plan_digest", raw.get("execution_plan_digest")),  # type: ignore[arg-type]
        request_set_digest=_digest("evidence execution reconciliation request_set_digest", raw.get("request_set_digest")),  # type: ignore[arg-type]
        authority_id=_identifier("evidence execution reconciliation authority_id", raw.get("authority_id")),
        authority_version=_identifier("evidence execution reconciliation authority_version", raw.get("authority_version")),
        outcomes=outcomes,
        receipt_digest=_digest("evidence execution reconciliation receipt_digest", raw.get("receipt_digest")),  # type: ignore[arg-type]
    )
    if raw.get("retention") != _RECONCILIATION_RETENTION or raw.get("secret_material") != _SECRET_MATERIAL:
        raise ArgumentError("evidence execution reconciliation receipt retention contract is invalid")
    canonical = normalized.to_dict()
    if content_digest(_reconciliation_receipt_payload(canonical)) != normalized.receipt_digest:
        raise ArgumentError("evidence execution reconciliation receipt digest is invalid")
    if len(canonical_json(canonical).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES:
        raise ArgumentError("evidence execution reconciliation receipt exceeds its byte bound")
    return normalized


def create_autonomous_evidence_execution_reconciliation_receipt(
    checkpoint: Mapping[str, Any] | AutonomousEvidenceExecutionCheckpoint,
    execution_plan: AutonomousEvidenceExecutionPlan,
    evidence_plan: AutonomousEvidencePlan,
    requests: Sequence[Mapping[str, Any]],
    *,
    authority_id: str,
    authority_version: str,
    outcomes: Sequence[Mapping[str, Any]],
) -> AutonomousEvidenceExecutionReconciliationReceipt:
    """Create an exact, caller-attested outcome ledger for one uncertain dispatch boundary."""
    current = validate_autonomous_evidence_execution_checkpoint(checkpoint)
    if current.status not in {"dispatch_pending", "reconciliation_required"}:
        raise ArgumentError("evidence execution checkpoint is not awaiting dispatch reconciliation")
    if current.reconciliation_authority_id is None or current.reconciliation_authority_version is None:
        raise ArgumentError("evidence execution checkpoint has no configured reconciliation authority")
    if not isinstance(execution_plan, AutonomousEvidenceExecutionPlan) or not isinstance(evidence_plan, AutonomousEvidencePlan):
        raise ArgumentError("evidence execution reconciliation requires typed plans")
    request_set_digest = evidence_execution_requests_digest(requests)
    if (
        current.evidence_plan_digest != evidence_plan.plan_digest
        or current.evidence_plan_digest != execution_plan.evidence_plan_digest
        or current.execution_plan_digest != execution_plan.plan_digest
        or current.request_digest != request_set_digest
        or current.required_requirement_count != len(evidence_plan.requirements)
    ):
        raise ArgumentError("evidence execution reconciliation inputs do not match the checkpoint")
    normalized_outcomes = tuple(sorted((_reconciliation_outcome(item) for item in outcomes), key=lambda item: item.request_digest))
    expected = {evidence_execution_reconciliation_request_digest(evidence_plan, request) for request in requests}
    observed = {item.request_digest for item in normalized_outcomes}
    if len(normalized_outcomes) != len(expected) or observed != expected:
        raise ArgumentError("evidence execution reconciliation must cover the exact request set")
    payload = {
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_SCHEMA,
        "job_id": current.job_id,
        "checkpoint_digest": current.checkpoint_digest,
        "evidence_plan_digest": current.evidence_plan_digest,
        "execution_plan_digest": current.execution_plan_digest,
        "request_set_digest": current.request_digest,
        "authority_id": _identifier("evidence execution reconciliation authority_id", authority_id),
        "authority_version": _identifier("evidence execution reconciliation authority_version", authority_version),
        "outcomes": [item.to_dict() for item in normalized_outcomes],
        "retention": _RECONCILIATION_RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    if (
        payload["authority_id"] != current.reconciliation_authority_id
        or payload["authority_version"] != current.reconciliation_authority_version
    ):
        raise ArgumentError("evidence execution reconciliation authority does not match the checkpoint trust root")
    return validate_autonomous_evidence_execution_reconciliation_receipt({
        **payload,
        "receipt_digest": content_digest(payload),
    })


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
        return self.store.write_if_unchanged(  # type: ignore[attr-defined]
            expected_checkpoint_digest,
            canonical_json(validated.to_dict()),
        )


def _explicit_execution_component_identity(name: str, value: Any) -> dict[str, str | None]:
    if not isinstance(value, Mapping) or set(value) - {"id", "version", "config_digest"}:
        raise ArgumentError(
            f"evidence execution policy identity {name} must contain only id, version, and optional config_digest"
        )
    if "id" not in value or "version" not in value:
        raise ArgumentError(f"evidence execution policy identity {name} requires id and version")
    return {
        "id": _identifier(f"evidence execution policy identity {name} id", value.get("id")),
        "version": _identifier(
            f"evidence execution policy identity {name} version", value.get("version")
        ),
        "config_digest": _digest(
            f"evidence execution policy identity {name} config_digest",
            value.get("config_digest"),
            allow_none=True,
        ),
    }


def _normalize_execution_policy_identity(value: Any) -> dict[str, dict[str, str | None]]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence execution resumable_policy_identity must be a mapping")
    unknown = sorted(
        str(key)
        for key in value
        if not isinstance(key, str) or key not in _EXECUTION_POLICY_IDENTITY_ROLES
    )
    if unknown:
        raise ArgumentError(
            "evidence execution resumable_policy_identity contains unsupported roles: "
            + ", ".join(unknown)
        )
    return {
        role: _explicit_execution_component_identity(role, value[role])
        for role in _EXECUTION_POLICY_IDENTITY_ROLES
        if role in value
    }


def _function_execution_identity(role: str, value: types.FunctionType) -> dict[str, str | None]:
    if value.__closure__:
        raise ArgumentError(
            f"evidence execution resumable {role} closes over caller state; provide "
            f"resumable_policy_identity.{role} with a config_digest"
        )
    constants: list[Any] = []
    for item in value.__code__.co_consts:
        if item is None or isinstance(item, (str, int, float, bool)):
            constants.append(item)
        elif isinstance(item, tuple) and all(
            child is None or isinstance(child, (str, int, float, bool)) for child in item
        ):
            constants.append(list(item))
        elif isinstance(item, types.CodeType):
            constants.append({"code": item.co_code.hex(), "names": list(item.co_names)})
        else:
            raise ArgumentError(
                f"evidence execution resumable {role} has an opaque code constant; provide "
                f"resumable_policy_identity.{role}"
            )
    descriptor = {
        "module": value.__module__,
        "qualname": value.__qualname__,
        "code": value.__code__.co_code.hex(),
        "names": list(value.__code__.co_names),
        "constants": constants,
        "defaults": value.__defaults__,
        "kwdefaults": value.__kwdefaults__,
    }
    try:
        version = content_digest(descriptor)
    except (TypeError, ValueError) as error:
        raise ArgumentError(
            f"evidence execution resumable {role} has opaque defaults; provide "
            f"resumable_policy_identity.{role}"
        ) from error
    location = content_digest({"module": value.__module__, "qualname": value.__qualname__})
    return {
        "id": f"python-function-{location[:24]}",
        "version": f"sha256-{version}",
        "config_digest": None,
    }


def _declared_execution_component_identity(role: str, value: Any) -> dict[str, str | None] | None:
    id_names: Sequence[str] = (f"{role}_id", "resumable_id", "adapter_id")
    version_names: Sequence[str] = (
        f"{role}_version",
        "resumable_version",
        "adapter_version",
        "version",
    )
    if role == "evaluator":
        id_names = ("evaluator_id", *id_names)
        version_names = ("evaluator_version", *version_names)

    def first(names: Sequence[str]) -> Any:
        for name in names:
            found = getattr(value, name, None)
            if found is not None:
                return found
        return None

    identity = first(id_names)
    version = first(version_names)
    if identity is None and version is None:
        return None
    if identity is None or version is None:
        raise ArgumentError(
            f"evidence execution resumable {role} declares an incomplete stable identity"
        )
    return {
        "id": _identifier(f"evidence execution resumable {role} id", identity),
        "version": _identifier(f"evidence execution resumable {role} version", version),
        "config_digest": _digest(
            f"evidence execution resumable {role} config_digest",
            first((f"{role}_config_digest", "config_digest", "manifest_digest")),
            allow_none=True,
        ),
    }


def _execution_component_identity(
    role: str,
    value: Any,
    explicit: dict[str, str | None] | None,
) -> dict[str, str | None] | None:
    if explicit is not None:
        if value is None and role not in {"journal", "value_rehydrator"}:
            raise ArgumentError(f"evidence execution policy identity declares absent component {role}")
        return explicit
    if value is None:
        return None
    if isinstance(value, types.MethodType):
        declared = _declared_execution_component_identity(role, value.__self__)
        if declared is None:
            raise ArgumentError(
                f"evidence execution resumable bound {role} has no stable identity; provide "
                f"resumable_policy_identity.{role}"
            )
        return declared
    if isinstance(value, types.FunctionType):
        return _function_execution_identity(role, value)
    declared = _declared_execution_component_identity(role, value)
    if declared is None:
        raise ArgumentError(
            f"evidence execution resumable {role} has no stable identity; provide "
            f"resumable_policy_identity.{role}"
        )
    return declared


def _execution_policy_digest(
    execute_options: Mapping[str, Any],
    explicit_identity: Mapping[str, Any] | None,
) -> str:
    supported = {
        "provider_contracts", "source_boundary", "projector", "evaluator", "journal",
        "rehydrate_value", "parent_evidence_digests", "stop_on_failure", "reevaluate_pending",
        "classify", "observe_failover", "observe_attempt", "clock", "sleep",
        "authorization_context", "authorization_domain", "authorization_capability",
        "authorization_risk_class",
    }
    unknown = sorted(str(key) for key in execute_options if key not in supported)
    if unknown:
        raise ArgumentError("evidence execution resumable options are unsupported: " + ", ".join(unknown))
    supplied = _normalize_execution_policy_identity(explicit_identity)
    role_values = {
        "journal": execute_options.get("journal"),
        "projector": execute_options.get("projector"),
        "evaluator": execute_options.get("evaluator"),
        "value_rehydrator": execute_options.get("rehydrate_value"),
        "classifier": execute_options.get("classify"),
        "failover_observer": execute_options.get("observe_failover"),
        "attempt_observer": execute_options.get("observe_attempt"),
        "clock": execute_options.get("clock"),
        "sleeper": execute_options.get("sleep"),
        "source_boundary": (
            None
            if execute_options.get("source_boundary") is None
            or not isinstance(execute_options.get("source_boundary"), Mapping)
            else execute_options["source_boundary"].get("describe_source")
        ),
        "authorization_context": execute_options.get("authorization_context"),
    }
    components = {
        role: _execution_component_identity(role, value, supplied.get(role))
        for role, value in role_values.items()
    }
    parents = execute_options.get("parent_evidence_digests", ())
    if not isinstance(parents, Sequence) or isinstance(parents, (str, bytes, bytearray)) or len(parents) > 64:
        raise ArgumentError("evidence execution parent_evidence_digests are outside their bound")
    normalized_parents = [
        _digest("evidence execution parent evidence digest", value) for value in parents
    ]
    stop_on_failure = execute_options.get("stop_on_failure", False)
    reevaluate_pending = execute_options.get("reevaluate_pending", False)
    if not isinstance(stop_on_failure, bool) or not isinstance(reevaluate_pending, bool):
        raise ArgumentError("evidence execution policy booleans are malformed")
    overrides: dict[str, str | None] = {}
    for key in ("authorization_domain", "authorization_capability", "authorization_risk_class"):
        value = execute_options.get(key)
        overrides[key] = None if value is None else _identifier(f"evidence execution {key}", value)
    context = execute_options.get("authorization_context")
    context_binding = None
    if context is not None:
        context_binding = {
            "grant_id": _identifier("evidence execution authorization grant_id", getattr(context, "grant_id", None)),
            "tenant_id": _identifier("evidence execution authorization tenant_id", getattr(context, "tenant_id", None)),
            "actor_id": _identifier("evidence execution authorization actor_id", getattr(context, "actor_id", None)),
            "session_id": _identifier("evidence execution authorization session_id", getattr(context, "session_id", None)),
            "authorization_digest": _digest("evidence execution authorization digest", getattr(context, "authorization_digest", None)),
            "domains": list(getattr(context, "domains", ())),
            "capability": getattr(context, "capability", None),
            "risk_class": getattr(context, "risk_class", None),
            "request_prefix": getattr(context, "request_prefix", None),
        }
    source_boundary = execute_options.get("source_boundary")
    source_boundary_binding = None
    if source_boundary is not None:
        if not isinstance(source_boundary, Mapping):
            raise ArgumentError("evidence execution source_boundary is malformed")
        policy = source_boundary.get("policy")
        describe_source = source_boundary.get("describe_source")
        if not callable(describe_source):
            raise ArgumentError("evidence execution source_boundary requires describe_source")
        source_policy_digest = _digest(
            "evidence execution source boundary policy digest",
            getattr(policy, "policy_digest", None),
        )
        source_kind = source_boundary.get("source_kind")
        source_boundary_binding = {
            "policy_digest": source_policy_digest,
            "source_kind": (
                None
                if source_kind is None
                else _identifier("evidence execution source boundary source_kind", source_kind)
            ),
        }
    provider_contracts = execute_options.get("provider_contracts")
    provider_contracts_digest = (
        None
        if provider_contracts is None
        else _digest(
            "evidence execution provider contract registry digest",
            getattr(provider_contracts, "registry_digest", None),
        )
    )
    payload = {
        "schema": AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
        "components": components,
        "parent_evidence_digests": normalized_parents,
        "stop_on_failure": stop_on_failure,
        "reevaluate_pending": reevaluate_pending,
        "authorization_context": context_binding,
        "authorization_overrides": overrides,
        "source_boundary": source_boundary_binding,
        "provider_contracts_digest": provider_contracts_digest,
    }
    try:
        encoded = canonical_json(payload).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("evidence execution policy identity is not JSON-safe") from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES:
        raise ArgumentError("evidence execution policy identity exceeds its byte bound")
    return content_digest(payload)


def _checkpoint_for(
    job_id: str,
    execution_plan: AutonomousEvidenceExecutionPlan,
    request_digest: str,
    required_requirement_count: int,
    execution_policy_digest: str,
    reconciliation_authority_id: str | None,
    reconciliation_authority_version: str | None,
    reconciliation_authority_config_digest: str | None,
    previous_checkpoint: AutonomousEvidenceExecutionCheckpoint | None,
    status: str,
    result: AutonomousEvidenceExecutionResult | None = None,
    *,
    reconciliation_receipt_digest: str | None = None,
) -> AutonomousEvidenceExecutionCheckpoint:
    runtime = None if result is None else result.runtime
    generation = 1 if previous_checkpoint is None else previous_checkpoint.checkpoint_generation + 1
    previous_digest = None if previous_checkpoint is None else previous_checkpoint.checkpoint_digest
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
        "required_requirement_count": required_requirement_count,
        "execution_policy_digest": execution_policy_digest,
        "reconciliation_authority_id": reconciliation_authority_id,
        "reconciliation_authority_version": reconciliation_authority_version,
        "reconciliation_authority_config_digest": reconciliation_authority_config_digest,
        "reconciliation_receipt_digest": reconciliation_receipt_digest,
        "checkpoint_generation": generation,
        "previous_checkpoint_digest": previous_digest,
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
        required_requirement_count=payload["required_requirement_count"],
        execution_policy_digest=payload["execution_policy_digest"],
        reconciliation_authority_id=payload["reconciliation_authority_id"],
        reconciliation_authority_version=payload["reconciliation_authority_version"],
        reconciliation_authority_config_digest=payload["reconciliation_authority_config_digest"],
        reconciliation_receipt_digest=payload["reconciliation_receipt_digest"],
        checkpoint_generation=payload["checkpoint_generation"],
        previous_checkpoint_digest=payload["previous_checkpoint_digest"],
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


class _SnapshotAutonomousEvidenceRuntimeJournal:
    """Validate and replay one stable journal view while forwarding new appends."""

    def __init__(
        self,
        journal: AutonomousEvidenceRuntimeJournal,
        records: Sequence[AutonomousEvidenceRuntimeJournalEntry],
    ) -> None:
        self._journal = journal
        self._records = list(records)

    def records(self) -> tuple[AutonomousEvidenceRuntimeJournalEntry, ...]:
        return tuple(self._records)

    def append(
        self, entry: AutonomousEvidenceRuntimeJournalEntry
    ) -> AutonomousEvidenceRuntimeJournalEntry:
        persisted = self._journal.append(entry)
        self._records.append(persisted)
        return persisted


def _validated_journal_records(
    journal: AutonomousEvidenceRuntimeJournal,
    evidence_plan: AutonomousEvidencePlan,
) -> tuple[AutonomousEvidenceRuntimeJournalEntry, ...]:
    records = tuple(journal.records())
    if len(records) > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS:
        raise ArgumentError("evidence execution reconciliation journal exceeds its bound")
    try:
        serialized = [entry.to_dict() for entry in records]
    except (AttributeError, TypeError) as error:
        raise ArgumentError(
            "evidence execution reconciliation journal contains a malformed entry"
        ) from error
    descriptor = {
        "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
        "snapshot_generation": 1,
        "previous_snapshot_digest": None,
        "plan_digest": evidence_plan.plan_digest,
        "entries": serialized,
        "head_digest": records[-1].entry_digest if records else None,
        "retention": "metadata_only_hash_bound",
        "secret_material": "never_returned",
    }
    snapshot = validate_autonomous_evidence_runtime_snapshot(
        {**descriptor, "snapshot_digest": content_digest(descriptor)},
        expected_plan_digest=evidence_plan.plan_digest,
    )
    return snapshot.entries


def _bind_reconciliation_to_journal(
    receipt: AutonomousEvidenceExecutionReconciliationReceipt,
    evidence_plan: AutonomousEvidencePlan,
    requests: Sequence[Mapping[str, Any]],
    journal: AutonomousEvidenceRuntimeJournal | None,
    rehydrate_value: Callable[[Mapping[str, Any]], Any] | None,
) -> tuple[
    AutonomousEvidenceRuntimeJournal | None,
    Callable[[Mapping[str, Any]], Any] | None,
]:
    records = () if journal is None else _validated_journal_records(journal, evidence_plan)
    snapshot_journal: AutonomousEvidenceRuntimeJournal | None = None
    if journal is not None:
        snapshot_journal = _SnapshotAutonomousEvidenceRuntimeJournal(journal, records)
    request_by_digest = {
        evidence_execution_reconciliation_request_digest(evidence_plan, request):
        _request_projection(request)
        for request in requests
    }
    latest: dict[str, Any] = {}
    for entry in records:
        runtime_receipt = getattr(entry, "receipt", None)
        request_digest = getattr(runtime_receipt, "request_digest", None)
        if not isinstance(request_digest, str):
            raise ArgumentError("evidence execution reconciliation journal contains a malformed receipt")
        latest[request_digest] = runtime_receipt
    rehydrated: dict[str, Any] = {}
    for outcome in receipt.outcomes:
        prior = latest.get(outcome.request_digest)
        if outcome.outcome == "not_executed":
            if prior is not None:
                raise ArgumentError("not_executed reconciliation contradicts an existing runtime receipt")
            continue
        if outcome.outcome == "succeeded":
            if prior is None or prior.receipt_digest != outcome.runtime_receipt_digest or prior.plan_digest != receipt.evidence_plan_digest:
                raise ArgumentError("succeeded reconciliation does not match an existing runtime receipt")
            expected_request = request_by_digest.get(outcome.request_digest)
            if expected_request is None or (
                prior.requirement_id != expected_request["requirement_id"]
                or prior.source_id != expected_request["source_id"]
                or prior.source_digest != expected_request["source_digest"]
            ):
                raise ArgumentError(
                    "succeeded reconciliation runtime receipt does not match its request"
                )
            if prior.status in {"failed", "reconciliation_required"} or prior.value_digest is None:
                raise ArgumentError("succeeded reconciliation requires a value-bearing successful runtime receipt")
            if rehydrate_value is None:
                raise ArgumentError("succeeded reconciliation requires the caller-owned value rehydrator")
            value = rehydrate_value(prior.to_dict())
            if value is None or content_digest(value) != prior.value_digest:
                raise ArgumentError(
                    "succeeded reconciliation value does not match its runtime receipt digest"
                )
            rehydrated[prior.receipt_digest] = value
            continue
        if outcome.runtime_receipt_digest is not None and (prior is None or prior.receipt_digest != outcome.runtime_receipt_digest):
            raise ArgumentError("unknown reconciliation runtime receipt does not match the journal")

    if rehydrate_value is None:
        return snapshot_journal, None

    def replay_value(runtime_receipt: Mapping[str, Any]) -> Any:
        receipt_digest = runtime_receipt.get("receipt_digest")
        if isinstance(receipt_digest, str) and receipt_digest in rehydrated:
            return rehydrated[receipt_digest]
        return rehydrate_value(runtime_receipt)

    return snapshot_journal, replay_value


class AutonomousEvidenceExecutionResumableController:
    """Serialize checkpoint transitions; source dispatch always requires compare-and-swap."""

    def __init__(
        self,
        controller: AutonomousEvidenceExecutionController,
        persistence: AutonomousEvidenceExecutionCheckpointStore,
        job_id: str,
        *,
        reconciliation_authority_id: str | None = None,
        reconciliation_authority_version: str | None = None,
        reconciliation_authority_config_digest: str | None = None,
    ) -> None:
        if not isinstance(controller, AutonomousEvidenceExecutionController):
            raise ArgumentError("evidence execution resumable controller requires a typed execution controller")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            raise ArgumentError("evidence execution resumable persistence is malformed")
        self.controller = controller
        self.persistence = persistence
        self.job_id = _identifier("evidence execution resumable job_id", job_id)
        if (reconciliation_authority_id is None) != (reconciliation_authority_version is None):
            raise ArgumentError("evidence execution reconciliation authority identity is incomplete")
        self.reconciliation_authority_id = (
            None
            if reconciliation_authority_id is None
            else _identifier(
                "evidence execution reconciliation authority_id",
                reconciliation_authority_id,
            )
        )
        self.reconciliation_authority_version = (
            None
            if reconciliation_authority_version is None
            else _identifier(
                "evidence execution reconciliation authority_version",
                reconciliation_authority_version,
            )
        )
        self.reconciliation_authority_config_digest = _digest(
            "evidence execution reconciliation authority config_digest",
            reconciliation_authority_config_digest,
            allow_none=True,
        )
        if self.reconciliation_authority_id is None and self.reconciliation_authority_config_digest is not None:
            raise ArgumentError("evidence execution reconciliation authority config has no identity")
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
        if self._checkpoint is not None and (
            self._checkpoint.reconciliation_authority_id != self.reconciliation_authority_id
            or self._checkpoint.reconciliation_authority_version
            != self.reconciliation_authority_version
            or self._checkpoint.reconciliation_authority_config_digest
            != self.reconciliation_authority_config_digest
        ):
            raise ArgumentError(
                "evidence execution checkpoint reconciliation authority differs from the configured trust root"
            )
        self._expected_checkpoint_digest = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
        self._restored = True
        return {"status": "restored" if self._checkpoint is not None else "empty", "checkpoint_digest": self._expected_checkpoint_digest}

    def _projection(self, checkpoint: AutonomousEvidenceExecutionCheckpoint, result: AutonomousEvidenceExecutionResult | None = None, replayed: bool = False) -> AutonomousEvidenceExecutionResumableRun:
        return AutonomousEvidenceExecutionResumableRun(self.job_id, checkpoint.status, checkpoint, result, replayed)

    def _commit(self, checkpoint: AutonomousEvidenceExecutionCheckpoint) -> None:
        validated = validate_autonomous_evidence_execution_checkpoint(checkpoint)
        expected_generation = 1 if self._checkpoint is None else self._checkpoint.checkpoint_generation + 1
        expected_predecessor = None if self._checkpoint is None else self._checkpoint.checkpoint_digest
        if (
            validated.checkpoint_generation != expected_generation
            or validated.previous_checkpoint_digest != expected_predecessor
        ):
            raise ArgumentError("evidence execution checkpoint does not extend the current lineage")
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
        reconciliation_receipt: Mapping[str, Any] | AutonomousEvidenceExecutionReconciliationReceipt | None = None,
        resumable_policy_identity: Mapping[str, Any] | None = None,
        **execute_options: Any,
    ) -> AutonomousEvidenceExecutionResumableRun:
        with self._lock:
            self._restore_locked()
            if not isinstance(execution_plan, AutonomousEvidenceExecutionPlan) or not isinstance(evidence_plan, AutonomousEvidencePlan):
                raise ArgumentError("evidence execution resumable run requires typed plans")
            request_digest = evidence_execution_requests_digest(requests)
            required_requirement_count = len(evidence_plan.requirements)
            if not 1 <= required_requirement_count <= MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_REQUESTS:
                raise ArgumentError("evidence execution requirement count is outside its bound")
            execution_policy_digest = _execution_policy_digest(
                execute_options,
                resumable_policy_identity,
            )
            current = self._checkpoint
            if current is not None and (
                current.evidence_plan_digest != execution_plan.evidence_plan_digest
                or current.execution_plan_digest != execution_plan.plan_digest
                or current.request_digest != request_digest
                or current.readiness_report_digest != execution_plan.readiness.report_digest
                or current.required_requirement_count != required_requirement_count
                or current.execution_policy_digest != execution_policy_digest
            ):
                raise ArgumentError(
                    "evidence execution checkpoint is bound to a different plan, request set, readiness report, or execution policy"
                )

            def make_checkpoint(
                status: str,
                result: AutonomousEvidenceExecutionResult | None = None,
                *,
                reconciliation_receipt_digest: str | None = None,
            ) -> AutonomousEvidenceExecutionCheckpoint:
                return _checkpoint_for(
                    self.job_id,
                    execution_plan,
                    request_digest,
                    required_requirement_count,
                    execution_policy_digest,
                    self.reconciliation_authority_id,
                    self.reconciliation_authority_version,
                    self.reconciliation_authority_config_digest,
                    self._checkpoint,
                    status,
                    result,
                    reconciliation_receipt_digest=reconciliation_receipt_digest,
                )

            journal = execute_options.get("journal")
            rehydrate_value = execute_options.get("rehydrate_value")
            if current is not None and current.status in {"completed", "awaiting_evaluation", "partial", "failed"} and journal is None:
                if reconciliation_receipt is not None:
                    raise ArgumentError("evidence execution reconciliation receipt was supplied outside a reconciliation boundary")
                return self._projection(current)
            if approve_source_dispatch is True:
                if not callable(getattr(self.persistence, "write_if_unchanged", None)):
                    raise ArgumentError(
                        "evidence execution source dispatch requires transactional compare-and-swap persistence"
                    )
                if self.reconciliation_authority_id is None:
                    raise ArgumentError(
                        "evidence execution source dispatch requires a configured reconciliation authority"
                    )
                if journal is None:
                    raise ArgumentError(
                        "evidence execution source dispatch requires a caller-owned runtime journal"
                    )
                if not (
                    isinstance(resumable_policy_identity, Mapping)
                    and "journal" in resumable_policy_identity
                ) and _declared_execution_component_identity("journal", journal) is None:
                    raise ArgumentError(
                        "evidence execution source dispatch requires a stable journal policy identity"
                    )
                if execute_options.get("rehydrate_value") is None and not (
                    isinstance(resumable_policy_identity, Mapping)
                    and "value_rehydrator" in resumable_policy_identity
                ):
                    raise ArgumentError(
                        "evidence execution source dispatch requires a reserved value_rehydrator policy identity"
                    )
            if journal is not None:
                records = _validated_journal_records(journal, evidence_plan)
                journal = _SnapshotAutonomousEvidenceRuntimeJournal(journal, records)
                execute_options["journal"] = journal
            accepted_reconciliation_digest: str | None = None
            if current is not None and current.status in {"dispatch_pending", "reconciliation_required"}:
                if resume_after_reconciliation:
                    raise ArgumentError("resume_after_reconciliation cannot authorize source redispatch; provide a typed reconciliation_receipt")
                if reconciliation_receipt is None:
                    return self._projection(current)
                receipt = validate_autonomous_evidence_execution_reconciliation_receipt(reconciliation_receipt)
                if (
                    receipt.job_id != self.job_id
                    or receipt.checkpoint_digest != current.checkpoint_digest
                    or receipt.evidence_plan_digest != evidence_plan.plan_digest
                    or receipt.execution_plan_digest != execution_plan.plan_digest
                    or receipt.request_set_digest != request_digest
                    or receipt.authority_id != self.reconciliation_authority_id
                    or receipt.authority_version != self.reconciliation_authority_version
                ):
                    raise ArgumentError("evidence execution reconciliation receipt does not match the current checkpoint")
                expected_request_digests = {
                    evidence_execution_reconciliation_request_digest(evidence_plan, request)
                    for request in requests
                }
                if {item.request_digest for item in receipt.outcomes} != expected_request_digests:
                    raise ArgumentError("evidence execution reconciliation receipt does not cover the current request set")
                reconciled_journal, reconciled_rehydrate_value = _bind_reconciliation_to_journal(
                    receipt,
                    evidence_plan,
                    requests,
                    journal,
                    rehydrate_value if callable(rehydrate_value) else None,
                )
                if any(item.outcome == "unknown" for item in receipt.outcomes):
                    held = make_checkpoint(
                        "reconciliation_required",
                        reconciliation_receipt_digest=receipt.receipt_digest,
                    )
                    self._commit(held)
                    return self._projection(held)
                accepted_reconciliation_digest = receipt.receipt_digest
                if reconciled_journal is not None:
                    execute_options["journal"] = reconciled_journal
                if reconciled_rehydrate_value is not None:
                    execute_options["rehydrate_value"] = reconciled_rehydrate_value
            elif reconciliation_receipt is not None:
                raise ArgumentError("evidence execution reconciliation receipt was supplied outside a reconciliation boundary")
            elif resume_after_reconciliation:
                raise ArgumentError("resume_after_reconciliation is not a dispatch authority")
            if approve_source_dispatch is not True:
                if accepted_reconciliation_digest is not None:
                    return self._projection(current)  # type: ignore[arg-type]
                status = "approval_required" if execution_plan.status == "ready_for_review" else "blocked"
                gated = make_checkpoint(status)
                self._commit(gated)
                return self._projection(gated)
            if execution_plan.status != "ready_for_review":
                blocked = make_checkpoint("blocked")
                self._commit(blocked)
                return self._projection(blocked)
            pending = make_checkpoint(
                "dispatch_pending",
                reconciliation_receipt_digest=accepted_reconciliation_digest,
            )
            self._commit(pending)
            try:
                result = self.controller.execute(execution_plan, evidence_plan, requests, approve_source_dispatch=True, **execute_options)
                settled = make_checkpoint(
                    _status_for_result(result),
                    result,
                    reconciliation_receipt_digest=accepted_reconciliation_digest,
                )
                self._commit(settled)
                runtime_receipts = result.runtime.receipts
                return self._projection(settled, result, any(receipt.replay == "replayed" for receipt in runtime_receipts))
            except Exception:
                reconciliation = make_checkpoint(
                    "reconciliation_required",
                    reconciliation_receipt_digest=accepted_reconciliation_digest,
                )
                self._commit(reconciliation)
                raise


__all__ = [
    "AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA",
    "AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_SCHEMA",
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
    "AutonomousEvidenceExecutionReconciliationOutcome",
    "AutonomousEvidenceExecutionReconciliationReceipt",
    "create_autonomous_evidence_execution_reconciliation_receipt",
    "evidence_execution_reconciliation_request_digest",
    "evidence_execution_requests_digest",
    "validate_autonomous_evidence_execution_checkpoint",
    "validate_autonomous_evidence_execution_reconciliation_receipt",
]
