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
    AutonomousGoalPersistenceCoordinator,
    TransactionalJsonAutonomousGoalSnapshotPersistence,
    goal_task_digest,
)
from prism_sdk.autonomous_goal_scheduler import (
    AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS,
    AutonomousGoalSchedulingSignal,
    AutonomousGoalScheduler,
    claim_autonomous_goals,
    schedule_autonomous_goals,
    validate_goal_schedule,
)
from prism_sdk.autonomous_goal_worker import AutonomousGoalWorker
from prism_sdk.autonomous_goal_control_loop import AutonomousGoalControlLoop
from prism_sdk.autonomous_goal_worker_journal import (
    AutonomousGoalWorkerJournal,
    JsonAutonomousGoalWorkerJournalPersistence,
    AutonomousGoalWorkerJournalPersistenceCoordinator,
)


def _digest(value: str) -> str:
    return goal_task_digest(value)


def test_goal_scheduler_prioritizes_dependency_closed_work_across_every_domain(tmp_path: Path) -> None:
    with AutonomousGoalLedger(str(tmp_path / "scheduler.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS) + 2) as ledger:
        for domain in AUTONOMOUS_DOMAINS:
            ledger.create(
                goal_id=f"goal-{domain}",
                task_digest=_digest(f"task-{domain}"),
                domain=domain,
                now_ns=0,
            )
        schedule = AutonomousGoalScheduler().plan(
            ledger.list(limit=len(AUTONOMOUS_DOMAINS)),
            {
                "now_ns": 1_000,
                "max_selected": len(AUTONOMOUS_DOMAINS),
                "max_concurrent": len(AUTONOMOUS_DOMAINS),
                "required_domains": list(AUTONOMOUS_DOMAINS),
                "signals": [
                    {"goal_id": "goal-coding", "priority": 0.2},
                    {"goal_id": "goal-science", "priority": 1.0, "urgency": 1.0, "dependencies": ["goal-coding"]},
                ],
            },
        )
        assert schedule.selected_goal_ids.index("goal-coding") < schedule.selected_goal_ids.index("goal-science")
        assert set(schedule.selected_goal_ids) == {f"goal-{domain}" for domain in AUTONOMOUS_DOMAINS}
        assert schedule.missing_domains == ()
        assert schedule.to_dict()["coverage"]["selected_domains"] == list(AUTONOMOUS_DOMAINS)
        assert "task-coding" not in json.dumps(schedule.to_dict())
        assert validate_goal_schedule(schedule.to_dict())["schedule_digest"] == schedule.schedule_digest
        assert schedule.schedule_digest == "30451f0e55e23ad929f23415a2ffe0a9281e3c3632c51ac9420d00995c789654"


def test_goal_scheduler_enforces_budgets_cycles_retries_and_stale_claims(tmp_path: Path) -> None:
    with AutonomousGoalLedger(str(tmp_path / "scheduler-claim.sqlite3"), clock=lambda: 20, max_goals=8) as ledger:
        ledger.create(goal_id="base", task_digest=_digest("base task"), domain="coding", now_ns=0)
        ledger.create(goal_id="dependent", task_digest=_digest("dependent task"), domain="science", now_ns=0)
        ledger.create(goal_id="cycle-a", task_digest=_digest("cycle a"), domain="data", now_ns=0)
        ledger.create(goal_id="cycle-b", task_digest=_digest("cycle b"), domain="operations", now_ns=0)
        failed = ledger.create(goal_id="retry", task_digest=_digest("retry task"), domain="evaluation", max_attempts=3, now_ns=0)
        failed = ledger.transition(failed.goal_id, "running", expected_revision=failed.revision, now_ns=1)
        ledger.transition(failed.goal_id, "failed", expected_revision=failed.revision, now_ns=2)
        schedule = schedule_autonomous_goals(
            ledger.list(limit=8),
            {
                "now_ns": 20,
                "max_selected": 2,
                "max_concurrent": 2,
                "max_cost": 3,
                "allow_failed_retry": True,
                "signals": [
                    AutonomousGoalSchedulingSignal("dependent", priority=1.0, urgency=1.0, dependencies=("base",), estimated_cost=2),
                    {"goal_id": "cycle-a", "dependencies": ["cycle-b"]},
                    {"goal_id": "cycle-b", "dependencies": ["cycle-a"]},
                    {"goal_id": "retry", "priority": 0.1},
                ],
            },
        )
        rows = {row.goal_id: row for row in schedule.rows}
        assert rows["cycle-a"].reason == "dependency_cycle"
        assert rows["cycle-b"].reason == "dependency_cycle"
        assert rows["dependent"].decision == "admit"
        assert rows["dependent"].unmet_dependencies == ()
        assert schedule.used_cost == 3
        claim = claim_autonomous_goals(ledger, schedule, now_ns=30)
        assert [item.goal_id for item in claim.claims] == ["base", "dependent"]
        assert ledger.get("dependent").status == "running"
        assert ledger.get("dependent").attempt == 1
        with pytest.raises(AutonomousGoalError, match="stale"):
            claim_autonomous_goals(ledger, schedule, now_ns=31)
        tampered = schedule.to_dict()
        tampered["selected_goal_ids"] = []
        with pytest.raises(AutonomousGoalError, match="schedule_digest"):
            validate_goal_schedule(tampered)


def test_goal_scheduler_admits_cross_domain_objectives() -> None:
    with AutonomousGoalLedger(clock=lambda: 100, max_goals=2) as ledger:
        ledger.create(goal_id="cross", task_digest=_digest("cross task"), domain="cross_domain", now_ns=0)
        schedule = schedule_autonomous_goals(
            ledger.list(limit=2),
            {"now_ns": 100, "max_selected": 1, "max_concurrent": 1, "required_domains": ["cross_domain"]},
        )
        assert schedule.selected_goal_ids == ("cross",)
        assert schedule.selected_domains == ("cross_domain",)
        assert schedule.missing_domains == ()
        assert "cross_domain" in AUTONOMOUS_GOAL_SCHEDULABLE_DOMAINS


def test_goal_worker_rehydrates_and_settles_every_domain_without_persisting_task_values() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains))
    for domain in domains:
        ledger.create(goal_id=f"worker-{domain}", task_digest=_digest(f"private task {domain}"), domain=domain, now_ns=0)
    observed_tasks: list[str] = []

    def resolve(goal, _row):
        return {"task": f"private task for {goal.domain}", "parameters": {"private": True}}

    def execute(request):
        observed_tasks.append(request.task)
        return {"status": "completed", "settlement_metadata": {"progress_digest": _digest(f"progress {request.goal.domain}")}}

    batch = AutonomousGoalWorker(ledger, resolver=resolve, executor=execute).run(
        schedule_options={
            "now_ns": 100,
            "max_selected": len(domains),
            "max_concurrent": len(domains),
            "required_domains": list(domains),
        }
    )
    assert len(observed_tasks) == len(domains)
    assert len(batch.runs) == len(domains)
    assert all(run.goal_status == "completed" for run in batch.runs)
    assert all(record.status == "completed" for record in ledger.list(limit=len(domains)))
    public = json.dumps(batch.to_dict())
    assert "private task for" not in public
    assert '"private"' not in public
    assert batch.to_dict()["counts"]["completed"] == len(domains)
    assert ledger.verify_integrity()["ok"] is True


def test_goal_worker_single_attempt_digest_matches_typescript_reference() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100)
    ledger.create(goal_id="parity", task_digest=_digest("private"), domain="coding", now_ns=0)
    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "private"},
        executor=lambda _request: {"status": "completed"},
    ).run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert batch.worker_digest == "ce6809a88e6a2c0c44748f9c3ec9e57b13915d8472f29da35ed8e1c1fc8baad2"


def test_goal_worker_converts_executor_failure_into_redacted_retry_state() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100)
    ledger.create(goal_id="failure", task_digest=_digest("private failure"), domain="operations", now_ns=0)

    def execute(_request):
        raise RuntimeError("private provider response must not cross the ledger boundary")

    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "private failure"},
        executor=execute,
    ).run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    run = batch.runs[0]
    assert run.execution_status == "failed"
    assert run.goal_status == "failed"
    assert run.error_class == "RuntimeError"
    assert run.error_digest is not None
    assert "private provider response" not in json.dumps(batch.to_dict())
    assert ledger.get("failure").status == "failed"
    assert ledger.get("failure").next_action_digest == _digest("goal-retry")


def test_goal_worker_journal_reconciles_pre_and_post_dispatch_restarts_without_replay() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    ledger.create(goal_id="pre", task_digest=_digest("pre task"), domain="coding", now_ns=0)
    ledger.create(goal_id="post", task_digest=_digest("post task"), domain="cross_domain", now_ns=0)
    schedule = schedule_autonomous_goals(
        ledger.list(limit=2),
        {"now_ns": 100, "max_selected": 2, "max_concurrent": 2, "required_domains": ["coding", "cross_domain"]},
    )
    claims = claim_autonomous_goals(ledger, schedule, now_ns=100)
    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    for claim in claims.claims:
        current = ledger.get(claim.goal_id)
        journal.record(
            batch_id="restart-batch",
            goal_id=claim.goal_id,
            phase="claimed",
            attempt=current.attempt,
            revision=current.revision,
            schedule_digest=schedule.schedule_digest,
            claim_digest=claims.claim_digest,
        )
    post = ledger.get("post")
    journal.record(
        batch_id="restart-batch",
        goal_id="post",
        phase="dispatch_started",
        attempt=post.attempt,
        revision=post.revision,
        schedule_digest=schedule.schedule_digest,
        claim_digest=claims.claim_digest,
    )
    recovery = journal.recover(ledger, now_ns=200)
    assert {row["goal_id"] for row in recovery["recovered"]} == {"pre", "post"}
    assert ledger.get("pre").status == "paused"
    assert ledger.get("pre").next_action_digest == _digest("goal-retry")
    assert ledger.get("post").status == "blocked"
    assert ledger.get("post").next_action_digest == _digest("goal-reconciliation-review")
    assert journal.active() == ()
    snapshot = journal.snapshot()
    restored = AutonomousGoalWorkerJournal(clock=lambda: 300)
    assert restored.restore(snapshot)["head_digest"] == snapshot["head_digest"]
    tampered = json.loads(json.dumps(snapshot))
    tampered["events"][0]["event_digest"] = "0" * 64
    with pytest.raises(AutonomousGoalError, match="digest"):
        restored.restore(tampered)

    class _Store:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

    store = _Store()
    coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        journal,
        JsonAutonomousGoalWorkerJournalPersistence(store),
    )
    flushed = coordinator.flush()
    assert coordinator.restore()["snapshot_digest"] == flushed["snapshot_digest"]


def test_goal_control_loop_continues_all_domains_and_retries_paused_work_with_fresh_signals() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains) + 1)
    for domain in domains:
        ledger.create(goal_id=f"loop-{domain}", task_digest=_digest(f"private loop task {domain}"), domain=domain, now_ns=0)
    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    seen_cycles: list[int] = []
    worker = AutonomousGoalWorker(
        ledger,
        journal=journal,
        resolver=lambda goal, _row: {"task": f"private loop task {goal.domain}"},
        executor=lambda request: {"status": "completed"},
    )
    loop = AutonomousGoalControlLoop(worker, batch_id_prefix="all-domain-loop")

    def signals(context):
        seen_cycles.append(context.cycle)
        return {"signals": [{"goal_id": "loop-coding", "priority": 1.0, "urgency": 1.0}]}

    result = loop.run(
        schedule_options={
            "now_ns": 100,
            "max_selected": len(domains),
            "max_concurrent": len(domains),
            "required_domains": list(domains),
        },
        options_factory=signals,
        max_cycles=4,
    )
    assert result.stop_reason == "all_terminal"
    assert len(result.cycles) == 1
    assert result.total_runs == len(domains)
    assert result.domain_counts == {domain: 1 for domain in domains}
    assert seen_cycles == [1]
    assert journal.active() == ()
    public = json.dumps(result.to_dict())
    assert "private loop task" not in public
    assert all(record.status == "completed" for record in ledger.list(limit=len(domains)))

    retry_ledger = AutonomousGoalLedger(clock=lambda: 200)
    retry_ledger.create(goal_id="paused-loop", task_digest=_digest("private paused loop"), domain="evaluation", now_ns=0)
    calls = {"count": 0}

    def execute_once_then_complete(_request):
        calls["count"] += 1
        return {"status": "paused" if calls["count"] == 1 else "completed"}

    retry_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            retry_ledger,
            resolver=lambda _goal, _row: {"task": "private paused loop"},
            executor=execute_once_then_complete,
        )
    )
    resumed = retry_loop.run(schedule_options={"now_ns": 200, "max_selected": 1, "max_concurrent": 1, "include_paused": True}, max_cycles=3)
    assert resumed.stop_reason == "all_terminal"
    assert len(resumed.cycles) == 2
    assert calls["count"] == 2
    assert retry_ledger.get("paused-loop").status == "completed"

    failure_ledger = AutonomousGoalLedger(clock=lambda: 300)
    failure_ledger.create(goal_id="failed-loop", task_digest=_digest("private failed loop"), domain="operations", max_attempts=2, now_ns=0)
    failed = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            failure_ledger,
            resolver=lambda _goal, _row: {"task": "private failed loop"},
            executor=lambda _request: (_ for _ in ()).throw(RuntimeError("private failure")),
        )
    ).run(schedule_options={"now_ns": 300, "max_selected": 1, "max_concurrent": 1}, max_cycles=2)
    assert failed.stop_reason == "no_admissible_work"
    assert failure_ledger.get("failed-loop").status == "failed"


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


def test_goal_snapshots_rehydrate_all_domains_and_fence_stale_writers(tmp_path: Path) -> None:
    backend = _CasTextStore()
    persistence = TransactionalJsonAutonomousGoalSnapshotPersistence(backend)
    source = AutonomousGoalLedger(str(tmp_path / "source-goals.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS) + 1)
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        source.create(
            goal_id=f"snapshot-{domain}",
            task_digest=_digest(f"snapshot task {domain}"),
            domain=domain,
            capability="review",
            risk_class="read_only",
            now_ns=index + 1,
        )
    source_coordinator = AutonomousGoalPersistenceCoordinator(source, persistence)
    flushed = source_coordinator.flush()
    assert flushed["sequence"] == len(AUTONOMOUS_DOMAINS)
    assert flushed["head_digest"] == flushed["events"][-1]["event_digest"]

    restored = AutonomousGoalLedger(str(tmp_path / "restored-goals.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS))
    restored_snapshot = AutonomousGoalPersistenceCoordinator(restored, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {record.domain for record in restored.list(limit=128)} == set(AUTONOMOUS_DOMAINS)
    assert restored.verify_integrity()["ok"] is True

    stale = AutonomousGoalLedger(str(tmp_path / "stale-goals.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS) + 1)
    stale_coordinator = AutonomousGoalPersistenceCoordinator(stale, persistence)
    stale_coordinator.restore()
    source.create(
        goal_id="snapshot-new",
        task_digest=_digest("snapshot new task"),
        domain=AUTONOMOUS_DOMAINS[0],
        now_ns=99,
    )
    source_coordinator.flush()
    with pytest.raises(AutonomousGoalError, match="compare-and-swap conflict"):
        stale_coordinator.flush()

    tampered = json.loads(backend.value)
    tampered["events"][0]["event_digest"] = "0" * 64
    backend.value = json.dumps(tampered)
    tampered_ledger = AutonomousGoalLedger(str(tmp_path / "tampered-goals.sqlite3"))
    with pytest.raises(AutonomousGoalError, match="digest"):
        AutonomousGoalPersistenceCoordinator(tampered_ledger, persistence).restore()
    source.close()
    restored.close()
    stale.close()
    tampered_ledger.close()


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
        ledger.transition("bounded-goal", "running", expected_revision=0)
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


def test_goal_learning_wrapper_settles_value_only_bandit_and_replan_identities(tmp_path: Path) -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    learning_result = SimpleNamespace(
        status="completed",
        bandit_state={"generation": 4, "arms": [{"arm_id": "model-a", "pulls": 2}]},
        evaluations=[
            {
                "decision": {
                    "evaluator_id": "coding-quality",
                    "evaluator_version": "v1",
                    "reward": 1.0,
                    "passed": True,
                    "failed": False,
                    "replan_requested": False,
                    "replan_instruction": "transient provider-shaped guidance must not persist",
                },
                "recording": {"status": "settled", "credited_reward": 1.0},
            }
        ],
        attempts=[SimpleNamespace(status="completed", run_id="cycle-42-attempt-0")],
        replan_count=0,
    )
    observed = {}

    def run_learning(**kwargs):
        observed.update(kwargs)
        return learning_result

    orchestrator.run_learning = run_learning
    with AutonomousGoalLedger(str(tmp_path / "goal-learning.sqlite3")) as ledger:
        completed = orchestrator.run_goal_learning_step(
            goal_store=ledger,
            goal_id="learning-goal",
            task="adapt model selection for a coding review",
            domain="coding",
            bandit_state={"generation": 3, "arms": [{"arm_id": "model-a", "pulls": 1}]},
            learning_mode="replan",
            max_replans=2,
            cycle_id="cycle-42",
            goal_criteria=[{"criterion_id": "quality", "criterion_digest": _digest("quality")}],
            criterion_updates=[{"criterion_id": "quality", "status": "satisfied", "evidence_digest": _digest("quality receipt")}],
        )
        assert completed["goal_status"] == "completed"
        assert completed["goal"]["learning_state_digest"] is not None
        assert completed["goal"]["evaluator_digest"] is not None
        assert completed["goal"]["progress_digest"] is not None
        assert observed["learn"] is True
        assert "run_id" not in observed
        serialized = json.dumps([record.to_dict() for record in ledger.list(limit=1)], sort_keys=True)
        assert "adapt model selection" not in serialized
        assert "transient provider-shaped guidance" not in serialized
        assert ledger.verify_integrity()["ok"] is True


def test_cross_domain_goal_learning_wrapper_selects_online_runner_without_provider_keys(tmp_path: Path) -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    result = SimpleNamespace(
        status="completed",
        bandit_state={"generation": 2},
        evaluations=[{"decision": {"evaluator_id": "cross", "evaluator_version": "v1", "reward": 0.8, "passed": True}}],
        cross_domain=SimpleNamespace(
            status="completed",
            child_results=(SimpleNamespace(status="completed"), SimpleNamespace(status="completed")),
            synthesis_result=SimpleNamespace(status="completed"),
        ),
    )
    calls = []
    orchestrator.run_cross_domain_learning = lambda **kwargs: (calls.append(kwargs) or result)
    subtasks = [{"domain": "coding", "task": "inspect"}, {"domain": "science", "task": "compare"}]
    with AutonomousGoalLedger(str(tmp_path / "cross-learning.sqlite3")) as ledger:
        completed = orchestrator.run_cross_domain_goal_learning_step(
            goal_store=ledger,
            goal_id="cross-learning-goal",
            task="coordinate adaptive cross-domain review",
            subtasks=subtasks,
            bandit_state={"generation": 1},
            learning_mode="online",
            cycle_id="cross-cycle-7",
        )
        assert completed["goal_status"] == "completed"
        assert calls and calls[0]["bandit_state"] == {"generation": 1}
        serialized = json.dumps([record.to_dict() for record in ledger.list(limit=1)], sort_keys=True)
        assert "coordinate adaptive" not in serialized
        assert "inspect" not in serialized
        assert "compare" not in serialized
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
