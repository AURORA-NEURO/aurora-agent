"""Crash-safe, metadata-only effect execution for autonomous tool calls.

An approved tool call is not the same thing as a safely completed external effect.  A process
can disappear after the remote system accepts a request but before the SDK receives a response.
This module makes that boundary explicit for the Python SDK: it writes a hash-chained
``dispatched`` marker before entering caller code, converts ambiguous failures to
``reconciliation_required``, and requires a caller-owned resolver before a retry can occur.

The journal intentionally stores only identifiers, digests, counters, bounded failure labels, and
retention markers.  Arguments, outputs, prompts, tasks, credentials, and provider envelopes are
never serialized by this boundary.  Exactly-once delivery is not claimed; the caller's external
system must honor the supplied idempotency key.
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
from .errors import ArgumentError
from .llm_runtime import ProviderToolCall, ProviderToolResult


AUTONOMOUS_EFFECT_SCHEMA = "bioprism-python-autonomous-effect/0.1"
AUTONOMOUS_EFFECT_EVENT_SCHEMA = "bioprism-python-autonomous-effect-event/0.1"
AUTONOMOUS_EFFECT_JOURNAL_SCHEMA = "bioprism-python-autonomous-effect-journal/0.1"
AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-effect-snapshot/0.1"
AUTONOMOUS_EFFECT_STATUSES = (
    "prepared",
    "dispatching",
    "dispatched",
    "completed",
    "uncertain",
    "reconciled",
    "failed",
)
MAX_AUTONOMOUS_EFFECT_EVENTS = 32_768
MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES = 64_000_000
MAX_AUTONOMOUS_EFFECT_EVENT_BYTES = 64_000
MAX_AUTONOMOUS_EFFECT_ARGUMENT_BYTES = 2_000_000
MAX_AUTONOMOUS_EFFECT_REASON_BYTES = 2_048
EFFECT_RETENTION = "metadata_only_no_arguments_outputs_credentials_or_provider_material"
EFFECT_SNAPSHOT_RETENTION = "metadata_only_hash_chained"


class AutonomousEffectError(ArgumentError):
    """The effect boundary rejected malformed or unsafe metadata."""


class AutonomousEffectPolicyError(AutonomousEffectError):
    """An effect identity or caller policy contract was violated."""


class AutonomousEffectReconciliationRequiredError(AutonomousEffectError):
    """A prior dispatch may have reached the external system."""

    def __init__(self, effect_id: str, idempotency_key: str, status: str) -> None:
        super().__init__(
            f"effect {effect_id} is {status}; caller-owned reconciliation is required before retry"
        )
        self.effect_id = effect_id
        self.idempotency_key = idempotency_key
        self.status = status


class AutonomousEffectExecutionError(AutonomousEffectError):
    """A definite external failure was recorded; retry requires a new decision."""

    def __init__(self, effect_id: str, failure_class: str) -> None:
        super().__init__(f"effect {effect_id} failed at the external boundary ({failure_class})")
        self.effect_id = effect_id
        self.failure_class = failure_class


def _clone(value: Any) -> Any:
    try:
        return json.loads(canonical_json(value))
    except (TypeError, ValueError) as error:
        raise AutonomousEffectError("effect value must be JSON-safe") from error


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomousEffectError(f"{name} must be bounded text")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomousEffectError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any, maximum: int = 512) -> str:
    text = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in text):
        raise AutonomousEffectError(f"{name} must be a bounded identifier")
    return text


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise AutonomousEffectError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise AutonomousEffectError(f"{name} must be an integer within [0, {maximum}]")
    return value


_FORBIDDEN_METADATA_FIELDS = {
    "apikey", "authorization", "bearer", "credential", "password", "secret",
    "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response",
    "rawpayload", "arguments", "output", "task", "messages",
}


def _assert_metadata(value: Any, *, name: str, maximum: int, depth: int = 0) -> None:
    if depth > 24:
        raise AutonomousEffectError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        pass
    elif isinstance(value, float):
        if not math.isfinite(value):
            raise AutonomousEffectError(f"{name} contains a non-finite number")
    elif isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                raise AutonomousEffectError(f"{name} contains a non-string key")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _FORBIDDEN_METADATA_FIELDS:
                raise AutonomousEffectError(f"{name} contains transient or secret-shaped fields")
            _assert_metadata(child, name=name, maximum=maximum, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _assert_metadata(child, name=name, maximum=maximum, depth=depth + 1)
    else:
        raise AutonomousEffectError(f"{name} is not JSON-safe")
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise AutonomousEffectError(f"{name} is not JSON-serializable") from error
    if len(encoded) > maximum:
        raise AutonomousEffectError(f"{name} exceeds its bounded byte size")


def _status(value: Any) -> str:
    if value not in AUTONOMOUS_EFFECT_STATUSES:
        raise AutonomousEffectError("effect status is unsupported")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousEffectRequest:
    tool: str
    call_id: str
    risk_class: str
    arguments: Mapping[str, Any]
    execution_id: str | None = None


@dataclass(frozen=True, slots=True)
class AutonomousEffectExecutionContext:
    effect_id: str
    execution_id: str | None
    tool: str
    call_id: str
    risk_class: str
    idempotency_key: str
    dispatch_attempt: int


@dataclass(frozen=True, slots=True)
class AutonomousEffectRecord:
    schema: str
    effect_id: str
    execution_id: str | None
    tool: str
    call_id: str
    risk_class: str
    arguments_digest: str
    idempotency_key_digest: str
    status: str
    dispatch_attempt: int
    result_digest: str | None
    failure_class: str | None
    reason: str | None
    last_sequence: int
    last_event_digest: str
    retention: str = EFFECT_RETENTION

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema, "effect_id": self.effect_id, "execution_id": self.execution_id,
            "tool": self.tool, "call_id": self.call_id, "risk_class": self.risk_class,
            "arguments_digest": self.arguments_digest, "idempotency_key_digest": self.idempotency_key_digest,
            "status": self.status, "dispatch_attempt": self.dispatch_attempt,
            "result_digest": self.result_digest, "failure_class": self.failure_class,
            "reason": self.reason, "last_sequence": self.last_sequence,
            "last_event_digest": self.last_event_digest, "retention": self.retention,
        }


@dataclass(frozen=True, slots=True)
class AutonomousEffectEvent:
    schema: str
    effect_id: str
    execution_id: str | None
    tool: str
    call_id: str
    risk_class: str
    arguments_digest: str
    idempotency_key_digest: str
    status: str
    dispatch_attempt: int
    result_digest: str | None = None
    failure_class: str | None = None
    reason: str | None = None
    metadata: Mapping[str, Any] | None = None
    retention: str = EFFECT_RETENTION

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": self.schema, "effect_id": self.effect_id, "execution_id": self.execution_id,
            "tool": self.tool, "call_id": self.call_id, "risk_class": self.risk_class,
            "arguments_digest": self.arguments_digest, "idempotency_key_digest": self.idempotency_key_digest,
            "status": self.status, "dispatch_attempt": self.dispatch_attempt,
            "result_digest": self.result_digest, "failure_class": self.failure_class,
            "reason": self.reason,
        }
        if self.metadata is not None:
            result["metadata"] = _clone(self.metadata)
        result["retention"] = self.retention
        return result


@dataclass(frozen=True, slots=True)
class AutonomousEffectJournalRow:
    schema: str
    sequence: int
    event: AutonomousEffectEvent
    previous_digest: str
    created_at: int
    event_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema, "sequence": self.sequence, "event": self.event.to_dict(),
            "previous_digest": self.previous_digest, "created_at": self.created_at,
            "event_digest": self.event_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousEffectJournalReceipt:
    schema: str
    sequence: int
    event_digest: str
    head_digest: str
    effect_id: str
    status: str
    retention: str = EFFECT_SNAPSHOT_RETENTION

    def to_dict(self) -> dict[str, Any]:
        return self.__dict__ if hasattr(self, "__dict__") else {
            "schema": self.schema, "sequence": self.sequence, "event_digest": self.event_digest,
            "head_digest": self.head_digest, "effect_id": self.effect_id, "status": self.status,
            "retention": self.retention,
        }


@dataclass(frozen=True, slots=True)
class AutonomousEffectJournalSnapshot:
    schema: str
    rows: tuple[AutonomousEffectJournalRow, ...]
    head_digest: str
    retention: str
    secret_material: str
    snapshot_digest: str

    def to_dict(self, *, include_digest: bool = True) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": self.schema,
            "rows": [row.to_dict() for row in self.rows],
            "head_digest": self.head_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        if include_digest:
            result["snapshot_digest"] = self.snapshot_digest
        return result


class AutonomousEffectJournal(Protocol):
    def append(self, event: Mapping[str, Any] | AutonomousEffectEvent) -> AutonomousEffectJournalReceipt: ...
    def get(self, effect_id: str) -> AutonomousEffectRecord | None: ...
    def events(self, *, effect_id: str | None = None, after_sequence: int = 0, limit: int = 256) -> tuple[AutonomousEffectJournalRow, ...]: ...
    def verify_integrity(self) -> Mapping[str, Any]: ...


class AutonomousEffectSnapshotJournal(AutonomousEffectJournal, Protocol):
    def snapshot(self) -> AutonomousEffectJournalSnapshot: ...
    def restore(self, snapshot: Mapping[str, Any] | AutonomousEffectJournalSnapshot) -> None: ...


class AutonomousEffectSnapshotPersistence(Protocol):
    def read(self) -> AutonomousEffectJournalSnapshot | Mapping[str, Any] | None: ...
    def write(self, snapshot: AutonomousEffectJournalSnapshot | Mapping[str, Any]) -> None: ...


class AutonomousEffectTransactionalSnapshotPersistence(AutonomousEffectSnapshotPersistence, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: AutonomousEffectJournalSnapshot | Mapping[str, Any]) -> bool: ...


def _event_from_raw(raw: Mapping[str, Any]) -> AutonomousEffectEvent:
    if not isinstance(raw, Mapping):
        raise AutonomousEffectError("effect event must be an object")
    allowed = {"schema", "effect_id", "execution_id", "tool", "call_id", "risk_class", "arguments_digest", "idempotency_key_digest", "status", "dispatch_attempt", "result_digest", "failure_class", "reason", "metadata", "retention"}
    if set(raw).difference(allowed):
        raise AutonomousEffectError("effect event contains unsupported fields")
    if raw.get("schema") != AUTONOMOUS_EFFECT_EVENT_SCHEMA:
        raise AutonomousEffectError("effect event schema is unsupported")
    execution_id = raw.get("execution_id")
    if execution_id is not None:
        execution_id = _identifier("effect execution_id", execution_id, 256)
    result_digest = raw.get("result_digest")
    if result_digest is not None:
        result_digest = _digest("effect result_digest", result_digest)
    failure_class = raw.get("failure_class")
    if failure_class is not None:
        failure_class = _identifier("effect failure_class", failure_class, 256)
    reason = raw.get("reason")
    if reason is not None:
        reason = _text("effect reason", reason, MAX_AUTONOMOUS_EFFECT_REASON_BYTES)
    metadata = raw.get("metadata")
    if metadata is not None:
        _assert_metadata(metadata, name="effect metadata", maximum=8_192)
    event = AutonomousEffectEvent(
        schema=AUTONOMOUS_EFFECT_EVENT_SCHEMA,
        effect_id=_identifier("effect_id", raw.get("effect_id"), 128),
        execution_id=execution_id,
        tool=_identifier("effect tool", raw.get("tool")),
        call_id=_identifier("effect call_id", raw.get("call_id")),
        risk_class=_identifier("effect risk_class", raw.get("risk_class"), 256),
        arguments_digest=_digest("effect arguments_digest", raw.get("arguments_digest")),
        idempotency_key_digest=_digest("effect idempotency_key_digest", raw.get("idempotency_key_digest")),
        status=_status(raw.get("status")),
        dispatch_attempt=_integer("effect dispatch_attempt", raw.get("dispatch_attempt"), 64),
        result_digest=result_digest,
        failure_class=failure_class,
        reason=reason,
        metadata=None if metadata is None else _clone(metadata),
        retention=raw.get("retention"),
    )
    if event.retention != EFFECT_RETENTION:
        raise AutonomousEffectError("effect event retention declaration is invalid")
    _assert_metadata(event.to_dict(), name="effect event", maximum=MAX_AUTONOMOUS_EFFECT_EVENT_BYTES)
    return event


def _event_from_value(value: Mapping[str, Any] | AutonomousEffectEvent) -> AutonomousEffectEvent:
    return value if isinstance(value, AutonomousEffectEvent) else _event_from_raw(value)


def _row_from_raw(raw: Mapping[str, Any], expected_sequence: int, previous: str) -> AutonomousEffectJournalRow:
    if not isinstance(raw, Mapping) or set(raw) != {"schema", "sequence", "event", "previous_digest", "created_at", "event_digest"}:
        raise AutonomousEffectError("effect journal row is malformed")
    if raw.get("schema") != AUTONOMOUS_EFFECT_EVENT_SCHEMA or raw.get("sequence") != expected_sequence or raw.get("previous_digest") != previous:
        raise AutonomousEffectError("effect journal hash chain sequence is invalid")
    created_at = raw.get("created_at")
    if isinstance(created_at, bool) or not isinstance(created_at, int) or created_at < 0:
        raise AutonomousEffectError("effect journal timestamp is invalid")
    event_digest = _digest("effect journal event_digest", raw.get("event_digest"))
    event = _event_from_raw(raw.get("event"))
    descriptor = {"schema": AUTONOMOUS_EFFECT_EVENT_SCHEMA, "sequence": expected_sequence, "event": event.to_dict(), "previous_digest": previous, "created_at": created_at}
    if content_digest(descriptor) != event_digest:
        raise AutonomousEffectError("effect journal event digest is invalid")
    return AutonomousEffectJournalRow(AUTONOMOUS_EFFECT_EVENT_SCHEMA, expected_sequence, event, previous, created_at, event_digest)


def validate_autonomous_effect_journal_snapshot(value: Mapping[str, Any] | AutonomousEffectJournalSnapshot) -> AutonomousEffectJournalSnapshot:
    raw = value.to_dict() if isinstance(value, AutonomousEffectJournalSnapshot) else value
    if not isinstance(raw, Mapping) or set(raw) != {"schema", "rows", "head_digest", "retention", "secret_material", "snapshot_digest"}:
        raise AutonomousEffectError("effect journal snapshot is malformed")
    if raw.get("schema") != AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA or raw.get("retention") != EFFECT_SNAPSHOT_RETENTION or raw.get("secret_material") != "never_returned":
        raise AutonomousEffectError("effect journal snapshot markers are invalid")
    rows_raw = raw.get("rows")
    if not isinstance(rows_raw, Sequence) or isinstance(rows_raw, (str, bytes, bytearray)) or len(rows_raw) > MAX_AUTONOMOUS_EFFECT_EVENTS:
        raise AutonomousEffectError("effect journal snapshot rows exceed their bound")
    previous = ""
    rows: list[AutonomousEffectJournalRow] = []
    total_bytes = 0
    for sequence, row_raw in enumerate(rows_raw, start=1):
        row = _row_from_raw(row_raw, sequence, previous)
        rows.append(row)
        total_bytes += len(canonical_json(row.to_dict()).encode("utf-8"))
        if total_bytes > MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES:
            raise AutonomousEffectError("effect journal snapshot exceeds its byte bound")
        previous = row.event_digest
    head = raw.get("head_digest")
    if head != previous:
        raise AutonomousEffectError("effect journal snapshot head does not match its rows")
    if head:
        _digest("effect snapshot head_digest", head)
    snapshot_digest = _digest("effect snapshot snapshot_digest", raw.get("snapshot_digest"))
    descriptor = {"schema": AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA, "rows": [row.to_dict() for row in rows], "head_digest": head, "retention": EFFECT_SNAPSHOT_RETENTION, "secret_material": "never_returned"}
    if content_digest(descriptor) != snapshot_digest:
        raise AutonomousEffectError("effect journal snapshot digest does not match")
    normalized = AutonomousEffectJournalSnapshot(AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA, tuple(rows), head, EFFECT_SNAPSHOT_RETENTION, "never_returned", snapshot_digest)
    if len(canonical_json(normalized.to_dict()).encode("utf-8")) > MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES:
        raise AutonomousEffectError("effect journal snapshot exceeds its byte bound")
    return normalized


def _record_from_row(row: AutonomousEffectJournalRow) -> AutonomousEffectRecord:
    event = row.event
    return AutonomousEffectRecord(
        AUTONOMOUS_EFFECT_SCHEMA, event.effect_id, event.execution_id, event.tool, event.call_id,
        event.risk_class, event.arguments_digest, event.idempotency_key_digest, event.status,
        event.dispatch_attempt, event.result_digest, event.failure_class, event.reason,
        row.sequence, row.event_digest,
    )


class InMemoryAutonomousEffectJournal:
    """Thread-safe hash-chained effect journal for tests and local workers."""

    def __init__(self, *, max_events: int = MAX_AUTONOMOUS_EFFECT_EVENTS, max_bytes: int = MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES, clock: Callable[[], float] | None = None) -> None:
        if isinstance(max_events, bool) or not isinstance(max_events, int) or not 1 <= max_events <= MAX_AUTONOMOUS_EFFECT_EVENTS:
            raise AutonomousEffectError("effect journal max_events is outside its bounds")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not MAX_AUTONOMOUS_EFFECT_EVENT_BYTES <= max_bytes <= MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES:
            raise AutonomousEffectError("effect journal max_bytes is outside its bounds")
        self.max_events = max_events
        self.max_bytes = max_bytes
        self.clock = clock or time.time
        self._rows: list[AutonomousEffectJournalRow] = []
        self._total_bytes = 0
        self._lock = threading.RLock()

    def append(self, event: Mapping[str, Any] | AutonomousEffectEvent) -> AutonomousEffectJournalReceipt:
        normalized = _event_from_value(event)
        with self._lock:
            if len(self._rows) >= self.max_events:
                raise AutonomousEffectError("effect journal event capacity is exhausted")
            observed = self.clock()
            if isinstance(observed, bool) or not isinstance(observed, (int, float)) or not math.isfinite(float(observed)) or observed < 0:
                raise AutonomousEffectError("effect journal clock returned an invalid timestamp")
            sequence = len(self._rows) + 1
            previous = self._rows[-1].event_digest if self._rows else ""
            descriptor = {"schema": AUTONOMOUS_EFFECT_EVENT_SCHEMA, "sequence": sequence, "event": normalized.to_dict(), "previous_digest": previous, "created_at": int(observed)}
            digest = content_digest(descriptor)
            row = AutonomousEffectJournalRow(AUTONOMOUS_EFFECT_EVENT_SCHEMA, sequence, normalized, previous, int(observed), digest)
            size = len(canonical_json(row.to_dict()).encode("utf-8"))
            if self._total_bytes + size > self.max_bytes:
                raise AutonomousEffectError("effect journal byte capacity is exhausted")
            self._rows.append(row)
            self._total_bytes += size
            return AutonomousEffectJournalReceipt(AUTONOMOUS_EFFECT_JOURNAL_SCHEMA, sequence, digest, digest, normalized.effect_id, normalized.status)

    def get(self, effect_id: str) -> AutonomousEffectRecord | None:
        effect_id = _identifier("effect_id", effect_id, 128)
        with self._lock:
            for row in reversed(self._rows):
                if row.event.effect_id == effect_id:
                    return _record_from_row(row)
        return None

    def events(self, *, effect_id: str | None = None, after_sequence: int = 0, limit: int = 256) -> tuple[AutonomousEffectJournalRow, ...]:
        if effect_id is not None:
            effect_id = _identifier("effect_id", effect_id, 128)
        _integer("effect journal after_sequence", after_sequence, self.max_events)
        _integer("effect journal limit", limit, self.max_events)
        if limit < 1:
            raise AutonomousEffectError("effect journal limit must be positive")
        with self._lock:
            return tuple(row for row in self._rows if row.sequence > after_sequence and (effect_id is None or row.event.effect_id == effect_id))[:limit]

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.snapshot()
        return {"schema": AUTONOMOUS_EFFECT_JOURNAL_SCHEMA, "verified": True, "events": len(snapshot.rows), "head_digest": snapshot.head_digest, "retention": "metadata_only"}

    def snapshot(self) -> AutonomousEffectJournalSnapshot:
        with self._lock:
            previous = self._rows[-1].event_digest if self._rows else ""
            descriptor = {"schema": AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA, "rows": [row.to_dict() for row in self._rows], "head_digest": previous, "retention": EFFECT_SNAPSHOT_RETENTION, "secret_material": "never_returned"}
            return AutonomousEffectJournalSnapshot(AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA, tuple(self._rows), previous, EFFECT_SNAPSHOT_RETENTION, "never_returned", content_digest(descriptor))

    def restore(self, snapshot: Mapping[str, Any] | AutonomousEffectJournalSnapshot) -> None:
        normalized = validate_autonomous_effect_journal_snapshot(snapshot)
        if len(normalized.rows) > self.max_events:
            raise AutonomousEffectError("effect journal restore exceeds max_events")
        total = sum(len(canonical_json(row.to_dict()).encode("utf-8")) for row in normalized.rows)
        if total > self.max_bytes:
            raise AutonomousEffectError("effect journal restore exceeds max_bytes")
        with self._lock:
            self._rows = list(normalized.rows)
            self._total_bytes = total


class InMemoryAutonomousEffectSnapshotTextStore:
    """Minimal caller-owned text store used by persistence and contract tests."""

    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value).get("snapshot_digest")
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


class JsonAutonomousEffectSnapshotPersistence:
    def __init__(self, store: Any) -> None:
        if not all(callable(getattr(store, method, None)) for method in ("read", "write")):
            raise AutonomousEffectError("effect JSON persistence store is malformed")
        self.store = store

    def read(self) -> AutonomousEffectJournalSnapshot | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES:
            raise AutonomousEffectError("effect JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except json.JSONDecodeError as error:
            raise AutonomousEffectError("effect JSON is invalid") from error
        snapshot = validate_autonomous_effect_journal_snapshot(raw)
        if canonical_json(snapshot.to_dict()) != encoded:
            raise AutonomousEffectError("effect JSON is not canonical")
        return snapshot

    def write(self, snapshot: Mapping[str, Any] | AutonomousEffectJournalSnapshot) -> None:
        normalized = validate_autonomous_effect_journal_snapshot(snapshot)
        self.store.write(canonical_json(normalized.to_dict()))


class TransactionalJsonAutonomousEffectSnapshotPersistence(JsonAutonomousEffectSnapshotPersistence):
    def __init__(self, store: Any) -> None:
        super().__init__(store)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise AutonomousEffectError("effect JSON persistence store lacks compare-and-set")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any] | AutonomousEffectJournalSnapshot) -> bool:
        if expected_snapshot_digest is not None:
            _digest("effect expected snapshot digest", expected_snapshot_digest)
        normalized = validate_autonomous_effect_journal_snapshot(snapshot)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized.to_dict())))


class AutonomousEffectPersistenceCoordinator:
    def __init__(self, journal: AutonomousEffectSnapshotJournal, persistence: Any) -> None:
        if not all(callable(getattr(journal, method, None)) for method in ("snapshot", "restore", "append")):
            raise AutonomousEffectError("effect persistence requires a snapshot-capable journal")
        if not all(callable(getattr(persistence, method, None)) for method in ("read", "write")):
            raise AutonomousEffectError("effect persistence adapter is malformed")
        self.journal = journal
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> AutonomousEffectJournalSnapshot | None:
        with self._lock:
            raw = self.persistence.read()
            if raw is None:
                self._expected_snapshot_digest = None
                return None
            snapshot = validate_autonomous_effect_journal_snapshot(raw)
            self.journal.restore(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot

    def flush(self) -> AutonomousEffectJournalSnapshot:
        with self._lock:
            snapshot = validate_autonomous_effect_journal_snapshot(self.journal.snapshot())
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise AutonomousEffectError("effect persistence compare-and-set conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousEffectResolution:
    status: str
    result: Any = None
    failure_class: str | None = None
    reason: str | None = None
    retry_safe: bool = False


class AutonomousEffectResolver(Protocol):
    def resolve(self, record: AutonomousEffectRecord) -> AutonomousEffectResolution | Mapping[str, Any]: ...


def _failure_class(error: BaseException) -> str:
    name = type(error).__name__
    return name if 1 <= len(name) <= 256 and all(character in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in name) else "effect_execution_error"


def _failure_reason(error: BaseException) -> str:
    name = type(error).__name__
    return name if 1 <= len(name) <= 256 and all(character in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in name) else "effect_execution_error"


class AutonomousEffectBoundary:
    """At-least-once effect protocol with explicit reconciliation before uncertain retry."""

    def __init__(self, *, journal: AutonomousEffectSnapshotJournal | None = None, resolver: AutonomousEffectResolver | None = None, execution: Any | None = None, clock: Callable[[], float] | None = None) -> None:
        self.journal = journal or InMemoryAutonomousEffectJournal()
        if not all(callable(getattr(self.journal, method, None)) for method in ("append", "get")):
            raise AutonomousEffectError("effect boundary journal is malformed")
        if resolver is not None and not callable(getattr(resolver, "resolve", None)):
            raise AutonomousEffectError("effect boundary resolver is malformed")
        self.resolver = resolver
        self.execution = execution
        self.clock = clock or time.time
        self._result_cache: dict[str, Any] = {}
        self._locks: dict[str, threading.Lock] = {}
        self._locks_guard = threading.RLock()

    def normalize_request(self, request: AutonomousEffectRequest | Mapping[str, Any]) -> AutonomousEffectRequest:
        raw = request if isinstance(request, Mapping) else request.__dict__ if hasattr(request, "__dict__") else {
            "execution_id": request.execution_id, "tool": request.tool, "call_id": request.call_id,
            "risk_class": request.risk_class, "arguments": request.arguments,
        }
        if not isinstance(raw, Mapping):
            raise AutonomousEffectError("effect request must be an object")
        execution_id = raw.get("execution_id")
        if execution_id is not None:
            execution_id = _identifier("effect execution_id", execution_id, 256)
        arguments = raw.get("arguments")
        if not isinstance(arguments, Mapping):
            raise AutonomousEffectError("effect arguments must be a JSON object")
        _assert_metadata(arguments, name="effect arguments", maximum=MAX_AUTONOMOUS_EFFECT_ARGUMENT_BYTES)
        return AutonomousEffectRequest(
            tool=_identifier("effect tool", raw.get("tool")),
            call_id=_identifier("effect call_id", raw.get("call_id")),
            risk_class=_identifier("effect risk_class", raw.get("risk_class"), 256),
            arguments=_clone(dict(arguments)),
            execution_id=execution_id,
        )

    def effect_id(self, request: AutonomousEffectRequest | Mapping[str, Any]) -> str:
        normalized = self.normalize_request(request)
        arguments_digest = content_digest(dict(normalized.arguments))
        return content_digest({"schema": AUTONOMOUS_EFFECT_SCHEMA, "execution_id": normalized.execution_id, "tool": normalized.tool, "call_id": normalized.call_id, "arguments_digest": arguments_digest})

    def execute(self, request: AutonomousEffectRequest | Mapping[str, Any], executor: Callable[[AutonomousEffectExecutionContext], Any], *, execution: Any | None = None) -> Any:
        if not callable(executor):
            raise AutonomousEffectError("effect executor must be callable")
        normalized = self.normalize_request(request)
        effect_id = self.effect_id(normalized)
        with self._exclusive(effect_id):
            return self._execute_exclusive(normalized, effect_id, executor, execution or self.execution)

    def reconcile(self, effect_id: str, resolver: AutonomousEffectResolver | None = None) -> AutonomousEffectRecord:
        effect_id = _identifier("effect_id", effect_id, 128)
        selected = resolver or self.resolver
        record = self.journal.get(effect_id)
        if record is None:
            raise AutonomousEffectError(f"effect {effect_id} is not present in the effect ledger")
        if selected is None or not callable(getattr(selected, "resolve", None)):
            raise AutonomousEffectReconciliationRequiredError(effect_id, self.idempotency_key(effect_id), record.status)
        with self._exclusive(effect_id):
            return self._reconcile_exclusive(record, selected, self.execution)

    def authorize_and_execute(self, calls: Sequence[ProviderToolCall], *, approve: Callable[[ProviderToolCall], bool], execute: Callable[..., Any], execution_id: str | None = None, execution: Any | None = None, is_read_only: Callable[[ProviderToolCall], bool] | None = None, risk_class: Callable[[ProviderToolCall], str] | None = None) -> tuple[ProviderToolResult, ...]:
        if isinstance(calls, (str, bytes)) or not isinstance(calls, Sequence) or len(calls) > 128:
            raise AutonomousEffectError("effect tool call count is outside its bounds")
        if not callable(approve) or not callable(execute):
            raise AutonomousEffectError("effect approval and executor callbacks must be callable")
        results: list[ProviderToolResult] = []
        for call in calls:
            if not isinstance(call, ProviderToolCall):
                raise AutonomousEffectError("effect calls contain an invalid provider tool call")
            try:
                approved = bool(approve(call))
            except Exception:
                approved = False
            if not approved:
                results.append(ProviderToolResult(call.call_id, {"status": "authorization_required", "tool": call.name, "secret_material": "never_returned"}, approved=False, is_error=True))
                continue
            readonly = bool(is_read_only(call)) if is_read_only is not None else False
            if readonly:
                results.append(ProviderToolResult(call.call_id, execute(call), approved=True, is_error=False))
                continue
            selected_risk = risk_class(call) if risk_class is not None else "external_effect"
            try:
                value = self.execute({"execution_id": execution_id, "tool": call.name, "call_id": call.call_id, "risk_class": selected_risk, "arguments": dict(call.arguments)}, lambda context: execute(call, context), execution=execution)
                results.append(ProviderToolResult(call.call_id, value, approved=True, is_error=False))
            except AutonomousEffectReconciliationRequiredError as error:
                results.append(ProviderToolResult(call.call_id, {"status": "reconciliation_required", "tool": call.name, "effect_id": error.effect_id, "idempotency_key": error.idempotency_key, "secret_material": "never_returned"}, approved=False, is_error=True))
        return tuple(results)

    def idempotency_key(self, effect_id: str) -> str:
        return f"aurora-effect-{effect_id}"

    def _execute_exclusive(self, request: AutonomousEffectRequest, effect_id: str, executor: Callable[[AutonomousEffectExecutionContext], Any], execution: Any | None) -> Any:
        record = self.journal.get(effect_id)
        arguments_digest = content_digest(dict(request.arguments))
        if record is not None:
            if record.tool != request.tool or record.call_id != request.call_id or record.arguments_digest != arguments_digest:
                raise AutonomousEffectPolicyError("effect id collides with different call metadata")
            if record.status in {"completed", "reconciled"}:
                if effect_id in self._result_cache:
                    return _clone(self._result_cache[effect_id])
                if self.resolver is None:
                    raise AutonomousEffectReconciliationRequiredError(effect_id, self.idempotency_key(effect_id), record.status)
                record = self._reconcile_exclusive(record, self.resolver, execution)
                if record.status in {"completed", "reconciled"} and effect_id in self._result_cache:
                    return _clone(self._result_cache[effect_id])
            if record.status in {"dispatching", "dispatched", "uncertain"}:
                if self.resolver is None:
                    raise AutonomousEffectReconciliationRequiredError(effect_id, self.idempotency_key(effect_id), record.status)
                record = self._reconcile_exclusive(record, self.resolver, execution)
                if record.status in {"completed", "reconciled"} and effect_id in self._result_cache:
                    return _clone(self._result_cache[effect_id])
                if record.status != "prepared":
                    raise AutonomousEffectReconciliationRequiredError(effect_id, self.idempotency_key(effect_id), record.status)
            if record.status == "failed":
                raise AutonomousEffectExecutionError(effect_id, record.failure_class or "previous_effect_failure")
        key_digest = content_digest(self.idempotency_key(effect_id))
        attempt = (record.dispatch_attempt if record is not None else 0) + 1
        base = {"execution_id": request.execution_id, "tool": request.tool, "call_id": request.call_id, "risk_class": request.risk_class, "arguments_digest": arguments_digest, "idempotency_key_digest": key_digest, "effect_id": effect_id, "dispatch_attempt": attempt}
        self._transition({**base, "status": "prepared", "reason": None}, execution)
        self._transition({**base, "status": "dispatching"}, execution)
        self._transition({**base, "status": "dispatched"}, execution)
        context = AutonomousEffectExecutionContext(effect_id, request.execution_id, request.tool, request.call_id, request.risk_class, self.idempotency_key(effect_id), attempt)
        try:
            result = executor(context)
            _assert_metadata(result, name="effect result", maximum=MAX_AUTONOMOUS_EFFECT_ARGUMENT_BYTES)
            result_digest = content_digest(result)
            self._result_cache[effect_id] = _clone(result)
            self._transition({**base, "status": "completed", "result_digest": result_digest}, execution)
            return _clone(result)
        except BaseException as error:
            self._transition({**base, "status": "uncertain", "failure_class": _failure_class(error), "reason": _failure_reason(error)}, execution)
            if isinstance(error, AutonomousEffectError):
                raise
            raise AutonomousEffectReconciliationRequiredError(effect_id, self.idempotency_key(effect_id), "uncertain") from error

    def _reconcile_exclusive(self, record: AutonomousEffectRecord, resolver: AutonomousEffectResolver, execution: Any | None = None) -> AutonomousEffectRecord:
        # The resolver receives an immutable, metadata-only record.  It can inspect the effect
        # identity and digests without gaining access to the original arguments or result.
        try:
            resolution_raw = resolver.resolve(record)
        except AutonomousEffectError:
            raise
        except Exception as error:
            raise AutonomousEffectError("effect resolver failed") from error
        if isinstance(resolution_raw, AutonomousEffectResolution):
            resolution = resolution_raw
        elif isinstance(resolution_raw, Mapping):
            resolution = AutonomousEffectResolution(str(resolution_raw.get("status")), resolution_raw.get("result"), resolution_raw.get("failure_class"), resolution_raw.get("reason"), resolution_raw.get("retry_safe") is True)
        else:
            raise AutonomousEffectError("effect resolver returned malformed resolution")
        base = {"execution_id": record.execution_id, "tool": record.tool, "call_id": record.call_id, "risk_class": record.risk_class, "arguments_digest": record.arguments_digest, "idempotency_key_digest": record.idempotency_key_digest, "effect_id": record.effect_id, "dispatch_attempt": record.dispatch_attempt}
        if resolution.status == "completed":
            if resolution.result is None:
                raise AutonomousEffectError("completed effect resolution must include a result")
            _assert_metadata(resolution.result, name="resolved effect result", maximum=MAX_AUTONOMOUS_EFFECT_ARGUMENT_BYTES)
            result_digest = content_digest(resolution.result)
            self._result_cache[record.effect_id] = _clone(resolution.result)
            self._transition({**base, "status": "reconciled", "result_digest": result_digest, "reason": "resolver_confirmed_completion"}, execution or self.execution)
            return self.journal.get(record.effect_id)  # type: ignore[return-value]
        if resolution.status == "failed":
            failure = resolution.failure_class if isinstance(resolution.failure_class, str) and resolution.failure_class else "resolved_effect_failure"
            reason = resolution.reason if isinstance(resolution.reason, str) and resolution.reason else "resolver_confirmed_failure"
            _identifier("resolved effect failure_class", failure, 256)
            _identifier("resolved effect reason", reason, 256)
            self._transition({**base, "status": "failed", "failure_class": failure, "reason": reason}, execution or self.execution)
            return self.journal.get(record.effect_id)  # type: ignore[return-value]
        if resolution.status == "not_found" and resolution.retry_safe:
            self._transition({**base, "status": "prepared", "reason": "resolver_confirmed_not_found_retry_safe"}, execution or self.execution)
            return self.journal.get(record.effect_id)  # type: ignore[return-value]
        if resolution.status not in {"unknown", "not_found"}:
            raise AutonomousEffectError("effect resolver returned an unsupported status")
        raise AutonomousEffectReconciliationRequiredError(record.effect_id, self.idempotency_key(record.effect_id), record.status)

    def _transition(self, value: Mapping[str, Any], execution: Any | None) -> None:
        event = {
            "schema": AUTONOMOUS_EFFECT_EVENT_SCHEMA,
            "effect_id": value["effect_id"], "execution_id": value.get("execution_id"), "tool": value["tool"],
            "call_id": value["call_id"], "risk_class": value["risk_class"], "arguments_digest": value["arguments_digest"],
            "idempotency_key_digest": value["idempotency_key_digest"], "status": value["status"],
            "dispatch_attempt": value["dispatch_attempt"], "retention": EFFECT_RETENTION,
        }
        for key in ("result_digest", "failure_class", "reason"):
            if key in value:
                event[key] = value[key]
        self.journal.append(event)
        controller = execution or self.execution
        if controller is not None:
            callback = getattr(controller, "record_effect_reconciliation", None)
            if callable(callback):
                callback(effect_id=value["effect_id"], tool=value["tool"], call_id=value["call_id"], status=value["status"], dispatch_attempt=value["dispatch_attempt"], result_digest=value.get("result_digest"), failure_class=value.get("failure_class"), reason=value.get("reason"))
            else:
                outcome = getattr(controller, "record_tool_outcome", None)
                if callable(outcome):
                    outcome(tool=value["tool"], call_id=value["call_id"], status=f"effect_{value['status']}", outcome_digest=value.get("result_digest"), reason=value.get("reason"))

    def _exclusive(self, effect_id: str):
        class _LockContext:
            def __init__(self, owner: "AutonomousEffectBoundary", key: str) -> None:
                self.owner, self.key = owner, key
                with owner._locks_guard:
                    self.lock = owner._locks.setdefault(key, threading.Lock())
            def __enter__(self) -> None:
                self.lock.acquire()
            def __exit__(self, *_args: Any) -> None:
                self.lock.release()
                with self.owner._locks_guard:
                    if not self.lock.locked():
                        self.owner._locks.pop(self.key, None)
        return _LockContext(self, effect_id)


__all__ = [
    "AUTONOMOUS_EFFECT_SCHEMA", "AUTONOMOUS_EFFECT_EVENT_SCHEMA", "AUTONOMOUS_EFFECT_JOURNAL_SCHEMA", "AUTONOMOUS_EFFECT_SNAPSHOT_SCHEMA", "AUTONOMOUS_EFFECT_STATUSES", "MAX_AUTONOMOUS_EFFECT_EVENTS", "MAX_AUTONOMOUS_EFFECT_JOURNAL_BYTES", "MAX_AUTONOMOUS_EFFECT_EVENT_BYTES", "MAX_AUTONOMOUS_EFFECT_ARGUMENT_BYTES", "MAX_AUTONOMOUS_EFFECT_REASON_BYTES", "EFFECT_RETENTION", "EFFECT_SNAPSHOT_RETENTION", "AutonomousEffectError", "AutonomousEffectPolicyError", "AutonomousEffectReconciliationRequiredError", "AutonomousEffectExecutionError", "AutonomousEffectRequest", "AutonomousEffectExecutionContext", "AutonomousEffectRecord", "AutonomousEffectEvent", "AutonomousEffectJournalRow", "AutonomousEffectJournalReceipt", "AutonomousEffectJournalSnapshot", "AutonomousEffectJournal", "AutonomousEffectSnapshotJournal", "AutonomousEffectSnapshotPersistence", "AutonomousEffectTransactionalSnapshotPersistence", "AutonomousEffectResolution", "AutonomousEffectResolver", "InMemoryAutonomousEffectJournal", "InMemoryAutonomousEffectSnapshotTextStore", "JsonAutonomousEffectSnapshotPersistence", "TransactionalJsonAutonomousEffectSnapshotPersistence", "AutonomousEffectPersistenceCoordinator", "AutonomousEffectBoundary", "validate_autonomous_effect_journal_snapshot",
]
