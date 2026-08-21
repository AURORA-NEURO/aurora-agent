import json
import hashlib
import sqlite3
from pathlib import Path
from types import SimpleNamespace

import pytest

from prism_sdk.autonomy import AUTONOMOUS_DOMAINS, AutonomousTaskOrchestrator
from prism_sdk.goals import (
    AutonomousGoalConflict,
    AutonomousGoalError,
    AutonomousGoalLedger,
    goal_task_digest,
)


def _digest(value: str) -> str:
    return goal_task_digest(value)


def test_goal_ledger_survives_restart_and_keeps_objective_value_only(tmp_path: Path) -> None:
    path = tmp_path / "goals.sqlite3"
    task = "prepare a cross-domain release evidence review"
    criterion_digest = _digest("release evidence is independently verified")
    with AutonomousGoalLedger(str(path), clock=lambda: 100) as ledger:
        record = ledger.create(
            goal_id="release-review",
            task_digest=_digest(task),
            domain="engineering",
            capability="release_review",
            risk_class="high_review",
            criteria=[
                {
                    "criterion_id": "evidence",
                    "criterion_digest": criterion_digest,
                }
            ],
            max_attempts=2,
        )
        assert record.status == "ready"
        assert record.attempt == 0
        assert record.required_criteria_complete is False
        running = ledger.transition("release-review", "running", expected_revision=0, now_ns=101)
        assert running.attempt == 1
        paused = ledger.transition(
            "release-review",
            "paused",
            expected_revision=1,
            criterion_updates=[
                {
                    "criterion_id": "evidence",
                    "status": "satisfied",
                    "evidence_digest": _digest("verified evidence receipt"),
                }
            ],
            next_action_digest=_digest("operator review"),
            now_ns=102,
        )
        assert paused.criteria[0].status == "satisfied"
        assert paused.required_criteria_complete
        resumed = ledger.transition("release-review", "running", expected_revision=2, now_ns=103)
        completed = ledger.transition("release-review", "completed", expected_revision=3, now_ns=104)
        assert completed.attempt == 2
        assert completed.status == "completed"
        assert ledger.stats()["statuses"] == {"completed": 1}
        assert ledger.verify_integrity()["ok"] is True
        serialized = json.dumps(completed.to_dict(), sort_keys=True)
        assert task not in serialized
        assert "release evidence is independently verified" not in serialized
        assert resumed.state_digest != completed.state_digest

    with AutonomousGoalLedger(str(path), clock=lambda: 200) as restored:
        loaded = restored.get("release-review")
        assert loaded is not None
        assert loaded.status == "completed"
        assert restored.verify_integrity()["events"] == 5


def test_goal_ledger_applies_optimistic_conflicts_and_fail_closed_completion(tmp_path: Path) -> None:
    with AutonomousGoalLedger(str(tmp_path / "goals.sqlite3"), clock=lambda: 1) as ledger:
        ledger.create(
            goal_id="bounded-goal",
            task_digest=_digest("bounded task"),
            domain="operations",
            criteria=[{"criterion_id": "safe", "criterion_digest": _digest("safe change")}],
            max_attempts=1,
        )
        with pytest.raises(AutonomousGoalConflict):
            ledger.transition("bounded-goal", "running", expected_revision=4)
        running = ledger.transition("bounded-goal", "running", expected_revision=0)
        with pytest.raises(AutonomousGoalError, match="required criterion"):
            ledger.transition("bounded-goal", "completed", expected_revision=1)
        failed = ledger.transition("bounded-goal", "failed", expected_revision=1)
        with pytest.raises(AutonomousGoalError, match="attempt budget"):
            ledger.transition("bounded-goal", "ready", expected_revision=2)
        assert failed.status == "failed"
        with pytest.raises(AutonomousGoalError, match="cannot transition"):
            ledger.transition("bounded-goal", "running", expected_revision=2)


def test_goal_creation_is_idempotent_across_clock_ticks_but_rejects_identity_drift(tmp_path: Path) -> None:
    ticks = iter((1, 2, 3))
    with AutonomousGoalLedger(str(tmp_path / "idempotent.sqlite3"), clock=lambda: next(ticks)) as ledger:
        first = ledger.create(goal_id="same", task_digest=_digest("same task"), domain="coding")
        second = ledger.create(goal_id="same", task_digest=_digest("same task"), domain="coding")
        assert second.state_digest == first.state_digest
        with pytest.raises(AutonomousGoalConflict, match="different identity"):
            ledger.create(goal_id="same", task_digest=_digest("different task"), domain="coding")


def test_goal_ledger_is_domain_neutral_across_all_builtin_domains(tmp_path: Path) -> None:
    with AutonomousGoalLedger(str(tmp_path / "all-domains.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS)) as ledger:
        for domain in AUTONOMOUS_DOMAINS:
            ledger.create(
                goal_id=f"goal-{domain}",
                task_digest=_digest(f"task for {domain}"),
                domain=domain,
            )
        assert len(ledger.list(limit=len(AUTONOMOUS_DOMAINS))) == len(AUTONOMOUS_DOMAINS)
        assert len(ledger.list(domain=AUTONOMOUS_DOMAINS[0])) == 1
        assert ledger.verify_integrity()["goals"] == len(AUTONOMOUS_DOMAINS)


def test_goal_execution_wrapper_advances_all_domains_and_retains_only_value_state(tmp_path: Path) -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    orchestrator.run = lambda **_: SimpleNamespace(status="approval_required")
    with AutonomousGoalLedger(str(tmp_path / "execution.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS)) as ledger:
        for domain in AUTONOMOUS_DOMAINS:
            step = orchestrator.run_goal_step(
                goal_store=ledger,
                goal_id=f"execution-{domain}",
                task=f"perform a bounded task for {domain}",
                domain=domain,
            )
            assert step["goal_status"] == "paused"
            assert step["result_status"] == "approval_required"
        assert len(ledger.list(statuses=("paused",), limit=len(AUTONOMOUS_DOMAINS))) == len(AUTONOMOUS_DOMAINS)
        serialized = json.dumps([record.to_dict() for record in ledger.list(limit=len(AUTONOMOUS_DOMAINS))], sort_keys=True)
        assert "perform a bounded task" not in serialized
        assert ledger.verify_integrity()["ok"] is True


def test_goal_execution_wrapper_completes_criteria_and_records_failures(tmp_path: Path) -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    with AutonomousGoalLedger(str(tmp_path / "settlement.sqlite3")) as ledger:
        orchestrator.run = lambda **_: SimpleNamespace(status="approval_required")
        paused = orchestrator.run_goal_step(
            goal_store=ledger,
            goal_id="settlement",
            task="settle an evidence review",
            domain="evaluation",
            goal_criteria=[{"criterion_id": "evidence", "criterion_digest": _digest("evidence")}],
        )
        assert paused["goal_status"] == "paused"
        orchestrator.run = lambda **_: SimpleNamespace(status="completed")
        completed = orchestrator.run_goal_step(
            goal_store=ledger,
            goal_id="settlement",
            task="settle an evidence review",
            domain="evaluation",
            criterion_updates=[{"criterion_id": "evidence", "status": "satisfied", "evidence_digest": _digest("receipt")}],
            settlement_metadata={
                "learning_state_digest": _digest("bandit state"),
                "progress_digest": _digest("evaluation progress"),
            },
        )
        assert completed["goal_status"] == "completed"
        assert completed["goal"]["attempt"] == 2
        assert completed["goal"]["evaluator_digest"] is not None
        assert completed["goal"]["learning_state_digest"] == _digest("bandit state")
        assert completed["goal"]["progress_digest"] == _digest("evaluation progress")

        orchestrator.run = lambda **_: (_ for _ in ()).throw(RuntimeError("synthetic provider failure"))
        with pytest.raises(RuntimeError, match="synthetic provider failure"):
            orchestrator.run_goal_step(
                goal_store=ledger,
                goal_id="failed-goal",
                task="retry an unavailable provider",
                domain="operations",
            )
        assert ledger.get("failed-goal").status == "failed"
        assert ledger.verify_integrity()["ok"] is True


def test_cross_domain_goal_execution_wrapper_persists_fanout_progress_without_payloads(tmp_path: Path) -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    orchestrator.run_cross_domain = lambda **_: SimpleNamespace(
        status="approval_required",
        child_results=(),
        completed_children=0,
        total_children=2,
    )
    subtasks = [{"domain": "coding", "task": "inspect"}, {"domain": "science", "task": "compare"}]
    with AutonomousGoalLedger(str(tmp_path / "cross-domain-execution.sqlite3")) as ledger:
        paused = orchestrator.run_cross_domain_goal_step(
            goal_store=ledger,
            goal_id="cross-domain-goal",
            task="coordinate a bounded cross-domain review",
            subtasks=subtasks,
            goal_criteria=[{"criterion_id": "synthesis", "criterion_digest": _digest("synthesis")}],
        )
        assert paused["goal_status"] == "paused"
        assert paused["goal"]["domain"] == "cross_domain"
        assert paused["progress_digest"] is not None
        serialized = json.dumps(ledger.list(domain="cross_domain", limit=1)[0].to_dict(), sort_keys=True)
        assert "inspect" not in serialized
        assert "compare" not in serialized

        orchestrator.run_cross_domain = lambda **_: SimpleNamespace(
            status="completed",
            child_results=(SimpleNamespace(status="completed"),),
            completed_children=2,
            total_children=2,
        )
        completed = orchestrator.run_cross_domain_goal_step(
            goal_store=ledger,
            goal_id="cross-domain-goal",
            task="coordinate a bounded cross-domain review",
            subtasks=subtasks,
            criterion_updates=[{"criterion_id": "synthesis", "status": "satisfied", "evidence_digest": _digest("synthesis receipt")}],
        )
        assert completed["goal_status"] == "completed"
        assert ledger.verify_integrity()["ok"] is True


def test_goal_digest_contract_matches_the_typescript_reference() -> None:
    with AutonomousGoalLedger(":memory:", clock=lambda: 100) as ledger:
        record = ledger.create(
            goal_id="parity-goal",
            task_digest=goal_task_digest("parity task"),
            domain="coding",
            capability="review",
            risk_class="research",
            criteria=[{"criterion_id": "done", "criterion_digest": goal_task_digest("done")}],
            max_attempts=2,
        )
    assert goal_task_digest("parity task") == "75c9dd12cec986f5aa50dcab2416229220e8c2b3e28283c550fb7fad9c8d9841"
    assert record.state_digest == "553312b08e201b99e81f39761bec11ed2127a9b7873f8e07859d867cdd1912cc"


def test_goal_ledger_detects_tampered_state_and_event(tmp_path: Path) -> None:
    path = tmp_path / "tamper.sqlite3"
    with AutonomousGoalLedger(str(path)) as ledger:
        ledger.create(goal_id="tamper", task_digest=_digest("task"), domain="coding")
        ledger._connection.execute(
            "UPDATE autonomous_goals SET state_json = json_set(state_json, '$.status', 'completed') WHERE goal_id = 'tamper'"
        )
        with pytest.raises(AutonomousGoalError, match="state_digest"):
            ledger.verify_integrity()

    connection = sqlite3.connect(path)
    connection.execute("UPDATE autonomous_goal_events SET payload_json = '{}' WHERE sequence = 1")
    connection.commit()
    connection.close()
    with AutonomousGoalLedger(str(path)) as ledger:
        with pytest.raises(AutonomousGoalError, match="hash chain"):
            ledger.verify_integrity()


def test_goal_ledger_migrates_pre_settlement_value_only_state(tmp_path: Path) -> None:
    path = tmp_path / "legacy-goals.sqlite3"
    with AutonomousGoalLedger(str(path), clock=lambda: 10) as ledger:
        record = ledger.create(goal_id="legacy", task_digest=_digest("legacy task"), domain="coding")
        legacy = record.to_dict()
        for field in ("outcome_digest", "evaluator_digest", "learning_state_digest", "progress_digest"):
            legacy.pop(field, None)
        legacy_payload = {key: value for key, value in legacy.items() if key not in {"state_digest", "retention", "secret_material"}}
        legacy["state_digest"] = hashlib.sha256(
            json.dumps(legacy_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
        ).hexdigest()
        ledger._connection.execute(
            "UPDATE autonomous_goals SET state_json = ?, state_digest = ? WHERE goal_id = ?",
            (json.dumps(legacy, sort_keys=True), legacy["state_digest"], "legacy"),
        )
    with AutonomousGoalLedger(str(path)) as restored:
        migrated = restored.get("legacy")
        assert migrated is not None
        assert migrated.outcome_digest is None
        assert restored.verify_integrity()["ok"] is True
