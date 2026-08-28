"""Restart-safe policy and execution metadata for long-horizon autonomous work.

The provider conversation, task text, credentials, tool arguments, and tool outputs remain
caller-owned and transient.  This module persists only the metadata needed to enforce a bounded
policy across process restarts and to audit what happened: digests, counters, domain labels,
approval posture, evaluator identity, and state transitions.

It is intentionally independent of the provider runtime and the MCP server.  A caller can use the
controller with native tool loops, staged workflows, cross-domain fan-out, or a custom executor.
The journal does not pretend that a metadata checkpoint can reconstruct a provider conversation;
the caller must rehydrate the task and prompt and explicitly resume with the same execution id.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import hashlib
import json
import math
import os
from pathlib import Path
import threading
import time
from typing import Any, Callable, Mapping, Protocol, Sequence
import uuid

from .authoring import canonical_bytes, content_digest
from .errors import ArgumentError


AUTONOMY_POLICY_SCHEMA = "bioprism-python-autonomous-execution-policy/0.1"
AUTONOMY_STATE_SCHEMA = "bioprism-python-autonomous-execution-state/0.1"
AUTONOMY_EVENT_SCHEMA = "bioprism-python-autonomous-execution-event/0.1"
AUTONOMY_JOURNAL_SCHEMA = "bioprism-python-autonomous-execution-journal/0.1"
AUTONOMY_EXECUTION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-execution-snapshot/0.1"
AUTONOMY_EVENT_KINDS = (
    "started",
    "resumed",
    "provider_call",
    "tool_intent",
    "tool_outcome",
    "effect_reconciliation",
    "checkpoint",
    "approval_required",
    "evaluation",
    "replan",
    "paused",
    "completed",
    "failed",
)
AUTONOMY_TERMINAL_STATUSES = ("completed", "failed", "cancelled", "reconciliation_required")
MAX_AUTONOMY_EXECUTION_ID_BYTES = 256
MAX_AUTONOMY_EVENT_BYTES = 256_000
MAX_AUTONOMY_JOURNAL_BYTES = 64_000_000
MAX_AUTONOMY_JOURNAL_EVENTS = 32_768
MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES = MAX_AUTONOMY_JOURNAL_BYTES
MAX_AUTONOMY_METADATA_DEPTH = 32
MAX_AUTONOMY_STEPS = 4_096
MAX_AUTONOMY_PROVIDER_CALLS = 1_024
MAX_AUTONOMY_PROVIDER_FAILOVERS = 8
MAX_AUTONOMY_TOOL_CALLS = 8_192
MAX_AUTONOMY_EFFECTFUL_CALLS = 512
MAX_AUTONOMY_REPLANS = 64
MAX_AUTONOMY_COST_UNITS = 1_000_000.0
_SAFE_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-"
)
_FORBIDDEN_FIELDS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "password",
        "secret",
        "accesstoken",
        "refreshtoken",
        "token",
        "privatekey",
        "prompt",
        "response",
        "rawpayload",
        "arguments",
        "output",
        "task",
        "messages",
    }
)


class AutonomyPersistenceError(ArgumentError):
    """A long-horizon policy, journal, or resume operation was refused."""


class AutonomyPolicyError(AutonomyPersistenceError):
    """A proposed autonomous action exceeded its caller-owned execution policy."""


class AutonomousExecutionSnapshotTextStore(Protocol):
    """Portable text storage for metadata-only execution snapshots."""

    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class AutonomousExecutionTransactionalSnapshotTextStore(AutonomousExecutionSnapshotTextStore, Protocol):
    """Text storage that can fence stale snapshot writers with compare-and-swap."""

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


def _text(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise AutonomyPersistenceError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise AutonomyPersistenceError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any, *, maximum: int = 512) -> str:
    result = _text(name, value, maximum=maximum)
    if any(character not in _SAFE_IDENTIFIER_CHARS for character in result):
        raise AutonomyPersistenceError(f"{name} must be a bounded identifier")
    return result


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise AutonomyPersistenceError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _json_safe(name: str, value: Any, *, maximum: int) -> Any:
    try:
        encoded = canonical_bytes(value)
    except (TypeError, ValueError, ArgumentError) as error:
        raise AutonomyPersistenceError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise AutonomyPersistenceError(f"{name} exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


def _assert_metadata_safe(value: Any, *, depth: int = 0) -> None:
    if depth > MAX_AUTONOMY_METADATA_DEPTH:
        raise AutonomyPersistenceError("autonomy metadata is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
            if normalized in _FORBIDDEN_FIELDS:
                raise AutonomyPersistenceError("autonomy metadata contains transient or secret-shaped fields")
            _assert_metadata_safe(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _assert_metadata_safe(child, depth=depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise AutonomyPersistenceError("autonomy metadata contains a non-finite number")


def _now_ns(clock: Callable[[], float]) -> int:
    try:
        value = float(clock())
    except Exception as error:
        raise AutonomyPersistenceError("autonomy clock failed") from error
    if not math.isfinite(value) or value < 0:
        raise AutonomyPersistenceError("autonomy clock returned an invalid value")
    return int(value * 1_000_000_000)


@dataclass(frozen=True, slots=True)
class AutonomousExecutionPolicy:
    """Explicit bounded action policy shared by long-horizon autonomous sessions."""

    max_steps: int = 32
    max_provider_calls: int = 16
    max_provider_failovers: int = 2
    max_tool_calls: int = 128
    max_effectful_calls: int = 0
    max_replans: int = 2
    max_cost_units: float = 100.0
    allow_side_effects: bool = False
    stop_on_error: bool = True
    pause_on_approval: bool = True

    def __post_init__(self) -> None:
        for name, value, maximum in (
            ("max_steps", self.max_steps, MAX_AUTONOMY_STEPS),
            ("max_provider_calls", self.max_provider_calls, MAX_AUTONOMY_PROVIDER_CALLS),
            ("max_provider_failovers", self.max_provider_failovers, MAX_AUTONOMY_PROVIDER_FAILOVERS),
            ("max_tool_calls", self.max_tool_calls, MAX_AUTONOMY_TOOL_CALLS),
            ("max_effectful_calls", self.max_effectful_calls, MAX_AUTONOMY_EFFECTFUL_CALLS),
            ("max_replans", self.max_replans, MAX_AUTONOMY_REPLANS),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= maximum:
                raise AutonomyPersistenceError(f"{name} must be within [0, {maximum}]")
        if not isinstance(self.max_cost_units, (int, float)) or isinstance(self.max_cost_units, bool) or not math.isfinite(float(self.max_cost_units)) or not 0 <= self.max_cost_units <= MAX_AUTONOMY_COST_UNITS:
            raise AutonomyPersistenceError(f"max_cost_units must be within [0, {MAX_AUTONOMY_COST_UNITS}]")
        for name, value in (
            ("allow_side_effects", self.allow_side_effects),
            ("stop_on_error", self.stop_on_error),
            ("pause_on_approval", self.pause_on_approval),
        ):
            if not isinstance(value, bool):
                raise AutonomyPersistenceError(f"{name} must be a boolean")
        if self.allow_side_effects and self.max_effectful_calls == 0:
            raise AutonomyPersistenceError("allow_side_effects requires max_effectful_calls greater than zero")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousExecutionPolicy":
        if not isinstance(value, Mapping):
            raise AutonomyPersistenceError("execution policy must be a mapping")
        if value.get("schema") not in (None, AUTONOMY_POLICY_SCHEMA):
            raise AutonomyPersistenceError("execution policy schema is unsupported")
        allowed = {
            "schema", "max_steps", "max_provider_calls", "max_provider_failovers", "max_tool_calls", "max_effectful_calls",
            "max_replans", "max_cost_units", "allow_side_effects", "stop_on_error", "pause_on_approval",
        }
        unknown = set(value).difference(allowed)
        if unknown:
            raise AutonomyPersistenceError("execution policy contains unsupported fields")
        return cls(
            max_steps=value.get("max_steps", 32),
            max_provider_calls=value.get("max_provider_calls", 16),
            max_provider_failovers=value.get("max_provider_failovers", 2),
            max_tool_calls=value.get("max_tool_calls", 128),
            max_effectful_calls=value.get("max_effectful_calls", 0),
            max_replans=value.get("max_replans", 2),
            max_cost_units=value.get("max_cost_units", 100.0),
            allow_side_effects=value.get("allow_side_effects", False),
            stop_on_error=value.get("stop_on_error", True),
            pause_on_approval=value.get("pause_on_approval", True),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMY_POLICY_SCHEMA,
            "max_steps": self.max_steps,
            "max_provider_calls": self.max_provider_calls,
            "max_provider_failovers": self.max_provider_failovers,
            "max_tool_calls": self.max_tool_calls,
            "max_effectful_calls": self.max_effectful_calls,
            "max_replans": self.max_replans,
            "max_cost_units": float(self.max_cost_units),
            "allow_side_effects": self.allow_side_effects,
            "stop_on_error": self.stop_on_error,
            "pause_on_approval": self.pause_on_approval,
            "authorization": "caller_owned_policy",
        }

    @property
    def digest(self) -> str:
        return content_digest(self.to_dict())


@dataclass(frozen=True, slots=True)
class AutonomousExecutionState:
    """Metadata-only state that can be rehydrated after a process restart."""

    execution_id: str
    domain: str
    capability: str
    risk_class: str
    policy_digest: str
    step_index: int = 0
    provider_calls: int = 0
    provider_failovers: int = 0
    tool_calls: int = 0
    effectful_calls: int = 0
    cost_units: float = 0.0
    replans: int = 0
    status: str = "started"
    last_event_kind: str = "started"
    last_tool: str | None = None
    last_call_id: str | None = None
    last_outcome_digest: str | None = None
    last_evaluation_digest: str | None = None
    checkpoint_digest: str | None = None
    journal_sequence: int = 0

    def __post_init__(self) -> None:
        _identifier("execution_id", self.execution_id, maximum=MAX_AUTONOMY_EXECUTION_ID_BYTES)
        for name, value in (("domain", self.domain), ("capability", self.capability), ("risk_class", self.risk_class)):
            _identifier(f"execution {name}", value)
        _digest("execution policy_digest", self.policy_digest)
        for name, value, maximum in (
            ("step_index", self.step_index, MAX_AUTONOMY_STEPS),
            ("provider_calls", self.provider_calls, MAX_AUTONOMY_PROVIDER_CALLS),
            ("provider_failovers", self.provider_failovers, MAX_AUTONOMY_PROVIDER_FAILOVERS),
            ("tool_calls", self.tool_calls, MAX_AUTONOMY_TOOL_CALLS),
            ("effectful_calls", self.effectful_calls, MAX_AUTONOMY_EFFECTFUL_CALLS),
            ("replans", self.replans, MAX_AUTONOMY_REPLANS),
            ("journal_sequence", self.journal_sequence, MAX_AUTONOMY_JOURNAL_EVENTS),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= maximum:
                raise AutonomyPersistenceError(f"execution {name} is outside its bound")
        if not isinstance(self.cost_units, (int, float)) or isinstance(self.cost_units, bool) or not math.isfinite(float(self.cost_units)) or not 0 <= self.cost_units <= MAX_AUTONOMY_COST_UNITS:
            raise AutonomyPersistenceError("execution cost_units must be finite and non-negative")
        if not isinstance(self.status, str) or not self.status.strip():
            raise AutonomyPersistenceError("execution status must be non-empty")
        _identifier("execution last_event_kind", self.last_event_kind)
        for name, value in (("last_tool", self.last_tool), ("last_call_id", self.last_call_id)):
            if value is not None:
                _identifier(f"execution {name}", value, maximum=512)
        for name, value in (("last_outcome_digest", self.last_outcome_digest), ("last_evaluation_digest", self.last_evaluation_digest), ("checkpoint_digest", self.checkpoint_digest)):
            if value is not None:
                _digest(f"execution {name}", value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMY_STATE_SCHEMA,
            "execution_id": self.execution_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "policy_digest": self.policy_digest,
            "step_index": self.step_index,
            "provider_calls": self.provider_calls,
            "provider_failovers": self.provider_failovers,
            "tool_calls": self.tool_calls,
            "effectful_calls": self.effectful_calls,
            "cost_units": float(self.cost_units),
            "replans": self.replans,
            "status": self.status,
            "last_event_kind": self.last_event_kind,
            "last_tool": self.last_tool,
            "last_call_id": self.last_call_id,
            "last_outcome_digest": self.last_outcome_digest,
            "last_evaluation_digest": self.last_evaluation_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "journal_sequence": self.journal_sequence,
            "retention": "metadata_only_no_task_prompt_credentials_or_payloads",
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousExecutionState":
        if not isinstance(value, Mapping):
            raise AutonomyPersistenceError("execution state must be a mapping")
        if value.get("schema") not in (None, AUTONOMY_STATE_SCHEMA):
            raise AutonomyPersistenceError("execution state schema is unsupported")
        return cls(
            execution_id=value.get("execution_id"),
            domain=value.get("domain"),
            capability=value.get("capability"),
            risk_class=value.get("risk_class"),
            policy_digest=value.get("policy_digest"),
            step_index=value.get("step_index", 0),
            provider_calls=value.get("provider_calls", 0),
            provider_failovers=value.get("provider_failovers", 0),
            tool_calls=value.get("tool_calls", 0),
            effectful_calls=value.get("effectful_calls", 0),
            cost_units=value.get("cost_units", 0.0),
            replans=value.get("replans", 0),
            status=value.get("status", "started"),
            last_event_kind=value.get("last_event_kind", "started"),
            last_tool=value.get("last_tool"),
            last_call_id=value.get("last_call_id"),
            last_outcome_digest=value.get("last_outcome_digest"),
            last_evaluation_digest=value.get("last_evaluation_digest"),
            checkpoint_digest=value.get("checkpoint_digest"),
            journal_sequence=value.get("journal_sequence", 0),
        )


class AutonomousExecutionJournal:
    """Append-only, hash-chained JSONL journal for metadata-only execution state."""

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_events: int = MAX_AUTONOMY_JOURNAL_EVENTS,
        max_bytes: int = MAX_AUTONOMY_JOURNAL_BYTES,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(path, (str, os.PathLike)) or not str(path):
            raise AutonomyPersistenceError("execution journal path must be non-empty")
        if not isinstance(max_events, int) or isinstance(max_events, bool) or not 1 <= max_events <= MAX_AUTONOMY_JOURNAL_EVENTS:
            raise AutonomyPersistenceError("execution journal max_events is outside its bound")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or not 1 <= max_bytes <= MAX_AUTONOMY_JOURNAL_BYTES:
            raise AutonomyPersistenceError("execution journal max_bytes is outside its bound")
        if not callable(clock):
            raise AutonomyPersistenceError("execution journal clock must be callable")
        self.path = Path(path)
        self.max_events = max_events
        self.max_bytes = max_bytes
        self._clock = clock
        self._lock = threading.RLock()

    def append(self, event: Mapping[str, Any]) -> dict[str, Any]:
        normalized = self._normalize_event(event)
        with self._lock:
            rows = self._read_rows_locked()
            if len(rows) >= self.max_events:
                raise AutonomyPersistenceError("execution journal event capacity is exhausted")
            previous = "" if not rows else rows[-1]["event_digest"]
            sequence = len(rows) + 1
            envelope = {
                "schema": AUTONOMY_EVENT_SCHEMA,
                "sequence": sequence,
                "event": normalized,
                "previous_digest": previous,
                "created_ns": _now_ns(self._clock),
            }
            event_digest = content_digest(envelope)
            envelope["event_digest"] = event_digest
            line = canonical_bytes(envelope) + b"\n"
            current_size = self.path.stat().st_size if self.path.exists() else 0
            if current_size + len(line) > self.max_bytes:
                raise AutonomyPersistenceError("execution journal byte capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            return {
                "schema": AUTONOMY_EVENT_SCHEMA,
                "sequence": sequence,
                "event_digest": event_digest,
                "head_digest": event_digest,
                "execution_id": normalized["execution_id"],
                "kind": normalized["kind"],
                "retention": "metadata_only_hash_chained",
            }

    def events(
        self,
        *,
        execution_id: str | None = None,
        after_sequence: int = 0,
        limit: int = 256,
    ) -> tuple[dict[str, Any], ...]:
        if execution_id is not None:
            execution_id = _identifier("execution_id", execution_id, maximum=MAX_AUTONOMY_EXECUTION_ID_BYTES)
        if not isinstance(after_sequence, int) or isinstance(after_sequence, bool) or after_sequence < 0:
            raise AutonomyPersistenceError("after_sequence must be a non-negative integer")
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_events:
            raise AutonomyPersistenceError("journal event limit is outside its bound")
        with self._lock:
            rows = self._read_rows_locked()
        selected = [row for row in rows if row["sequence"] > after_sequence and (execution_id is None or row["event"]["execution_id"] == execution_id)]
        return tuple(selected[:limit])

    def state(self, execution_id: str) -> AutonomousExecutionState | None:
        rows = self.events(execution_id=execution_id, limit=self.max_events)
        if not rows:
            return None
        latest_row = rows[-1]
        latest = latest_row["event"]
        state = AutonomousExecutionState.from_mapping(latest["state"] if isinstance(latest.get("state"), Mapping) else latest)
        # The event digest and sequence are only known after the envelope is assembled.  Expose
        # the authoritative envelope position to callers even though the embedded snapshot is
        # intentionally hashed before those two fields exist.
        return replace(
            state,
            journal_sequence=latest_row["sequence"],
            checkpoint_digest=latest_row["event_digest"],
        )

    def begin(
        self,
        *,
        execution_id: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy: AutonomousExecutionPolicy,
        resume: bool = False,
    ) -> AutonomousExecutionState:
        execution_id = _identifier("execution_id", execution_id, maximum=MAX_AUTONOMY_EXECUTION_ID_BYTES)
        domain = _identifier("execution domain", domain)
        capability = _identifier("execution capability", capability)
        risk_class = _identifier("execution risk_class", risk_class)
        if not isinstance(policy, AutonomousExecutionPolicy):
            raise AutonomyPersistenceError("execution journal requires an AutonomousExecutionPolicy")
        previous = self.state(execution_id)
        if previous is not None:
            if not resume:
                raise AutonomyPersistenceError("execution id already exists; resume must be explicit")
            if previous.policy_digest != policy.digest:
                raise AutonomyPersistenceError("resume policy digest does not match the persisted execution")
            if previous.status in AUTONOMY_TERMINAL_STATUSES and previous.status != "reconciliation_required":
                raise AutonomyPersistenceError("terminal execution cannot be resumed")
            resumed = replace(previous, status="resumed", last_event_kind="resumed")
            self.append({"execution_id": execution_id, "kind": "resumed", "domain": domain, "capability": capability, "risk_class": risk_class, "status": "resumed", "policy_digest": policy.digest, "state": resumed.to_dict()})
            return replace(resumed, journal_sequence=len(self._read_rows()), checkpoint_digest=content_digest(resumed.to_dict()))
        initial = AutonomousExecutionState(
            execution_id=execution_id,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            policy_digest=policy.digest,
        )
        receipt = self.append({"execution_id": execution_id, "kind": "started", "domain": domain, "capability": capability, "risk_class": risk_class, "status": "started", "policy_digest": policy.digest, "state": initial.to_dict()})
        return replace(initial, journal_sequence=receipt["sequence"], checkpoint_digest=receipt["event_digest"])

    def verify_integrity(self) -> dict[str, Any]:
        with self._lock:
            rows = self._read_rows_locked()
        previous = ""
        for expected_sequence, row in enumerate(rows, start=1):
            if row["sequence"] != expected_sequence or row["previous_digest"] != previous:
                raise AutonomyPersistenceError("execution journal hash chain sequence is invalid")
            payload = {
                "schema": row["schema"],
                "sequence": row["sequence"],
                "event": row["event"],
                "previous_digest": row["previous_digest"],
                "created_ns": row["created_ns"],
            }
            expected = content_digest(payload)
            if expected != row["event_digest"]:
                raise AutonomyPersistenceError("execution journal hash chain digest is invalid")
            previous = expected
        return {"schema": AUTONOMY_JOURNAL_SCHEMA, "verified": True, "events": len(rows), "head_digest": previous, "retention": "metadata_only"}

    def snapshot(self) -> dict[str, Any]:
        """Return an integrity-checked, provider-independent journal snapshot.

        The snapshot contains only normalized event envelopes.  It is deliberately separate
        from the JSONL file so callers can move the same journal through an object store,
        database, or HTTP text store without teaching this module about transport details.
        """

        with self._lock:
            rows = self._read_rows_locked()
            descriptor = {
                "schema": AUTONOMY_EXECUTION_SNAPSHOT_SCHEMA,
                "rows": rows,
                "head_digest": rows[-1]["event_digest"] if rows else "",
                "retention": "metadata_only_hash_chained",
                "secret_material": "never_returned",
            }
            snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
            if len(canonical_bytes(snapshot)) > MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES:
                raise AutonomyPersistenceError("execution journal snapshot exceeds max_bytes")
            return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        """Atomically replace the local JSONL journal with a verified snapshot."""

        normalized = _normalize_execution_snapshot(
            snapshot,
            max_events=self.max_events,
            max_bytes=self.max_bytes,
        )
        encoded_rows = b"".join(canonical_bytes(row) + b"\n" for row in normalized["rows"])
        with self._lock:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            temporary = self.path.with_name(f".{self.path.name}.{uuid.uuid4().hex}.restore")
            try:
                with temporary.open("wb") as handle:
                    handle.write(encoded_rows)
                    handle.flush()
                    os.fsync(handle.fileno())
                os.replace(temporary, self.path)
            finally:
                if temporary.exists():
                    temporary.unlink()

    def _read_rows(self) -> list[dict[str, Any]]:
        with self._lock:
            return self._read_rows_locked()

    def _read_rows_locked(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise AutonomyPersistenceError("execution journal exceeds max_bytes")
        rows: list[dict[str, Any]] = []
        previous_digest = ""
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_events:
                    raise AutonomyPersistenceError("execution journal exceeds max_events")
                try:
                    row = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise AutonomyPersistenceError("execution journal contains invalid JSON") from error
                if not isinstance(row, Mapping) or row.get("schema") != AUTONOMY_EVENT_SCHEMA:
                    raise AutonomyPersistenceError("execution journal contains an invalid event schema")
                if set(row) != {"schema", "sequence", "event", "previous_digest", "created_ns", "event_digest"}:
                    raise AutonomyPersistenceError("execution journal contains unsupported envelope fields")
                expected_sequence = len(rows) + 1
                if row.get("sequence") != expected_sequence:
                    raise AutonomyPersistenceError("execution journal contains an invalid sequence")
                if row.get("previous_digest") != previous_digest:
                    raise AutonomyPersistenceError("execution journal contains an invalid previous digest")
                if not isinstance(row.get("created_ns"), int) or isinstance(row.get("created_ns"), bool) or row["created_ns"] < 0:
                    raise AutonomyPersistenceError("execution journal contains an invalid timestamp")
                if not isinstance(row.get("event_digest"), str) or len(row["event_digest"]) != 64 or any(character not in "0123456789abcdef" for character in row["event_digest"]):
                    raise AutonomyPersistenceError("execution journal contains an invalid event digest")
                event = row.get("event")
                if not isinstance(event, Mapping):
                    raise AutonomyPersistenceError("execution journal event is not a mapping")
                normalized = self._normalize_event(event)
                if row.get("event_digest") != content_digest({
                    "schema": row.get("schema"),
                    "sequence": row.get("sequence"),
                    "event": normalized,
                    "previous_digest": row.get("previous_digest"),
                    "created_ns": row.get("created_ns"),
                }):
                    raise AutonomyPersistenceError("execution journal contains an invalid event digest")
                rows.append({
                    "schema": row["schema"],
                    "sequence": row["sequence"],
                    "event": dict(normalized),
                    "previous_digest": row.get("previous_digest", ""),
                    "created_ns": row.get("created_ns"),
                    "event_digest": row["event_digest"],
                })
                previous_digest = row["event_digest"]
        return rows

    @staticmethod
    def _normalize_event(event: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(event, Mapping):
            raise AutonomyPersistenceError("execution event must be a mapping")
        allowed = {
            "execution_id", "kind", "domain", "capability", "risk_class", "status", "policy_digest", "state",
            "step_index", "provider_calls", "provider_failovers", "tool_calls", "effectful_calls", "cost_units", "replans",
            "tool", "call_id", "read_only", "approval_required", "schema_digest", "arguments_digest",
            "output_digest", "outcome_digest", "evaluation_digest", "evaluator_id", "evaluator_version",
            "reward", "passed", "failure_class", "reason", "metadata", "provider", "model",
            "invocation_kind", "attempt", "turn", "selection_digest", "provider_outcome",
            "latency_ms", "input_tokens", "output_tokens", "estimated_cost_units", "actual_cost_units",
            "request_id_digest", "status_code", "effect_id", "effect_status", "dispatch_attempt",
            "reconciliation_digest", "instruction_digest", "failover", "retryable",
        }
        if set(event).difference(allowed):
            raise AutonomyPersistenceError("execution event contains unsupported fields")
        execution_id = _identifier("event execution_id", event.get("execution_id"), maximum=MAX_AUTONOMY_EXECUTION_ID_BYTES)
        kind = _identifier("event kind", event.get("kind"))
        if kind not in AUTONOMY_EVENT_KINDS:
            raise AutonomyPersistenceError("execution event kind is unsupported")
        normalized: dict[str, Any] = {
            "execution_id": execution_id,
            "kind": kind,
            "domain": _identifier("event domain", event.get("domain")),
            "capability": _identifier("event capability", event.get("capability")),
            "risk_class": _identifier("event risk_class", event.get("risk_class")),
            "status": _identifier("event status", event.get("status")),
            "policy_digest": _digest("event policy_digest", event.get("policy_digest")),
        }
        for name, maximum in (("step_index", MAX_AUTONOMY_STEPS), ("provider_calls", MAX_AUTONOMY_PROVIDER_CALLS), ("provider_failovers", MAX_AUTONOMY_PROVIDER_FAILOVERS), ("tool_calls", MAX_AUTONOMY_TOOL_CALLS), ("effectful_calls", MAX_AUTONOMY_EFFECTFUL_CALLS), ("replans", MAX_AUTONOMY_REPLANS)):
            if name in event:
                value = event[name]
                if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= maximum:
                    raise AutonomyPersistenceError(f"event {name} is outside its bound")
                normalized[name] = value
        if "cost_units" in event:
            value = event["cost_units"]
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)) or not 0 <= value <= MAX_AUTONOMY_COST_UNITS:
                raise AutonomyPersistenceError("event cost_units is outside its bound")
            normalized["cost_units"] = float(value)
        for name in ("tool", "call_id", "evaluator_id", "evaluator_version", "failure_class", "reason"):
            if name in event and event[name] is not None:
                normalized[name] = _identifier(f"event {name}", event[name], maximum=2048 if name == "reason" else 512)
        for name in ("schema_digest", "arguments_digest", "output_digest", "outcome_digest", "evaluation_digest", "instruction_digest"):
            if name in event and event[name] is not None:
                normalized[name] = _digest(f"event {name}", event[name])
        for name in ("selection_digest", "request_id_digest"):
            if name in event and event[name] is not None:
                normalized[name] = _digest(f"event {name}", event[name])
        for name in ("effect_id", "effect_status"):
            if name in event and event[name] is not None:
                normalized[name] = _identifier(f"event {name}", event[name], maximum=512)
        if "reconciliation_digest" in event and event["reconciliation_digest"] is not None:
            normalized["reconciliation_digest"] = _digest(
                "event reconciliation_digest", event["reconciliation_digest"]
            )
        for name in ("provider", "model", "invocation_kind", "provider_outcome"):
            if name in event and event[name] is not None:
                normalized[name] = _text(f"event {name}", event[name], maximum=512)
        for name, maximum in (("attempt", 8), ("turn", 32), ("input_tokens", 100_000_000), ("output_tokens", 100_000_000), ("status_code", 999), ("dispatch_attempt", 64)):
            if name in event and event[name] is not None:
                value = event[name]
                if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= maximum:
                    raise AutonomyPersistenceError(f"event {name} is outside its bound")
                normalized[name] = value
        for name in ("latency_ms", "estimated_cost_units", "actual_cost_units"):
            if name in event and event[name] is not None:
                value = event[name]
                if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)) or value < 0:
                    raise AutonomyPersistenceError(f"event {name} is outside its bound")
                normalized[name] = float(value)
        for name in ("read_only", "approval_required", "passed", "failover", "retryable"):
            if name in event and event[name] is not None:
                if not isinstance(event[name], bool):
                    raise AutonomyPersistenceError(f"event {name} must be a boolean")
                normalized[name] = event[name]
        if "reward" in event and event["reward"] is not None:
            reward = event["reward"]
            if not isinstance(reward, (int, float)) or isinstance(reward, bool) or not math.isfinite(float(reward)) or not -1 <= float(reward) <= 1:
                raise AutonomyPersistenceError("event reward must be finite and within [-1, 1]")
            normalized["reward"] = float(reward)
        if "state" in event:
            state = AutonomousExecutionState.from_mapping(event["state"])
            if state.execution_id != execution_id:
                raise AutonomyPersistenceError("event state execution_id does not match its event")
            normalized["state"] = state.to_dict()
        if "metadata" in event:
            metadata = _json_safe("event metadata", event["metadata"], maximum=64_000)
            _assert_metadata_safe(metadata)
            normalized["metadata"] = metadata
        _assert_metadata_safe(normalized)
        encoded = canonical_bytes(normalized)
        if len(encoded) > MAX_AUTONOMY_EVENT_BYTES:
            raise AutonomyPersistenceError("execution event exceeds max bytes")
        return normalized


def _normalize_execution_snapshot(
    value: Mapping[str, Any],
    *,
    max_events: int = MAX_AUTONOMY_JOURNAL_EVENTS,
    max_bytes: int = MAX_AUTONOMY_JOURNAL_BYTES,
) -> dict[str, Any]:
    """Validate and canonicalize a complete execution journal snapshot."""

    expected_keys = {"schema", "rows", "head_digest", "retention", "secret_material", "snapshot_digest"}
    if not isinstance(value, Mapping) or set(value) != expected_keys:
        raise AutonomyPersistenceError("execution journal snapshot is malformed")
    if value.get("schema") != AUTONOMY_EXECUTION_SNAPSHOT_SCHEMA:
        raise AutonomyPersistenceError("execution journal snapshot schema is unsupported")
    if value.get("retention") != "metadata_only_hash_chained" or value.get("secret_material") != "never_returned":
        raise AutonomyPersistenceError("execution journal snapshot retention is invalid")
    rows_raw = value.get("rows")
    if not isinstance(rows_raw, Sequence) or isinstance(rows_raw, (str, bytes, bytearray)) or len(rows_raw) > max_events:
        raise AutonomyPersistenceError("execution journal snapshot exceeds its event capacity")
    rows: list[dict[str, Any]] = []
    previous_digest = ""
    total_bytes = 0
    envelope_keys = {"schema", "sequence", "event", "previous_digest", "created_ns", "event_digest"}
    for expected_sequence, raw_row in enumerate(rows_raw, start=1):
        if not isinstance(raw_row, Mapping) or set(raw_row) != envelope_keys:
            raise AutonomyPersistenceError("execution journal snapshot contains an invalid event envelope")
        if raw_row.get("schema") != AUTONOMY_EVENT_SCHEMA or raw_row.get("sequence") != expected_sequence:
            raise AutonomyPersistenceError("execution journal snapshot contains an invalid sequence")
        if raw_row.get("previous_digest") != previous_digest:
            raise AutonomyPersistenceError("execution journal snapshot hash chain is discontinuous")
        created_ns = raw_row.get("created_ns")
        if not isinstance(created_ns, int) or isinstance(created_ns, bool) or created_ns < 0:
            raise AutonomyPersistenceError("execution journal snapshot contains an invalid timestamp")
        event_digest = raw_row.get("event_digest")
        if not isinstance(event_digest, str) or len(event_digest) != 64 or any(character not in "0123456789abcdef" for character in event_digest):
            raise AutonomyPersistenceError("execution journal snapshot contains an invalid event digest")
        normalized_event = AutonomousExecutionJournal._normalize_event(raw_row.get("event"))
        descriptor = {
            "schema": AUTONOMY_EVENT_SCHEMA,
            "sequence": expected_sequence,
            "event": normalized_event,
            "previous_digest": previous_digest,
            "created_ns": created_ns,
        }
        if content_digest(descriptor) != event_digest:
            raise AutonomyPersistenceError("execution journal snapshot contains an invalid event digest")
        normalized_row = {**descriptor, "event_digest": event_digest}
        rows.append(normalized_row)
        total_bytes += len(canonical_bytes(normalized_row)) + 1
        if total_bytes > max_bytes:
            raise AutonomyPersistenceError("execution journal snapshot exceeds max_bytes")
        previous_digest = event_digest
    head_digest = value.get("head_digest")
    if not isinstance(head_digest, str) or (head_digest and (len(head_digest) != 64 or any(character not in "0123456789abcdef" for character in head_digest))):
        raise AutonomyPersistenceError("execution journal snapshot head_digest is invalid")
    if head_digest != previous_digest:
        raise AutonomyPersistenceError("execution journal snapshot head_digest is inconsistent")
    descriptor = {
        "schema": AUTONOMY_EXECUTION_SNAPSHOT_SCHEMA,
        "rows": rows,
        "head_digest": head_digest,
        "retention": "metadata_only_hash_chained",
        "secret_material": "never_returned",
    }
    snapshot_digest = value.get("snapshot_digest")
    if not isinstance(snapshot_digest, str) or len(snapshot_digest) != 64 or any(character not in "0123456789abcdef" for character in snapshot_digest):
        raise AutonomyPersistenceError("execution journal snapshot_digest is invalid")
    if content_digest(descriptor) != snapshot_digest:
        raise AutonomyPersistenceError("execution journal snapshot digest does not match its metadata")
    normalized = {**descriptor, "snapshot_digest": snapshot_digest}
    if len(canonical_bytes(normalized)) > MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES:
        raise AutonomyPersistenceError("execution journal snapshot exceeds its byte capacity")
    return normalized


def validate_autonomous_execution_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Public strict validator for metadata-only execution snapshots."""

    return _normalize_execution_snapshot(value)


class JsonAutonomousExecutionSnapshotPersistence:
    """Canonical JSON persistence over any caller-owned text store."""

    def __init__(self, store: AutonomousExecutionSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("execution JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES:
            raise ArgumentError("execution JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("execution JSON snapshot exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ArgumentError("execution JSON snapshot is invalid") from error
        if not isinstance(raw, Mapping):
            raise ArgumentError("execution JSON snapshot must be an object")
        normalized = _normalize_execution_snapshot(raw)
        if encoded != canonical_bytes(normalized).decode("utf-8"):
            raise ArgumentError("execution JSON snapshot is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _normalize_execution_snapshot(snapshot)
        encoded = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("execution JSON snapshot exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousExecutionSnapshotPersistence(JsonAutonomousExecutionSnapshotPersistence):
    """Canonical JSON execution persistence with compare-and-swap fencing."""

    def __init__(self, store: AutonomousExecutionTransactionalSnapshotTextStore, *, max_bytes: int = MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional execution persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None and (
            not isinstance(expected_snapshot_digest, str)
            or len(expected_snapshot_digest) != 64
            or any(character not in "0123456789abcdef" for character in expected_snapshot_digest)
        ):
            raise ArgumentError("execution expected snapshot digest is invalid")
        normalized = _normalize_execution_snapshot(snapshot)
        encoded = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("execution JSON snapshot exceeds its byte bound")
        return self.store.write_if_unchanged(expected_snapshot_digest, encoded)


class AutonomousExecutionPersistenceCoordinator:
    """Flush and restore a hash-checked execution journal through caller-owned storage."""

    def __init__(self, journal: AutonomousExecutionJournal, persistence: Any) -> None:
        if not isinstance(journal, AutonomousExecutionJournal):
            raise ArgumentError("execution persistence requires an AutonomousExecutionJournal")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("execution persistence adapter is malformed")
        self.journal = journal
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any] | None:
        with self._lock:
            raw = self.persistence.read()
            if raw is None:
                self._expected_snapshot_digest = None
                return None
            snapshot = _normalize_execution_snapshot(raw, max_events=self.journal.max_events, max_bytes=self.journal.max_bytes)
            self.journal.restore(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return snapshot

    def flush(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.journal.snapshot()
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise AutonomyPersistenceError("execution persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return snapshot


class AutonomousExecutionController:
    """Policy gate and metadata state machine for one long-horizon execution id."""

    def __init__(
        self,
        *,
        execution_id: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy: AutonomousExecutionPolicy | Mapping[str, Any] | None = None,
        journal: AutonomousExecutionJournal | None = None,
        resume: bool = False,
    ) -> None:
        self.policy = policy if isinstance(policy, AutonomousExecutionPolicy) else AutonomousExecutionPolicy.from_mapping(policy or {})
        if journal is not None and not isinstance(journal, AutonomousExecutionJournal):
            raise AutonomyPersistenceError("controller journal must be an AutonomousExecutionJournal or None")
        self.journal = journal
        self.state = (
            journal.begin(
                execution_id=execution_id,
                domain=domain,
                capability=capability,
                risk_class=risk_class,
                policy=self.policy,
                resume=resume,
            )
            if journal is not None
            else AutonomousExecutionState(
                execution_id=_identifier("execution_id", execution_id, maximum=MAX_AUTONOMY_EXECUTION_ID_BYTES),
                domain=_identifier("execution domain", domain),
                capability=_identifier("execution capability", capability),
                risk_class=_identifier("execution risk_class", risk_class),
                policy_digest=self.policy.digest,
            )
        )
        self._terminal = False

    def admit_provider_call(
        self,
        *,
        cost_units: float = 0.0,
        provider: str | None = None,
        model: str | None = None,
        invocation_kind: str | None = None,
        attempt: int | None = None,
        turn: int | None = None,
        selection_digest: str | None = None,
        estimated_cost_units: float | None = None,
        failover: bool = False,
    ) -> AutonomousExecutionState:
        self._ensure_active()
        self._ensure_step()
        if self.state.provider_calls >= self.policy.max_provider_calls:
            raise AutonomyPolicyError("max_provider_calls exceeded")
        if not isinstance(failover, bool):
            raise AutonomyPersistenceError("failover must be a boolean")
        if failover and self.state.provider_failovers >= self.policy.max_provider_failovers:
            raise AutonomyPolicyError("max_provider_failovers exceeded")
        if provider is not None:
            provider = _text("provider", provider)
        if model is not None:
            model = _text("model", model)
        if invocation_kind is not None:
            invocation_kind = _identifier("invocation_kind", invocation_kind, maximum=128)
        if attempt is not None and (not isinstance(attempt, int) or isinstance(attempt, bool) or not 0 <= attempt <= 8):
            raise AutonomyPersistenceError("attempt is outside its bound")
        if turn is not None and (not isinstance(turn, int) or isinstance(turn, bool) or not 0 <= turn <= 32):
            raise AutonomyPersistenceError("turn is outside its bound")
        if selection_digest is not None:
            selection_digest = _digest("selection_digest", selection_digest)
        self._ensure_cost(cost_units)
        estimated_cost = None if estimated_cost_units is None else float(estimated_cost_units)
        if estimated_cost_units is not None:
            self._ensure_cost(estimated_cost_units)
        self.state = replace(self.state, step_index=self.state.step_index + 1, provider_calls=self.state.provider_calls + 1, provider_failovers=self.state.provider_failovers + int(failover), cost_units=self.state.cost_units + float(cost_units), last_event_kind="provider_call", status="running")
        fields: dict[str, Any] = {"cost_units": float(cost_units), "failover": failover}
        for name, value in (
            ("provider", provider),
            ("model", model),
            ("invocation_kind", invocation_kind),
            ("attempt", attempt),
            ("turn", turn),
            ("selection_digest", selection_digest),
            ("estimated_cost_units", estimated_cost),
        ):
            if value is not None:
                fields[name] = value
        return self._persist("provider_call", "running", **fields)

    def record_provider_outcome(
        self,
        *,
        provider: str,
        model: str,
        invocation_kind: str,
        attempt: int,
        turn: int,
        status: str,
        outcome: str,
        latency_ms: float,
        input_tokens: int,
        output_tokens: int,
        estimated_cost_units: float,
        actual_cost_units: float,
        selection_digest: str | None,
        outcome_digest: str,
        request_id_digest: str | None = None,
        failure_class: str | None = None,
        status_code: int | None = None,
        retryable: bool = False,
    ) -> AutonomousExecutionState:
        """Persist one bounded provider result without changing call-count admission."""

        self._ensure_active()
        provider = _text("provider", provider)
        model = _text("model", model)
        invocation_kind = _identifier("invocation_kind", invocation_kind, maximum=128)
        if not isinstance(attempt, int) or isinstance(attempt, bool) or not 0 <= attempt <= 8:
            raise AutonomyPersistenceError("attempt is outside its bound")
        if not isinstance(turn, int) or isinstance(turn, bool) or not 0 <= turn <= 32:
            raise AutonomyPersistenceError("turn is outside its bound")
        if outcome not in {"success", "failure"}:
            raise AutonomyPersistenceError("provider outcome must be success or failure")
        if status not in {"completed", "provider_refused"}:
            raise AutonomyPersistenceError("provider outcome status is unsupported")
        if not isinstance(retryable, bool):
            raise AutonomyPersistenceError("retryable must be a boolean")
        if not isinstance(input_tokens, int) or isinstance(input_tokens, bool) or not 0 <= input_tokens <= 100_000_000:
            raise AutonomyPersistenceError("input_tokens is outside its bound")
        if not isinstance(output_tokens, int) or isinstance(output_tokens, bool) or not 0 <= output_tokens <= 100_000_000:
            raise AutonomyPersistenceError("output_tokens is outside its bound")
        for name, value in (("latency_ms", latency_ms), ("estimated_cost_units", estimated_cost_units), ("actual_cost_units", actual_cost_units)):
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(float(value)) or value < 0:
                raise AutonomyPersistenceError(f"{name} must be finite and non-negative")
        if selection_digest is not None:
            selection_digest = _digest("selection_digest", selection_digest)
        outcome_digest = _digest("outcome_digest", outcome_digest)
        if request_id_digest is not None:
            request_id_digest = _digest("request_id_digest", request_id_digest)
        if failure_class is not None:
            failure_class = _identifier("failure_class", failure_class, maximum=512)
        if status_code is not None and (not isinstance(status_code, int) or isinstance(status_code, bool) or not 0 <= status_code <= 999):
            raise AutonomyPersistenceError("status_code is outside its bound")
        lifecycle_status = "error" if outcome == "failure" and not retryable and self.policy.stop_on_error else "running"
        self.state = replace(
            self.state,
            last_event_kind="provider_call",
            last_outcome_digest=outcome_digest,
            status=lifecycle_status,
        )
        return self._persist(
            "provider_call",
            status,
            provider=provider,
            model=model,
            invocation_kind=invocation_kind,
            attempt=attempt,
            turn=turn,
            provider_outcome=outcome,
            latency_ms=latency_ms,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            estimated_cost_units=estimated_cost_units,
            actual_cost_units=actual_cost_units,
            selection_digest=selection_digest,
            outcome_digest=outcome_digest,
            request_id_digest=request_id_digest,
            failure_class=failure_class,
            status_code=status_code,
            retryable=retryable,
        )

    def admit_tool_call(
        self,
        *,
        tool: str,
        call_id: str,
        read_only: bool,
        approval_required: bool,
        cost_units: float = 0.0,
    ) -> AutonomousExecutionState:
        self._ensure_active()
        self._ensure_step()
        if not isinstance(read_only, bool):
            raise AutonomyPersistenceError("read_only must be a boolean")
        if not isinstance(approval_required, bool):
            raise AutonomyPersistenceError("approval_required must be a boolean")
        tool = _identifier("tool", tool)
        call_id = _identifier("call_id", call_id, maximum=512)
        if self.state.tool_calls >= self.policy.max_tool_calls:
            raise AutonomyPolicyError("max_tool_calls exceeded")
        effectful = not read_only
        if effectful and not self.policy.allow_side_effects:
            raise AutonomyPolicyError("side effects are disabled by the execution policy")
        if effectful and self.state.effectful_calls >= self.policy.max_effectful_calls:
            raise AutonomyPolicyError("max_effectful_calls exceeded")
        self._ensure_cost(cost_units)
        self.state = replace(
            self.state,
            step_index=self.state.step_index + 1,
            tool_calls=self.state.tool_calls + 1,
            effectful_calls=self.state.effectful_calls + int(effectful),
            cost_units=self.state.cost_units + float(cost_units),
            last_tool=tool,
            last_call_id=call_id,
            last_event_kind="tool_intent",
            status="approval_required" if approval_required and self.policy.pause_on_approval else "running",
        )
        return self._persist(
            "tool_intent",
            self.state.status,
            tool=tool,
            call_id=call_id,
            read_only=read_only,
            approval_required=approval_required,
            cost_units=cost_units,
        )

    def record_tool_outcome(
        self,
        *,
        tool: str,
        call_id: str,
        status: str,
        outcome_digest: str | None = None,
        reason: str | None = None,
    ) -> AutonomousExecutionState:
        self._ensure_active()
        tool = _identifier("tool", tool)
        call_id = _identifier("call_id", call_id, maximum=512)
        outcome_status = _identifier("tool outcome status", status)
        outcome_digest = None if outcome_digest is None else _digest("outcome_digest", outcome_digest)
        reason = None if reason is None else _identifier("tool reason", reason, maximum=2_048)
        lifecycle_status = (
            "reconciliation_required"
            if outcome_status == "reconciliation_required"
            else "approval_required"
            if outcome_status == "authorization_required" and self.policy.pause_on_approval
            else "error"
            if outcome_status == "failed" and self.policy.stop_on_error
            else "running"
        )
        self.state = replace(self.state, last_tool=tool, last_call_id=call_id, last_outcome_digest=outcome_digest, last_event_kind="tool_outcome", status=lifecycle_status)
        return self._persist("tool_outcome", outcome_status, tool=tool, call_id=call_id, outcome_digest=outcome_digest, reason=reason)

    def record_effect_reconciliation(
        self,
        *,
        effect_id: str,
        tool: str,
        call_id: str,
        status: str,
        dispatch_attempt: int,
        result_digest: str | None = None,
        failure_class: str | None = None,
        reason: str | None = None,
    ) -> AutonomousExecutionState:
        """Persist effect uncertainty without retaining arguments or external results.

        ``uncertain`` is a recoverable execution boundary, not ordinary tool failure.  The
        controller therefore moves to ``reconciliation_required`` and remains resumable until a
        caller-owned resolver records ``reconciled`` or a definite ``failed`` outcome.
        """

        if (
            (self._terminal or self.state.status in AUTONOMY_TERMINAL_STATUSES)
            and self.state.status != "reconciliation_required"
        ):
            raise AutonomyPolicyError("execution cannot record an effect after terminal completion")
        effect_id = _identifier("effect_id", effect_id, maximum=128)
        tool = _identifier("effect tool", tool)
        call_id = _identifier("effect call_id", call_id, maximum=512)
        if status not in {"prepared", "dispatching", "dispatched", "completed", "uncertain", "reconciled", "failed"}:
            raise AutonomyPersistenceError("effect reconciliation status is unsupported")
        if not isinstance(dispatch_attempt, int) or isinstance(dispatch_attempt, bool) or not 0 <= dispatch_attempt <= 64:
            raise AutonomyPersistenceError("effect dispatch_attempt is outside its bound")
        if result_digest is not None:
            result_digest = _digest("effect result_digest", result_digest)
        if failure_class is not None:
            failure_class = _identifier("effect failure_class", failure_class, maximum=256)
        if reason is not None:
            reason = _identifier("effect reason", reason, maximum=2_048)
        lifecycle_status = (
            "reconciliation_required"
            if status == "uncertain"
            else "error"
            if status == "failed" and self.policy.stop_on_error
            else "running"
        )
        reconciliation_digest = content_digest(
            {
                "effect_id": effect_id,
                "status": status,
                "dispatch_attempt": dispatch_attempt,
                "result_digest": result_digest,
                "failure_class": failure_class,
                "reason": reason,
            }
        )
        self.state = replace(
            self.state,
            last_tool=tool,
            last_call_id=call_id,
            last_outcome_digest=result_digest,
            last_event_kind="effect_reconciliation",
            status=lifecycle_status,
        )
        if status in {"reconciled", "completed", "failed"}:
            self._terminal = False
        return self._persist(
            "effect_reconciliation",
            status,
            effect_id=effect_id,
            effect_status=status,
            tool=tool,
            call_id=call_id,
            dispatch_attempt=dispatch_attempt,
            reconciliation_digest=reconciliation_digest,
            outcome_digest=result_digest,
            failure_class=failure_class,
            reason=reason,
        )

    def record_evaluation(
        self,
        *,
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        evaluation_digest: str,
        failure_class: str | None = None,
    ) -> AutonomousExecutionState:
        self._ensure_active()
        evaluation_digest = _digest("evaluation_digest", evaluation_digest)
        if not isinstance(passed, bool):
            raise AutonomyPersistenceError("evaluation passed must be a boolean")
        if not isinstance(reward, (int, float)) or isinstance(reward, bool) or not math.isfinite(float(reward)) or not -1 <= float(reward) <= 1:
            raise AutonomyPersistenceError("evaluation reward must be finite and within [-1, 1]")
        self.state = replace(self.state, last_evaluation_digest=evaluation_digest, last_event_kind="evaluation", status="evaluated")
        return self._persist("evaluation", "evaluated", evaluator_id=evaluator_id, evaluator_version=evaluator_version, reward=reward, passed=passed, evaluation_digest=evaluation_digest, failure_class=failure_class)

    def replan(
        self,
        *,
        instruction_digest: str | None = None,
        reason: str | None = None,
        attempt: int | None = None,
    ) -> AutonomousExecutionState:
        """Record an evaluator-approved planning transition without persisting its instruction."""

        self._ensure_active()
        if self.state.replans >= self.policy.max_replans:
            raise AutonomyPolicyError("max_replans exceeded")
        if instruction_digest is not None:
            instruction_digest = _digest("replan instruction_digest", instruction_digest)
        if reason is not None:
            reason = _identifier("replan reason", reason, maximum=2_048)
        if attempt is not None and (not isinstance(attempt, int) or isinstance(attempt, bool) or not 0 <= attempt <= 64):
            raise AutonomyPersistenceError("replan attempt is outside its bound")
        self.state = replace(
            self.state,
            replans=self.state.replans + 1,
            last_event_kind="replan",
            status="replanning",
        )
        return self._persist(
            "replan",
            "replanning",
            instruction_digest=instruction_digest,
            reason=reason,
            attempt=attempt,
        )

    def checkpoint(self, *, status: str = "paused", reason: str | None = None) -> AutonomousExecutionState:
        self._ensure_active()
        status = _identifier("checkpoint status", status)
        if status in AUTONOMY_TERMINAL_STATUSES:
            raise AutonomyPolicyError("checkpoint status cannot be terminal")
        reason = None if reason is None else _identifier("checkpoint reason", reason, maximum=2_048)
        self.state = replace(self.state, status=status, last_event_kind="checkpoint")
        return self._persist("checkpoint", self.state.status, reason=reason)

    def complete(self, *, status: str = "completed") -> AutonomousExecutionState:
        self._ensure_active()
        self.state = replace(self.state, status=_identifier("completion status", status), last_event_kind="completed")
        self._persist("completed", self.state.status)
        self._terminal = True
        return self.state

    def fail(self, *, reason: str, status: str = "failed") -> AutonomousExecutionState:
        if self._terminal or self.state.status in AUTONOMY_TERMINAL_STATUSES:
            raise AutonomyPolicyError("execution is terminal")
        reason = _identifier("failure reason", reason, maximum=2_048)
        self.state = replace(self.state, status=_identifier("failure status", status), last_event_kind="failed")
        self._persist("failed", self.state.status, reason=reason)
        self._terminal = True
        return self.state

    def _persist(self, kind: str, status: str, **fields: Any) -> AutonomousExecutionState:
        event = {
            "execution_id": self.state.execution_id,
            "kind": kind,
            "domain": self.state.domain,
            "capability": self.state.capability,
            "risk_class": self.state.risk_class,
            "status": status,
            "policy_digest": self.state.policy_digest,
            "state": self.state.to_dict(),
            **fields,
        }
        if self.journal is not None:
            receipt = self.journal.append(event)
            self.state = replace(self.state, journal_sequence=receipt["sequence"], checkpoint_digest=receipt["event_digest"])
        return self.state

    def _ensure_active(self) -> None:
        if self._terminal or self.state.status in AUTONOMY_TERMINAL_STATUSES or (self.policy.stop_on_error and self.state.status == "error"):
            raise AutonomyPolicyError("execution is terminal or halted")

    def _ensure_step(self) -> None:
        if self.state.step_index >= self.policy.max_steps:
            raise AutonomyPolicyError("max_steps exceeded")

    def _ensure_cost(self, cost_units: float) -> None:
        if not isinstance(cost_units, (int, float)) or isinstance(cost_units, bool) or not math.isfinite(float(cost_units)) or cost_units < 0:
            raise AutonomyPolicyError("cost_units must be finite and non-negative")
        if self.state.cost_units + float(cost_units) > self.policy.max_cost_units:
            raise AutonomyPolicyError("max_cost_units exceeded")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMY_STATE_SCHEMA,
            "policy": self.policy.to_dict(),
            "state": self.state.to_dict(),
            "journal": None if self.journal is None else str(self.journal.path),
            "retention": "metadata_only",
        }


__all__ = [
    "AUTONOMY_EVENT_KINDS",
    "AUTONOMY_EVENT_SCHEMA",
    "AUTONOMY_EXECUTION_SNAPSHOT_SCHEMA",
    "AUTONOMY_JOURNAL_SCHEMA",
    "AUTONOMY_POLICY_SCHEMA",
    "AUTONOMY_STATE_SCHEMA",
    "MAX_AUTONOMY_PROVIDER_FAILOVERS",
    "MAX_AUTONOMY_JOURNAL_SNAPSHOT_BYTES",
    "AutonomousExecutionController",
    "AutonomousExecutionJournal",
    "AutonomousExecutionPersistenceCoordinator",
    "AutonomousExecutionPolicy",
    "AutonomousExecutionSnapshotTextStore",
    "AutonomousExecutionTransactionalSnapshotTextStore",
    "AutonomousExecutionState",
    "AutonomyPersistenceError",
    "AutonomyPolicyError",
    "JsonAutonomousExecutionSnapshotPersistence",
    "TransactionalJsonAutonomousExecutionSnapshotPersistence",
    "validate_autonomous_execution_snapshot",
]
