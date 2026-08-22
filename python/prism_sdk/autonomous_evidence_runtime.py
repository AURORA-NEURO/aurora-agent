"""Execution boundary for the autonomous evidence contract.

``autonomous_evidence`` deliberately stops at planning.  This module is the next layer: it
binds a fully-qualified requirement to an application-owned acquisition adapter, projects the
transient value into bounded observations, and optionally asks an independent evaluator for an
explicit verdict.  Only digests, labels, statuses, and evaluator metadata enter the journal.

The adapter may read a file, call a connector, inspect a browser result, or hand work to a human;
the SDK does not assume which.  It never accepts a credential as part of the runtime contract and
never turns transport success into evidence quality or learning reward.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_evidence import AutonomousEvidencePlan, AutonomousEvidenceRequirement
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA = "bioprism-python-autonomous-evidence-runtime/0.1"
AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA = "bioprism-python-autonomous-evidence-receipt/0.1"
AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA = "bioprism-python-autonomous-evidence-assessment/0.1"
AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA = "bioprism-python-autonomous-evidence-runtime-journal/0.1"
AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-evidence-runtime-snapshot/0.1"
AUTONOMOUS_EVIDENCE_OBSERVATION_SCHEMA = "bioprism-python-autonomous-evidence-observation/0.1"
MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS = 128
MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS = 4_096
MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES = 64_000
MAX_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_BYTES = 512_000

_ACQUISITION_STATUSES = frozenset({"observed", "partial", "failed", "reconciliation_required"})
_EVALUATOR_STATUSES = frozenset({"not_evaluated", "accepted", "rejected", "indeterminate", "failed"})
_VERDICTS = frozenset({"accepted", "rejected", "indeterminate"})
_SECRET_KEYS = frozenset({"apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "privatekey", "refreshtoken"})


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, 256)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+- /" for character in result):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _list(name: str, value: Any, maximum: int, *, identifiers: bool = True) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} must be a bounded sequence")
    result = tuple(_identifier(f"{name}[{index}]", item) if identifiers else _text(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicates")
    return result


def _assert_metadata(value: Any, name: str, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = str(key).lower().replace("_", "")
            if normalized in {item.replace("_", "") for item in _SECRET_KEYS}:
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _assert_metadata(child, f"{name}.{key}", depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_metadata(child, f"{name}[{index}]", depth + 1)
    elif isinstance(value, float) and (value != value or value in {float("inf"), float("-inf")}):
        raise ArgumentError(f"{name} contains a non-finite number")


def _json_bytes(value: Any, name: str) -> int:
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded) > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES:
        raise ArgumentError(f"{name} exceeds its metadata byte bound")
    return len(encoded)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceObservation:
    label: str
    kind: str = "fact"
    status: str = "observed"
    value_digest: str | None = None
    source_digest: str | None = None
    confidence: float | None = None
    limitations: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _identifier("evidence runtime observation label", self.label)
        if self.kind not in {"fact", "measurement", "provenance", "limitation", "warning"} or self.status not in {"observed", "inferred", "missing"}:
            raise ArgumentError("evidence runtime observation kind or status is invalid")
        _digest("evidence runtime observation value_digest", self.value_digest, allow_none=True)
        _digest("evidence runtime observation source_digest", self.source_digest, allow_none=True)
        if self.confidence is not None and (isinstance(self.confidence, bool) or not isinstance(self.confidence, (int, float)) or not 0 <= float(self.confidence) <= 1):
            raise ArgumentError("evidence runtime observation confidence is invalid")
        _list("evidence runtime observation limitations", self.limitations, 32, identifiers=False)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_OBSERVATION_SCHEMA,
            "label": self.label,
            "kind": self.kind,
            "status": self.status,
            "value_digest": self.value_digest,
            "source_digest": self.source_digest,
            "confidence": self.confidence,
            "limitations": list(self.limitations),
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceReceipt:
    request_digest: str
    plan_digest: str
    requirement_id: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_id: str
    source_id: str
    source_digest: str | None
    attempt: int
    status: str
    replay: str
    value_digest: str | None
    value_bytes: int
    observations: tuple[AutonomousEvidenceObservation, ...]
    observed_requirement_ids: tuple[str, ...]
    missing_requirement_ids: tuple[str, ...]
    evidence_status: str
    evaluator_status: str
    assessment_digest: str | None
    limitations: tuple[str, ...]
    error_class: str | None
    duration_ms: int
    receipt_digest: str

    def __post_init__(self) -> None:
        for name, value in (("request_digest", self.request_digest), ("plan_digest", self.plan_digest), ("workflow_digest", self.workflow_digest), ("value_digest", self.value_digest), ("assessment_digest", self.assessment_digest)):
            _digest(f"evidence runtime receipt {name}", value, allow_none=name in {"value_digest", "assessment_digest"})
        for name, value in (("requirement_id", self.requirement_id), ("domain", self.domain), ("workflow_id", self.workflow_id), ("stage_id", self.stage_id), ("source_id", self.source_id)):
            _identifier(f"evidence runtime receipt {name}", value)
        _digest("evidence runtime receipt receipt_digest", self.receipt_digest)
        if self.status not in _ACQUISITION_STATUSES or self.replay not in {"fresh", "replayed"} or self.evidence_status not in {"not_evaluated", "missing_required_outputs", "declared_for_evaluator", "projection_failed"} or self.evaluator_status not in _EVALUATOR_STATUSES:
            raise ArgumentError("evidence runtime receipt status is invalid")
        if isinstance(self.attempt, bool) or not isinstance(self.attempt, int) or self.attempt < 1 or isinstance(self.value_bytes, bool) or not isinstance(self.value_bytes, int) or self.value_bytes < 0 or isinstance(self.duration_ms, bool) or not isinstance(self.duration_ms, int) or self.duration_ms < 0:
            raise ArgumentError("evidence runtime receipt numeric fields are invalid")
        if len(set(self.observed_requirement_ids)) != len(self.observed_requirement_ids) or len(set(self.missing_requirement_ids)) != len(self.missing_requirement_ids):
            raise ArgumentError("evidence runtime receipt requirement IDs must be unique")
        _list("evidence runtime receipt limitations", self.limitations, 32, identifiers=False)
        if self.error_class is not None:
            _identifier("evidence runtime receipt error_class", self.error_class)
        if content_digest(self._payload()) != self.receipt_digest:
            raise ArgumentError("evidence runtime receipt digest is invalid")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA,
            "request_digest": self.request_digest,
            "plan_digest": self.plan_digest,
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage_id,
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "attempt": self.attempt,
            "status": self.status,
            "replay": self.replay,
            "value_digest": self.value_digest,
            "value_bytes": self.value_bytes,
            "observations": [item.to_dict() for item in self.observations],
            "observed_requirement_ids": list(self.observed_requirement_ids),
            "missing_requirement_ids": list(self.missing_requirement_ids),
            "evidence_status": self.evidence_status,
            "evaluator_status": self.evaluator_status,
            "assessment_digest": self.assessment_digest,
            "limitations": list(self.limitations),
            "error_class": self.error_class,
            "duration_ms": self.duration_ms,
            "retention": "metadata_only;raw_acquisition_values_caller_owned",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "receipt_digest": self.receipt_digest}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceAssessment:
    receipt_digest: str
    requirement_id: str
    evaluator_id: str
    evaluator_version: str
    verdict: str
    score: float
    feedback_digest: str | None
    evidence_digest: str | None
    failure_class: str | None
    assessment_digest: str

    def __post_init__(self) -> None:
        _digest("evidence runtime assessment receipt_digest", self.receipt_digest)
        _identifier("evidence runtime assessment requirement_id", self.requirement_id)
        _identifier("evidence runtime assessment evaluator_id", self.evaluator_id)
        _identifier("evidence runtime assessment evaluator_version", self.evaluator_version)
        if self.verdict not in _VERDICTS or isinstance(self.score, bool) or not isinstance(self.score, (int, float)) or not 0 <= float(self.score) <= 1:
            raise ArgumentError("evidence runtime assessment verdict or score is invalid")
        _digest("evidence runtime assessment feedback_digest", self.feedback_digest, allow_none=True)
        _digest("evidence runtime assessment evidence_digest", self.evidence_digest, allow_none=True)
        if self.failure_class is not None:
            _identifier("evidence runtime assessment failure_class", self.failure_class)
        _digest("evidence runtime assessment assessment_digest", self.assessment_digest)
        if content_digest(self._payload()) != self.assessment_digest:
            raise ArgumentError("evidence runtime assessment digest is invalid")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA,
            "receipt_digest": self.receipt_digest,
            "requirement_id": self.requirement_id,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": self.verdict,
            "score": float(self.score),
            "feedback_digest": self.feedback_digest,
            "evidence_digest": self.evidence_digest,
            "failure_class": self.failure_class,
            "retention": "value_only;evaluator_payloads_caller_owned",
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "assessment_digest": self.assessment_digest}


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRuntimeJournalEntry:
    sequence: int
    previous_entry_digest: str | None
    receipt: AutonomousEvidenceReceipt
    assessment: AutonomousEvidenceAssessment | None
    entry_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA,
            "sequence": self.sequence,
            "previous_entry_digest": self.previous_entry_digest,
            "receipt": self.receipt.to_dict(),
            "assessment": None if self.assessment is None else self.assessment.to_dict(),
            "entry_digest": self.entry_digest,
            "retention": "metadata_only;raw_acquisition_and_evaluator_values_excluded",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRuntimeSnapshot:
    plan_digest: str
    entries: tuple[AutonomousEvidenceRuntimeJournalEntry, ...]
    head_digest: str | None
    snapshot_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
            "plan_digest": self.plan_digest,
            "entries": [entry.to_dict() for entry in self.entries],
            "head_digest": self.head_digest,
            "snapshot_digest": self.snapshot_digest,
            "retention": "metadata_only_hash_bound",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRuntimeResult:
    status: str
    plan: AutonomousEvidencePlan
    receipts: tuple[AutonomousEvidenceReceipt, ...]
    assessments: tuple[AutonomousEvidenceAssessment, ...]
    completed_requirement_ids: tuple[str, ...]
    pending_evaluation_requirement_ids: tuple[str, ...]
    missing_requirement_ids: tuple[str, ...]
    next_stage_ids: tuple[str, ...]
    omitted_request_digests: tuple[str, ...]
    result_digest: str
    values: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
            "status": self.status,
            "plan": self.plan.to_dict(),
            "receipts": [receipt.to_dict() for receipt in self.receipts],
            "assessments": [assessment.to_dict() for assessment in self.assessments],
            "completed_requirement_ids": list(self.completed_requirement_ids),
            "pending_evaluation_requirement_ids": list(self.pending_evaluation_requirement_ids),
            "missing_requirement_ids": list(self.missing_requirement_ids),
            "next_stage_ids": list(self.next_stage_ids),
            "omitted_request_digests": list(self.omitted_request_digests),
            "result_digest": self.result_digest,
            "retention": "metadata_only;raw_values_caller_owned",
            "secret_material": "never_returned",
        }


class AutonomousEvidenceAcquirer(Protocol):
    def acquire(self, context: Mapping[str, Any]) -> Any: ...


class AutonomousEvidenceProjector(Protocol):
    def project(self, value: Any, context: Mapping[str, Any]) -> Sequence[Mapping[str, Any]]: ...


class AutonomousEvidenceEvaluator(Protocol):
    evaluator_id: str
    evaluator_version: str

    def evaluate(self, input_value: Mapping[str, Any]) -> Mapping[str, Any]: ...


class AutonomousEvidenceRuntimeJournal(Protocol):
    def append(self, entry: AutonomousEvidenceRuntimeJournalEntry) -> AutonomousEvidenceRuntimeJournalEntry: ...
    def records(self) -> Sequence[AutonomousEvidenceRuntimeJournalEntry]: ...


def _observation(value: Mapping[str, Any], index: int) -> AutonomousEvidenceObservation:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"evidence runtime observation {index} must be a mapping")
    confidence = value.get("confidence")
    return AutonomousEvidenceObservation(
        label=_identifier(f"evidence runtime observation {index}.label", value.get("label")),
        kind=_text(f"evidence runtime observation {index}.kind", value.get("kind", "fact"), 32),
        status=_text(f"evidence runtime observation {index}.status", value.get("status", "observed"), 32),
        value_digest=_digest(f"evidence runtime observation {index}.value_digest", value.get("value_digest"), allow_none=True),
        source_digest=_digest(f"evidence runtime observation {index}.source_digest", value.get("source_digest"), allow_none=True),
        confidence=confidence,
        limitations=_list(f"evidence runtime observation {index}.limitations", value.get("limitations", ()), 32, identifiers=False),
    )


def _receipt_from_payload(payload: Mapping[str, Any]) -> AutonomousEvidenceReceipt:
    if payload.get("schema") != AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA:
        raise ArgumentError("evidence runtime receipt schema is invalid")
    if payload.get("retention") != "metadata_only;raw_acquisition_values_caller_owned" or payload.get("secret_material") != "never_returned":
        raise ArgumentError("evidence runtime receipt retention is invalid")
    return AutonomousEvidenceReceipt(
        request_digest=_digest("evidence runtime receipt request_digest", payload.get("request_digest")),  # type: ignore[arg-type]
        plan_digest=_digest("evidence runtime receipt plan_digest", payload.get("plan_digest")),  # type: ignore[arg-type]
        requirement_id=_identifier("evidence runtime receipt requirement_id", payload.get("requirement_id")),
        domain=_identifier("evidence runtime receipt domain", payload.get("domain")),
        workflow_id=_identifier("evidence runtime receipt workflow_id", payload.get("workflow_id")),
        workflow_digest=_digest("evidence runtime receipt workflow_digest", payload.get("workflow_digest")),  # type: ignore[arg-type]
        stage_id=_identifier("evidence runtime receipt stage_id", payload.get("stage_id")),
        source_id=_identifier("evidence runtime receipt source_id", payload.get("source_id")),
        source_digest=_digest("evidence runtime receipt source_digest", payload.get("source_digest"), allow_none=True),
        attempt=payload.get("attempt"),
        status=payload.get("status"),
        replay=payload.get("replay"),
        value_digest=_digest("evidence runtime receipt value_digest", payload.get("value_digest"), allow_none=True),
        value_bytes=payload.get("value_bytes"),
        observations=tuple(_observation(item, index) for index, item in enumerate(payload.get("observations", ()))),
        observed_requirement_ids=_list("evidence runtime receipt observed_requirement_ids", payload.get("observed_requirement_ids", ()), 128),
        missing_requirement_ids=_list("evidence runtime receipt missing_requirement_ids", payload.get("missing_requirement_ids", ()), 128),
        evidence_status=payload.get("evidence_status"),
        evaluator_status=payload.get("evaluator_status"),
        assessment_digest=_digest("evidence runtime receipt assessment_digest", payload.get("assessment_digest"), allow_none=True),
        limitations=_list("evidence runtime receipt limitations", payload.get("limitations", ()), 32, identifiers=False),
        error_class=None if payload.get("error_class") is None else _identifier("evidence runtime receipt error_class", payload.get("error_class")),
        duration_ms=payload.get("duration_ms"),
        receipt_digest=_digest("evidence runtime receipt receipt_digest", payload.get("receipt_digest")),  # type: ignore[arg-type]
    )


def _assessment_from_payload(payload: Mapping[str, Any]) -> AutonomousEvidenceAssessment:
    if payload.get("schema") != AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA:
        raise ArgumentError("evidence runtime assessment schema is invalid")
    if payload.get("retention") != "value_only;evaluator_payloads_caller_owned" or payload.get("secret_material") != "never_returned":
        raise ArgumentError("evidence runtime assessment retention is invalid")
    return AutonomousEvidenceAssessment(
        receipt_digest=_digest("evidence runtime assessment receipt_digest", payload.get("receipt_digest")),  # type: ignore[arg-type]
        requirement_id=_identifier("evidence runtime assessment requirement_id", payload.get("requirement_id")),
        evaluator_id=_identifier("evidence runtime assessment evaluator_id", payload.get("evaluator_id")),
        evaluator_version=_identifier("evidence runtime assessment evaluator_version", payload.get("evaluator_version")),
        verdict=payload.get("verdict"),
        score=payload.get("score"),
        feedback_digest=_digest("evidence runtime assessment feedback_digest", payload.get("feedback_digest"), allow_none=True),
        evidence_digest=_digest("evidence runtime assessment evidence_digest", payload.get("evidence_digest"), allow_none=True),
        failure_class=None if payload.get("failure_class") is None else _identifier("evidence runtime assessment failure_class", payload.get("failure_class")),
        assessment_digest=_digest("evidence runtime assessment assessment_digest", payload.get("assessment_digest")),  # type: ignore[arg-type]
    )


class InMemoryAutonomousEvidenceRuntimeJournal:
    """Small reference journal; production applications can implement the same protocol."""

    def __init__(self) -> None:
        self._entries: list[AutonomousEvidenceRuntimeJournalEntry] = []
        self._lock = threading.RLock()

    def append(self, entry: AutonomousEvidenceRuntimeJournalEntry) -> AutonomousEvidenceRuntimeJournalEntry:
        with self._lock:
            existing = next((item for item in self._entries if item.receipt.request_digest == entry.receipt.request_digest), None)
            if existing is not None:
                if content_digest(existing.to_dict()) != content_digest(entry.to_dict()):
                    raise ArgumentError("evidence runtime journal request identity conflicts with an existing receipt")
                return existing
            if entry.sequence != len(self._entries) + 1 or entry.previous_entry_digest != (self._entries[-1].entry_digest if self._entries else None):
                raise ArgumentError("evidence runtime journal chain position is invalid")
            if content_digest({key: value for key, value in entry.to_dict().items() if key != "entry_digest"}) != entry.entry_digest:
                raise ArgumentError("evidence runtime journal entry digest is invalid")
            if len(self._entries) >= MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS:
                raise ArgumentError("evidence runtime journal capacity is exhausted")
            self._entries.append(entry)
            return entry

    def records(self) -> tuple[AutonomousEvidenceRuntimeJournalEntry, ...]:
        with self._lock:
            return tuple(self._entries)

    def snapshot(self, plan_digest: str) -> AutonomousEvidenceRuntimeSnapshot:
        plan = _digest("evidence runtime snapshot plan_digest", plan_digest)  # type: ignore[assignment]
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
            "plan_digest": plan,
            "entries": [entry.to_dict() for entry in self.records()],
            "head_digest": self._entries[-1].entry_digest if self._entries else None,
            "retention": "metadata_only_hash_bound",
            "secret_material": "never_returned",
        }
        snapshot = AutonomousEvidenceRuntimeSnapshot(plan, self.records(), descriptor["head_digest"], content_digest(descriptor))
        if _json_bytes(snapshot.to_dict(), "evidence runtime snapshot") > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_BYTES:
            raise ArgumentError("evidence runtime snapshot exceeds its byte bound")
        return snapshot

    def restore(self, snapshot: AutonomousEvidenceRuntimeSnapshot | Mapping[str, Any], plan_digest: str) -> None:
        if isinstance(snapshot, Mapping):
            if snapshot.get("schema") != AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA:
                raise ArgumentError("evidence runtime snapshot schema is invalid")
            raw_entries = snapshot.get("entries", ())
            entries: list[AutonomousEvidenceRuntimeJournalEntry] = []
            for raw in raw_entries:
                if not isinstance(raw, Mapping):
                    raise ArgumentError("evidence runtime snapshot journal entry is invalid")
                receipt = _receipt_from_payload(raw.get("receipt", {}))
                assessment_raw = raw.get("assessment")
                assessment = None if assessment_raw is None else _assessment_from_payload(assessment_raw)
                entries.append(AutonomousEvidenceRuntimeJournalEntry(raw.get("sequence"), raw.get("previous_entry_digest"), receipt, assessment, raw.get("entry_digest")))
            snapshot = AutonomousEvidenceRuntimeSnapshot(snapshot.get("plan_digest"), tuple(entries), snapshot.get("head_digest"), snapshot.get("snapshot_digest"))
        if not isinstance(snapshot, AutonomousEvidenceRuntimeSnapshot) or snapshot.plan_digest != plan_digest or snapshot.entries and snapshot.entries[0].receipt.plan_digest != plan_digest:
            raise ArgumentError("evidence runtime snapshot belongs to a different plan")
        descriptor = {
            "schema": AUTONOMOUS_EVIDENCE_RUNTIME_SNAPSHOT_SCHEMA,
            "plan_digest": snapshot.plan_digest,
            "entries": [entry.to_dict() for entry in snapshot.entries],
            "head_digest": snapshot.head_digest,
            "retention": "metadata_only_hash_bound",
            "secret_material": "never_returned",
        }
        if content_digest(descriptor) != snapshot.snapshot_digest:
            raise ArgumentError("evidence runtime snapshot digest is invalid")
        with self._lock:
            restored: list[AutonomousEvidenceRuntimeJournalEntry] = []
            for entry in snapshot.entries:
                if entry.sequence != len(restored) + 1 or entry.previous_entry_digest != (restored[-1].entry_digest if restored else None):
                    raise ArgumentError("evidence runtime snapshot journal chain is invalid")
                if content_digest({key: value for key, value in entry.to_dict().items() if key != "entry_digest"}) != entry.entry_digest:
                    raise ArgumentError("evidence runtime snapshot entry digest is invalid")
                restored.append(entry)
            if snapshot.head_digest != (restored[-1].entry_digest if restored else None):
                raise ArgumentError("evidence runtime snapshot head digest is invalid")
            self._entries = restored


def _request_mapping(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence runtime acquisition request must be a mapping")
    requirement_id = _identifier("evidence runtime requirement_id", value.get("requirement_id"))
    source_id = _identifier("evidence runtime source_id", value.get("source_id"))
    source_digest = _digest("evidence runtime source_digest", value.get("source_digest"), allow_none=True)
    request_id = None if value.get("request_id") is None else _identifier("evidence runtime request_id", value.get("request_id"))
    metadata = value.get("metadata", {})
    if not isinstance(metadata, Mapping):
        raise ArgumentError("evidence runtime request metadata must be a mapping")
    safe_metadata = dict(metadata)
    _assert_metadata(safe_metadata, "evidence runtime request metadata")
    _json_bytes(safe_metadata, "evidence runtime request metadata")
    return {"requirement_id": requirement_id, "source_id": source_id, "source_digest": source_digest, "request_id": request_id, "metadata": safe_metadata}


class AutonomousEvidenceRuntime:
    """Run bounded acquisition/projection/evaluation without persisting raw values."""

    def __init__(self, plan: AutonomousEvidencePlan, *, journal: AutonomousEvidenceRuntimeJournal | None = None) -> None:
        if not isinstance(plan, AutonomousEvidencePlan):
            raise ArgumentError("evidence runtime requires an AutonomousEvidencePlan")
        if journal is not None and not all(callable(getattr(journal, name, None)) for name in ("append", "records")):
            raise ArgumentError("evidence runtime journal is malformed")
        self.plan = plan
        self.journal = journal
        self._records: dict[str, AutonomousEvidenceRuntimeJournalEntry] = {}
        self._values: dict[str, Any] = {}

    def rehydrate(self) -> dict[str, Any]:
        if self.journal is None:
            return {"restored": 0, "replayable": 0, "value_retention": "transient_caller_value_only"}
        entries = tuple(self.journal.records())
        if len(entries) > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS:
            raise ArgumentError("evidence runtime journal returned too many records")
        self._records.clear()
        self._values.clear()
        for entry in entries:
            if entry.receipt.plan_digest != self.plan.plan_digest:
                raise ArgumentError("evidence runtime journal belongs to a different evidence plan")
            self._records[entry.receipt.request_digest] = entry
        return {"restored": len(entries), "replayable": len(entries), "value_retention": "transient_caller_value_only"}

    def _requirement(self, requirement_id: str) -> AutonomousEvidenceRequirement:
        normalized = _identifier("evidence runtime requirement_id", requirement_id)
        requirement = next((item for item in self.plan.requirements if item.requirement_id == normalized), None)
        if requirement is None:
            raise ArgumentError(f"evidence runtime requirement is not in the plan: {normalized}")
        return requirement

    def _request_digest(self, request: Mapping[str, Any]) -> str:
        return content_digest({"schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA, "plan_digest": self.plan.plan_digest, "requirement_id": request["requirement_id"], "source_id": request["source_id"], "source_digest": request["source_digest"], "request_id": request["request_id"], "metadata": request["metadata"]})

    def _make_receipt(self, **values: Any) -> AutonomousEvidenceReceipt:
        clean_values = {
            key: value
            for key, value in values.items()
            if key not in {"schema", "retention", "secret_material", "receipt_digest"}
        }
        payload = {
            "schema": AUTONOMOUS_EVIDENCE_RECEIPT_SCHEMA,
            **clean_values,
            "retention": "metadata_only;raw_acquisition_values_caller_owned",
            "secret_material": "never_returned",
        }
        raw_observations = payload.get("observations", ())
        observations = tuple(
            item if isinstance(item, AutonomousEvidenceObservation) else _observation(item, index)
            for index, item in enumerate(raw_observations)
        )
        payload["observations"] = [item.to_dict() for item in observations]
        return AutonomousEvidenceReceipt(
            request_digest=payload["request_digest"], plan_digest=payload["plan_digest"], requirement_id=payload["requirement_id"], domain=payload["domain"], workflow_id=payload["workflow_id"], workflow_digest=payload["workflow_digest"], stage_id=payload["stage_id"], source_id=payload["source_id"], source_digest=payload["source_digest"], attempt=payload["attempt"], status=payload["status"], replay=payload["replay"], value_digest=payload["value_digest"], value_bytes=payload["value_bytes"], observations=observations, observed_requirement_ids=tuple(payload["observed_requirement_ids"]), missing_requirement_ids=tuple(payload["missing_requirement_ids"]), evidence_status=payload["evidence_status"], evaluator_status=payload["evaluator_status"], assessment_digest=payload["assessment_digest"], limitations=tuple(payload["limitations"]), error_class=payload["error_class"], duration_ms=payload["duration_ms"], receipt_digest=content_digest(payload),
        )

    def _make_assessment(self, **values: Any) -> AutonomousEvidenceAssessment:
        payload = {"schema": AUTONOMOUS_EVIDENCE_ASSESSMENT_SCHEMA, **values, "retention": "value_only;evaluator_payloads_caller_owned", "secret_material": "never_returned"}
        return AutonomousEvidenceAssessment(
            receipt_digest=payload["receipt_digest"], requirement_id=payload["requirement_id"], evaluator_id=payload["evaluator_id"], evaluator_version=payload["evaluator_version"], verdict=payload["verdict"], score=payload["score"], feedback_digest=payload["feedback_digest"], evidence_digest=payload["evidence_digest"], failure_class=payload["failure_class"], assessment_digest=content_digest(payload),
        )

    def _append(self, receipt: AutonomousEvidenceReceipt, assessment: AutonomousEvidenceAssessment | None) -> AutonomousEvidenceRuntimeJournalEntry:
        previous = max(self._records.values(), key=lambda item: item.sequence, default=None)
        descriptor = {"schema": AUTONOMOUS_EVIDENCE_RUNTIME_JOURNAL_SCHEMA, "sequence": (previous.sequence if previous else 0) + 1, "previous_entry_digest": previous.entry_digest if previous else None, "receipt": receipt.to_dict(), "assessment": None if assessment is None else assessment.to_dict(), "retention": "metadata_only;raw_acquisition_and_evaluator_values_excluded", "secret_material": "never_returned"}
        entry = AutonomousEvidenceRuntimeJournalEntry(descriptor["sequence"], descriptor["previous_entry_digest"], receipt, assessment, content_digest(descriptor))
        persisted = self.journal.append(entry) if self.journal is not None else entry
        self._records[receipt.request_digest] = persisted
        return persisted

    def _call_acquirer(self, acquirer: Any, context: Mapping[str, Any]) -> Any:
        callback = getattr(acquirer, "acquire", None)
        if callable(callback):
            return callback(context)
        if callable(acquirer):
            return acquirer(context)
        raise ArgumentError("evidence runtime acquirer must be callable or implement acquire")

    def _call_projector(self, projector: Any, value: Any, context: Mapping[str, Any]) -> Sequence[Mapping[str, Any]]:
        callback = getattr(projector, "project", None)
        result = callback(value, context) if callable(callback) else projector(value, context) if callable(projector) else None
        if not isinstance(result, Sequence) or isinstance(result, (str, bytes, bytearray)):
            raise ArgumentError("evidence runtime projector must return a sequence")
        return result

    def _call_evaluator(self, evaluator: Any, input_value: Mapping[str, Any]) -> Mapping[str, Any]:
        callback = getattr(evaluator, "evaluate", None)
        result = callback(input_value) if callable(callback) else evaluator(input_value) if callable(evaluator) else None
        if not isinstance(result, Mapping):
            raise ArgumentError("evidence runtime evaluator must return a mapping")
        return result

    def execute(self, requests: Sequence[Mapping[str, Any]], *, acquirer: Any, projector: Any | None = None, evaluator: Any | None = None, rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None, parent_evidence_digests: Sequence[str] = (), stop_on_failure: bool = False) -> AutonomousEvidenceRuntimeResult:
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)) or not 1 <= len(requests) <= MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS:
            raise ArgumentError(f"evidence runtime requests must contain 1..{MAX_AUTONOMOUS_EVIDENCE_RUNTIME_REQUESTS} entries")
        parents = _list("evidence runtime parent_evidence_digests", parent_evidence_digests, 64)
        if evaluator is not None:
            evaluator_id = _identifier("configured evidence runtime evaluator_id", getattr(evaluator, "evaluator_id", None))
            evaluator_version = _identifier("configured evidence runtime evaluator_version", getattr(evaluator, "evaluator_version", None))
        else:
            evaluator_id = evaluator_version = None
        receipts: list[AutonomousEvidenceReceipt] = []
        assessments: list[AutonomousEvidenceAssessment] = []
        values: dict[str, Any] = {}
        available = set(self.plan.available_evidence)
        completed: set[str] = set()
        pending: set[str] = set()
        omitted: list[str] = []
        saw_failure = False
        saw_reconciliation = False
        saw_pending = False
        for raw_request in requests:
            request = _request_mapping(raw_request)
            requirement = self._requirement(request["requirement_id"])
            request_digest = self._request_digest(request)
            prior = self._records.get(request_digest)
            if prior is not None:
                value = self._values.get(request_digest)
                if value is None and rehydrate_value is not None and prior.receipt.value_digest is not None:
                    restored = rehydrate_value(prior.receipt.to_dict())
                    if restored is not None and content_digest(restored) != prior.receipt.value_digest:
                        raise ArgumentError("rehydrated evidence value does not match its receipt digest")
                    value = restored
                replayed = prior.receipt
                if value is None and replayed.value_digest is not None:
                    replayed = self._make_receipt(**{**replayed.to_dict(), "receipt_digest": None, "status": "reconciliation_required", "replay": "replayed", "limitations": list(replayed.limitations) + ["caller-owned evidence value requires rehydration"]})
                    saw_reconciliation = True
                else:
                    replayed = self._make_receipt(**{**replayed.to_dict(), "receipt_digest": None, "replay": "replayed"})
                receipts.append(replayed)
                if prior.assessment is not None:
                    assessments.append(prior.assessment)
                if value is not None:
                    values[request_digest] = value
                    self._values[request_digest] = value
                for evidence_id in replayed.observed_requirement_ids:
                    available.add(evidence_id)
                if prior.assessment is not None and prior.assessment.verdict == "accepted":
                    completed.add(replayed.requirement_id)
                elif replayed.evaluator_status in {"not_evaluated", "indeterminate", "failed"}:
                    pending.add(replayed.requirement_id)
                continue
            if saw_failure and stop_on_failure:
                omitted.append(request_digest)
                continue
            started = time.monotonic()
            context = {"plan_digest": self.plan.plan_digest, "requirement": requirement, "request": request, "attempt": 1, "parent_evidence_digests": list(parents), "execution": "caller_owned_adapter;raw_value_transient"}
            try:
                raw_value = self._call_acquirer(acquirer, context)
                _json_bytes(raw_value, "evidence runtime acquisition value")
            except Exception as error:
                saw_failure = True
                receipt = self._make_receipt(request_digest=request_digest, plan_digest=self.plan.plan_digest, requirement_id=requirement.requirement_id, domain=requirement.domain, workflow_id=requirement.workflow_id, workflow_digest=requirement.workflow_digest, stage_id=requirement.stage_id, source_id=request["source_id"], source_digest=request["source_digest"], attempt=1, status="failed", replay="fresh", value_digest=None, value_bytes=0, observations=(), observed_requirement_ids=(), missing_requirement_ids=(requirement.requirement_id,), evidence_status="not_evaluated", evaluator_status="not_evaluated", assessment_digest=None, limitations=("caller-owned acquisition failed",), error_class=error.__class__.__name__, duration_ms=max(0, int((time.monotonic() - started) * 1000)))
                self._append(receipt, None)
                receipts.append(receipt)
                values[request_digest] = None
                continue
            value_digest = content_digest(raw_value)
            value_bytes = len(canonical_json(raw_value).encode("utf-8"))
            self._values[request_digest] = raw_value
            values[request_digest] = raw_value
            observations: tuple[AutonomousEvidenceObservation, ...] = ()
            evidence_status = "missing_required_outputs"
            projection_failure: str | None = None
            if projector is not None:
                try:
                    observations = tuple(_observation(item, index) for index, item in enumerate(self._call_projector(projector, raw_value, context)))
                    if any(item.label in {requirement.requirement_id, requirement.label} for item in observations):
                        evidence_status = "declared_for_evaluator"
                except Exception as error:
                    projection_failure = error.__class__.__name__
                    evidence_status = "projection_failed"
            observed_ids = (requirement.requirement_id,) if evidence_status == "declared_for_evaluator" else ()
            if observed_ids:
                available.add(requirement.requirement_id)
            missing_ids = () if observed_ids else (requirement.requirement_id,)
            limitations = ("raw acquisition value is transient and caller-owned", "observation projection failed", projection_failure) if projection_failure else ("raw acquisition value is transient and caller-owned",)
            base = self._make_receipt(request_digest=request_digest, plan_digest=self.plan.plan_digest, requirement_id=requirement.requirement_id, domain=requirement.domain, workflow_id=requirement.workflow_id, workflow_digest=requirement.workflow_digest, stage_id=requirement.stage_id, source_id=request["source_id"], source_digest=request["source_digest"], attempt=1, status="observed" if observed_ids else "partial", replay="fresh", value_digest=value_digest, value_bytes=value_bytes, observations=observations, observed_requirement_ids=observed_ids, missing_requirement_ids=missing_ids, evidence_status=evidence_status, evaluator_status="not_evaluated", assessment_digest=None, limitations=limitations, error_class=None, duration_ms=max(0, int((time.monotonic() - started) * 1000)))
            receipt = base
            assessment: AutonomousEvidenceAssessment | None = None
            if evaluator is not None and observed_ids:
                try:
                    decision = self._call_evaluator(evaluator, {"requirement": requirement, "receipt": base.to_dict(), "observations": [item.to_dict() for item in observations], "value": raw_value})
                    decision_id = _identifier("evidence runtime evaluator_id", decision.get("evaluator_id"))
                    decision_version = _identifier("evidence runtime evaluator_version", decision.get("evaluator_version"))
                    if decision_id != evaluator_id or decision_version != evaluator_version:
                        raise ArgumentError("evidence runtime evaluator identity does not match configured evaluator")
                    verdict = _text("evidence runtime evaluator verdict", decision.get("verdict"), 32)
                    score = decision.get("score")
                    if verdict not in _VERDICTS or isinstance(score, bool) or not isinstance(score, (int, float)) or not 0 <= float(score) <= 1:
                        raise ArgumentError("evidence runtime evaluator verdict is malformed")
                    assessment = self._make_assessment(receipt_digest=base.receipt_digest, requirement_id=requirement.requirement_id, evaluator_id=decision_id, evaluator_version=decision_version, verdict=verdict, score=float(score), feedback_digest=_digest("evidence runtime feedback_digest", decision.get("feedback_digest"), allow_none=True), evidence_digest=_digest("evidence runtime evidence_digest", decision.get("evidence_digest"), allow_none=True), failure_class=None if decision.get("failure_class") is None else _identifier("evidence runtime failure_class", decision.get("failure_class")))
                    receipt = self._make_receipt(**{**base.to_dict(), "receipt_digest": None, "evaluator_status": verdict, "assessment_digest": assessment.assessment_digest})
                    if verdict == "accepted":
                        completed.add(requirement.requirement_id)
                    else:
                        pending.add(requirement.requirement_id)
                except Exception as error:
                    saw_pending = True
                    receipt = self._make_receipt(**{**base.to_dict(), "receipt_digest": None, "evaluator_status": "failed", "limitations": list(base.limitations) + ["caller-owned evaluator failed", error.__class__.__name__]})
                    pending.add(requirement.requirement_id)
            elif observed_ids:
                saw_pending = True
                pending.add(requirement.requirement_id)
            if assessment is not None:
                assessments.append(assessment)
            self._append(receipt, assessment)
            receipts.append(receipt)
        next_plan = self.plan.with_available_evidence(tuple(sorted(available)))
        accepted = {item.requirement_id for item in assessments if item.verdict == "accepted"}
        completed.update(accepted)
        all_covered = not next_plan.missing_requirement_ids
        all_accepted = all(item.requirement_id in accepted for item in next_plan.requirements)
        status = "reconciliation_required" if saw_reconciliation else "failed" if saw_failure and receipts and all(item.status == "failed" for item in receipts) else "completed" if all_covered and all_accepted else "awaiting_evaluation" if saw_pending or (all_covered and not all_accepted) else "partial"
        descriptor = {"schema": AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA, "status": status, "plan_digest": next_plan.plan_digest, "receipt_digests": [item.receipt_digest for item in receipts], "assessment_digests": [item.assessment_digest for item in assessments], "completed_requirement_ids": sorted(completed), "pending_evaluation_requirement_ids": sorted(pending), "missing_requirement_ids": sorted(next_plan.missing_requirement_ids), "next_stage_ids": sorted(next_plan.next_stage_ids), "omitted_request_digests": sorted(omitted), "retention": "metadata_only;raw_values_caller_owned", "secret_material": "never_returned"}
        return AutonomousEvidenceRuntimeResult(status, next_plan, tuple(receipts), tuple(assessments), tuple(sorted(completed)), tuple(sorted(pending)), tuple(sorted(next_plan.missing_requirement_ids)), tuple(sorted(next_plan.next_stage_ids)), tuple(sorted(omitted)), content_digest(descriptor), values)
