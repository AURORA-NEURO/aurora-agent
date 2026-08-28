from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import json
import sqlite3

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPersistenceCoordinator,
    AutonomousExecutionPolicy,
    TransactionalJsonAutonomousExecutionSnapshotPersistence,
    AutonomyPersistenceError,
    AutonomyPolicyError,
    ProviderToolCall,
    SQLiteAutonomousExecutionSnapshotPersistence,
)


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


def test_policy_bounds_and_effects_are_explicit() -> None:
    with pytest.raises(AutonomyPersistenceError):
        AutonomousExecutionPolicy(allow_side_effects=True)
    policy = AutonomousExecutionPolicy(
        max_steps=3,
        max_provider_calls=2,
        max_provider_failovers=1,
        max_tool_calls=2,
        max_effectful_calls=1,
        allow_side_effects=True,
    )
    assert policy.to_dict()["authorization"] == "caller_owned_policy"
    assert policy.to_dict()["max_provider_failovers"] == 1
    assert len(policy.digest) == 64


def test_journal_is_restart_safe_hash_chained_and_resume_is_explicit(tmp_path) -> None:
    path = tmp_path / "execution.jsonl"
    journal = AutonomousExecutionJournal(path)
    policy = AutonomousExecutionPolicy(max_steps=8, max_tool_calls=4)
    controller = AutonomousExecutionController(
        execution_id="execution-1",
        domain="operations",
        capability="observability",
        risk_class="read_only",
        policy=policy,
        journal=journal,
    )
    controller.admit_tool_call(tool="status", call_id="call-1", read_only=True, approval_required=False)
    controller.record_tool_outcome(tool="status", call_id="call-1", status="executed", outcome_digest="a" * 64)
    controller.checkpoint(status="paused", reason="awaiting_rehydrated_provider_context")

    assert journal.verify_integrity()["verified"] is True
    state = journal.state("execution-1")
    assert state is not None
    assert state.status == "paused"
    assert state.tool_calls == 1
    assert all('"prompt":' not in json.dumps(event).lower() for event in journal.events(execution_id="execution-1"))

    with pytest.raises(AutonomyPersistenceError):
        AutonomousExecutionController(
            execution_id="execution-1",
            domain="operations",
            capability="observability",
            risk_class="read_only",
            policy=policy,
            journal=journal,
        )
    resumed = AutonomousExecutionController(
        execution_id="execution-1",
        domain="operations",
        capability="observability",
        risk_class="read_only",
        policy=policy,
        journal=AutonomousExecutionJournal(path),
        resume=True,
    )
    resumed.complete()
    assert resumed.state.status == "completed"
    latest = journal.state("execution-1")
    assert latest is not None
    assert resumed.state.journal_sequence == latest.journal_sequence
    assert resumed.state.checkpoint_digest == latest.checkpoint_digest
    with pytest.raises(AutonomyPersistenceError):
        AutonomousExecutionController(
            execution_id="execution-1",
            domain="operations",
            capability="observability",
            risk_class="read_only",
            policy=policy,
            journal=journal,
            resume=True,
        )


def test_journal_rejects_tampering_and_transient_metadata(tmp_path) -> None:
    path = tmp_path / "execution.jsonl"
    journal = AutonomousExecutionJournal(path)
    policy = AutonomousExecutionPolicy()
    AutonomousExecutionController(
        execution_id="execution-2",
        domain="data",
        capability="quality_control",
        risk_class="data_integrity",
        policy=policy,
        journal=journal,
    )
    rows = path.read_text(encoding="utf-8").splitlines()
    row = json.loads(rows[0])
    row["event"]["metadata"] = {"response": "must-not-persist"}
    path.write_text(json.dumps(row) + "\n", encoding="utf-8")
    with pytest.raises(AutonomyPersistenceError):
        journal.verify_integrity()


def test_execution_budget_persists_failovers_and_replans_across_restart(tmp_path) -> None:
    path = tmp_path / "bounded-recovery.jsonl"
    policy = AutonomousExecutionPolicy(
        max_steps=8,
        max_provider_calls=4,
        max_provider_failovers=1,
        max_replans=1,
    )
    controller = AutonomousExecutionController(
        execution_id="bounded-recovery",
        domain="coding",
        capability="implementation_review",
        risk_class="read_only",
        policy=policy,
        journal=AutonomousExecutionJournal(path),
    )
    controller.admit_provider_call(
        provider="primary",
        model="model-a",
        invocation_kind="answer",
        attempt=0,
        turn=0,
    )
    assert controller.state.provider_failovers == 0
    controller.admit_provider_call(
        provider="fallback",
        model="model-b",
        invocation_kind="answer",
        attempt=1,
        turn=0,
        failover=True,
    )
    assert controller.state.provider_failovers == 1
    controller.replan(
        instruction_digest="c" * 64,
        reason="evaluator_requested_revision",
        attempt=1,
    )
    assert controller.state.replans == 1
    serialized = path.read_text(encoding="utf-8")
    assert "evaluator_requested_revision" in serialized
    assert "raw replan instruction" not in serialized
    assert {row["event"]["kind"] for row in AutonomousExecutionJournal(path).events()} >= {"replan"}

    resumed = AutonomousExecutionController(
        execution_id="bounded-recovery",
        domain="coding",
        capability="implementation_review",
        risk_class="read_only",
        policy=policy,
        journal=AutonomousExecutionJournal(path),
        resume=True,
    )
    assert resumed.state.provider_failovers == 1
    assert resumed.state.replans == 1
    with pytest.raises(AutonomyPolicyError, match="max_provider_failovers"):
        resumed.admit_provider_call(
            provider="second-fallback",
            model="model-c",
            invocation_kind="answer",
            attempt=2,
            turn=0,
            failover=True,
        )
    with pytest.raises(AutonomyPolicyError, match="max_replans"):
        resumed.replan(instruction_digest="d" * 64, reason="second_revision")


def test_shared_controller_serializes_concurrent_domain_transitions(tmp_path) -> None:
    path = tmp_path / "concurrent-execution.jsonl"
    controller = AutonomousExecutionController(
        execution_id="concurrent-execution",
        domain="operations",
        capability="observability",
        risk_class="read_only",
        policy=AutonomousExecutionPolicy(
            max_steps=16,
            max_provider_calls=16,
            max_cost_units=16,
        ),
        journal=AutonomousExecutionJournal(path),
    )

    def admit(index: int) -> int:
        return controller.admit_provider_call(
            provider="local",
            model=f"model-{index}",
            invocation_kind="parallel_domain_worker",
            attempt=0,
            turn=0,
        ).provider_calls

    with ThreadPoolExecutor(max_workers=8) as workers:
        observed = list(workers.map(admit, range(16)))

    assert sorted(observed) == list(range(1, 17))
    assert controller.state.step_index == 16
    assert controller.state.provider_calls == 16
    rows = AutonomousExecutionJournal(path).events(execution_id="concurrent-execution")
    assert len(rows) == 17
    assert [row["sequence"] for row in rows] == list(range(1, 18))
    assert len({row["event"]["state"]["provider_calls"] for row in rows}) == 17


def test_execution_snapshot_persistence_rehydrates_all_domains_and_fences_stale_writers(tmp_path) -> None:
    store = _CasTextStore()
    persistence = TransactionalJsonAutonomousExecutionSnapshotPersistence(store)
    journal = AutonomousExecutionJournal(tmp_path / "all-domains.jsonl")
    policy = AutonomousExecutionPolicy(max_steps=8, max_tool_calls=4)
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        controller = AutonomousExecutionController(
            execution_id=f"domain-execution-{index}",
            domain=domain,
            capability="observability",
            risk_class="read_only",
            policy=policy,
            journal=journal,
        )
        controller.checkpoint(status="paused", reason="remote_snapshot_round_trip")

    coordinator = AutonomousExecutionPersistenceCoordinator(journal, persistence)
    snapshot = coordinator.flush()
    assert snapshot["head_digest"]
    assert set(row["event"]["domain"] for row in snapshot["rows"]) == set(AUTONOMOUS_DOMAINS)

    restored_journal = AutonomousExecutionJournal(tmp_path / "restored.jsonl")
    restored_coordinator = AutonomousExecutionPersistenceCoordinator(restored_journal, persistence)
    restored = restored_coordinator.restore()
    assert restored is not None
    assert restored_journal.verify_integrity()["events"] == len(AUTONOMOUS_DOMAINS) * 2
    assert {row["event"]["domain"] for row in restored_journal.events()} == set(AUTONOMOUS_DOMAINS)

    stale_journal = AutonomousExecutionJournal(tmp_path / "stale.jsonl")
    stale_coordinator = AutonomousExecutionPersistenceCoordinator(stale_journal, persistence)
    stale_coordinator.restore()
    resumed = AutonomousExecutionController(
        execution_id="domain-execution-0",
        domain=AUTONOMOUS_DOMAINS[0],
        capability="observability",
        risk_class="read_only",
        policy=policy,
        journal=journal,
        resume=True,
    )
    resumed.checkpoint(status="paused", reason="newer_writer")
    coordinator.flush()
    with pytest.raises(AutonomyPersistenceError, match="compare-and-swap conflict"):
        stale_coordinator.flush()

    tampered = json.loads(store.value)
    tampered["head_digest"] = "a" * 64
    store.value = json.dumps(tampered)
    with pytest.raises(AutonomyPersistenceError):
        AutonomousExecutionPersistenceCoordinator(AutonomousExecutionJournal(tmp_path / "tampered.jsonl"), persistence).restore()


def test_sqlite_execution_snapshot_persistence_reopens_and_fences_concurrent_writers(tmp_path) -> None:
    database = tmp_path / "execution-snapshots.sqlite3"
    policy = AutonomousExecutionPolicy(max_steps=8, max_tool_calls=4)
    journal = AutonomousExecutionJournal(tmp_path / "initial.jsonl")
    controller = AutonomousExecutionController(
        execution_id="sqlite-execution",
        domain="operations",
        capability="observability",
        risk_class="read_only",
        policy=policy,
        journal=journal,
    )
    controller.checkpoint(status="paused", reason="sqlite_initial_checkpoint")

    with SQLiteAutonomousExecutionSnapshotPersistence(database) as persistence:
        coordinator = AutonomousExecutionPersistenceCoordinator(journal, persistence)
        initial = coordinator.flush()
        assert persistence.read() == initial

    reopened = SQLiteAutonomousExecutionSnapshotPersistence(database)
    try:
        restored_journal = AutonomousExecutionJournal(tmp_path / "reopened.jsonl")
        restored = AutonomousExecutionPersistenceCoordinator(restored_journal, reopened).restore()
        assert restored is not None
        assert restored["snapshot_digest"] == initial["snapshot_digest"]
        assert restored_journal.verify_integrity()["verified"] is True

        def candidate(name: str) -> dict[str, object]:
            candidate_journal = AutonomousExecutionJournal(tmp_path / f"{name}.jsonl")
            candidate_journal.restore(initial)
            candidate_controller = AutonomousExecutionController(
                execution_id="sqlite-execution",
                domain="operations",
                capability="observability",
                risk_class="read_only",
                policy=policy,
                journal=candidate_journal,
                resume=True,
            )
            candidate_controller.checkpoint(status="paused", reason=name)
            return candidate_journal.snapshot()

        candidates = (candidate("sqlite-writer-a"), candidate("sqlite-writer-b"))
    finally:
        reopened.close()

    writer_a = SQLiteAutonomousExecutionSnapshotPersistence(database)
    writer_b = SQLiteAutonomousExecutionSnapshotPersistence(database)
    try:
        expected = initial["snapshot_digest"]

        def compare_and_swap(args: tuple[SQLiteAutonomousExecutionSnapshotPersistence, dict[str, object]]) -> bool:
            writer, snapshot = args
            return writer.write_if_unchanged(expected, snapshot)

        with ThreadPoolExecutor(max_workers=2) as workers:
            outcomes = list(workers.map(compare_and_swap, ((writer_a, candidates[0]), (writer_b, candidates[1]))))
        assert sorted(outcomes) == [False, True]
        winner = writer_a.read() if outcomes[0] else writer_b.read()
        assert winner is not None
        assert winner["snapshot_digest"] == candidates[0 if outcomes[0] else 1]["snapshot_digest"]
    finally:
        writer_a.close()
        writer_b.close()

    with pytest.raises(AutonomyPersistenceError, match="incompatible schema"):
        connection = sqlite3.connect(database)
        try:
            connection.execute(
                "UPDATE autonomy_execution_snapshot_meta SET value = 'wrong-schema' WHERE key = 'schema'"
            )
            connection.commit()
        finally:
            connection.close()
        SQLiteAutonomousExecutionSnapshotPersistence(database)


def test_runtime_session_enforces_policy_and_journals_read_only_outcomes(tmp_path) -> None:
    tool = AutonomousDomainTool(
        name="observe_status",
        domains=("operations",),
        capability="observability",
        description="Read bounded status.",
        parameters={"type": "object", "additionalProperties": False},
    )
    effect = AutonomousDomainTool(
        name="apply_change",
        domains=("operations",),
        capability="rollback",
        description="Apply an approved change.",
        parameters={"type": "object", "additionalProperties": False},
        risk_class="external_effect",
        read_only=False,
        approval_required=True,
    )
    registry = AutonomousDomainToolRegistry([tool, effect])
    executed: list[str] = []
    base = AutonomousDomainToolRuntime(
        registry,
        executor=lambda resolved, _arguments: executed.append(resolved.name) or {"ok": True},
    )
    journal = AutonomousExecutionJournal(tmp_path / "runtime.jsonl")
    session = base.session(
        execution_id="execution-3",
        domain="operations",
        capability="observability",
        risk_class="read_only",
        journal=journal,
        policy=AutonomousExecutionPolicy(max_tool_calls=2),
    )
    read = session((ProviderToolCall("read-1", "observe_status", {}),))
    effect_result = session((ProviderToolCall("effect-1", "apply_change", {}),))
    assert read[0].approved is True
    assert effect_result[0].approved is False
    assert session.receipts[-1].status == "policy_refused"
    assert executed == ["observe_status"]
    assert {event["event"]["kind"] for event in journal.events(execution_id="execution-3")} == {"started", "tool_intent", "tool_outcome"}
