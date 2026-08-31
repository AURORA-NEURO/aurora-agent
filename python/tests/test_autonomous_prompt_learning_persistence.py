from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousPromptLearningPersistenceCoordinator,
    AutonomousPromptLearningState,
    JsonAutonomousPromptLearningSnapshotPersistence,
    TransactionalJsonAutonomousPromptLearningSnapshotPersistence,
    builtin_autonomous_prompt_registry,
)
from prism_sdk.errors import ArgumentError


class _CasTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


def _requests() -> list[dict[str, object]]:
    return [
        {"domain": domain, "stage": "answer", "required_capabilities": ()}
        for domain in AUTONOMOUS_DOMAINS
    ]


def test_prompt_learning_persistence_recovers_all_domains_and_settles_idempotently() -> None:
    registry = builtin_autonomous_prompt_registry()
    store = _CasTextStore()
    persistence = TransactionalJsonAutonomousPromptLearningSnapshotPersistence(store)
    controller = AutonomousPromptLearningPersistenceCoordinator(registry, persistence=persistence)

    selection = controller.select(_requests())
    assert len(selection.arm_ids) == len(AUTONOMOUS_DOMAINS)
    settlement = controller.settle(
        selection,
        arm_id=selection.arm_ids[0],
        evaluator_id="workflow-evaluator",
        evaluator_version="1",
        reward=0.8,
        passed=True,
        outcome_digest="a" * 64,
    )
    assert settlement.status == "settled"
    assert controller.state.generation == 1
    assert store.value is not None
    assert "transient prompt" not in store.value
    assert "provider response" not in store.value

    recovered = AutonomousPromptLearningPersistenceCoordinator(registry, persistence=persistence)
    snapshot = recovered.restore()
    assert snapshot is not None
    assert snapshot.snapshot_generation == 1
    assert recovered.state.state_digest == controller.state.state_digest
    replay = recovered.settle(
        selection,
        arm_id=selection.arm_ids[0],
        evaluator_id="workflow-evaluator",
        evaluator_version="1",
        reward=0.8,
        passed=True,
        outcome_digest="a" * 64,
    )
    assert replay.status == "replayed"
    assert recovered.state.generation == 1


def test_prompt_learning_persistence_fences_stale_writers_and_registry_replacement() -> None:
    registry = builtin_autonomous_prompt_registry()
    store = _CasTextStore()
    persistence = TransactionalJsonAutonomousPromptLearningSnapshotPersistence(store)
    first = AutonomousPromptLearningPersistenceCoordinator(registry, persistence=persistence)
    first.flush()
    stale = AutonomousPromptLearningPersistenceCoordinator(registry, persistence=persistence)
    stale.restore()
    selection = first.select(_requests())
    first.settle(
        selection,
        arm_id=selection.arm_ids[0],
        evaluator_id="evaluator",
        evaluator_version="1",
        reward=0.2,
        passed=True,
        outcome_digest="b" * 64,
    )
    stale_selection = stale.select(_requests())
    with pytest.raises(ArgumentError, match="compare-and-swap"):
        stale.settle(
            stale_selection,
            arm_id=stale_selection.arm_ids[0],
            evaluator_id="evaluator",
            evaluator_version="1",
            reward=0.2,
            passed=True,
            outcome_digest="c" * 64,
        )

    replacement = AutonomousPromptLearningPersistenceCoordinator(
        builtin_autonomous_prompt_registry(("coding",)),
        persistence=JsonAutonomousPromptLearningSnapshotPersistence(store),
    )
    with pytest.raises(ArgumentError, match="stale"):
        replacement.restore()


def test_prompt_learning_persistence_rejects_tampering_and_requires_cas_store() -> None:
    registry = builtin_autonomous_prompt_registry()
    store = _CasTextStore()
    persistence = JsonAutonomousPromptLearningSnapshotPersistence(store)
    controller = AutonomousPromptLearningPersistenceCoordinator(registry, persistence=persistence)
    snapshot = controller.flush()
    payload = snapshot.to_dict()
    payload["snapshot_digest"] = "f" * 64
    store.value = json.dumps(payload, separators=(",", ":"))
    with pytest.raises(ArgumentError, match="digest"):
        persistence.read()

    with pytest.raises(ArgumentError, match="text store"):
        TransactionalJsonAutonomousPromptLearningSnapshotPersistence(object())  # type: ignore[arg-type]
