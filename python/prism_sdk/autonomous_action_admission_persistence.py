"""Restart-safe, metadata-only persistence for autonomous action-plan admission.

The action-plan module produces a recommendation and the action-execution module produces an
explicit gate decision. This module supplies the missing deployment seam between those values and
an operator-controlled durable store: revisioned records, reviewer identity digests, predecessor
links, canonical snapshots, and compare-and-set persistence. It deliberately stores only the
already-redacted plan/admission projections; task text, prompts, credentials, provider output,
connector values, evaluator evidence, and effect authority remain caller-owned.
"""

from __future__ import annotations

from typing import Any, Mapping, Protocol
import json

from .authoring import content_digest
from .autonomous_action_execution import AutonomousActionAdmission, admit_autonomous_action_plan
from .autonomous_action_plan import AutonomousActionPlan
from .errors import ArgumentError


AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA = "bioprism-python-autonomous-action-admission-record/0.1"
AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-action-admission-snapshot/0.1"
AUTONOMOUS_ACTION_ADMISSION_RETENTION = "metadata_only;plan_admission_and_review_digests;task_prompt_provider_connector_credential_and_effect_values_not_retained"
AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL = "never_returned"
AUTONOMOUS_ACTION_ADMISSION_AUTHORITY = "caller_review_record_only;does_not_authorize_provider_source_tool_effect_or_credentials"
AUTONOMOUS_ACTION_ADMISSION_EXECUTION = "admission_only;downstream_provider_source_tool_effect_and_credential_gates_remain_required"
MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS = 4_096
MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES = 4_000_000
MAX_AUTONOMOUS_ACTION_ADMISSION_ACTION_ID_BYTES = 256

_RECORD_KEYS = {
    "schema", "action_id", "revision", "status", "decision", "plan", "admission", "reviewer_digest",
    "reason_digest", "previous_record_digest", "authority", "retention", "execution", "secret_material",
}
_SNAPSHOT_KEYS = {"schema", "generation", "records", "previous_snapshot_digest", "retention", "secret_material"}
_STATUSES = {"pending_review", "admitted", "blocked"}
_DECISIONS = {"submitted", "reviewed"}


def _fail(message: str) -> None:
    raise ArgumentError(f"autonomous action admission persistence {message}")


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False))
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous action admission persistence value is not canonical JSON") from error


def _text(name: str, value: Any, maximum: int = 2_048) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its text bound")
    return value


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, MAX_AUTONOMOUS_ACTION_ADMISSION_ACTION_ID_BYTES)
    if not all(character.isalnum() or character in "_.:+/-" for character in result):
        _fail(f"{name} contains unsupported identifier characters")
    return result


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bound")
    return value


def _digest(name: str, value: Any, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _safe_metadata(value: Any, depth: int = 0) -> None:
    if depth > 16:
        _fail("metadata nesting exceeds its bound")
    if value is None or isinstance(value, (str, bool, int, float)):
        if isinstance(value, float) and (value != value or value in (float("inf"), -float("inf"))):
            _fail("metadata contains a non-finite number")
        return
    if isinstance(value, Mapping):
        if len(value) > 4_096:
            _fail("metadata object exceeds its bound")
        for key, child in value.items():
            normalized = str(key).replace("_", "").replace("-", "").lower()
            if normalized in {"task", "prompt", "credential", "credentials", "secret", "token", "password", "response", "messages", "body", "headers"}:
                _fail("metadata contains transient or secret-shaped material")
            _safe_metadata(child, depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 4_096:
            _fail("metadata sequence exceeds its bound")
        for child in value:
            _safe_metadata(child, depth + 1)
        return
    _fail("metadata contains an unsupported value")


def _plan(value: AutonomousActionPlan | Mapping[str, Any]) -> AutonomousActionPlan:
    if isinstance(value, AutonomousActionPlan):
        return value
    if not isinstance(value, Mapping):
        _fail("record plan must be a metadata mapping")
    try:
        return AutonomousActionPlan.from_dict(value)
    except Exception as error:
        raise ArgumentError("autonomous action admission persistence plan is invalid") from error


def _admission(value: AutonomousActionAdmission | Mapping[str, Any]) -> AutonomousActionAdmission:
    if isinstance(value, AutonomousActionAdmission):
        return value
    if not isinstance(value, Mapping):
        _fail("record admission must be a metadata mapping")
    try:
        return AutonomousActionAdmission.from_dict(value)
    except Exception as error:
        raise ArgumentError("autonomous action admission persistence admission is invalid") from error


def _record_status(admission: AutonomousActionAdmission) -> str:
    if admission.status == "admitted":
        return "admitted"
    if admission.status == "blocked":
        return "blocked"
    return "pending_review"


def _record_body(
    *,
    action_id: Any,
    revision: Any,
    plan: AutonomousActionPlan,
    admission: AutonomousActionAdmission,
    reviewer_digest: Any,
    reason_digest: Any,
    previous_record_digest: Any,
) -> dict[str, Any]:
    status = _record_status(admission)
    reviewer = _digest("reviewer_digest", reviewer_digest, allow_none=True)
    if status == "admitted" and reviewer is None:
        _fail("an admitted record requires a reviewer digest")
    return {
        "schema": AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA,
        "action_id": _identifier("action_id", action_id),
        "revision": _integer("revision", revision, 1, 2_147_483_647),
        "status": status,
        "decision": "submitted" if reviewer is None else "reviewed",
        "plan": plan.to_dict(),
        "admission": admission.to_dict(),
        "reviewer_digest": reviewer,
        "reason_digest": _digest("reason_digest", reason_digest, allow_none=True),
        "previous_record_digest": _digest("previous_record_digest", previous_record_digest, allow_none=True),
        "authority": AUTONOMOUS_ACTION_ADMISSION_AUTHORITY,
        "retention": AUTONOMOUS_ACTION_ADMISSION_RETENTION,
        "execution": AUTONOMOUS_ACTION_ADMISSION_EXECUTION,
        "secret_material": AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
    }


def validate_autonomous_action_admission_record(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or set(value) != _RECORD_KEYS | {"record_digest"}:
        _fail("record contains unsupported or missing fields")
    _safe_metadata(value)
    if value["schema"] != AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA or value["authority"] != AUTONOMOUS_ACTION_ADMISSION_AUTHORITY or value["retention"] != AUTONOMOUS_ACTION_ADMISSION_RETENTION or value["execution"] != AUTONOMOUS_ACTION_ADMISSION_EXECUTION or value["secret_material"] != AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL:
        _fail("record markers are invalid")
    plan = _plan(value["plan"])
    admission = _admission(value["admission"])
    if plan.plan_digest != admission.plan_digest:
        _fail("record admission is bound to a different plan")
    body = _record_body(
        action_id=value["action_id"],
        revision=value["revision"],
        plan=plan,
        admission=admission,
        reviewer_digest=value["reviewer_digest"],
        reason_digest=value["reason_digest"],
        previous_record_digest=value["previous_record_digest"],
    )
    if value["status"] != body["status"] or value["decision"] != body["decision"] or body["status"] not in _STATUSES or body["decision"] not in _DECISIONS:
        _fail("record status or decision is invalid")
    supplied = _digest("record_digest", value["record_digest"])
    expected = content_digest(body)
    if supplied != expected:
        _fail("record digest does not match metadata")
    return _clone({**body, "record_digest": expected})


def create_autonomous_action_admission_record(
    plan: AutonomousActionPlan | Mapping[str, Any],
    admission: AutonomousActionAdmission | Mapping[str, Any],
    *,
    action_id: str,
    reviewer_digest: str | None = None,
    reason: str | None = None,
    previous_record_digest: str | None = None,
) -> dict[str, Any]:
    normalized_plan = _plan(plan)
    normalized_admission = _admission(admission)
    if normalized_plan.plan_digest != normalized_admission.plan_digest:
        _fail("admission is bound to a different plan")
    reason_digest = None if reason is None else content_digest(_text("reason", reason, 4_096))
    body = _record_body(
        action_id=action_id,
        revision=1,
        plan=normalized_plan,
        admission=normalized_admission,
        reviewer_digest=reviewer_digest,
        reason_digest=reason_digest,
        previous_record_digest=previous_record_digest,
    )
    return _clone({**body, "record_digest": content_digest(body)})


def review_autonomous_action_admission_record(
    record: Mapping[str, Any],
    *,
    approvals: Mapping[str, bool] | None = None,
    reviewed: bool = False,
    reviewer_digest: str,
    reason: str | None = None,
    expected_record_digest: str | None = None,
) -> dict[str, Any]:
    current = validate_autonomous_action_admission_record(record)
    if expected_record_digest is not None and expected_record_digest != current["record_digest"]:
        _fail("review expected_record_digest does not match the current record")
    reviewer = _digest("reviewer_digest", reviewer_digest)
    admission = admit_autonomous_action_plan(
        _plan(current["plan"]),
        approvals=approvals,
        reviewed=reviewed,
    )
    reason_digest = None if reason is None else content_digest(_text("reason", reason, 4_096))
    body = _record_body(
        action_id=current["action_id"],
        revision=current["revision"] + 1,
        plan=_plan(current["plan"]),
        admission=admission,
        reviewer_digest=reviewer,
        reason_digest=reason_digest,
        previous_record_digest=current["record_digest"],
    )
    return _clone({**body, "record_digest": content_digest(body)})


def validate_autonomous_action_admission_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or set(value) != _SNAPSHOT_KEYS | {"snapshot_digest"}:
        _fail("snapshot contains unsupported or missing fields")
    _safe_metadata(value)
    if value["schema"] != AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA or value["retention"] != AUTONOMOUS_ACTION_ADMISSION_RETENTION or value["secret_material"] != AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL:
        _fail("snapshot markers are invalid")
    generation = _integer("snapshot generation", value["generation"], 0, 2_147_483_647)
    records = value["records"]
    if isinstance(records, (str, bytes, bytearray)) or not isinstance(records, list) or len(records) > MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS:
        _fail("snapshot records exceed their bound")
    normalized_records = [validate_autonomous_action_admission_record(record) for record in records]
    ids = [record["action_id"] for record in normalized_records]
    if len(set(ids)) != len(ids):
        _fail("snapshot contains duplicate action ids")
    body = {
        "schema": AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "records": sorted(normalized_records, key=lambda record: record["action_id"]),
        "previous_snapshot_digest": _digest("snapshot previous_snapshot_digest", value["previous_snapshot_digest"], allow_none=True),
        "retention": AUTONOMOUS_ACTION_ADMISSION_RETENTION,
        "secret_material": AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
    }
    supplied = _digest("snapshot_digest", value["snapshot_digest"])
    expected = content_digest(body)
    if supplied != expected:
        _fail("snapshot digest does not match metadata")
    result = _clone({**body, "snapshot_digest": expected})
    if len(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return result


def seal_autonomous_action_admission_snapshot(
    *,
    generation: int,
    records: list[Mapping[str, Any]],
    previous_snapshot_digest: str | None = None,
) -> dict[str, Any]:
    generation = _integer("snapshot generation", generation, 0, 2_147_483_647)
    normalized_records = [validate_autonomous_action_admission_record(record) for record in records]
    if len(normalized_records) > MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS:
        _fail("snapshot records exceed their bound")
    body = {
        "schema": AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "records": sorted(normalized_records, key=lambda record: record["action_id"]),
        "previous_snapshot_digest": _digest("snapshot previous_snapshot_digest", previous_snapshot_digest, allow_none=True),
        "retention": AUTONOMOUS_ACTION_ADMISSION_RETENTION,
        "secret_material": AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL,
    }
    result = _clone({**body, "snapshot_digest": content_digest(body)})
    if len(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return result


class InMemoryAutonomousActionAdmissionLedger:
    """A bounded revision-fenced action-admission ledger for one caller-owned process."""

    def __init__(self, *, max_records: int = MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS) -> None:
        self.max_records = _integer("max_records", max_records, 1, MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS)
        self._records: dict[str, dict[str, Any]] = {}

    def put(self, record: Mapping[str, Any]) -> dict[str, Any]:
        normalized = validate_autonomous_action_admission_record(record)
        action_id = normalized["action_id"]
        existing = self._records.get(action_id)
        if existing is not None and existing["record_digest"] not in {normalized["previous_record_digest"], normalized["record_digest"]}:
            _fail("record predecessor conflicts with the current action record")
        if existing is None and normalized["revision"] != 1:
            _fail("new action records must begin at revision one")
        if existing is not None and normalized["revision"] != existing["revision"] + 1 and normalized["record_digest"] != existing["record_digest"]:
            _fail("action record revision is not contiguous")
        if existing is None and len(self._records) >= self.max_records:
            _fail("ledger capacity is exhausted")
        self._records[action_id] = normalized
        return _clone(normalized)

    def submit(self, plan: AutonomousActionPlan | Mapping[str, Any], admission: AutonomousActionAdmission | Mapping[str, Any], **kwargs: Any) -> dict[str, Any]:
        return self.put(create_autonomous_action_admission_record(plan, admission, **kwargs))

    def review(self, action_id: str, **kwargs: Any) -> dict[str, Any]:
        current = self.get(action_id)
        if current is None:
            _fail("cannot review an unknown action record")
        return self.put(review_autonomous_action_admission_record(current, **kwargs))

    def get(self, action_id: str) -> dict[str, Any] | None:
        normalized = _identifier("action_id", action_id)
        value = self._records.get(normalized)
        return None if value is None else _clone(value)

    def list(self) -> list[dict[str, Any]]:
        return [_clone(self._records[action_id]) for action_id in sorted(self._records)]

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_action_admission_snapshot(snapshot)
        if len(normalized["records"]) > self.max_records:
            _fail("snapshot exceeds ledger capacity")
        self._records = {record["action_id"]: record for record in normalized["records"]}


class AutonomousActionAdmissionSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousActionAdmissionSnapshotTextStore(AutonomousActionAdmissionSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousActionAdmissionSnapshotPersistence:
    def __init__(self, store: AutonomousActionAdmissionSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _integer("max_bytes", max_bytes, 1, MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("stored JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("autonomous action admission persistence stored JSON is invalid") from error
        normalized = validate_autonomous_action_admission_snapshot(raw)
        canonical = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if canonical != encoded:
            _fail("stored JSON is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_action_admission_snapshot(snapshot)
        encoded = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("snapshot exceeds the configured byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousActionAdmissionSnapshotPersistence(JsonAutonomousActionAdmissionSnapshotPersistence):
    def __init__(self, store: TransactionalAutonomousActionAdmissionSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional JSON persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected_snapshot_digest", expected_snapshot_digest, allow_none=True)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, json.dumps(validate_autonomous_action_admission_snapshot(snapshot), ensure_ascii=False, sort_keys=True, separators=(",", ":"))))


class AutonomousActionAdmissionPersistenceCoordinator:
    def __init__(self, ledger: InMemoryAutonomousActionAdmissionLedger, persistence: JsonAutonomousActionAdmissionSnapshotPersistence) -> None:
        if not isinstance(ledger, InMemoryAutonomousActionAdmissionLedger):
            _fail("coordinator requires a typed ledger")
        if not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("coordinator persistence is malformed")
        self.ledger = ledger
        self.persistence = persistence
        self.expected_snapshot_digest: str | None = None
        self.expected_generation = 0

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            self.expected_snapshot_digest = None
            self.expected_generation = 0
            return None
        normalized = validate_autonomous_action_admission_snapshot(snapshot)
        self.ledger.restore(normalized)
        self.expected_snapshot_digest = normalized["snapshot_digest"]
        self.expected_generation = normalized["generation"]
        return normalized

    def flush(self) -> dict[str, Any]:
        snapshot = seal_autonomous_action_admission_snapshot(
            generation=self.expected_generation + 1,
            records=self.ledger.list(),
            previous_snapshot_digest=self.expected_snapshot_digest,
        )
        transactional = getattr(self.persistence, "write_if_unchanged", None)
        if callable(transactional):
            if not transactional(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-set conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        self.expected_generation = snapshot["generation"]
        return snapshot


__all__ = [
    "AUTONOMOUS_ACTION_ADMISSION_RECORD_SCHEMA",
    "AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_ACTION_ADMISSION_RETENTION",
    "AUTONOMOUS_ACTION_ADMISSION_SECRET_MATERIAL",
    "AUTONOMOUS_ACTION_ADMISSION_AUTHORITY",
    "AUTONOMOUS_ACTION_ADMISSION_EXECUTION",
    "MAX_AUTONOMOUS_ACTION_ADMISSION_RECORDS",
    "MAX_AUTONOMOUS_ACTION_ADMISSION_SNAPSHOT_BYTES",
    "create_autonomous_action_admission_record",
    "review_autonomous_action_admission_record",
    "validate_autonomous_action_admission_record",
    "seal_autonomous_action_admission_snapshot",
    "validate_autonomous_action_admission_snapshot",
    "InMemoryAutonomousActionAdmissionLedger",
    "JsonAutonomousActionAdmissionSnapshotPersistence",
    "TransactionalJsonAutonomousActionAdmissionSnapshotPersistence",
    "AutonomousActionAdmissionPersistenceCoordinator",
]
