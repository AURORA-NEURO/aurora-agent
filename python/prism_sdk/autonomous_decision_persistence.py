"""Restart-safe metadata for provider-assisted autonomous decision cycles.

The high-level brain deliberately keeps task text, prompts, provider responses, credentials,
and tool arguments outside durable state.  This module persists only the joins required to
resume a caller-owned cycle: routing, plan refinement, selection, outcome, evaluation, learning
episodes, and settlement identities.  It is the Python counterpart of the TypeScript
``autonomous-decision-persistence`` contract and is intentionally usable without an API key or
network transport.

Applications can use :class:`AutonomousDecisionCycle` around ``run_auto`` or around a custom
orchestrator.  The state store is replaceable; the in-memory implementation exists for tests and
local development, while production callers should implement ``load``/``save``/``snapshot``/
``restore`` transactionally and pair it with :class:`AutonomousDecisionCyclePersistenceCoordinator`.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import re
import threading
from typing import Any, Mapping, Protocol, Sequence

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA = "bioprism-python-autonomous-decision-cycle-state/0.2"
AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-decision-cycle-snapshot/0.2"
MAX_AUTONOMOUS_DECISION_CYCLE_STATES = 8_192
MAX_AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_BYTES = 8_000_000
MAX_AUTONOMOUS_DECISION_CYCLE_METADATA_BYTES = 1_000_000
MAX_AUTONOMOUS_DECISION_CYCLE_LIST_ITEMS = 256

AUTONOMOUS_DECISION_CYCLE_MODES = ("single_domain", "cross_domain")
AUTONOMOUS_DECISION_CYCLE_PHASES = (
    "route_pending",
    "planning_pending",
    "execution_pending",
    "evaluation_pending",
    "settlement_pending",
    "terminal",
)

_STATE_KEYS = frozenset({
    "schema", "cycle_id", "task_digest", "mode", "learning_enabled", "evaluation_enabled", "phase",
    "route_digest", "plan_refinement_digest", "selection_digest", "outcome_digest", "evaluation_digest",
    "learning_episode_ids", "trajectory_id", "settlement_digests", "terminal_status", "generation",
    "previous_state_digest", "state_digest", "retention", "secret_material",
})
_SNAPSHOT_KEYS = frozenset({"schema", "states", "retention", "secret_material", "snapshot_digest"})
_PRIVATE_SHAPE = re.compile(
    r"(?:api[_-]?key|authorization|bearer|password|private[_-]?key|access[_-]?token|"
    r"refresh[_-]?token|credential|prompt|response|arguments?|output|payload|transcript|"
    r"secret)(?!_material)",
    re.IGNORECASE,
)
_UNSET = object()


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > 256 or "\x00" in value:
        raise ArgumentError(f"{name} must be a bounded identifier")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_sequence(name: str, value: Any, *, maximum: int) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its capacity")
    return tuple(value)


def _identifier_list(name: str, value: Any) -> tuple[str, ...]:
    values = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(_bounded_sequence(name, value, maximum=MAX_AUTONOMOUS_DECISION_CYCLE_LIST_ITEMS)))
    if len(set(values)) != len(values):
        raise ArgumentError(f"{name} must not contain duplicates")
    return values


def _digest_list(name: str, value: Any) -> tuple[str, ...]:
    values = tuple(_digest(f"{name}[{index}]", item) for index, item in enumerate(_bounded_sequence(name, value, maximum=MAX_AUTONOMOUS_DECISION_CYCLE_LIST_ITEMS)))
    if len(set(values)) != len(values):
        raise ArgumentError(f"{name} must not contain duplicates")
    return values  # type: ignore[return-value]


def _private_shape_free(value: Mapping[str, Any], name: str) -> None:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    marker_free = encoded.replace("metadata_only_hash_chained_no_private_payloads", "").replace("metadata_only_hash_bound", "").replace("never_returned", "")
    if _PRIVATE_SHAPE.search(marker_free):
        raise ArgumentError(f"{name} contains private or payload-shaped material")


def _exact_keys(name: str, value: Mapping[str, Any], expected: frozenset[str]) -> None:
    if set(value) != expected:
        raise ArgumentError(f"{name} contains unsupported or missing fields")


@dataclass(frozen=True, slots=True)
class AutonomousDecisionCycleState:
    """One hash-chained, value-only state for a single autonomous decision cycle."""

    schema: str
    cycle_id: str
    task_digest: str
    mode: str
    learning_enabled: bool
    evaluation_enabled: bool
    phase: str
    route_digest: str | None
    plan_refinement_digest: str | None
    selection_digest: str | None
    outcome_digest: str | None
    evaluation_digest: str | None
    learning_episode_ids: tuple[str, ...]
    trajectory_id: str | None
    settlement_digests: tuple[str, ...]
    terminal_status: str | None
    generation: int
    previous_state_digest: str | None
    retention: str
    secret_material: str
    state_digest: str = ""

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA:
            raise ArgumentError("autonomous decision-cycle state schema is invalid")
        if self.retention != "metadata_only_hash_chained_no_private_payloads" or self.secret_material != "never_returned":
            raise ArgumentError("autonomous decision-cycle state retention markers are invalid")
        cycle_id = _identifier("autonomous decision-cycle state cycle_id", self.cycle_id)
        task_digest = _digest("autonomous decision-cycle state task_digest", self.task_digest)
        if self.mode not in AUTONOMOUS_DECISION_CYCLE_MODES:
            raise ArgumentError("autonomous decision-cycle state mode is invalid")
        if not isinstance(self.learning_enabled, bool) or not isinstance(self.evaluation_enabled, bool):
            raise ArgumentError("autonomous decision-cycle learning flags are invalid")
        if self.phase not in AUTONOMOUS_DECISION_CYCLE_PHASES:
            raise ArgumentError("autonomous decision-cycle state phase is invalid")
        route_digest = _digest("autonomous decision-cycle state route_digest", self.route_digest, allow_none=True)
        plan_refinement_digest = _digest("autonomous decision-cycle state plan_refinement_digest", self.plan_refinement_digest, allow_none=True)
        selection_digest = _digest("autonomous decision-cycle state selection_digest", self.selection_digest, allow_none=True)
        outcome_digest = _digest("autonomous decision-cycle state outcome_digest", self.outcome_digest, allow_none=True)
        evaluation_digest = _digest("autonomous decision-cycle state evaluation_digest", self.evaluation_digest, allow_none=True)
        learning_episode_ids = _identifier_list("autonomous decision-cycle state learning_episode_ids", self.learning_episode_ids)
        trajectory_id = None if self.trajectory_id is None else _identifier("autonomous decision-cycle state trajectory_id", self.trajectory_id)
        settlement_digests = _digest_list("autonomous decision-cycle state settlement_digests", self.settlement_digests)
        terminal_status = None if self.terminal_status is None else _identifier("autonomous decision-cycle state terminal_status", self.terminal_status)
        if isinstance(self.generation, bool) or not isinstance(self.generation, int) or not 1 <= self.generation <= 9_007_199_254_740_991:
            raise ArgumentError("autonomous decision-cycle state generation is outside its bound")
        previous_state_digest = _digest("autonomous decision-cycle state previous_state_digest", self.previous_state_digest, allow_none=True)
        state_digest = None if self.state_digest == "" else _digest("autonomous decision-cycle state state_digest", self.state_digest)
        if (self.generation == 1 and previous_state_digest is not None) or (self.generation > 1 and previous_state_digest is None):
            raise ArgumentError("autonomous decision-cycle state hash chain is malformed")
        if self.phase == "route_pending" and route_digest is not None and (
            plan_refinement_digest is not None or selection_digest is not None or outcome_digest is not None
            or evaluation_digest is not None or learning_episode_ids or settlement_digests or terminal_status is not None
        ):
            raise ArgumentError("route-pending decision state cannot contain later-cycle metadata")
        if self.phase != "route_pending" and route_digest is None:
            raise ArgumentError("decision-cycle state phase requires a route digest")
        if self.phase == "planning_pending" and (
            selection_digest is not None or outcome_digest is not None or evaluation_digest is not None
            or learning_episode_ids or settlement_digests or terminal_status is not None
        ):
            raise ArgumentError("planning-pending decision state cannot contain execution metadata")
        if self.phase in {"evaluation_pending", "settlement_pending", "terminal"} and outcome_digest is None:
            raise ArgumentError("decision-cycle state phase requires an outcome digest")
        if self.phase == "settlement_pending" and (not self.evaluation_enabled or evaluation_digest is None):
            raise ArgumentError("settlement-pending decision state requires an evaluation digest")
        if not self.evaluation_enabled and evaluation_digest is not None:
            raise ArgumentError("decision-cycle state cannot retain evaluation when disabled")
        if self.phase == "terminal" and terminal_status is None:
            raise ArgumentError("terminal decision-cycle state requires a terminal status")
        if self.phase != "terminal" and terminal_status is not None:
            raise ArgumentError("non-terminal decision-cycle state cannot contain a terminal status")
        if self.mode == "single_domain" and trajectory_id is not None:
            raise ArgumentError("single-domain decision state cannot contain a trajectory ID")
        if self.mode == "cross_domain" and self.learning_enabled and trajectory_id is None:
            raise ArgumentError("cross-domain learning state requires a trajectory ID")
        if self.evaluation_enabled and terminal_status == "completed" and learning_episode_ids and not settlement_digests:
            raise ArgumentError("completed evaluated decision state requires a settlement digest")
        object.__setattr__(self, "cycle_id", cycle_id)
        object.__setattr__(self, "task_digest", task_digest)
        object.__setattr__(self, "route_digest", route_digest)
        object.__setattr__(self, "plan_refinement_digest", plan_refinement_digest)
        object.__setattr__(self, "selection_digest", selection_digest)
        object.__setattr__(self, "outcome_digest", outcome_digest)
        object.__setattr__(self, "evaluation_digest", evaluation_digest)
        object.__setattr__(self, "learning_episode_ids", learning_episode_ids)
        object.__setattr__(self, "trajectory_id", trajectory_id)
        object.__setattr__(self, "settlement_digests", settlement_digests)
        object.__setattr__(self, "terminal_status", terminal_status)
        object.__setattr__(self, "previous_state_digest", previous_state_digest)
        descriptor = self._descriptor()
        if len(json.dumps(descriptor, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_DECISION_CYCLE_METADATA_BYTES:
            raise ArgumentError("autonomous decision-cycle state exceeds its metadata budget")
        _private_shape_free(descriptor, "autonomous decision-cycle state")
        computed = content_digest(descriptor)
        if state_digest is not None and state_digest != computed:
            raise ArgumentError("autonomous decision-cycle state digest does not match metadata")
        object.__setattr__(self, "state_digest", computed)

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "cycle_id": self.cycle_id,
            "task_digest": self.task_digest,
            "mode": self.mode,
            "learning_enabled": self.learning_enabled,
            "evaluation_enabled": self.evaluation_enabled,
            "phase": self.phase,
            "route_digest": self.route_digest,
            "plan_refinement_digest": self.plan_refinement_digest,
            "selection_digest": self.selection_digest,
            "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest,
            "learning_episode_ids": list(self.learning_episode_ids),
            "trajectory_id": self.trajectory_id,
            "settlement_digests": list(self.settlement_digests),
            "terminal_status": self.terminal_status,
            "generation": self.generation,
            "previous_state_digest": self.previous_state_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "state_digest": self.state_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousDecisionCycleState":
        if not isinstance(value, Mapping):
            raise ArgumentError("autonomous decision-cycle state must be a mapping")
        _exact_keys("autonomous decision-cycle state", value, _STATE_KEYS)
        return cls(
            schema=value.get("schema"), cycle_id=value.get("cycle_id"), task_digest=value.get("task_digest"), mode=value.get("mode"),
            learning_enabled=value.get("learning_enabled"), evaluation_enabled=value.get("evaluation_enabled"), phase=value.get("phase"),
            route_digest=value.get("route_digest"), plan_refinement_digest=value.get("plan_refinement_digest"), selection_digest=value.get("selection_digest"),
            outcome_digest=value.get("outcome_digest"), evaluation_digest=value.get("evaluation_digest"), learning_episode_ids=tuple(value.get("learning_episode_ids")) if isinstance(value.get("learning_episode_ids"), Sequence) and not isinstance(value.get("learning_episode_ids"), (str, bytes)) else value.get("learning_episode_ids"),
            trajectory_id=value.get("trajectory_id"), settlement_digests=tuple(value.get("settlement_digests")) if isinstance(value.get("settlement_digests"), Sequence) and not isinstance(value.get("settlement_digests"), (str, bytes)) else value.get("settlement_digests"),
            terminal_status=value.get("terminal_status"), generation=value.get("generation"), previous_state_digest=value.get("previous_state_digest"),
            retention=value.get("retention"), secret_material=value.get("secret_material"), state_digest=value.get("state_digest"),
        )


def seal_autonomous_decision_cycle_state(value: Mapping[str, Any]) -> AutonomousDecisionCycleState:
    """Normalize and seal a state descriptor with its canonical metadata digest."""

    if not isinstance(value, Mapping):
        raise ArgumentError("autonomous decision-cycle state descriptor must be a mapping")
    payload = dict(value)
    payload.setdefault("schema", AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA)
    payload.setdefault("retention", "metadata_only_hash_chained_no_private_payloads")
    payload.setdefault("secret_material", "never_returned")
    payload.setdefault("state_digest", "")
    return AutonomousDecisionCycleState.from_mapping(payload)


def validate_autonomous_decision_cycle_state(value: Mapping[str, Any]) -> AutonomousDecisionCycleState:
    return AutonomousDecisionCycleState.from_mapping(value)


@dataclass(frozen=True, slots=True)
class AutonomousDecisionCycleSnapshot:
    """Atomic, digest-bound image of one latest state per cycle."""

    schema: str
    states: tuple[AutonomousDecisionCycleState, ...]
    retention: str
    secret_material: str
    snapshot_digest: str = ""

    def __post_init__(self) -> None:
        if self.schema != AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA or self.retention != "metadata_only_hash_bound" or self.secret_material != "never_returned":
            raise ArgumentError("autonomous decision-cycle snapshot markers are invalid")
        states = tuple(self.states)
        if len(states) > MAX_AUTONOMOUS_DECISION_CYCLE_STATES or any(not isinstance(state, AutonomousDecisionCycleState) for state in states):
            raise ArgumentError("autonomous decision-cycle snapshot exceeds its state capacity")
        if len({state.cycle_id for state in states}) != len(states):
            raise ArgumentError("autonomous decision-cycle snapshot contains duplicate cycle IDs")
        object.__setattr__(self, "states", states)
        descriptor = {"schema": self.schema, "states": [state.to_dict() for state in states], "retention": self.retention, "secret_material": self.secret_material}
        computed = content_digest(descriptor)
        supplied = None if self.snapshot_digest == "" else _digest("autonomous decision-cycle snapshot snapshot_digest", self.snapshot_digest)
        if supplied is not None and supplied != computed:
            raise ArgumentError("autonomous decision-cycle snapshot digest does not match metadata")
        normalized = {**descriptor, "snapshot_digest": computed}
        if len(json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_BYTES:
            raise ArgumentError("autonomous decision-cycle snapshot exceeds its byte capacity")
        object.__setattr__(self, "snapshot_digest", computed)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "states": [state.to_dict() for state in self.states],
            "retention": self.retention,
            "secret_material": self.secret_material,
            "snapshot_digest": self.snapshot_digest,
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousDecisionCycleSnapshot":
        if not isinstance(value, Mapping):
            raise ArgumentError("autonomous decision-cycle snapshot must be a mapping")
        _exact_keys("autonomous decision-cycle snapshot", value, _SNAPSHOT_KEYS)
        raw_states = value.get("states")
        if not isinstance(raw_states, Sequence) or isinstance(raw_states, (str, bytes)):
            raise ArgumentError("autonomous decision-cycle snapshot states must be a sequence")
        if len(raw_states) > MAX_AUTONOMOUS_DECISION_CYCLE_STATES:
            raise ArgumentError("autonomous decision-cycle snapshot exceeds its state capacity")
        return cls(
            schema=value.get("schema"),
            states=tuple(AutonomousDecisionCycleState.from_mapping(raw) if isinstance(raw, Mapping) else raw for raw in raw_states),
            retention=value.get("retention"),
            secret_material=value.get("secret_material"),
            snapshot_digest=value.get("snapshot_digest"),
        )


class AutonomousDecisionCycleStateStore(Protocol):
    def load(self, cycle_id: str) -> AutonomousDecisionCycleState | None: ...
    def save(self, state: AutonomousDecisionCycleState) -> None: ...
    def snapshot(self) -> AutonomousDecisionCycleSnapshot: ...
    def restore(self, snapshot: AutonomousDecisionCycleSnapshot | Mapping[str, Any]) -> None: ...


class AutonomousDecisionCycleSnapshotPersistence(Protocol):
    def read(self) -> AutonomousDecisionCycleSnapshot | Mapping[str, Any] | None: ...
    def write(self, snapshot: AutonomousDecisionCycleSnapshot) -> None: ...


class InMemoryAutonomousDecisionCycleStateStore:
    """Thread-safe reference store; production callers should provide durable transactions."""

    def __init__(self, *, max_states: int = MAX_AUTONOMOUS_DECISION_CYCLE_STATES) -> None:
        if isinstance(max_states, bool) or not isinstance(max_states, int) or not 1 <= max_states <= MAX_AUTONOMOUS_DECISION_CYCLE_STATES:
            raise ArgumentError("autonomous decision-cycle state store max_states is outside its bound")
        self.max_states = max_states
        self._states: dict[str, AutonomousDecisionCycleState] = {}
        self._lock = threading.RLock()

    def load(self, cycle_id: str) -> AutonomousDecisionCycleState | None:
        cycle_id = _identifier("autonomous decision-cycle cycle_id", cycle_id)
        with self._lock:
            return self._states.get(cycle_id)

    def save(self, state: AutonomousDecisionCycleState | Mapping[str, Any]) -> None:
        normalized = state if isinstance(state, AutonomousDecisionCycleState) else validate_autonomous_decision_cycle_state(state)
        with self._lock:
            prior = self._states.get(normalized.cycle_id)
            if prior is not None and prior.state_digest == normalized.state_digest:
                return
            if prior is None and (normalized.generation != 1 or normalized.previous_state_digest is not None):
                raise ArgumentError("autonomous decision-cycle initial state must start at generation one")
            if prior is not None and (normalized.generation != prior.generation + 1 or normalized.previous_state_digest != prior.state_digest):
                raise ArgumentError("autonomous decision-cycle state generation chain is not contiguous")
            if prior is None and len(self._states) >= self.max_states:
                raise ArgumentError("autonomous decision-cycle state store is full")
            self._states[normalized.cycle_id] = normalized

    def snapshot(self) -> AutonomousDecisionCycleSnapshot:
        with self._lock:
            states = tuple(self._states[key] for key in sorted(self._states))
        return AutonomousDecisionCycleSnapshot(AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA, states, "metadata_only_hash_bound", "never_returned")

    def restore(self, snapshot: AutonomousDecisionCycleSnapshot | Mapping[str, Any]) -> None:
        normalized = snapshot if isinstance(snapshot, AutonomousDecisionCycleSnapshot) else AutonomousDecisionCycleSnapshot.from_mapping(snapshot)
        if len(normalized.states) > self.max_states:
            raise ArgumentError("autonomous decision-cycle snapshot exceeds max_states")
        restored = {state.cycle_id: state for state in normalized.states}
        with self._lock:
            self._states = restored


class AutonomousDecisionCyclePersistenceCoordinator:
    """Flush and restore verified decision-cycle snapshots through caller-owned storage."""

    def __init__(self, store: AutonomousDecisionCycleStateStore, persistence: AutonomousDecisionCycleSnapshotPersistence) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("load", "save", "snapshot", "restore")):
            raise ArgumentError("decision-cycle persistence requires a complete state store")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("decision-cycle persistence adapter is malformed")
        self.store = store
        self.persistence = persistence

    def flush(self) -> AutonomousDecisionCycleSnapshot:
        snapshot = self.store.snapshot()
        self.persistence.write(snapshot)
        return snapshot

    def restore(self) -> AutonomousDecisionCycleSnapshot | None:
        raw = self.persistence.read()
        if raw is None:
            return None
        snapshot = raw if isinstance(raw, AutonomousDecisionCycleSnapshot) else AutonomousDecisionCycleSnapshot.from_mapping(raw)
        self.store.restore(snapshot)
        return snapshot


@dataclass(frozen=True, slots=True)
class AutonomousDecisionCycleRehydrationContext:
    """Value-only callback context for restoring private route/run/evaluation state."""

    cycle_id: str
    task_digest: str
    mode: str
    learning_enabled: bool
    evaluation_enabled: bool
    phase: str
    route_digest: str | None
    plan_refinement_digest: str | None
    selection_digest: str | None
    outcome_digest: str | None
    evaluation_digest: str | None
    learning_episode_ids: tuple[str, ...]
    trajectory_id: str | None
    settlement_digests: tuple[str, ...]
    terminal_status: str | None
    generation: int
    state_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "cycle_id": self.cycle_id, "task_digest": self.task_digest, "mode": self.mode,
            "learning_enabled": self.learning_enabled, "evaluation_enabled": self.evaluation_enabled,
            "phase": self.phase, "route_digest": self.route_digest, "plan_refinement_digest": self.plan_refinement_digest,
            "selection_digest": self.selection_digest, "outcome_digest": self.outcome_digest,
            "evaluation_digest": self.evaluation_digest, "learning_episode_ids": list(self.learning_episode_ids),
            "trajectory_id": self.trajectory_id, "settlement_digests": list(self.settlement_digests),
            "terminal_status": self.terminal_status, "generation": self.generation, "state_digest": self.state_digest,
            "retention": "metadata_only_hash_chained_no_private_payloads", "secret_material": "never_returned",
        }


class AutonomousDecisionCycle:
    """Open and advance one persisted route/plan/execute/evaluate/settle cycle."""

    def __init__(
        self,
        store: AutonomousDecisionCycleStateStore,
        *,
        cycle_id: str,
        task: str,
        mode: str,
        learning_enabled: bool = False,
        evaluation_enabled: bool = False,
        trajectory_id: str | None = None,
    ) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("load", "save")):
            raise ArgumentError("autonomous decision cycle requires a load/save state store")
        if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > 16_000:
            raise ArgumentError("autonomous decision-cycle task must be bounded non-empty text")
        cycle_id = _identifier("autonomous decision cycle_id", cycle_id)
        if mode not in AUTONOMOUS_DECISION_CYCLE_MODES:
            raise ArgumentError("autonomous decision-cycle mode is invalid")
        if not isinstance(learning_enabled, bool) or not isinstance(evaluation_enabled, bool):
            raise ArgumentError("autonomous decision-cycle learning flags are invalid")
        task_digest = content_digest({"task": task})
        loaded = store.load(cycle_id)
        if loaded is not None:
            if not isinstance(loaded, AutonomousDecisionCycleState):
                loaded = validate_autonomous_decision_cycle_state(loaded)
            if (loaded.task_digest != task_digest or loaded.mode != mode or loaded.learning_enabled != learning_enabled
                    or loaded.evaluation_enabled != evaluation_enabled or loaded.trajectory_id != trajectory_id):
                raise ArgumentError("persisted decision-cycle state does not match the requested contract")
            self.restored = True
            self.state = loaded
        else:
            self.restored = False
            self.state = seal_autonomous_decision_cycle_state({
                "schema": AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA,
                "cycle_id": cycle_id,
                "task_digest": task_digest,
                "mode": mode,
                "learning_enabled": learning_enabled,
                "evaluation_enabled": evaluation_enabled,
                "phase": "route_pending",
                "route_digest": None,
                "plan_refinement_digest": None,
                "selection_digest": None,
                "outcome_digest": None,
                "evaluation_digest": None,
                "learning_episode_ids": [],
                "trajectory_id": trajectory_id,
                "settlement_digests": [],
                "terminal_status": None,
                "generation": 1,
                "previous_state_digest": None,
                "retention": "metadata_only_hash_chained_no_private_payloads",
                "secret_material": "never_returned",
                "state_digest": "",
            })
            store.save(self.state)
        self.store = store
        self.cycle_id = cycle_id
        self.task_digest = task_digest
        self.mode = mode
        self.learning_enabled = learning_enabled
        self.evaluation_enabled = evaluation_enabled
        self.trajectory_id = trajectory_id

    def advance(self, *, phase: str, **changes: Any) -> AutonomousDecisionCycleState:
        """Commit one contiguous transition; omitted fields retain their prior value."""

        if phase not in AUTONOMOUS_DECISION_CYCLE_PHASES:
            raise ArgumentError("autonomous decision-cycle phase is invalid")
        allowed = {
            "route_digest", "plan_refinement_digest", "selection_digest", "outcome_digest", "evaluation_digest",
            "learning_episode_ids", "trajectory_id", "settlement_digests", "terminal_status",
        }
        unknown = set(changes).difference(allowed)
        if unknown:
            raise ArgumentError("unsupported decision-cycle transition fields: " + ", ".join(sorted(unknown)))
        payload = self.state.to_dict()
        payload.update(changes)
        payload.update({
            "phase": phase,
            "generation": self.state.generation + 1,
            "previous_state_digest": self.state.state_digest,
            "state_digest": "",
        })
        next_state = seal_autonomous_decision_cycle_state(payload)
        self.store.save(next_state)
        self.state = next_state
        return next_state

    def context(self) -> AutonomousDecisionCycleRehydrationContext:
        state = self.state
        return AutonomousDecisionCycleRehydrationContext(
            cycle_id=self.cycle_id, task_digest=self.task_digest, mode=self.mode,
            learning_enabled=self.learning_enabled, evaluation_enabled=self.evaluation_enabled,
            phase=state.phase, route_digest=state.route_digest, plan_refinement_digest=state.plan_refinement_digest,
            selection_digest=state.selection_digest, outcome_digest=state.outcome_digest,
            evaluation_digest=state.evaluation_digest, learning_episode_ids=state.learning_episode_ids,
            trajectory_id=state.trajectory_id, settlement_digests=state.settlement_digests,
            terminal_status=state.terminal_status, generation=state.generation, state_digest=state.state_digest,
        )

    def terminal(self, status: str, *, outcome_digest: str | None = None, settlement_digests: Sequence[str] | None = None) -> AutonomousDecisionCycleState:
        return self.advance(
            phase="terminal",
            outcome_digest=self.state.outcome_digest if outcome_digest is None else outcome_digest,
            settlement_digests=self.state.settlement_digests if settlement_digests is None else tuple(settlement_digests),
            terminal_status=status,
        )


__all__ = [
    "AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA",
    "AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA",
    "MAX_AUTONOMOUS_DECISION_CYCLE_STATES",
    "MAX_AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_DECISION_CYCLE_METADATA_BYTES",
    "MAX_AUTONOMOUS_DECISION_CYCLE_LIST_ITEMS",
    "AUTONOMOUS_DECISION_CYCLE_MODES",
    "AUTONOMOUS_DECISION_CYCLE_PHASES",
    "AutonomousDecisionCycleState",
    "AutonomousDecisionCycleSnapshot",
    "AutonomousDecisionCycleStateStore",
    "AutonomousDecisionCycleSnapshotPersistence",
    "AutonomousDecisionCyclePersistenceCoordinator",
    "AutonomousDecisionCycleRehydrationContext",
    "AutonomousDecisionCycle",
    "InMemoryAutonomousDecisionCycleStateStore",
    "seal_autonomous_decision_cycle_state",
    "validate_autonomous_decision_cycle_state",
]
