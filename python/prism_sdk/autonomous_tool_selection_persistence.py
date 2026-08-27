"""Restart-safe persistence for evaluator-approved adaptive tool selection.

The selector itself is intentionally provider-neutral and value-only.  This module adds the
missing application lifecycle boundary: a bounded, canonical, digest-chained snapshot that can
be restored before planning and flushed after evaluator settlement.  It never stores task text,
tool arguments or outputs, prompts, credentials, or evaluator prose.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import threading
from typing import Any, Callable, Mapping, Protocol

from .authoring import canonical_json, content_digest
from .errors import ArgumentError


AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-tool-selection-snapshot/0.1"
AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_RETENTION = "tool_selection_arm_and_evaluator_digest_metadata_only"
MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES = 1_000_000


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _positive_integer(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1 or value > 2_147_483_647:
        raise ArgumentError(f"{name} must be a positive bounded integer")
    return value


def _normalized_state(value: Mapping[str, Any] | None) -> dict[str, Any]:
    # Import lazily so autonomy.py can expose the persistence types without an import cycle.
    from .autonomy import normalize_autonomous_tool_selection_state

    return normalize_autonomous_tool_selection_state(value)


@dataclass(frozen=True, slots=True)
class AutonomousToolSelectionSnapshot:
    """Canonical restart image containing only arm statistics and settlement identities."""

    state: Mapping[str, Any]
    snapshot_generation: int = 1
    previous_snapshot_digest: str | None = None
    retention: str = AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_RETENTION
    secret_material: str = "never_returned"
    state_digest: str = ""
    snapshot_digest: str = ""

    def __post_init__(self) -> None:
        normalized = _normalized_state(self.state)
        object.__setattr__(self, "state", normalized)
        generation = _positive_integer("tool selection snapshot_generation", self.snapshot_generation)
        object.__setattr__(self, "snapshot_generation", generation)
        previous = None if self.previous_snapshot_digest is None else _digest(
            "tool selection previous_snapshot_digest", self.previous_snapshot_digest
        )
        object.__setattr__(self, "previous_snapshot_digest", previous)
        if generation == 1 and previous is not None or generation > 1 and previous is None:
            raise ArgumentError("tool selection snapshot generation chain is malformed")
        if self.retention != AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_RETENTION or self.secret_material != "never_returned":
            raise ArgumentError("tool selection snapshot retention markers are invalid")
        expected_state_digest = content_digest(normalized)
        if not isinstance(self.state_digest, str):
            raise ArgumentError("tool selection state_digest must be a lowercase SHA-256 digest")
        if self.state_digest:
            if _digest("tool selection state_digest", self.state_digest) != expected_state_digest:
                raise ArgumentError("tool selection state digest does not match its contents")
        else:
            object.__setattr__(self, "state_digest", expected_state_digest)
        expected_snapshot_digest = content_digest(self._descriptor())
        if not isinstance(self.snapshot_digest, str):
            raise ArgumentError("tool selection snapshot_digest must be a lowercase SHA-256 digest")
        if self.snapshot_digest:
            if _digest("tool selection snapshot_digest", self.snapshot_digest) != expected_snapshot_digest:
                raise ArgumentError("tool selection snapshot digest does not match its contents")
        else:
            object.__setattr__(self, "snapshot_digest", expected_snapshot_digest)
        if len(canonical_json(self.to_dict()).encode("utf-8")) > MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES:
            raise ArgumentError("tool selection snapshot exceeds its byte bound")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA,
            "snapshot_generation": self.snapshot_generation,
            "previous_snapshot_digest": self.previous_snapshot_digest,
            "state": dict(self.state),
            "state_digest": self.state_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "snapshot_digest": self.snapshot_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousToolSelectionSnapshot":
        if not isinstance(value, Mapping):
            raise ArgumentError("tool selection snapshot must be a mapping")
        expected = {
            "schema", "snapshot_generation", "previous_snapshot_digest", "state", "state_digest",
            "retention", "secret_material", "snapshot_digest",
        }
        if set(value) != expected or value.get("schema") != AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA:
            raise ArgumentError("tool selection snapshot fields are invalid")
        raw_state = value.get("state")
        if not isinstance(raw_state, Mapping):
            raise ArgumentError("tool selection snapshot state is malformed")
        return cls(
            state=raw_state,
            snapshot_generation=value.get("snapshot_generation"),
            previous_snapshot_digest=value.get("previous_snapshot_digest"),
            retention=value.get("retention"),
            secret_material=value.get("secret_material"),
            state_digest=value.get("state_digest"),
            snapshot_digest=value.get("snapshot_digest"),
        )


def snapshot_autonomous_tool_selection(
    state: Mapping[str, Any] | None,
    *,
    snapshot_generation: int = 1,
    previous_snapshot_digest: str | None = None,
) -> AutonomousToolSelectionSnapshot:
    return AutonomousToolSelectionSnapshot(
        state=_normalized_state(state),
        snapshot_generation=snapshot_generation,
        previous_snapshot_digest=previous_snapshot_digest,
    )


def validate_autonomous_tool_selection_snapshot(
    value: AutonomousToolSelectionSnapshot | Mapping[str, Any],
) -> AutonomousToolSelectionSnapshot:
    """Validate and normalize a restart image without mutating the live agent."""

    if isinstance(value, AutonomousToolSelectionSnapshot):
        return AutonomousToolSelectionSnapshot.from_dict(value.to_dict())
    return AutonomousToolSelectionSnapshot.from_dict(value)


class AutonomousToolSelectionSnapshotPersistence(Protocol):
    def read(self) -> AutonomousToolSelectionSnapshot | Mapping[str, Any] | None: ...
    def write(self, snapshot: AutonomousToolSelectionSnapshot | Mapping[str, Any]) -> None: ...


class AutonomousToolSelectionTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousToolSelectionTransactionalTextStore(AutonomousToolSelectionTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonAutonomousToolSelectionPersistence:
    """Canonical JSON persistence over a caller-owned text store."""

    def __init__(self, store: AutonomousToolSelectionTextStore, *, max_bytes: int = MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, method, None)) for method in ("read", "write")):
            raise ArgumentError("tool selection JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES:
            raise ArgumentError("tool selection JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> AutonomousToolSelectionSnapshot | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("tool selection JSON exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except json.JSONDecodeError as error:
            raise ArgumentError("tool selection JSON is invalid") from error
        if not isinstance(raw, Mapping) or canonical_json(raw) != encoded:
            raise ArgumentError("tool selection JSON is not canonical")
        return validate_autonomous_tool_selection_snapshot(raw)

    def write(self, snapshot: AutonomousToolSelectionSnapshot | Mapping[str, Any]) -> None:
        normalized = snapshot if isinstance(snapshot, AutonomousToolSelectionSnapshot) else AutonomousToolSelectionSnapshot.from_dict(snapshot)
        encoded = canonical_json(normalized.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("tool selection JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousToolSelectionPersistence(JsonAutonomousToolSelectionPersistence):
    """Canonical JSON persistence with compare-and-swap writer fencing."""

    def __init__(self, store: AutonomousToolSelectionTransactionalTextStore, *, max_bytes: int = MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("tool selection transactional persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: AutonomousToolSelectionSnapshot | Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest("tool selection expected_snapshot_digest", expected_snapshot_digest)
        normalized = snapshot if isinstance(snapshot, AutonomousToolSelectionSnapshot) else AutonomousToolSelectionSnapshot.from_dict(snapshot)
        encoded = canonical_json(normalized.to_dict())
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("tool selection JSON exceeds its byte bound")
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, encoded))


class AutonomousToolSelectionPersistenceCoordinator:
    """Serialize state restore/flush and keep a per-agent snapshot chain."""

    def __init__(
        self,
        get_state: Callable[[], Mapping[str, Any]],
        set_state: Callable[[Mapping[str, Any]], None],
        persistence: AutonomousToolSelectionSnapshotPersistence,
    ) -> None:
        if not callable(get_state) or not callable(set_state):
            raise ArgumentError("tool selection state binding is malformed")
        if not all(callable(getattr(persistence, method, None)) for method in ("read", "write")):
            raise ArgumentError("tool selection persistence adapter is malformed")
        self._get_state = get_state
        self._set_state = set_state
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._snapshot_generation = 0
        self._lock = threading.RLock()

    @property
    def state(self) -> dict[str, Any]:
        with self._lock:
            return _normalized_state(self._get_state())

    def restore(self) -> AutonomousToolSelectionSnapshot | None:
        with self._lock:
            raw = self.persistence.read()
            if raw is None:
                self._expected_snapshot_digest = None
                self._snapshot_generation = 0
                return None
            snapshot = validate_autonomous_tool_selection_snapshot(raw)
            self._set_state(snapshot.state)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            self._snapshot_generation = snapshot.snapshot_generation
            return snapshot

    def flush(self) -> AutonomousToolSelectionSnapshot:
        with self._lock:
            snapshot = snapshot_autonomous_tool_selection(
                self._get_state(),
                snapshot_generation=self._snapshot_generation + 1,
                previous_snapshot_digest=None if self._snapshot_generation == 0 else self._expected_snapshot_digest,
            )
            write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
            if callable(write_if_unchanged):
                if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                    raise ArgumentError("tool selection persistence compare-and-swap conflict")
            else:
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot.snapshot_digest
            self._snapshot_generation = snapshot.snapshot_generation
            return snapshot


__all__ = [
    "AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_RETENTION",
    "MAX_AUTONOMOUS_TOOL_SELECTION_SNAPSHOT_BYTES",
    "AutonomousToolSelectionSnapshot",
    "snapshot_autonomous_tool_selection",
    "validate_autonomous_tool_selection_snapshot",
    "AutonomousToolSelectionSnapshotPersistence",
    "AutonomousToolSelectionTextStore",
    "AutonomousToolSelectionTransactionalTextStore",
    "JsonAutonomousToolSelectionPersistence",
    "TransactionalJsonAutonomousToolSelectionPersistence",
    "AutonomousToolSelectionPersistenceCoordinator",
]
