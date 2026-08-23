from __future__ import annotations

import json
import sqlite3
import threading

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    BRAIN_LEARNING_EPISODE_SCHEMA,
    BrainLearningPersistenceCoordinator,
    BrainLearningLedger,
    BrainRunError,
    SQLiteBrainLearningLedger,
    TransactionalJsonBrainLearningSnapshotPersistence,
)


def _report(index: int = 1) -> dict[str, object]:
    return {
        "learning_evidence": {
            "evidence_digest": f"{index:064x}",
            "evaluator_id": "test-evaluator",
            "evaluator_version": "1",
            "arm_id": "offline/model",
            "reward": 0.75,
            "failed": False,
        },
        "next_state": {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": index,
            "arms": [{"arm_id": "offline/model", "pulls": index, "reward_sum": 0.75}],
        },
    }


def _episode(episode_id: str = "episode-1") -> dict[str, object]:
    return {
        "schema": BRAIN_LEARNING_EPISODE_SCHEMA,
        "episode_id": episode_id,
        "evaluation_input": {
            "schema": "bioprism-brain-evaluator-input/0.1",
            "run_id": "run-1",
            "result_kind": "provider",
            "selected_model": {"provider": "offline", "model": "model"},
            "outcome_digest": "a" * 64,
        },
        "arm_id": "offline/model",
        "evidence_digest": None,
        "status": "pending",
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


def test_learning_snapshot_rehydrates_all_domains_and_fences_stale_writers(tmp_path) -> None:
    backend = _CasTextStore()
    persistence = TransactionalJsonBrainLearningSnapshotPersistence(backend)
    source = BrainLearningLedger(tmp_path / "source-learning.jsonl")
    for index, domain in enumerate(AUTONOMOUS_DOMAINS, start=1):
        source.append(
            _report(index),
            context_digest=f"{index:064x}",
            replay={"run_id": f"run-{domain}", "domain": domain},
        )
    source_coordinator = BrainLearningPersistenceCoordinator(source, persistence)
    flushed = source_coordinator.flush()
    assert flushed["head_digest"] == flushed["record_digests"][-1]

    restored = BrainLearningLedger(tmp_path / "restored-learning.jsonl")
    restored_coordinator = BrainLearningPersistenceCoordinator(restored, persistence)
    restored_snapshot = restored_coordinator.restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {row["domain"] for row in restored.replays(limit=128)} == set(AUTONOMOUS_DOMAINS)
    assert restored.snapshot()["snapshot_digest"] == flushed["snapshot_digest"]

    stale = BrainLearningLedger(tmp_path / "stale-learning.jsonl")
    stale_coordinator = BrainLearningPersistenceCoordinator(stale, persistence)
    stale_coordinator.restore()
    source.append(_report(99), context_digest="f" * 64, replay={"run_id": "new-run", "domain": "engineering"})
    source_coordinator.flush()
    with pytest.raises(BrainRunError, match="compare-and-swap conflict"):
        stale_coordinator.flush()

    tampered = json.loads(backend.value)
    tampered["records"][0]["record"]["replay"]["domain"] = "tampered"
    backend.value = json.dumps(tampered)
    with pytest.raises(BrainRunError, match="digest"):
        BrainLearningPersistenceCoordinator(BrainLearningLedger(tmp_path / "tampered.jsonl"), persistence).restore()


def test_learning_snapshot_transfers_between_jsonl_and_sqlite_without_losing_append_indices(tmp_path) -> None:
    source = BrainLearningLedger(tmp_path / "portable-source.jsonl")
    source.append(_report(1), context_digest="a" * 64, replay={"run_id": "portable"})
    snapshot = source.snapshot()
    with SQLiteBrainLearningLedger(tmp_path / "portable.sqlite3") as restored:
        restored.append(_report(99), context_digest="f" * 64)
        restored.restore(snapshot)
        assert restored.snapshot()["snapshot_digest"] == snapshot["snapshot_digest"]
        receipt = restored.append(_report(2), context_digest="b" * 64)
        assert receipt["record_index"] == 1
        assert len(restored.records()) == 2


def test_sqlite_learning_ledger_is_restart_safe_and_compatible_with_the_parent_interface(tmp_path) -> None:
    path = tmp_path / "learning" / "brain.sqlite3"
    with SQLiteBrainLearningLedger(path) as ledger:
        assert isinstance(ledger, BrainLearningLedger)
        receipt = ledger.append(_report(), context_digest="b" * 64, replay={"run_id": "run-1"})
        episode_receipt = ledger.begin_episode(_episode())
        assert receipt["record_index"] == 0
        assert episode_receipt["record_index"] == 1
        assert ledger.latest_state()["generation"] == 1
        assert ledger.pending_episodes()[0].episode_id == "episode-1"
        assert ledger.contextual_state({
            "domain": "coding",
            "capability": "review",
            "risk_class": "standard",
        })["observed"] is False

    with SQLiteBrainLearningLedger(path) as restarted:
        assert len(restarted.records()) == 2
        assert restarted.replays(run_id="run-1")[0]["run_id"] == "run-1"
        assert restarted.begin_episode(_episode())["idempotent"] is True
        with pytest.raises(BrainRunError):
            restarted.begin_episode({**_episode(), "evaluation_input": {
                **_episode()["evaluation_input"],
                "run_id": "different-run",
            }})


def test_sqlite_learning_ledger_serializes_concurrent_appenders(tmp_path) -> None:
    path = tmp_path / "concurrent.sqlite3"
    ledgers = [SQLiteBrainLearningLedger(path) for _ in range(2)]
    errors: list[BaseException] = []

    def append(ledger: SQLiteBrainLearningLedger, index: int) -> None:
        try:
            ledger.append(_report(index))
        except BaseException as error:  # pragma: no cover - assertion below captures failures
            errors.append(error)

    threads = [threading.Thread(target=append, args=(ledgers[index % 2], index + 1)) for index in range(12)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    for ledger in ledgers:
        ledger.close()
    assert errors == []
    with SQLiteBrainLearningLedger(path) as ledger:
        assert len(ledger.records()) == 12


def test_sqlite_learning_ledger_rejects_tampering_and_never_persists_secret_shaped_values(tmp_path) -> None:
    path = tmp_path / "tampered.sqlite3"
    with SQLiteBrainLearningLedger(path) as ledger:
        ledger.append(_report())
    raw = path.read_bytes()
    assert b"api_key" not in raw
    assert b"secret" not in raw

    connection = sqlite3.connect(path)
    try:
        connection.execute(
            "UPDATE brain_learning_records SET record_json = ? WHERE record_index = 1",
            (json.dumps({"schema": "bioprism-brain-learning-ledger/0.1", "record": {}}),),
        )
        connection.commit()
    finally:
        connection.close()
    with SQLiteBrainLearningLedger(path) as ledger:
        with pytest.raises(BrainRunError, match="digest mismatch"):
            ledger.records()
