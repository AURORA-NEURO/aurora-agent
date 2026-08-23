from __future__ import annotations

import json

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
    ProviderToolCall,
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
