from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA,
    AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AutonomousDecisionCycle,
    AutonomousDecisionCyclePersistenceCoordinator,
    InMemoryAutonomousDecisionCycleStateStore,
    content_digest,
)
from prism_sdk.errors import ArgumentError


class _SnapshotStore:
    def __init__(self):
        self.snapshot = None

    def read(self):
        return self.snapshot

    def write(self, snapshot):
        self.snapshot = snapshot.to_dict()


def _digest(seed: str) -> str:
    return seed * 64


def test_decision_cycle_hash_chain_covers_all_phases_and_all_domains() -> None:
    store = InMemoryAutonomousDecisionCycleStateStore(max_states=len(AUTONOMOUS_DOMAINS) + 1)
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        cycle = AutonomousDecisionCycle(
            store,
            cycle_id=f"cycle-{domain}",
            task=f"bounded task for {domain}",
            mode="cross_domain",
            learning_enabled=True,
            evaluation_enabled=True,
            trajectory_id=f"trajectory-{domain}",
        )
        route = _digest("a")
        plan = _digest("b")
        selection = _digest("c")
        outcome = _digest("d")
        evaluation = _digest("e")
        settlement = _digest("f")
        assert cycle.state.schema == AUTONOMOUS_DECISION_CYCLE_STATE_SCHEMA
        assert cycle.state.phase == "route_pending"
        cycle.advance(phase="route_pending", route_digest=route)
        cycle.advance(phase="planning_pending", plan_refinement_digest=plan)
        cycle.advance(phase="execution_pending", selection_digest=selection)
        cycle.advance(phase="evaluation_pending", outcome_digest=outcome, learning_episode_ids=(f"episode-{domain}",))
        cycle.advance(phase="settlement_pending", evaluation_digest=evaluation)
        terminal = cycle.terminal("completed", settlement_digests=(settlement,))
        assert terminal.phase == "terminal"
        assert terminal.generation == 7
        assert terminal.previous_state_digest is not None
        assert cycle.context().to_dict()["secret_material"] == "never_returned"
        assert "bounded task" not in json.dumps(cycle.context().to_dict())
    assert len(store.snapshot().states) == len(AUTONOMOUS_DOMAINS)


def test_decision_cycle_snapshot_is_atomic_restart_safe_and_tamper_evident() -> None:
    store = InMemoryAutonomousDecisionCycleStateStore()
    cycle = AutonomousDecisionCycle(
        store,
        cycle_id="restart-cycle",
        task="restart-safe provider decision",
        mode="single_domain",
        evaluation_enabled=False,
    )
    cycle.advance(phase="route_pending", route_digest=_digest("a"))
    cycle.advance(phase="planning_pending", plan_refinement_digest=_digest("b"))
    snapshot = store.snapshot()
    assert snapshot.schema == AUTONOMOUS_DECISION_CYCLE_SNAPSHOT_SCHEMA
    assert "restart-safe provider decision" not in json.dumps(snapshot.to_dict())

    persistence = _SnapshotStore()
    coordinator = AutonomousDecisionCyclePersistenceCoordinator(store, persistence)
    flushed = coordinator.flush()
    assert flushed.snapshot_digest == persistence.snapshot["snapshot_digest"]
    restored_store = InMemoryAutonomousDecisionCycleStateStore()
    restored = AutonomousDecisionCyclePersistenceCoordinator(restored_store, persistence).restore()
    assert restored is not None
    assert restored_store.load("restart-cycle").state_digest == cycle.state.state_digest

    tampered = json.loads(json.dumps(persistence.snapshot))
    tampered["states"][0]["phase"] = "terminal"
    with pytest.raises(ArgumentError, match="snapshot digest|state digest|phase requires"):
        restored_store.restore(tampered)


def test_decision_cycle_rejects_private_shape_contract_drift_and_noncontiguous_writes() -> None:
    store = InMemoryAutonomousDecisionCycleStateStore()
    cycle = AutonomousDecisionCycle(store, cycle_id="contract-cycle", task="contract checks", mode="single_domain")
    state = cycle.state.to_dict()
    state["prompt"] = "must never persist"
    with pytest.raises(ArgumentError, match="unsupported or missing"):
        store.save(state)

    malformed = cycle.state.to_dict()
    malformed["generation"] = 3
    malformed["previous_state_digest"] = _digest("z")
    malformed["state_digest"] = ""
    with pytest.raises(ArgumentError, match="generation|digest"):
        store.save(malformed)

    next_state = cycle.advance(phase="route_pending", route_digest=_digest("a"))
    assert store.save(next_state) is None
    with pytest.raises(ArgumentError, match="does not match"):
        AutonomousDecisionCycle(store, cycle_id="contract-cycle", task="different task", mode="single_domain")


def test_decision_cycle_persistence_requires_complete_adapters_and_rehydration_context_is_value_only() -> None:
    store = InMemoryAutonomousDecisionCycleStateStore()
    with pytest.raises(ArgumentError, match="persistence"):
        AutonomousDecisionCyclePersistenceCoordinator(store, object())
    cycle = AutonomousDecisionCycle(store, cycle_id="context-cycle", task="rehydrate route", mode="single_domain")
    context = cycle.context().to_dict()
    assert context["phase"] == "route_pending"
    assert "rehydrate route" not in json.dumps(context)
    assert content_digest({"task": "rehydrate route"}) == context["task_digest"]
