"""Durable, metadata-only operator decisions for autonomous goal previews.

The control-loop preview is deliberately explanatory rather than authoritative.  Deployments
still need a durable handoff between an operator reviewing that explanation and a later execution
attempt.  This module supplies that handoff without becoming an authentication system: it stores
only the preview projection, identity digests, bounded time fields, revision links, and canonical
content digests.  The caller remains responsible for authenticating the operator and for storing
the actual task, prompt, provider, credential, connector, and effect values.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
import json
import re
from typing import Any, Protocol

from .authoring import canonical_json, content_digest
from .goals import AutonomousGoalError


AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA = "bioprism-autonomous-goal-preview-admission-record/0.1"
AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA = "bioprism-autonomous-goal-preview-admission-snapshot/0.1"
AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION = "metadata_only_goal_preview_approval;tasks_prompts_parameters_credentials_and_results_not_retained"
AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL = "never_returned"
AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY = "caller_operator_review_only;does_not_authenticate_or_authorize_provider_source_tool_effect_or_credentials"
AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION = "approval_only;execution_requires_current_preview_digest_and_downstream_policy_gates"
MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS = 4_096
MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES = 4_000_000
MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_ID_BYTES = 256
MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES = 4_096
MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_TTL_NS = 7 * 24 * 60 * 60 * 1_000_000_000

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:/-]+$")
_RECORD_KEYS = {
    "schema", "admission_id", "revision", "status", "decision", "preview", "preview_digest",
    "requested_by_digest", "reviewer_digest", "issued_at_ns", "expires_at_ns", "reason_digest",
    "previous_record_digest", "authority", "retention", "execution", "secret_material",
}
_SNAPSHOT_KEYS = {"schema", "generation", "records", "previous_snapshot_digest", "retention", "secret_material"}
_STATUSES = {"pending_review", "approved", "rejected"}
_DECISIONS = {"submitted", "approved", "rejected"}
_PREVIEW_KEYS = {
    "schema", "schedule", "status", "eligible_goal_count", "decision_counts", "reason_counts",
    "status_counts", "dependency_blocked_goal_ids", "learning_state_digest", "retention",
    "secret_material", "preview_digest",
}


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal preview admission {message}")


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(canonical_json(value))
    except (TypeError, ValueError) as error:
        raise AutonomousGoalError("autonomous goal preview admission value is not canonical JSON") from error


def _text(value: Any, *, name: str, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its text bound")
    return value.strip()


def _identifier(value: Any, *, name: str, maximum: int = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_ID_BYTES) -> str:
    result = _text(value, name=name, maximum=maximum)
    if _IDENTIFIER.fullmatch(result) is None:
        _fail(f"{name} contains unsupported identifier characters")
    return result


def _integer(value: Any, *, name: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bound")
    return value


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or _DIGEST.fullmatch(value) is None:
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _safe_metadata(value: Any, *, depth: int = 0) -> None:
    if depth > 18:
        _fail("metadata nesting exceeds its bound")
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if value != value or value in {float("inf"), -float("inf")}:
            _fail("metadata contains a non-finite number")
        return
    if isinstance(value, Mapping):
        if len(value) > 4_096:
            _fail("metadata object exceeds its bound")
        for key, child in value.items():
            normalized = str(key).replace("_", "").replace("-", "").lower()
            if normalized in {"task", "prompt", "credential", "credentials", "secret", "token", "password", "messages", "body", "headers", "response", "result"}:
                _fail("metadata contains transient or secret-shaped material")
            _safe_metadata(child, depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 4_096:
            _fail("metadata sequence exceeds its bound")
        for child in value:
            _safe_metadata(child, depth=depth + 1)
        return
    _fail("metadata contains an unsupported value")


def _counts(value: Any, *, name: str) -> dict[str, int]:
    if not isinstance(value, Mapping) or len(value) > 256:
        _fail(f"{name} is malformed")
    result: dict[str, int] = {}
    for key, raw in value.items():
        result[_identifier(key, name=f"{name} key", maximum=128)] = _integer(raw, name=f"{name} value", minimum=0, maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS)
    return dict(sorted(result.items()))


def _sequence(value: Any, *, name: str, maximum: int) -> list[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        _fail(f"{name} is outside its sequence bound")
    return list(value)


def _preview(value: Any) -> dict[str, Any]:
    if callable(getattr(value, "to_dict", None)):
        value = value.to_dict()
    if not isinstance(value, Mapping) or set(value) != _PREVIEW_KEYS:
        _fail("preview projection contains unsupported or missing fields")
    if value["schema"] != "bioprism-autonomous-goal-control-preview/0.1":
        _fail("preview schema is invalid")
    if value["status"] not in {"admissible_work", "all_terminal", "no_admissible_work"}:
        _fail("preview status is invalid")
    if value["retention"] != "metadata_only_goal_control_preview;tasks_prompts_parameters_credentials_and_results_not_retained" or value["secret_material"] != "never_returned":
        _fail("preview retention markers are invalid")
    _integer(value["eligible_goal_count"], name="preview eligible_goal_count", minimum=0, maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS)
    _counts(value["decision_counts"], name="preview decision_counts")
    _counts(value["reason_counts"], name="preview reason_counts")
    _counts(value["status_counts"], name="preview status_counts")
    blocked = [_identifier(item, name="preview dependency_blocked_goal_id") for item in _sequence(value["dependency_blocked_goal_ids"], name="preview dependency_blocked_goal_ids", maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS)]
    learning = _digest(value["learning_state_digest"], name="preview learning_state_digest", allow_none=True)
    schedule = value["schedule"]
    if not isinstance(schedule, Mapping):
        _fail("preview schedule is malformed")
    _digest(schedule.get("schedule_digest"), name="preview schedule_digest")
    selected = [_identifier(item, name="preview selected_goal_id") for item in _sequence(schedule.get("selected_goal_ids"), name="preview selected_goal_ids", maximum=128)]
    coverage = schedule.get("coverage")
    if not isinstance(coverage, Mapping):
        _fail("preview schedule coverage is malformed")
    for field in ("required_domains", "selected_domains", "missing_domains"):
        [_identifier(item, name=f"preview coverage {field}", maximum=128) for item in _sequence(coverage.get(field), name=f"preview coverage {field}", maximum=128)]
    normalized = {
        "schema": value["schema"],
        "schedule": _clone(schedule),
        "status": value["status"],
        "eligible_goal_count": value["eligible_goal_count"],
        "decision_counts": _counts(value["decision_counts"], name="preview decision_counts"),
        "reason_counts": _counts(value["reason_counts"], name="preview reason_counts"),
        "status_counts": _counts(value["status_counts"], name="preview status_counts"),
        "dependency_blocked_goal_ids": sorted(set(blocked)),
        "learning_state_digest": learning,
        "retention": value["retention"],
        "secret_material": value["secret_material"],
    }
    supplied = _digest(value["preview_digest"], name="preview_digest")
    if supplied != content_digest(normalized):
        _fail("preview digest does not match its projection")
    return {**normalized, "preview_digest": supplied}


def _record_body(
    *,
    admission_id: Any,
    revision: Any,
    status: str,
    decision: str,
    preview: Mapping[str, Any],
    requested_by_digest: Any,
    reviewer_digest: Any,
    issued_at_ns: Any,
    expires_at_ns: Any,
    reason_digest: Any,
    previous_record_digest: Any,
) -> dict[str, Any]:
    issued = _integer(issued_at_ns, name="issued_at_ns", minimum=0, maximum=2**63 - 1)
    expires = _integer(expires_at_ns, name="expires_at_ns", minimum=1, maximum=2**63 - 1)
    if expires <= issued or expires - issued > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_TTL_NS:
        _fail("approval expiry is outside its bounded lifetime")
    requested = _digest(requested_by_digest, name="requested_by_digest", allow_none=True)
    reviewer = _digest(reviewer_digest, name="reviewer_digest", allow_none=True)
    if status == "pending_review" and reviewer is not None:
        _fail("pending review cannot contain a reviewer")
    if status in {"approved", "rejected"} and reviewer is None:
        _fail("reviewed approval records require a reviewer digest")
    return {
        "schema": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA,
        "admission_id": _identifier(admission_id, name="admission_id"),
        "revision": _integer(revision, name="revision", minimum=1, maximum=2**31 - 1),
        "status": status,
        "decision": decision,
        "preview": _clone(preview),
        "preview_digest": _digest(preview["preview_digest"], name="preview_digest"),
        "requested_by_digest": requested,
        "reviewer_digest": reviewer,
        "issued_at_ns": issued,
        "expires_at_ns": expires,
        "reason_digest": _digest(reason_digest, name="reason_digest", allow_none=True),
        "previous_record_digest": _digest(previous_record_digest, name="previous_record_digest", allow_none=True),
        "authority": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY,
        "retention": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
        "execution": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION,
        "secret_material": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
    }


def validate_autonomous_goal_preview_admission_record(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or set(value) != _RECORD_KEYS | {"record_digest"}:
        _fail("record contains unsupported or missing fields")
    _safe_metadata(value)
    if value["schema"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORD_SCHEMA or value["authority"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_AUTHORITY or value["retention"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION or value["execution"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_EXECUTION or value["secret_material"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL:
        _fail("record markers are invalid")
    preview = _preview(value["preview"])
    if value["preview_digest"] != preview["preview_digest"]:
        _fail("record preview digest does not match the preview")
    status = value["status"]
    decision = value["decision"]
    if status not in _STATUSES or decision not in _DECISIONS or (status == "pending_review" and decision != "submitted") or (status == "approved" and decision != "approved") or (status == "rejected" and decision != "rejected"):
        _fail("record status or decision is invalid")
    body = _record_body(
        admission_id=value["admission_id"], revision=value["revision"], status=status, decision=decision,
        preview=preview, requested_by_digest=value["requested_by_digest"], reviewer_digest=value["reviewer_digest"],
        issued_at_ns=value["issued_at_ns"], expires_at_ns=value["expires_at_ns"], reason_digest=value["reason_digest"],
        previous_record_digest=value["previous_record_digest"],
    )
    supplied = _digest(value["record_digest"], name="record_digest")
    if supplied != content_digest(body):
        _fail("record digest does not match metadata")
    return _clone({**body, "record_digest": supplied})


def create_autonomous_goal_preview_admission_record(
    preview: Any,
    *,
    admission_id: str,
    issued_at_ns: int,
    expires_at_ns: int,
    requested_by_digest: str | None = None,
    reason: str | None = None,
    previous_record_digest: str | None = None,
) -> dict[str, Any]:
    normalized_preview = _preview(preview)
    reason_digest = None if reason is None else content_digest(_text(reason, name="reason", maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES))
    body = _record_body(
        admission_id=admission_id, revision=1, status="pending_review", decision="submitted", preview=normalized_preview,
        requested_by_digest=requested_by_digest, reviewer_digest=None, issued_at_ns=issued_at_ns, expires_at_ns=expires_at_ns,
        reason_digest=reason_digest, previous_record_digest=previous_record_digest,
    )
    return _clone({**body, "record_digest": content_digest(body)})


def review_autonomous_goal_preview_admission_record(
    record: Mapping[str, Any],
    *,
    approved: bool,
    reviewer_digest: str,
    reason: str | None = None,
    expected_record_digest: str | None = None,
) -> dict[str, Any]:
    current = validate_autonomous_goal_preview_admission_record(record)
    if current["status"] != "pending_review":
        _fail("only a pending preview admission can be reviewed")
    if not isinstance(approved, bool):
        _fail("approved must be boolean")
    if expected_record_digest is not None and expected_record_digest != current["record_digest"]:
        _fail("review expected_record_digest does not match the current record")
    reason_digest = None if reason is None else content_digest(_text(reason, name="reason", maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_REASON_BYTES))
    decision = "approved" if approved else "rejected"
    body = _record_body(
        admission_id=current["admission_id"], revision=current["revision"] + 1, status=decision, decision=decision,
        preview=current["preview"], requested_by_digest=current["requested_by_digest"], reviewer_digest=reviewer_digest,
        issued_at_ns=current["issued_at_ns"], expires_at_ns=current["expires_at_ns"], reason_digest=reason_digest,
        previous_record_digest=current["record_digest"],
    )
    return _clone({**body, "record_digest": content_digest(body)})


def verify_autonomous_goal_preview_approval(
    record: Mapping[str, Any],
    *,
    current_preview_digest: str,
    now_ns: int,
    reviewer_digest: str | None = None,
) -> dict[str, Any]:
    normalized = validate_autonomous_goal_preview_admission_record(record)
    if normalized["status"] != "approved":
        _fail("preview admission is not approved")
    _digest(current_preview_digest, name="current_preview_digest")
    _integer(now_ns, name="now_ns", minimum=0, maximum=2**63 - 1)
    if now_ns >= normalized["expires_at_ns"]:
        _fail("preview admission has expired")
    if normalized["preview_digest"] != current_preview_digest:
        _fail("preview admission does not match the current preview")
    if reviewer_digest is not None and normalized["reviewer_digest"] != _digest(reviewer_digest, name="reviewer_digest"):
        _fail("preview admission reviewer does not match")
    return normalized


def validate_autonomous_goal_preview_admission_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or set(value) != _SNAPSHOT_KEYS | {"snapshot_digest"}:
        _fail("snapshot contains unsupported or missing fields")
    _safe_metadata(value)
    if value["schema"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA or value["retention"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION or value["secret_material"] != AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL:
        _fail("snapshot markers are invalid")
    generation = _integer(value["generation"], name="snapshot generation", minimum=0, maximum=2**31 - 1)
    raw_records = value["records"]
    if not isinstance(raw_records, list) or len(raw_records) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS:
        _fail("snapshot records exceed their bound")
    records = [validate_autonomous_goal_preview_admission_record(item) for item in raw_records]
    ids = [record["admission_id"] for record in records]
    if len(ids) != len(set(ids)):
        _fail("snapshot contains duplicate admission ids")
    body = {
        "schema": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "records": sorted(records, key=lambda record: record["admission_id"]),
        "previous_snapshot_digest": _digest(value["previous_snapshot_digest"], name="snapshot previous_snapshot_digest", allow_none=True),
        "retention": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
        "secret_material": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
    }
    supplied = _digest(value["snapshot_digest"], name="snapshot_digest")
    if supplied != content_digest(body):
        _fail("snapshot digest does not match metadata")
    result = _clone({**body, "snapshot_digest": supplied})
    if len(canonical_json(result).encode("utf-8")) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return result


def seal_autonomous_goal_preview_admission_snapshot(
    *,
    generation: int,
    records: Sequence[Mapping[str, Any]],
    previous_snapshot_digest: str | None = None,
) -> dict[str, Any]:
    generation = _integer(generation, name="snapshot generation", minimum=0, maximum=2**31 - 1)
    normalized_records = [validate_autonomous_goal_preview_admission_record(item) for item in records]
    if len(normalized_records) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS:
        _fail("snapshot records exceed their bound")
    body = {
        "schema": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_SCHEMA,
        "generation": generation,
        "records": sorted(normalized_records, key=lambda record: record["admission_id"]),
        "previous_snapshot_digest": _digest(previous_snapshot_digest, name="snapshot previous_snapshot_digest", allow_none=True),
        "retention": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RETENTION,
        "secret_material": AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SECRET_MATERIAL,
    }
    result = _clone({**body, "snapshot_digest": content_digest(body)})
    if len(canonical_json(result).encode("utf-8")) > MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES:
        _fail("snapshot exceeds its byte bound")
    return result


class InMemoryAutonomousGoalPreviewAdmissionLedger:
    """Bounded revision-fenced preview decisions for one caller-owned process."""

    def __init__(self, *, max_records: int = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS) -> None:
        self.max_records = _integer(max_records, name="max_records", minimum=1, maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_RECORDS)
        self._records: dict[str, dict[str, Any]] = {}

    def put(self, record: Mapping[str, Any]) -> dict[str, Any]:
        normalized = validate_autonomous_goal_preview_admission_record(record)
        admission_id = normalized["admission_id"]
        existing = self._records.get(admission_id)
        if existing is not None and existing["record_digest"] not in {normalized["previous_record_digest"], normalized["record_digest"]}:
            _fail("record predecessor conflicts with the current admission")
        if existing is None and normalized["revision"] != 1:
            _fail("new preview admissions must begin at revision one")
        if existing is not None and normalized["revision"] != existing["revision"] + 1 and normalized["record_digest"] != existing["record_digest"]:
            _fail("preview admission revision is not contiguous")
        if existing is None and len(self._records) >= self.max_records:
            _fail("ledger capacity is exhausted")
        self._records[admission_id] = normalized
        return _clone(normalized)

    def submit(self, preview: Any, **kwargs: Any) -> dict[str, Any]:
        return self.put(create_autonomous_goal_preview_admission_record(preview, **kwargs))

    def review(self, admission_id: str, **kwargs: Any) -> dict[str, Any]:
        current = self.get(admission_id)
        if current is None:
            _fail("cannot review an unknown preview admission")
        return self.put(review_autonomous_goal_preview_admission_record(current, **kwargs))

    def get(self, admission_id: str) -> dict[str, Any] | None:
        value = self._records.get(_identifier(admission_id, name="admission_id"))
        return None if value is None else _clone(value)

    def list(self) -> list[dict[str, Any]]:
        return [_clone(self._records[key]) for key in sorted(self._records)]

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_goal_preview_admission_snapshot(snapshot)
        if len(normalized["records"]) > self.max_records:
            _fail("snapshot exceeds ledger capacity")
        self._records = {record["admission_id"]: record for record in normalized["records"]}


class AutonomousGoalPreviewAdmissionSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalAutonomousGoalPreviewAdmissionSnapshotTextStore(AutonomousGoalPreviewAdmissionSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousGoalPreviewAdmissionSnapshotPersistence:
    def __init__(self, store: AutonomousGoalPreviewAdmissionSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _integer(max_bytes, name="max_bytes", minimum=1, maximum=MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("stored JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousGoalError("autonomous goal preview admission stored JSON is invalid") from error
        normalized = validate_autonomous_goal_preview_admission_snapshot(raw)
        if canonical_json(normalized) != encoded:
            _fail("stored JSON is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_goal_preview_admission_snapshot(snapshot)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("snapshot exceeds the configured byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence(JsonAutonomousGoalPreviewAdmissionSnapshotPersistence):
    def __init__(self, store: TransactionalAutonomousGoalPreviewAdmissionSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMOUS_GOAL_PREVIEW_ADMISSION_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional JSON persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest(expected_snapshot_digest, name="expected_snapshot_digest", allow_none=True)
        normalized = validate_autonomous_goal_preview_admission_snapshot(snapshot)
        return self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized))


class AutonomousGoalPreviewAdmissionPersistenceCoordinator:
    def __init__(self, ledger: InMemoryAutonomousGoalPreviewAdmissionLedger, persistence: Any) -> None:
        if not isinstance(ledger, InMemoryAutonomousGoalPreviewAdmissionLedger):
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
        normalized = validate_autonomous_goal_preview_admission_snapshot(snapshot)
        self.ledger.restore(normalized)
        self.expected_snapshot_digest = normalized["snapshot_digest"]
        self.expected_generation = normalized["generation"]
        return normalized

    def flush(self) -> dict[str, Any]:
        snapshot = seal_autonomous_goal_preview_admission_snapshot(
            generation=self.expected_generation + 1,
            records=self.ledger.list(),
            previous_snapshot_digest=self.expected_snapshot_digest,
        )
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        self.expected_generation = snapshot["generation"]
        return snapshot
