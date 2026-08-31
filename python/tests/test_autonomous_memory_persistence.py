import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainEpisodicMemory,
    BrainMemoryPersistenceCoordinator,
)
from prism_sdk.brain import BrainRunError
from prism_sdk.llm_runtime import LLMRuntime


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _episode(*, episode_id: str, domain: str, task: str) -> dict[str, object]:
    return {
        "episode_id": episode_id,
        "run_id": f"{episode_id}-run",
        "result_kind": "mission",
        "status": "completed",
        "task_digest": _digest(task),
        "context": {
            "domain": domain,
            "capability": "bounded_review",
            "risk_class": "release_review",
        },
        "selected_model": {"provider": "offline", "model": "test-model"},
        "digests": {
            "selection_digest": "a" * 64,
            "prompt_digest": "b" * 64,
            "plan_digest": "c" * 64,
            "outcome_digest": "d" * 64,
        },
        "tags": ["restart-safe", "metadata-only"],
        "lesson": "Keep the review bounded and explicit.",
        "provenance": {"source": "autonomous-agent-test", "catalog_version": "test-v1"},
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


def test_python_agent_memory_lifecycle_is_restart_safe_across_all_domains(tmp_path) -> None:
    backend = _CasTextStore()
    source = BrainEpisodicMemory(tmp_path / "source.sqlite3")
    source_persistence = BrainMemoryPersistenceCoordinator(source, backend)
    source_agent = AutonomousAgent(
        object(),
        LLMRuntime(),
        memory=source,
        memory_persistence=source_persistence,
    )
    task = "review the autonomous release evidence without retaining task text"

    try:
        for index, domain in enumerate(AUTONOMOUS_DOMAINS):
            source.record_episode(
                _episode(
                    episode_id=f"agent-memory-{index}",
                    domain=domain,
                    task=f"{task} for {domain}",
                )
            )

        flushed = source_agent.flush_memory()
        assert flushed["event_count"] == len(AUTONOMOUS_DOMAINS)
        assert task not in json.dumps(flushed)
        assert "provider-secret" not in json.dumps(flushed)

        restored = BrainEpisodicMemory(tmp_path / "restored.sqlite3")
        restored_persistence = BrainMemoryPersistenceCoordinator(restored, backend)
        restored_agent = AutonomousAgent(
            object(),
            LLMRuntime(),
            memory=restored,
            memory_persistence=restored_persistence,
        )
        restored_snapshot = restored_agent.restore_memory()
        assert restored_snapshot is not None
        assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
        assert {
            row["context"]["domain"] for row in restored.retrieve(limit=128)
        } == set(AUTONOMOUS_DOMAINS)
        assert restored.verify_integrity()["ok"] is True

        unconfigured = AutonomousAgent(tmp_path, LLMRuntime(), memory=restored)
        with pytest.raises(BrainRunError, match="memory persistence is not configured"):
            unconfigured.flush_memory()

        mismatched = BrainEpisodicMemory(tmp_path / "mismatched.sqlite3")
        with pytest.raises(BrainRunError, match="bound to the supplied memory"):
            AutonomousAgent(
                object(),
                LLMRuntime(),
                memory=mismatched,
                memory_persistence=source_persistence,
            )
        mismatched.close()
        restored.close()
    finally:
        source.close()


def test_python_agent_memory_lifecycle_requires_memory() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())

    with pytest.raises(BrainRunError, match="has no episodic memory"):
        agent.restore_memory()
    with pytest.raises(BrainRunError, match="has no episodic memory"):
        agent.flush_memory()


def test_python_agent_rejects_non_coordinator_memory_persistence() -> None:
    memory = BrainEpisodicMemory(":memory:")
    try:
        with pytest.raises(BrainRunError, match="BrainMemoryPersistenceCoordinator"):
            AutonomousAgent(
                object(),
                LLMRuntime(),
                memory=memory,
                memory_persistence=object(),
            )
    finally:
        memory.close()
