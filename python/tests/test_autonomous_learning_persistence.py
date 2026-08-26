from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainLearningLedger,
    BrainLearningPersistenceCoordinator,
    LLMRuntime,
    TransactionalJsonBrainLearningSnapshotPersistence,
)
from prism_sdk.brain import BrainRunError


def _digest(value: object) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _report(index: int, domain: str) -> dict[str, object]:
    return {
        "learning_evidence": {
            "evidence_digest": f"{index:064x}",
            "evaluator_id": f"{domain}-quality",
            "evaluator_version": "test-v1",
            "arm_id": "offline/model",
            "reward": 0.5 + index / 100,
            "failed": index % 3 == 0,
        },
        "next_state": {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": index,
            "arms": [{
                "arm_id": "offline/model",
                "pulls": index,
                "reward_sum": 0.5 + index / 100,
            }],
        },
    }


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


def test_python_agent_learning_lifecycle_restarts_all_domains(tmp_path) -> None:
    backend = _CasTextStore()
    source = BrainLearningLedger(tmp_path / "source-learning.jsonl")
    source_persistence = BrainLearningPersistenceCoordinator(
        source,
        TransactionalJsonBrainLearningSnapshotPersistence(backend),
    )
    source_agent = AutonomousAgent(
        object(),
        LLMRuntime(),
        ledger=source,
        learning_persistence=source_persistence,
    )

    for index, domain in enumerate(AUTONOMOUS_DOMAINS, start=1):
        source.append(
            _report(index, domain),
            context_digest=_digest({"domain": domain, "index": index}),
            replay={
                "run_id": f"learning-run-{index}",
                "domain": domain,
                "capability": "model_selection",
            },
        )

    flushed = source_agent.flush_online_learning()
    assert flushed["snapshot_generation"] == 1
    assert flushed["previous_snapshot_digest"] is None
    assert len(flushed["records"]) == len(AUTONOMOUS_DOMAINS)
    assert "provider-secret" not in json.dumps(flushed)
    assert "inspect the private task" not in json.dumps(flushed)

    restored = BrainLearningLedger(tmp_path / "restored-learning.jsonl")
    restored_persistence = BrainLearningPersistenceCoordinator(
        restored,
        TransactionalJsonBrainLearningSnapshotPersistence(backend),
    )
    restored_agent = AutonomousAgent(
        tmp_path,
        LLMRuntime(),
        ledger=restored,
        learning_persistence=restored_persistence,
    )
    restored_snapshot = restored_agent.restore_learning()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert restored_agent.learning_state()["generation"] == len(AUTONOMOUS_DOMAINS)
    assert {
        row["domain"] for row in restored.replays(limit=128)
    } == set(AUTONOMOUS_DOMAINS)

    restored.append(
        _report(99, AUTONOMOUS_DOMAINS[0]),
        context_digest="f" * 64,
        replay={"run_id": "learning-run-99", "domain": AUTONOMOUS_DOMAINS[0]},
    )
    advanced = restored_agent.flush_learning()
    assert advanced["snapshot_generation"] == 2
    assert advanced["previous_snapshot_digest"] == flushed["snapshot_digest"]
    mismatched = BrainLearningLedger(tmp_path / "mismatched-learning.jsonl")
    with pytest.raises(BrainRunError, match="bound to the supplied ledger"):
        AutonomousAgent(
            object(),
            LLMRuntime(),
            ledger=mismatched,
            learning_persistence=source_persistence,
        )


def test_python_agent_learning_lifecycle_fails_closed_without_ledger_or_persistence(tmp_path) -> None:
    without_ledger = AutonomousAgent(object(), LLMRuntime())
    with pytest.raises(BrainRunError, match="has no learning ledger"):
        without_ledger.restore_learning()
    with pytest.raises(BrainRunError, match="has no learning ledger"):
        without_ledger.flush_online_learning()

    ledger = BrainLearningLedger(tmp_path / "unconfigured-learning.jsonl")
    agent = AutonomousAgent(object(), LLMRuntime(), ledger=ledger)
    with pytest.raises(BrainRunError, match="learning persistence is not configured"):
        agent.restore_learning()
    with pytest.raises(BrainRunError, match="learning persistence is not configured"):
        agent.flush_online_learning()


def test_python_agent_rejects_non_coordinator_learning_persistence(tmp_path) -> None:
    ledger = BrainLearningLedger(tmp_path / "learning.jsonl")
    with pytest.raises(BrainRunError, match="BrainLearningPersistenceCoordinator"):
        AutonomousAgent(
            object(),
            LLMRuntime(),
            ledger=ledger,
            learning_persistence=object(),
        )
