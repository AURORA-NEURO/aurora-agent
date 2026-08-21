import json
import sqlite3
from pathlib import Path

import pytest

from prism_sdk.autonomy import AUTONOMOUS_DOMAINS
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
    assert record.state_digest == "3d90744da6795394cde9323d93c03b22fccef0de32810a4fdc8fd39f81b8496b"


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
