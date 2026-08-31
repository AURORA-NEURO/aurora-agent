"""First-class reviewed capability execution for the Python autonomous embedding layer.

The provider-facing brain and the domain-tool runtime are intentionally separate.  This module
adds the missing application boundary: a caller-declared request is checked against the reviewed
domain/tool catalogue, executed through the existing approval runtime, projected into evaluator
metadata, and optionally journaled without retaining arguments or raw adapter values.
"""

from __future__ import annotations

from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor
import json
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AutonomousDomainTool, AutonomousDomainToolRuntime
from .errors import ArgumentError
from .llm_runtime import ProviderToolCall


AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA = "bioprism-python-autonomous-capability-execution/0.1"
AUTONOMOUS_CAPABILITY_BATCH_SCHEMA = "bioprism-python-autonomous-capability-batch/0.1"
AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA = "bioprism-python-autonomous-capability-observation/0.1"
AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA = "bioprism-python-autonomous-capability-journal/0.1"
_LEGACY_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-capability-journal-snapshot/0.1"
AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-capability-journal-snapshot/0.2"
MAX_AUTONOMOUS_CAPABILITY_BATCH = 64
MAX_AUTONOMOUS_CAPABILITY_HISTORY = 512
MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS = 128
MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES = 4096
MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES = 64_000_000

_EXECUTION_STATUSES = frozenset({"completed", "approval_required", "reconciliation_required", "refused", "failed"})
_REPLAYABLE_STATUSES = frozenset({"completed", "reconciliation_required"})
_EVIDENCE_STATUSES = frozenset({"not_evaluated", "missing_required_outputs", "declared_for_evaluator", "projection_failed"})
_OBSERVATION_KINDS = frozenset({"fact", "measurement", "provenance", "limitation", "warning"})
_OBSERVATION_STATUSES = frozenset({"observed", "inferred", "missing"})
_RAW_KEYS = frozenset({"task", "prompt", "response", "content", "instruction", "evidence", "output", "argument", "arguments", "credential", "password", "secret", "token", "payload", "transcript", "value"})
_RECORD_KEYS = (
    "schema", "record_kind", "request_digest", "execution_id", "call_id", "domain", "workflow_id", "workflow_digest", "stage_id",
    "stage_contract_digest", "tool", "capability", "risk_class", "schema_digest", "input_digest", "subject_digest",
    "parent_evidence_digests", "arguments_digest", "replay_key_digest", "status", "replay", "output_digest", "output_bytes",
    "observations", "evidence_digest", "evidence_status", "required_evidence_outputs", "missing_evidence_outputs", "limitations",
    "effect", "effect_id", "error_class", "duration_ms", "does_not_claim", "secret_material",
)


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    text = _text(name, value, 256)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in text):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return text


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _json_bytes(value: Any, name: str = "capability metadata") -> int:
    try:
        return len(canonical_json(value).encode("utf-8"))
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error


def _inspect_metadata(value: Any, path: str = "$", depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{path} is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if isinstance(key, str) and key.lower() in _RAW_KEYS:
                raise ArgumentError(f"{path}.{key} is not allowed in metadata-only capability records")
            _inspect_metadata(child, f"{path}.{key}", depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) > 8192:
            raise ArgumentError(f"{path} contains too many rows")
        for index, child in enumerate(value):
            _inspect_metadata(child, f"{path}[{index}]", depth + 1)


def _list_of_text(name: str, value: Any, maximum: int, text_maximum: int = 512) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} is malformed")
    return tuple(_text(f"{name}[{index}]", item, text_maximum) for index, item in enumerate(value))


def _list_of_digests(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} is malformed")
    return tuple(_digest(f"{name}[{index}]", item) for index, item in enumerate(value))  # type: ignore[misc]


def _clone_json(value: Any) -> Any:
    return json.loads(canonical_json(value))


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityObservation:
    id: str
    label: str
    kind: str
    status: str
    value_digest: str | None = None
    source_digest: str | None = None
    confidence: float | None = None
    limitations: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _identifier("capability observation id", self.id)
        _text("capability observation label", self.label, 256)
        if self.kind not in _OBSERVATION_KINDS or self.status not in _OBSERVATION_STATUSES:
            raise ArgumentError("capability observation kind or status is unsupported")
        _digest("capability observation value_digest", self.value_digest, allow_none=True)
        _digest("capability observation source_digest", self.source_digest, allow_none=True)
        if self.confidence is not None and (not isinstance(self.confidence, (int, float)) or isinstance(self.confidence, bool) or not 0 <= self.confidence <= 1):
            raise ArgumentError("capability observation confidence must be within [0, 1]")
        _list_of_text("capability observation limitations", self.limitations, 32, 2048)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA,
            "id": self.id,
            "label": self.label,
            "kind": self.kind,
            "status": self.status,
            "value_digest": self.value_digest,
            "source_digest": self.source_digest,
            "confidence": self.confidence,
            "limitations": list(self.limitations),
        }


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityExecutionRecord:
    request_digest: str
    execution_id: str | None
    call_id: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_id: str
    stage_contract_digest: str | None
    tool: str
    capability: str | None
    risk_class: str | None
    schema_digest: str | None
    input_digest: str
    subject_digest: str | None
    parent_evidence_digests: tuple[str, ...]
    arguments_digest: str
    replay_key_digest: str | None
    status: str
    replay: str
    output_digest: str | None
    output_bytes: int
    observations: tuple[AutonomousCapabilityObservation, ...]
    evidence_digest: str | None
    evidence_status: str
    required_evidence_outputs: tuple[str, ...]
    missing_evidence_outputs: tuple[str, ...]
    limitations: tuple[str, ...]
    effect: str | None
    effect_id: str | None
    error_class: str | None
    duration_ms: int
    does_not_claim: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
            "record_kind": "capability_execution_record",
            "request_digest": self.request_digest,
            "execution_id": self.execution_id,
            "call_id": self.call_id,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage_id,
            "stage_contract_digest": self.stage_contract_digest,
            "tool": self.tool,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "schema_digest": self.schema_digest,
            "input_digest": self.input_digest,
            "subject_digest": self.subject_digest,
            "parent_evidence_digests": list(self.parent_evidence_digests),
            "arguments_digest": self.arguments_digest,
            "replay_key_digest": self.replay_key_digest,
            "status": self.status,
            "replay": self.replay,
            "output_digest": self.output_digest,
            "output_bytes": self.output_bytes,
            "observations": [observation.to_dict() for observation in self.observations],
            "evidence_digest": self.evidence_digest,
            "evidence_status": self.evidence_status,
            "required_evidence_outputs": list(self.required_evidence_outputs),
            "missing_evidence_outputs": list(self.missing_evidence_outputs),
            "limitations": list(self.limitations),
            "effect": self.effect,
            "effect_id": self.effect_id,
            "error_class": self.error_class,
            "duration_ms": self.duration_ms,
            "does_not_claim": list(self.does_not_claim),
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityExecutionResult:
    record: AutonomousCapabilityExecutionRecord
    value: Any = None
    value_retention: str = "transient_caller_value_only"

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
            "record": self.record.to_dict(),
            "value": self.value,
            "value_retention": self.value_retention,
            "secret_material": "never_returned",
        }


def _normalize_request(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("capability execution request must be a mapping")
    if value.get("schema") not in (None, AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA):
        raise ArgumentError("capability execution request schema is unsupported")
    arguments = value.get("arguments")
    if not isinstance(arguments, Mapping):
        raise ArgumentError("capability execution arguments must be a mapping")
    normalized_arguments = _clone_json(dict(arguments))
    if not isinstance(normalized_arguments, dict):
        raise ArgumentError("capability execution arguments must be a JSON object")
    workflow_context = value.get("workflow_context")
    if not isinstance(workflow_context, Mapping):
        raise ArgumentError("capability workflow_context must be a mapping")
    context = {
        "domain": _identifier("capability workflow_context domain", workflow_context.get("domain")),
        "workflow_id": _identifier("capability workflow_context workflow_id", workflow_context.get("workflow_id")),
        "workflow_digest": _digest("capability workflow_context workflow_digest", workflow_context.get("workflow_digest")),
        "stage_id": _identifier("capability workflow_context stage_id", workflow_context.get("stage_id")),
    }
    parents = value.get("parent_evidence_digests", ())
    parent_digests = _list_of_digests("capability parent_evidence_digests", parents, 64)
    if len(set(parent_digests)) != len(parent_digests):
        raise ArgumentError("capability parent_evidence_digests must not contain duplicates")
    input_digest = _digest("capability input_digest", value.get("input_digest"))
    return {
        "call_id": _text("capability call_id", value.get("call_id"), 256),
        "tool": _identifier("capability tool", value.get("tool")),
        "arguments": normalized_arguments,
        "workflow_context": context,
        "input_digest": input_digest,
        "subject_digest": _digest("capability subject_digest", value.get("subject_digest"), allow_none=True),
        "parent_evidence_digests": parent_digests,
        "replay_key": None if value.get("replay_key") is None else _text("capability replay_key", value.get("replay_key"), 256),
        "execution_id": None if value.get("execution_id") is None else _text("capability execution_id", value.get("execution_id"), 256),
    }


def _request_identity(request: Mapping[str, Any]) -> tuple[str, str, str | None]:
    arguments_digest = content_digest(request["arguments"])
    replay_key_digest = None if request["replay_key"] is None else content_digest(request["replay_key"])
    descriptor = {
        "schema": AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
        "call_id": request["call_id"],
        "tool": request["tool"],
        "arguments_digest": arguments_digest,
        "workflow_context": request["workflow_context"],
        "input_digest": request["input_digest"],
        "subject_digest": request["subject_digest"],
        "parent_evidence_digests": list(request["parent_evidence_digests"]),
        "replay_key_digest": replay_key_digest,
        "execution_id": request["execution_id"],
    }
    return content_digest(descriptor), arguments_digest, replay_key_digest


def _stage_details(request: Mapping[str, Any], tool: AutonomousDomainTool) -> tuple[str | None, tuple[str, ...]]:
    """Resolve built-in stage evidence lazily so custom domain registries remain supported."""

    try:
        from .autonomy import builtin_autonomous_workflow_strategies, _AUTONOMOUS_CAPABILITY_TOOL_ALIASES

        workflow = next((item for item in builtin_autonomous_workflow_strategies() if item.domain == request["workflow_context"]["domain"]), None)
        if workflow is None or workflow.workflow_id != request["workflow_context"]["workflow_id"] or workflow.workflow_digest != request["workflow_context"]["workflow_digest"]:
            return None, ()
        stage = next((item for item in workflow.stages if item.id == request["workflow_context"]["stage_id"]), None)
        if stage is None:
            return None, ()
        aliases = _AUTONOMOUS_CAPABILITY_TOOL_ALIASES.get(workflow.domain, {})
        if not any(tool.capability == required or tool.capability in aliases.get(required, ()) for required in stage.required_capabilities):
            return None, tuple(stage.evidence_outputs)
        descriptor = {"workflow_id": workflow.workflow_id, "workflow_digest": workflow.workflow_digest, "stage": stage.to_dict()}
        return content_digest(descriptor), tuple(stage.evidence_outputs)
    except Exception:
        return None, ()


def _observation(value: Mapping[str, Any], index: int) -> AutonomousCapabilityObservation:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"capability observation {index} must be a mapping")
    limitations = value.get("limitations", ())
    return AutonomousCapabilityObservation(
        id=_identifier(f"capability observation {index} id", value.get("id")),
        label=_text(f"capability observation {index} label", value.get("label"), 256),
        kind=_text(f"capability observation {index} kind", value.get("kind"), 32),
        status=_text(f"capability observation {index} status", value.get("status"), 32),
        value_digest=_digest(f"capability observation {index} value_digest", value.get("value_digest"), allow_none=True),
        source_digest=_digest(f"capability observation {index} source_digest", value.get("source_digest"), allow_none=True),
        confidence=value.get("confidence"),
        limitations=tuple(_list_of_text(f"capability observation {index} limitations", limitations, 32, 2048)),
    )


def _common_claims() -> tuple[str, ...]:
    return (
        "capability execution is not proof that the overall task succeeded",
        "an adapter output digest is not a claim about external-world truth",
        "declared observations require evaluator and provenance review",
        "a complete evidence label set does not authorize effects or certify correctness",
    )


def _record_from_mapping(value: Mapping[str, Any], *, require_fresh: bool = True) -> AutonomousCapabilityExecutionRecord:
    if not isinstance(value, Mapping) or set(value) != set(_RECORD_KEYS):
        raise ArgumentError("capability journal record has unsupported or missing fields")
    if value.get("schema") != AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA or value.get("record_kind") != "capability_execution_record":
        raise ArgumentError("capability journal record schema is invalid")
    if value.get("secret_material") != "never_returned" or (require_fresh and value.get("replay") != "fresh"):
        raise ArgumentError("capability journal record retention markers are invalid")
    if value.get("status") not in _EXECUTION_STATUSES or value.get("evidence_status") not in _EVIDENCE_STATUSES:
        raise ArgumentError("capability journal record status is invalid")
    observations_raw = value.get("observations")
    if not isinstance(observations_raw, Sequence) or isinstance(observations_raw, (str, bytes, bytearray)) or len(observations_raw) > MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS:
        raise ArgumentError("capability journal observations are malformed")
    record = AutonomousCapabilityExecutionRecord(
        request_digest=_digest("capability journal request_digest", value.get("request_digest")),  # type: ignore[arg-type]
        execution_id=None if value.get("execution_id") is None else _text("capability journal execution_id", value.get("execution_id"), 256),
        call_id=_text("capability journal call_id", value.get("call_id"), 256),
        domain=_identifier("capability journal domain", value.get("domain")),
        workflow_id=_identifier("capability journal workflow_id", value.get("workflow_id")),
        workflow_digest=_digest("capability journal workflow_digest", value.get("workflow_digest")),  # type: ignore[arg-type]
        stage_id=_identifier("capability journal stage_id", value.get("stage_id")),
        stage_contract_digest=_digest("capability journal stage_contract_digest", value.get("stage_contract_digest"), allow_none=True),
        tool=_identifier("capability journal tool", value.get("tool")),
        capability=None if value.get("capability") is None else _text("capability journal capability", value.get("capability"), 256),
        risk_class=None if value.get("risk_class") is None else _text("capability journal risk_class", value.get("risk_class"), 256),
        schema_digest=_digest("capability journal schema_digest", value.get("schema_digest"), allow_none=True),
        input_digest=_digest("capability journal input_digest", value.get("input_digest")),  # type: ignore[arg-type]
        subject_digest=_digest("capability journal subject_digest", value.get("subject_digest"), allow_none=True),
        parent_evidence_digests=_list_of_digests("capability journal parent_evidence_digests", value.get("parent_evidence_digests"), 64),
        arguments_digest=_digest("capability journal arguments_digest", value.get("arguments_digest")),  # type: ignore[arg-type]
        replay_key_digest=_digest("capability journal replay_key_digest", value.get("replay_key_digest"), allow_none=True),
        status=value["status"],
        replay=value["replay"],
        output_digest=_digest("capability journal output_digest", value.get("output_digest"), allow_none=True),
        output_bytes=value["output_bytes"],
        observations=tuple(_observation(item, index) for index, item in enumerate(observations_raw)),
        evidence_digest=_digest("capability journal evidence_digest", value.get("evidence_digest"), allow_none=True),
        evidence_status=value["evidence_status"],
        required_evidence_outputs=tuple(_list_of_text("capability journal required_evidence_outputs", value.get("required_evidence_outputs"), 128)),
        missing_evidence_outputs=tuple(_list_of_text("capability journal missing_evidence_outputs", value.get("missing_evidence_outputs"), 128)),
        limitations=tuple(_list_of_text("capability journal limitations", value.get("limitations"), 64, 2048)),
        effect=None if value.get("effect") is None else _text("capability journal effect", value.get("effect"), 256),
        effect_id=None if value.get("effect_id") is None else _text("capability journal effect_id", value.get("effect_id"), 256),
        error_class=None if value.get("error_class") is None else _identifier("capability journal error_class", value.get("error_class")),
        duration_ms=value["duration_ms"],
        does_not_claim=tuple(_list_of_text("capability journal does_not_claim", value.get("does_not_claim"), 32, 1024)),
    )
    if not isinstance(record.output_bytes, int) or isinstance(record.output_bytes, bool) or not 0 <= record.output_bytes <= 64_000_000:
        raise ArgumentError("capability journal output_bytes is outside its bounds")
    if not isinstance(record.duration_ms, int) or isinstance(record.duration_ms, bool) or not 0 <= record.duration_ms <= 86_400_000:
        raise ArgumentError("capability journal duration_ms is outside its bounds")
    if record.status == "completed" and record.output_digest is None:
        raise ArgumentError("completed capability records require an output digest")
    if record.status != "completed" and record.output_digest is not None:
        raise ArgumentError("non-completed capability records cannot contain an output digest")
    if record.evidence_digest is not None:
        if record.output_digest is None:
            raise ArgumentError("capability evidence digest requires an output digest")
        evidence_descriptor = {
            "schema": AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
            "request_digest": record.request_digest,
            "input_digest": record.input_digest,
            "arguments_digest": record.arguments_digest,
            "output_digest": record.output_digest,
            "required_evidence_outputs": list(record.required_evidence_outputs),
            "observations": [item.to_dict() for item in record.observations],
            "evidence_status": record.evidence_status,
        }
        if content_digest(evidence_descriptor) != record.evidence_digest:
            raise ArgumentError("capability evidence digest does not match its metadata")
    metadata = record.to_dict()
    _inspect_metadata(metadata)
    if _json_bytes(metadata) > 8_000_000:
        raise ArgumentError("capability journal record exceeds its byte capacity")
    return record


def _record_input(
    record: AutonomousCapabilityExecutionRecord | Mapping[str, Any],
    *,
    require_fresh: bool = True,
) -> AutonomousCapabilityExecutionRecord:
    if isinstance(record, AutonomousCapabilityExecutionRecord):
        return _record_from_mapping(record.to_dict(), require_fresh=require_fresh)
    return _record_from_mapping(record, require_fresh=require_fresh)


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityJournalEntry:
    sequence: int
    previous_entry_digest: str | None
    record: AutonomousCapabilityExecutionRecord
    entry_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA,
            "sequence": self.sequence,
            "previous_entry_digest": self.previous_entry_digest,
            "record": self.record.to_dict(),
            "entry_digest": self.entry_digest,
            "retention": "metadata_only_hash_chained_no_private_payloads",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousCapabilityJournalSnapshot:
    entries: tuple[AutonomousCapabilityJournalEntry, ...]
    head_digest: str | None
    snapshot_digest: str
    snapshot_generation: int | None = None
    previous_snapshot_digest: str | None = None

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA if self.snapshot_generation is not None else _LEGACY_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
            "entries": [entry.to_dict() for entry in self.entries],
            "head_digest": self.head_digest,
            "retention": "metadata_only_hash_bound",
            "secret_material": "never_returned",
        }
        if self.snapshot_generation is not None:
            descriptor = {
                **descriptor,
                "snapshot_generation": self.snapshot_generation,
                "previous_snapshot_digest": self.previous_snapshot_digest,
            }
        return {**descriptor, "snapshot_digest": self.snapshot_digest}


class AutonomousCapabilityJournalStore(Protocol):
    def append(self, record: AutonomousCapabilityExecutionRecord) -> AutonomousCapabilityJournalEntry: ...
    def find(self, request_digest: str) -> AutonomousCapabilityExecutionRecord | None: ...
    def records(self) -> Sequence[AutonomousCapabilityExecutionRecord]: ...


def _entry_from_mapping(value: Mapping[str, Any]) -> AutonomousCapabilityJournalEntry:
    expected = {"schema", "sequence", "previous_entry_digest", "record", "entry_digest", "retention", "secret_material"}
    if not isinstance(value, Mapping) or set(value) != expected or value.get("schema") != AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA or value.get("retention") != "metadata_only_hash_chained_no_private_payloads" or value.get("secret_material") != "never_returned":
        raise ArgumentError("capability journal entry is malformed")
    sequence = value.get("sequence")
    if not isinstance(sequence, int) or isinstance(sequence, bool) or not 1 <= sequence <= MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES:
        raise ArgumentError("capability journal entry sequence is invalid")
    previous = _digest("capability journal previous_entry_digest", value.get("previous_entry_digest"), allow_none=True)
    record = _record_input(value.get("record"))
    entry_digest = _digest("capability journal entry_digest", value.get("entry_digest"))
    descriptor = {
        "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA,
        "sequence": sequence,
        "previous_entry_digest": previous,
        "record": record.to_dict(),
        "retention": "metadata_only_hash_chained_no_private_payloads",
        "secret_material": "never_returned",
    }
    if content_digest(descriptor) != entry_digest:
        raise ArgumentError("capability journal entry digest does not match its metadata")
    return AutonomousCapabilityJournalEntry(sequence, previous, record, entry_digest)


def validate_autonomous_capability_journal_snapshot(value: Mapping[str, Any] | AutonomousCapabilityJournalSnapshot) -> AutonomousCapabilityJournalSnapshot:
    raw = value.to_dict() if isinstance(value, AutonomousCapabilityJournalSnapshot) else value
    legacy = isinstance(raw, Mapping) and raw.get("schema") == _LEGACY_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA
    expected = {"schema", "entries", "head_digest", "retention", "secret_material", "snapshot_digest"}
    if not legacy:
        expected.update({"snapshot_generation", "previous_snapshot_digest"})
    if not isinstance(raw, Mapping) or set(raw) != expected or raw.get("schema") not in {_LEGACY_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA, AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA} or raw.get("retention") != "metadata_only_hash_bound" or raw.get("secret_material") != "never_returned":
        raise ArgumentError("capability journal snapshot is malformed")
    snapshot_generation = raw.get("snapshot_generation")
    previous_snapshot_digest = raw.get("previous_snapshot_digest")
    if not legacy:
        if not isinstance(snapshot_generation, int) or isinstance(snapshot_generation, bool) or snapshot_generation < 1:
            raise ArgumentError("capability journal snapshot generation is outside its bound")
        if previous_snapshot_digest is not None:
            previous_snapshot_digest = _digest("capability journal previous_snapshot_digest", previous_snapshot_digest)
        if (snapshot_generation == 1) != (previous_snapshot_digest is None):
            raise ArgumentError("capability journal snapshot generation and previous_snapshot_digest are inconsistent")
    entries_raw = raw.get("entries")
    if not isinstance(entries_raw, Sequence) or isinstance(entries_raw, (str, bytes, bytearray)) or len(entries_raw) > MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES:
        raise ArgumentError("capability journal snapshot exceeds its entry capacity")
    entries = tuple(_entry_from_mapping(entry) for entry in entries_raw)
    for index, entry in enumerate(entries):
        if entry.sequence != index + 1 or entry.previous_entry_digest != (None if index == 0 else entries[index - 1].entry_digest):
            raise ArgumentError("capability journal hash-chain continuity check failed")
    head = _digest("capability journal snapshot head_digest", raw.get("head_digest"), allow_none=True)
    if head != (entries[-1].entry_digest if entries else None):
        raise ArgumentError("capability journal snapshot head digest is inconsistent")
    snapshot_digest = _digest("capability journal snapshot snapshot_digest", raw.get("snapshot_digest"))
    descriptor = {
        "schema": raw["schema"],
        "entries": [entry.to_dict() for entry in entries],
        "head_digest": head,
        "retention": "metadata_only_hash_bound",
        "secret_material": "never_returned",
    }
    if not legacy:
        descriptor = {
            **descriptor,
            "snapshot_generation": snapshot_generation,
            "previous_snapshot_digest": previous_snapshot_digest,
        }
    if content_digest(descriptor) != snapshot_digest:
        raise ArgumentError("capability journal snapshot digest does not match its metadata")
    if _json_bytes(raw) > MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES:
        raise ArgumentError("capability journal snapshot exceeds its byte capacity")
    return AutonomousCapabilityJournalSnapshot(
        entries,
        head,
        snapshot_digest,
        None if legacy else snapshot_generation,
        None if legacy else previous_snapshot_digest,
    )


def _canonical_capability_journal_json(
    value: Mapping[str, Any] | AutonomousCapabilityJournalSnapshot,
) -> str:
    """Return the canonical wire representation used by durable journal stores."""

    return canonical_json(validate_autonomous_capability_journal_snapshot(value).to_dict())


class AutonomousCapabilityJournalSnapshotTextStore(Protocol):
    """Portable text persistence for metadata-only capability replay journals."""

    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousCapabilityJournalSnapshotTextStore(
    AutonomousCapabilityJournalSnapshotTextStore,
    Protocol,
):
    """Capability journal text persistence with stale-writer fencing."""

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousCapabilityJournalSnapshotPersistence:
    """Strict canonical JSON persistence over a caller-owned text store."""

    def __init__(
        self,
        store: AutonomousCapabilityJournalSnapshotTextStore,
        *,
        max_bytes: int = MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES,
    ) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("capability journal JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES:
            raise ArgumentError("capability journal JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("capability journal JSON snapshot exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ArgumentError("capability journal JSON snapshot is invalid") from error
        if not isinstance(raw, Mapping):
            raise ArgumentError("capability journal JSON snapshot must be an object")
        normalized = validate_autonomous_capability_journal_snapshot(raw).to_dict()
        if encoded != canonical_json(normalized):
            raise ArgumentError("capability journal JSON snapshot is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any] | AutonomousCapabilityJournalSnapshot) -> None:
        encoded = _canonical_capability_journal_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("capability journal JSON snapshot exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousCapabilityJournalSnapshotPersistence(
    JsonAutonomousCapabilityJournalSnapshotPersistence,
):
    """Canonical JSON capability persistence with compare-and-swap fencing."""

    def __init__(
        self,
        store: TransactionalAutonomousCapabilityJournalSnapshotTextStore,
        *,
        max_bytes: int = MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES,
    ) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional capability journal persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(
        self,
        expected_snapshot_digest: str | None,
        snapshot: Mapping[str, Any] | AutonomousCapabilityJournalSnapshot,
    ) -> bool:
        if expected_snapshot_digest is not None:
            _digest("capability journal expected snapshot digest", expected_snapshot_digest)
        encoded = _canonical_capability_journal_json(snapshot)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("capability journal JSON snapshot exceeds its byte bound")
        return self.store.write_if_unchanged(expected_snapshot_digest, encoded)


class InMemoryAutonomousCapabilityJournalStore:
    """Bounded reference journal for tests and small caller-owned workers."""

    def __init__(self) -> None:
        self._entries: list[AutonomousCapabilityJournalEntry] = []
        self._lock = threading.RLock()
        self._snapshot_generation = 0
        self._previous_snapshot_digest: str | None = None
        self._snapshot_cache: AutonomousCapabilityJournalSnapshot | None = None
        self._snapshot_cache_entry_signature: tuple[str, ...] | None = None

    def append(self, record: AutonomousCapabilityExecutionRecord) -> AutonomousCapabilityJournalEntry:
        normalized = _record_input(record)
        with self._lock:
            existing = next((entry for entry in reversed(self._entries) if entry.record.request_digest == normalized.request_digest), None)
            if existing is not None:
                if content_digest(existing.record.to_dict()) == content_digest(normalized.to_dict()):
                    return existing
                if existing.record.status in _REPLAYABLE_STATUSES:
                    raise ArgumentError("capability journal replay identity is already committed with a different outcome")
            if len(self._entries) >= MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES:
                raise ArgumentError("capability journal capacity exhausted")
            descriptor = {
                "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA,
                "sequence": len(self._entries) + 1,
                "previous_entry_digest": self._entries[-1].entry_digest if self._entries else None,
                "record": normalized.to_dict(),
                "retention": "metadata_only_hash_chained_no_private_payloads",
                "secret_material": "never_returned",
            }
            entry = AutonomousCapabilityJournalEntry(descriptor["sequence"], descriptor["previous_entry_digest"], normalized, content_digest(descriptor))
            self._entries.append(entry)
            self._snapshot_cache = None
            self._snapshot_cache_entry_signature = None
            return entry

    def find(self, request_digest: str) -> AutonomousCapabilityExecutionRecord | None:
        digest = _digest("capability journal request_digest", request_digest)
        with self._lock:
            entry = next((candidate for candidate in reversed(self._entries) if candidate.record.request_digest == digest), None)
            return None if entry is None else _record_input(entry.record)

    def records(self) -> tuple[AutonomousCapabilityExecutionRecord, ...]:
        with self._lock:
            return tuple(_record_input(entry.record) for entry in self._entries)

    def snapshot(self) -> AutonomousCapabilityJournalSnapshot:
        with self._lock:
            entries = tuple(_entry_from_mapping(entry.to_dict()) for entry in self._entries)
            signature = tuple(entry.entry_digest for entry in entries)
            if self._snapshot_cache is not None and self._snapshot_cache_entry_signature == signature:
                return self._snapshot_cache
            descriptor = {
                "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
                "snapshot_generation": self._snapshot_generation + 1,
                "previous_snapshot_digest": self._previous_snapshot_digest if self._snapshot_generation else None,
                "entries": [entry.to_dict() for entry in entries],
                "head_digest": entries[-1].entry_digest if entries else None,
                "retention": "metadata_only_hash_bound",
                "secret_material": "never_returned",
            }
            snapshot = validate_autonomous_capability_journal_snapshot({**descriptor, "snapshot_digest": content_digest(descriptor)})
            self._snapshot_generation = snapshot.snapshot_generation or 0
            self._previous_snapshot_digest = snapshot.snapshot_digest
            self._snapshot_cache = snapshot
            self._snapshot_cache_entry_signature = signature
            return snapshot

    def restore(self, snapshot: AutonomousCapabilityJournalSnapshot | Mapping[str, Any]) -> None:
        validated = validate_autonomous_capability_journal_snapshot(snapshot)
        with self._lock:
            self._entries = list(validated.entries)
            self._snapshot_generation = validated.snapshot_generation or 0
            self._previous_snapshot_digest = validated.snapshot_digest if self._snapshot_generation else None
            self._snapshot_cache = validated if validated.snapshot_generation is not None else None
            self._snapshot_cache_entry_signature = None if self._snapshot_cache is None else tuple(entry.entry_digest for entry in validated.entries)


class AutonomousCapabilityJournalPersistenceCoordinator:
    def __init__(self, store: InMemoryAutonomousCapabilityJournalStore, persistence: Any) -> None:
        if not isinstance(store, InMemoryAutonomousCapabilityJournalStore) or not hasattr(persistence, "read") or not hasattr(persistence, "write"):
            raise ArgumentError("capability journal persistence requires a snapshot store and read/write persistence")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def flush(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.store.snapshot()
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("capability journal persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot.to_dict())
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return {
                "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
                "bytes": _json_bytes(snapshot.to_dict()),
                "snapshot_digest": snapshot.snapshot_digest,
                "snapshot_generation": snapshot.snapshot_generation,
                "retention": "metadata_only",
            }

    def restore(self) -> dict[str, Any]:
        with self._lock:
            raw = self.persistence.read()
            if raw is None:
                self._expected_snapshot_digest = None
                return {"restored": False, "entry_count": 0, "snapshot_digest": None, "snapshot_generation": None}
            snapshot = validate_autonomous_capability_journal_snapshot(raw)
            self.store.restore(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return {
                "restored": True,
                "entry_count": len(snapshot.entries),
                "snapshot_digest": snapshot.snapshot_digest,
                "snapshot_generation": snapshot.snapshot_generation,
            }


class AutonomousCapabilityRuntime:
    """Execute one reviewed capability with bounded replay and evaluator-facing metadata."""

    def __init__(self, runtime: AutonomousDomainToolRuntime, *, journal: AutonomousCapabilityJournalStore | None = None) -> None:
        if not isinstance(runtime, AutonomousDomainToolRuntime):
            raise ArgumentError("autonomous capability runtime requires an AutonomousDomainToolRuntime")
        if journal is not None and not all(callable(getattr(journal, name, None)) for name in ("append", "find", "records")):
            raise ArgumentError("autonomous capability journal is malformed")
        self.runtime = runtime
        self.journal = journal
        self._cache: dict[str, AutonomousCapabilityExecutionResult] = {}
        self._rehydrated_by_request: dict[str, AutonomousCapabilityExecutionRecord] = {}
        self._rehydrated_by_replay: dict[str, AutonomousCapabilityExecutionRecord] = {}
        self._history: list[AutonomousCapabilityExecutionRecord] = []
        self._lock = threading.RLock()
        self._inflight: dict[str, tuple[str, threading.Event]] = {}
        self._inflight_results: dict[str, AutonomousCapabilityExecutionResult | BaseException] = {}

    def _copy_result(self, result: AutonomousCapabilityExecutionResult, replay: str, *, include_value: bool = True) -> AutonomousCapabilityExecutionResult:
        record = _record_input({**result.record.to_dict(), "replay": replay}, require_fresh=False)
        return AutonomousCapabilityExecutionResult(record, result.value if include_value else None)

    def _record_result(self, result: AutonomousCapabilityExecutionResult) -> AutonomousCapabilityExecutionResult:
        if self.journal is not None and result.record.replay == "fresh":
            self.journal.append(result.record)
        with self._lock:
            self._history.append(_record_input(result.record))
            while len(self._history) > MAX_AUTONOMOUS_CAPABILITY_HISTORY:
                self._history.pop(0)
            if result.record.status == "completed" and result.record.replay == "fresh":
                key = result.record.replay_key_digest or result.record.request_digest
                self._cache[key] = result
                while len(self._cache) > MAX_AUTONOMOUS_CAPABILITY_HISTORY:
                    self._cache.pop(next(iter(self._cache)))
        return result

    def _refusal(self, request: Mapping[str, Any], request_digest: str, arguments_digest: str, replay_key_digest: str | None, reason: str, started: float, tool: AutonomousDomainTool | None = None, required: Sequence[str] = ()) -> AutonomousCapabilityExecutionResult:
        record = self._build_record(request, request_digest, arguments_digest, replay_key_digest, status="refused", started=started, tool=tool, required=required, error_class=reason, limitations=(reason,))
        return self._record_result(AutonomousCapabilityExecutionResult(record))

    def _build_record(self, request: Mapping[str, Any], request_digest: str, arguments_digest: str, replay_key_digest: str | None, *, status: str, started: float, tool: AutonomousDomainTool | None, required: Sequence[str], output_digest: str | None = None, output_bytes: int = 0, observations: Sequence[AutonomousCapabilityObservation] = (), evidence_digest: str | None = None, evidence_status: str = "not_evaluated", missing: Sequence[str] | None = None, limitations: Sequence[str] = (), error_class: str | None = None, stage_contract_digest: str | None = None) -> AutonomousCapabilityExecutionRecord:
        context = request["workflow_context"]
        return AutonomousCapabilityExecutionRecord(
            request_digest=request_digest,
            execution_id=request["execution_id"],
            call_id=request["call_id"],
            domain=context["domain"],
            workflow_id=context["workflow_id"],
            workflow_digest=context["workflow_digest"],
            stage_id=context["stage_id"],
            stage_contract_digest=stage_contract_digest,
            tool=request["tool"],
            capability=None if tool is None else tool.capability,
            risk_class=None if tool is None else tool.risk_class,
            schema_digest=None if tool is None else tool.schema_digest,
            input_digest=request["input_digest"],
            subject_digest=request["subject_digest"],
            parent_evidence_digests=tuple(request["parent_evidence_digests"]),
            arguments_digest=arguments_digest,
            replay_key_digest=replay_key_digest,
            status=status,
            replay="fresh",
            output_digest=output_digest,
            output_bytes=output_bytes,
            observations=tuple(observations),
            evidence_digest=evidence_digest,
            evidence_status=evidence_status,
            required_evidence_outputs=tuple(required),
            missing_evidence_outputs=tuple(required if missing is None else missing),
            limitations=tuple(limitations) or (("raw adapter output is transient and not part of the durable record",) if status == "completed" else ("capability did not produce a durable success observation",)),
            effect=None if tool is None else tool.risk_class,
            effect_id=None,
            error_class=error_class,
            duration_ms=max(0, int((time.monotonic() - started) * 1000)),
            does_not_claim=_common_claims(),
        )

    def _execute_fresh(self, request: Mapping[str, Any], request_digest: str, arguments_digest: str, replay_key_digest: str | None, *, project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None) -> AutonomousCapabilityExecutionResult:
        started = time.monotonic()
        tool: AutonomousDomainTool | None = None
        required: tuple[str, ...] = ()
        stage_contract_digest: str | None = None
        try:
            tool = self.runtime.registry.resolve(request["tool"])
            if request["workflow_context"]["domain"] not in tool.domains and "cross_domain" not in tool.domains:
                return self._refusal(request, request_digest, arguments_digest, replay_key_digest, "tool_domain_mismatch", started, tool=tool)
            stage_contract_digest, required = _stage_details(request, tool)
            if stage_contract_digest is None and request["workflow_context"]["domain"] in {"coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"}:
                return self._refusal(request, request_digest, arguments_digest, replay_key_digest, "stage_contract_mismatch", started, tool=tool, required=required)
            result = self.runtime((ProviderToolCall(request["call_id"], request["tool"], request["arguments"]),))[0]
        except Exception as error:
            return self._refusal(request, request_digest, arguments_digest, replay_key_digest, type(error).__name__, started, tool=tool, required=required)
        content = result.content
        declared = content.get("status") if isinstance(content, Mapping) else None
        if declared == "approval_required":
            status = "approval_required"
        elif declared == "reconciliation_required":
            status = "reconciliation_required"
        elif declared in {"refused", "activation_required", "authorization_required"}:
            status = "refused"
        else:
            status = "completed" if result.approved and not result.is_error else "failed"
        if status != "completed":
            record = self._build_record(request, request_digest, arguments_digest, replay_key_digest, status=status, started=started, tool=tool, required=required, error_class=declared if isinstance(declared, str) else None, stage_contract_digest=stage_contract_digest)
            return self._record_result(AutonomousCapabilityExecutionResult(record))
        value = _clone_json(content)
        output_digest = content_digest(value)
        observations: tuple[AutonomousCapabilityObservation, ...] = ()
        evidence_status = "missing_required_outputs"
        projection_failure: str | None = None
        if project_observations is not None:
            try:
                projected = project_observations(value, request)
                if not isinstance(projected, Sequence) or isinstance(projected, (str, bytes, bytearray)) or len(projected) > MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS:
                    raise ArgumentError("capability observations exceed their bound")
                observations = tuple(_observation(item, index) for index, item in enumerate(projected))
                labels = {item.label for item in observations}
                evidence_status = "declared_for_evaluator" if all(label in labels for label in required) else "missing_required_outputs"
            except Exception as error:
                projection_failure = type(error).__name__
                evidence_status = "projection_failed"
        missing = tuple(label for label in required if label not in {item.label for item in observations})
        evidence_digest = content_digest({"schema": AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA, "request_digest": request_digest, "input_digest": request["input_digest"], "arguments_digest": arguments_digest, "output_digest": output_digest, "required_evidence_outputs": list(required), "observations": [item.to_dict() for item in observations], "evidence_status": evidence_status})
        limitations = ("observation projection failed", projection_failure) if projection_failure else ("raw adapter output is transient and not part of the durable record",)
        record = self._build_record(request, request_digest, arguments_digest, replay_key_digest, status="completed", started=started, tool=tool, required=required, output_digest=output_digest, output_bytes=_json_bytes(value), observations=observations, evidence_digest=evidence_digest, evidence_status=evidence_status, missing=missing, limitations=limitations, stage_contract_digest=stage_contract_digest)
        return self._record_result(AutonomousCapabilityExecutionResult(record, value))

    def execute(self, request: Mapping[str, Any], *, project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None) -> AutonomousCapabilityExecutionResult:
        normalized = _normalize_request(request)
        request_digest, arguments_digest, replay_key_digest = _request_identity(normalized)
        key = replay_key_digest or request_digest
        with self._lock:
            cached = self._cache.get(key)
            if cached is not None:
                if cached.record.request_digest != request_digest:
                    raise ArgumentError("capability replay key collides with different request metadata")
                return self._copy_result(cached, "replayed")
            rehydrated = self._rehydrated_by_replay.get(replay_key_digest) if replay_key_digest else None
            rehydrated = rehydrated or self._rehydrated_by_request.get(request_digest)
            if rehydrated is not None:
                result = AutonomousCapabilityExecutionResult(rehydrated)
                return self._copy_result(result, "replayed", include_value=False)
            pending = self._inflight.get(key)
            if pending is None:
                event = threading.Event()
                self._inflight[key] = (request_digest, event)
                owner = True
            else:
                pending_digest, event = pending
                if pending_digest != request_digest:
                    raise ArgumentError("capability replay key collides with different in-flight request metadata")
                owner = False
        if not owner:
            event.wait()
            with self._lock:
                outcome = self._inflight_results.get(key)
            if isinstance(outcome, BaseException):
                raise outcome
            if outcome is None:
                raise ArgumentError("capability in-flight execution completed without an outcome")
            return self._copy_result(outcome, "replayed")
        try:
            outcome = self._execute_fresh(normalized, request_digest, arguments_digest, replay_key_digest, project_observations=project_observations)
            with self._lock:
                self._inflight_results[key] = outcome
            return outcome
        except BaseException as error:
            with self._lock:
                self._inflight_results[key] = error
            raise
        finally:
            with self._lock:
                pending = self._inflight.pop(key, None)
                if pending is not None:
                    pending[1].set()
                while len(self._inflight_results) > MAX_AUTONOMOUS_CAPABILITY_HISTORY:
                    self._inflight_results.pop(next(iter(self._inflight_results)))

    def execute_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None,
        max_parallelism: int = 1,
    ) -> tuple[AutonomousCapabilityExecutionResult, ...]:
        """Execute a bounded batch while preserving request order and replay guarantees.

        Parallelism is an application hint only: each request still crosses the same schema,
        approval, domain-contract, journal, and in-flight deduplication boundaries. Results are
        returned in input order so callers can join them to their own task graph without
        retaining provider or adapter payloads in the runtime history.
        """

        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)):
            raise ArgumentError("capability batch must be a sequence")
        if not requests or len(requests) > MAX_AUTONOMOUS_CAPABILITY_BATCH:
            raise ArgumentError(
                f"capability batch must contain between 1 and {MAX_AUTONOMOUS_CAPABILITY_BATCH} requests"
            )
        if not isinstance(max_parallelism, int) or isinstance(max_parallelism, bool) or not 1 <= max_parallelism <= 16:
            raise ArgumentError("capability batch max_parallelism must be between 1 and 16")
        if max_parallelism == 1 or len(requests) == 1:
            return tuple(
                self.execute(request, project_observations=project_observations)
                for request in requests
            )
        with ThreadPoolExecutor(max_workers=min(max_parallelism, len(requests))) as pool:
            futures = tuple(
                pool.submit(
                    self.execute,
                    request,
                    project_observations=project_observations,
                )
                for request in requests
            )
            return tuple(future.result() for future in futures)

    def rehydrate(self) -> dict[str, Any]:
        if self.journal is None:
            return {"restored": 0, "replayable": 0, "value_retention": "transient_caller_value_only"}
        with self._lock:
            if self._inflight:
                raise ArgumentError("cannot rehydrate capability journal while execution is in flight")
            records = tuple(_record_input(item) for item in self.journal.records())
            if len(records) > MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES:
                raise ArgumentError("capability journal returned too many records")
            self._cache.clear()
            self._rehydrated_by_request.clear()
            self._rehydrated_by_replay.clear()
            self._history.clear()
            for record in records:
                self._rehydrated_by_request.pop(record.request_digest, None)
                if record.replay_key_digest is not None:
                    self._rehydrated_by_replay.pop(record.replay_key_digest, None)
                if record.status in _REPLAYABLE_STATUSES:
                    self._rehydrated_by_request[record.request_digest] = record
                    if record.replay_key_digest is not None:
                        prior = self._rehydrated_by_replay.get(record.replay_key_digest)
                        if prior is not None and prior.request_digest != record.request_digest:
                            raise ArgumentError("capability journal contains a replay-key collision")
                        self._rehydrated_by_replay[record.replay_key_digest] = record
                self._history.append(record)
            self._history = self._history[-MAX_AUTONOMOUS_CAPABILITY_HISTORY:]
            return {"restored": len(records), "replayable": len(self._rehydrated_by_request), "value_retention": "transient_caller_value_only"}

    def execution_evidence(self) -> list[dict[str, Any]]:
        with self._lock:
            return [_clone_json(record.to_dict()) for record in self._history]


__all__ = [
    "AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA",
    "AUTONOMOUS_CAPABILITY_BATCH_SCHEMA",
    "AUTONOMOUS_CAPABILITY_OBSERVATION_SCHEMA",
    "AUTONOMOUS_CAPABILITY_JOURNAL_SCHEMA",
    "AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA",
    "MAX_AUTONOMOUS_CAPABILITY_BATCH",
    "MAX_AUTONOMOUS_CAPABILITY_HISTORY",
    "MAX_AUTONOMOUS_CAPABILITY_OBSERVATIONS",
    "MAX_AUTONOMOUS_CAPABILITY_JOURNAL_ENTRIES",
    "MAX_AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_BYTES",
    "AutonomousCapabilityObservation",
    "AutonomousCapabilityExecutionRecord",
    "AutonomousCapabilityExecutionResult",
    "AutonomousCapabilityJournalEntry",
    "AutonomousCapabilityJournalSnapshot",
    "AutonomousCapabilityJournalStore",
    "AutonomousCapabilityJournalSnapshotTextStore",
    "TransactionalAutonomousCapabilityJournalSnapshotTextStore",
    "JsonAutonomousCapabilityJournalSnapshotPersistence",
    "TransactionalJsonAutonomousCapabilityJournalSnapshotPersistence",
    "InMemoryAutonomousCapabilityJournalStore",
    "AutonomousCapabilityJournalPersistenceCoordinator",
    "AutonomousCapabilityRuntime",
    "validate_autonomous_capability_journal_snapshot",
]
