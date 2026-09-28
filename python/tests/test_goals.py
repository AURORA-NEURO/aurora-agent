import asyncio
import json
import hashlib
import hmac
import sqlite3
import time
from collections.abc import Mapping
from pathlib import Path
from threading import Barrier, Event, Lock, Thread, get_ident
from types import SimpleNamespace

import pytest

from prism_sdk.autonomy import AUTONOMOUS_DOMAINS, AutonomousAgent, AutonomousTaskOrchestrator
from prism_sdk.authoring import AuthoringError, canonical_json, content_digest
from prism_sdk.autonomous_action_admission_controller import AutonomousActionAdmissionController
from prism_sdk.autonomous_action_admission_persistence import InMemoryAutonomousActionAdmissionLedger
from prism_sdk.llm_runtime import LLMRuntime, ProviderInvocationMetadata, ProviderResponse
from prism_sdk.goals import (
    GOAL_AUTH_SCHEMA,
    MAX_AUTHENTICATED_GOAL_SNAPSHOT_BYTES,
    AuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence,
    AutonomousGoalConflict,
    AutonomousGoalError,
    AutonomousGoalLedger,
    AutonomousGoalPersistenceCoordinator,
    MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence,
    AutonomousGoalRecord,
    JsonAutonomousGoalSnapshotPersistence,
    TransactionalJsonAutonomousGoalSnapshotPersistence,
    goal_task_digest,
    migrate_legacy_goal_snapshot,
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
from prism_sdk.autonomous_goal_control_loop import AutonomousGoalBanditLearner, AutonomousGoalControlLoop
from prism_sdk.autonomous_goal_control_persistence import (
    AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA,
    AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
    AutonomousGoalControlLoopPersistenceCoordinator,
    JsonAutonomousGoalControlLoopSnapshotPersistence,
    TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence,
    seal_autonomous_goal_control_loop_snapshot,
    migrate_legacy_autonomous_goal_control_loop_snapshot,
    validate_autonomous_goal_control_loop_snapshot,
)
from prism_sdk.autonomous_goal_preview import (
    InMemoryAutonomousGoalPreviewAdmissionLedger,
    AutonomousGoalPreviewAdmissionPersistenceCoordinator,
    TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence,
    create_autonomous_goal_preview_admission_record,
    revoke_autonomous_goal_preview_admission_record,
    verify_autonomous_goal_preview_approval,
)
from prism_sdk.autonomous_goal_recovery import AutonomousGoalRecoveryCoordinator, validate_autonomous_goal_recovery_report
from prism_sdk.autonomous_goal_agent import AutonomousGoalAgentRuntime
from prism_sdk.autonomous_run_trace import InMemoryAutonomousRunTraceStore
from prism_sdk.autonomous_run_trace_registry import AutonomousRunTraceRegistry
from prism_sdk.autonomous_goal_worker_journal import (
    AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA,
    AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01,
    AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence,
    GOAL_DISPATCH_RESOLUTION_RETENTION,
    GOAL_DISPATCH_RESOLUTION_SCHEMA,
    GOAL_DISPATCH_RESOLUTION_SCHEMA_V01,
    GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01,
    GOAL_WORKER_JOURNAL_RETENTION,
    GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    AutonomousGoalWorkerJournal,
    MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence,
    JsonAutonomousGoalWorkerJournalPersistence,
    AutonomousGoalWorkerJournalPersistenceCoordinator,
    migrate_legacy_autonomous_goal_worker_journal_snapshot,
    migrate_legacy_authenticated_autonomous_goal_worker_journal_envelope,
    validate_goal_dispatch_resolution,
)
from prism_sdk.autonomous_protected_rehydration import (
    AutonomousProtectedRehydrationAdapter,
    AutonomousProtectedRehydrationBoundary,
    AutonomousProtectedRehydrationContext,
)


def _digest(value: str) -> str:
    return goal_task_digest(value)


def _status_verifier(*, verifier_id: str = "deployment.status-adapter.v1", accepted: bool = True):
    return SimpleNamespace(verifier_id=verifier_id, verify=lambda _receipt: accepted)


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
        assert schedule.schedule_digest == "f3415809692fee49b5bca897c2c2376c65570800af1d5875e3770d0b3f6d3587"


def test_goal_scheduler_handles_maximum_supported_dependency_chain_without_recursion() -> None:
    count = 4_096
    goals = tuple(
        AutonomousGoalRecord(
            goal_id=f"goal-{index:04d}",
            task_digest="0" * 64,
            domain="coding",
            capability=None,
            risk_class=None,
            status="ready",
            attempt=0,
            max_attempts=1,
            revision=0,
            created_ns=0,
            updated_ns=0,
        )
        for index in range(count)
    )
    schedule = schedule_autonomous_goals(
        goals,
        {
            "now_ns": 100,
            "max_selected": 128,
            "max_concurrent": 128,
            "signals": [
                {
                    "goal_id": f"goal-{index:04d}",
                    "dependencies": [f"goal-{index + 1:04d}"] if index + 1 < count else [],
                }
                for index in range(count)
            ],
        },
    )
    assert len(schedule.rows) == count
    assert len(schedule.selected_goal_ids) == 128
    assert schedule.selected_goal_ids == tuple(
        f"goal-{index:04d}" for index in range(count - 1, count - 129, -1)
    )


def test_schema_0_2_goal_replay_artifacts_require_canonical_timestamp_strings() -> None:
    ledger = AutonomousGoalLedger()
    goal = ledger.create(goal_id="wire-time", task_digest=_digest("wire time"), domain="coding", now_ns="100")
    schedule = schedule_autonomous_goals(
        [goal],
        {"now_ns": "200", "signals": [{"goal_id": goal.goal_id, "deadline_ns": "300"}]},
    )
    numeric_now = schedule.to_dict()
    numeric_now["now_ns"] = 200
    numeric_now.pop("schedule_digest")
    numeric_now["schedule_digest"] = content_digest(numeric_now)
    with pytest.raises(AutonomousGoalError, match="canonical decimal-string wire format"):
        validate_goal_schedule(numeric_now)

    numeric_deadline = schedule.to_dict()
    numeric_deadline["rows"][0]["deadline_ns"] = 300
    numeric_deadline.pop("schedule_digest")
    numeric_deadline["schedule_digest"] = content_digest(numeric_deadline)
    with pytest.raises(AutonomousGoalError, match="canonical decimal-string wire format"):
        validate_goal_schedule(numeric_deadline)

    preview_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(ledger, resolver=lambda _goal, _row: {"task": "wire time"}, executor=lambda _request: {"status": "completed"})
    )
    nested_preview = preview_loop.preview(schedule_options={"now_ns": "200"}).to_dict()
    nested_preview["schedule"]["now_ns"] = 200
    nested_schedule = nested_preview["schedule"]
    nested_schedule.pop("schedule_digest")
    nested_schedule["schedule_digest"] = content_digest(nested_schedule)
    nested_preview.pop("preview_digest")
    nested_preview["preview_digest"] = content_digest(nested_preview)
    with pytest.raises(AutonomousGoalError, match="canonical decimal-string wire format"):
        create_autonomous_goal_preview_admission_record(
            nested_preview,
            admission_id="wire-preview",
            issued_at_ns="200",
            expires_at_ns="300",
        )


def test_goal_schedule_rejects_unmodeled_nested_fields_even_when_restamped() -> None:
    ledger = AutonomousGoalLedger()
    goal = ledger.create(goal_id="closed-schedule", task_digest=_digest("closed task"), domain="coding", now_ns=0)
    schedule = schedule_autonomous_goals([goal], {"now_ns": 100})

    row_with_payload = schedule.to_dict()
    row_with_payload["rows"][0]["private_task"] = "must not survive schedule validation"
    row_with_payload.pop("schedule_digest")
    row_with_payload["schedule_digest"] = content_digest(row_with_payload)
    with pytest.raises(AutonomousGoalError, match="schedule row contains unsupported fields"):
        validate_goal_schedule(row_with_payload)

    coverage_with_payload = schedule.to_dict()
    coverage_with_payload["coverage"]["private_task"] = "must not survive schedule validation"
    coverage_with_payload.pop("schedule_digest")
    coverage_with_payload["schedule_digest"] = content_digest(coverage_with_payload)
    with pytest.raises(AutonomousGoalError, match="schedule coverage contains unsupported fields"):
        validate_goal_schedule(coverage_with_payload)


def test_goal_bandit_separates_capability_and_risk_contexts_with_deterministic_restore() -> None:
    goals = [
        {"goal_id": "coding-low-risk", "domain": "coding", "capability": "implementation", "risk_class": "low", "status": "ready"},
        {"goal_id": "coding-high-risk", "domain": "coding", "capability": "implementation", "risk_class": "high", "status": "ready"},
    ]
    learner = AutonomousGoalBanditLearner(exploration=0.35)
    learner.update(
        [
            {"goal_id": "coding-low-risk", "domain": "coding", "reward": -1.0, "passed": False},
            {"goal_id": "coding-high-risk", "domain": "coding", "reward": 0.75, "passed": True},
        ],
        goals,
    )
    snapshot = learner.snapshot()
    assert snapshot["retention"] == "value_only_goal_contextual_bandit_state"
    assert len(snapshot["arms"]) == 2
    assert {row["risk_class"] for row in snapshot["arms"]} == {"low", "high"}
    assert all(len(row["arm_id"]) == 64 for row in snapshot["arms"])
    assert snapshot["arms"][0]["domain"] == "coding"
    assert learner.update([], goals)["signals"][0]["goal_id"] == "coding-high-risk"
    restored = AutonomousGoalBanditLearner(state=snapshot)
    assert restored.snapshot() == snapshot
    checkpoint = seal_autonomous_goal_control_loop_snapshot(
        {
            "schema": AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA,
            "run_id": "contextual-bandit-checkpoint",
            "next_cycle": 1,
            "cycle_summaries": [],
            "previous_cycle": None,
            "completed_cycles": 0,
            "total_selected": 0,
            "total_claimed": 0,
            "total_runs": 0,
            "status_counts": {},
            "domain_counts": {},
            "evaluation_count": 0,
            "evaluation_digests": [],
            "learning_state_digest": None,
            "learned_signals": [],
            "learner_state": snapshot,
            "stop_reason": "cycle_budget_exhausted",
            "generation": 1,
            "previous_snapshot_digest": None,
            "retention": "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
            "secret_material": "never_returned",
        }
    )
    assert validate_autonomous_goal_control_loop_snapshot(checkpoint)["learner_state"] == snapshot


def test_goal_control_loop_preview_is_provider_free_and_explains_all_domain_admission() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains) + 1)
    for domain in domains:
        ledger.create(goal_id=f"preview-{domain}", task_digest=_digest(f"private preview task {domain}"), domain=domain, now_ns=0)
    ledger.create(goal_id="preview-blocked", task_digest=_digest("private preview blocked task"), domain="evaluation", now_ns=0)
    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    calls = {"resolve": 0, "execute": 0}
    learner = AutonomousGoalBanditLearner()

    def must_not_resolve(_goal, _row):
        calls["resolve"] += 1
        raise AssertionError("preview must not rehydrate tasks")

    def must_not_execute(_request):
        calls["execute"] += 1
        raise AssertionError("preview must not dispatch work")

    loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(ledger, journal=journal, resolver=must_not_resolve, executor=must_not_execute),
        evaluator=lambda _cycle: (),
        learner=learner,
    )
    preview_options = {
        "now_ns": 100,
        "max_selected": len(domains),
        "max_concurrent": len(domains),
        "required_domains": list(domains),
        "signals": [{"goal_id": "preview-blocked", "dependencies": ["missing-preview-goal"], "priority": 1.0}],
    }
    preview = loop.preview(schedule_options=preview_options)
    assert preview.preview_digest == loop.preview(schedule_options=preview_options).preview_digest
    assert preview.status == "admissible_work"
    assert preview.eligible_goal_count == len(domains) + 1
    assert set(preview.schedule.selected_domains) == set(domains)
    assert preview.schedule.missing_domains == ()
    assert preview.dependency_blocked_goal_ids == ("preview-blocked",)
    assert preview.reason_counts["dependency_not_ready"] == 1
    assert preview.learning_state_digest == learner.snapshot()["state_digest"]
    assert calls == {"resolve": 0, "execute": 0}
    assert journal.events() == ()
    assert learner.snapshot()["generation"] == 0
    public = json.dumps(preview.to_dict(), sort_keys=True)
    assert "private preview task" not in public
    assert "private preview blocked task" not in public
    facade_preview = AutonomousAgent(None, LLMRuntime()).preview_goal_control_loop(ledger, schedule_options=preview_options)
    assert facade_preview.schedule.schedule_digest == preview.schedule.schedule_digest
    preview_runtime = AutonomousAgent(None, LLMRuntime()).goal_agent_runtime(ledger, task_resolver=None)
    assert preview_runtime.metadata()["task_rehydration"] == "not_configured_preview_only"
    with pytest.raises(AutonomousGoalError, match="task rehydration is not configured"):
        preview_runtime.run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})


def test_goal_control_loop_preview_reports_terminal_and_policy_blocked_states() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 200, max_goals=2)
    ledger.create(goal_id="preview-completed", task_digest=_digest("private completed preview"), domain="coding", now_ns=0)
    ledger.transition("preview-completed", "running", expected_revision=0, now_ns=1)
    ledger.transition("preview-completed", "completed", expected_revision=1, now_ns=2)
    terminal_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(ledger, resolver=lambda _goal, _row: {"task": "private completed preview"}, executor=lambda _request: {"status": "completed"})
    )
    terminal = terminal_loop.preview(schedule_options={"now_ns": 200})
    assert terminal.status == "all_terminal"
    assert terminal.schedule.selected_goal_ids == ()

    blocked_ledger = AutonomousGoalLedger(clock=lambda: 201, max_goals=1)
    blocked_ledger.create(goal_id="preview-failed", task_digest=_digest("private failed preview"), domain="operations", max_attempts=1, now_ns=0)
    blocked_ledger.transition("preview-failed", "running", expected_revision=0, now_ns=1)
    blocked_ledger.transition("preview-failed", "failed", expected_revision=1, now_ns=2)
    blocked_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(blocked_ledger, resolver=lambda _goal, _row: {"task": "private failed preview"}, executor=lambda _request: {"status": "completed"})
    )
    blocked = blocked_loop.preview(schedule_options={"now_ns": 201, "allow_failed_retry": True})
    assert blocked.status == "no_admissible_work"
    assert blocked.reason_counts["retry_budget_exhausted"] == 1


def test_goal_control_loop_requires_an_unchanged_preview_before_dispatch() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 250, max_goals=2)
    ledger.create(goal_id="preview-bound-a", task_digest=_digest("private preview bound a"), domain="coding", now_ns=0)
    calls = {"resolve": 0, "execute": 0}

    def resolve(goal, _row):
        calls["resolve"] += 1
        return {"task": f"private preview bound {goal.goal_id[-1]}"}

    def execute(_request):
        calls["execute"] += 1
        return {"status": "completed"}

    loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            resolver=resolve,
            executor=execute,
        )
    )
    schedule_options = {"now_ns": 250, "max_selected": 1, "max_concurrent": 1}
    preview = loop.preview(schedule_options=schedule_options)
    ledger.create(goal_id="preview-bound-b", task_digest=_digest("private preview bound b"), domain="science", now_ns=0)
    with pytest.raises(AutonomousGoalError, match="expected_preview_digest"):
        loop.run(schedule_options=schedule_options, max_total_runs=1, expected_preview_digest=preview.preview_digest)
    assert calls == {"resolve": 0, "execute": 0}

    result = loop.run(
        schedule_options=schedule_options,
        max_total_runs=1,
        expected_preview_digest=loop.preview(schedule_options=schedule_options).preview_digest,
    )
    assert result.stop_reason == "run_budget_exhausted"
    assert calls == {"resolve": 1, "execute": 1}


def test_goal_preview_admission_is_operator_reviewed_expiring_persisted_and_all_domain_bound(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("prism_sdk.autonomous_goal_control_loop.time.time_ns", lambda: 100)
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains))
    for domain in domains:
        ledger.create(goal_id=f"approval-{domain}", task_digest=_digest(f"approval task {domain}"), domain=domain, now_ns=0)
    calls = {"resolve": 0, "execute": 0}

    def resolve(goal, _row):
        calls["resolve"] += 1
        return {"task": f"approval task {goal.domain}"}

    def execute(_request):
        calls["execute"] += 1
        return {"status": "completed"}

    options = {
        "now_ns": 100,
        "max_selected": len(domains),
        "max_concurrent": len(domains),
        "required_domains": list(domains),
    }
    loop = AutonomousGoalControlLoop(AutonomousGoalWorker(ledger, resolver=resolve, executor=execute))
    preview = loop.preview(schedule_options=options)
    admissions = InMemoryAutonomousGoalPreviewAdmissionLedger(max_records=4)
    submitted = admissions.submit(
        preview,
        admission_id="all-domain-preview",
        issued_at_ns=100,
        expires_at_ns=100 + 1_000_000_000,
        requested_by_digest=content_digest("operator-requester"),
        reason="operator review of the exact all-domain admission",
    )
    approved = admissions.review(
        "all-domain-preview",
        approved=True,
        reviewer_digest=content_digest("operator-reviewer"),
        reason="approved after reviewing the bounded schedule",
        expected_record_digest=submitted["record_digest"],
    )
    verified = verify_autonomous_goal_preview_approval(approved, current_preview_digest=preview.preview_digest, now_ns=100)
    assert verified["status"] == "approved"
    with pytest.raises(AutonomousGoalError, match="not yet valid"):
        verify_autonomous_goal_preview_approval(approved, current_preview_digest=preview.preview_digest, now_ns=99)
    with pytest.raises(AutonomousGoalError, match="expired"):
        verify_autonomous_goal_preview_approval(approved, current_preview_digest=preview.preview_digest, now_ns=approved["expires_at_ns"])

    exact_ns_issued = "1700000000000000000"
    exact_ns_submitted = admissions.submit(
        preview,
        admission_id="exact-ns-preview",
        issued_at_ns=exact_ns_issued,
        expires_at_ns="1700000001000000000",
    )
    exact_ns_approved = admissions.review(
        "exact-ns-preview",
        approved=True,
        reviewer_digest=content_digest("operator-reviewer"),
        expected_record_digest=exact_ns_submitted["record_digest"],
    )
    assert exact_ns_approved["issued_at_ns"] == exact_ns_issued
    assert verify_autonomous_goal_preview_approval(exact_ns_approved, current_preview_digest=preview.preview_digest, now_ns="1700000000500000000")["status"] == "approved"

    legacy_approval = dict(exact_ns_approved)
    legacy_approval["schema"] = "bioprism-autonomous-goal-preview-admission-record/0.1"
    legacy_approval.pop("record_digest")
    legacy_approval["issued_at_ns"] = 1_700_000_000_000
    legacy_approval["expires_at_ns"] = 1_700_000_001_000
    legacy_approval["record_digest"] = _canonical_digest(legacy_approval)
    with pytest.raises(AutonomousGoalError, match="must be re-reviewed"):
        verify_autonomous_goal_preview_approval(legacy_approval, current_preview_digest=preview.preview_digest, now_ns="1700000000500000000")

    rejected = admissions.submit(preview, admission_id="rejected-preview", issued_at_ns=100, expires_at_ns=100 + 1_000_000_000)
    rejected = admissions.review("rejected-preview", approved=False, reviewer_digest=content_digest("operator-reviewer"), expected_record_digest=rejected["record_digest"])
    with pytest.raises(AutonomousGoalError, match="not approved"):
        verify_autonomous_goal_preview_approval(rejected, current_preview_digest=preview.preview_digest, now_ns=100)

    revoked = admissions.revoke(
        "all-domain-preview",
        reviewer_digest=content_digest("operator-reviewer"),
        reason="operator policy changed before dispatch",
        expected_record_digest=approved["record_digest"],
    )
    assert revoked["status"] == "revoked"
    assert revoke_autonomous_goal_preview_admission_record(
        approved,
        reviewer_digest=content_digest("operator-reviewer"),
        reason="operator policy changed before dispatch",
        expected_record_digest=approved["record_digest"],
    )["record_digest"] == revoked["record_digest"]
    with pytest.raises(AutonomousGoalError, match="not approved"):
        verify_autonomous_goal_preview_approval(revoked, current_preview_digest=preview.preview_digest, now_ns=100)
    with pytest.raises(AutonomousGoalError, match="only an approved"):
        admissions.revoke("all-domain-preview", reviewer_digest=content_digest("operator-reviewer"), reason="duplicate revoke")

    stale = dict(approved)
    stale["preview_digest"] = "0" * 64
    with pytest.raises(AutonomousGoalError, match="digest"):
        admissions.put(stale)

    class Store:
        encoded: str | None = None

        def read(self) -> str | None:
            return self.encoded

        def write(self, value: str) -> None:
            self.encoded = value

        def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
            actual = None if self.encoded is None else json.loads(self.encoded)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.encoded = value
            return True

    store = Store()
    persistence = TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence(store)
    coordinator = AutonomousGoalPreviewAdmissionPersistenceCoordinator(admissions, persistence)
    first_snapshot = coordinator.flush()
    restored = InMemoryAutonomousGoalPreviewAdmissionLedger(max_records=4)
    restored_coordinator = AutonomousGoalPreviewAdmissionPersistenceCoordinator(restored, persistence)
    assert restored_coordinator.restore()["snapshot_digest"] == first_snapshot["snapshot_digest"]
    stale_coordinator = AutonomousGoalPreviewAdmissionPersistenceCoordinator(InMemoryAutonomousGoalPreviewAdmissionLedger(max_records=4), persistence)
    stale_coordinator.restore()
    admissions.submit(preview, admission_id="third-preview", issued_at_ns=100, expires_at_ns=100 + 1_000_000_000)
    coordinator.flush()
    with pytest.raises(AutonomousGoalError, match="compare-and-swap"):
        stale_coordinator.flush()

    gated_ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains))
    for domain in domains:
        gated_ledger.create(goal_id=f"approval-{domain}", task_digest=_digest(f"approval task {domain}"), domain=domain, now_ns=0)
    gated_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(gated_ledger, resolver=resolve, executor=execute),
        preview_admission_ledger=admissions,
    )
    with pytest.raises(AutonomousGoalError, match="stale relative to the live admission ledger"):
        gated_loop.run(schedule_options=options, max_cycles=1, max_total_runs=len(domains), preview_approval=approved)
    assert calls == {"resolve": 0, "execute": 0}

    ledger.transition("approval-coding", "running", expected_revision=0, now_ns=101)
    with pytest.raises(AutonomousGoalError, match="expected_preview_digest|current preview"):
        loop.run(schedule_options=options, max_cycles=1, max_total_runs=len(domains), preview_approval=approved)
    assert calls == {"resolve": 0, "execute": 0}
    with pytest.raises(AutonomousGoalError, match="scoped to one scheduler cycle"):
        loop.run(schedule_options=options, max_cycles=2, max_total_runs=len(domains), preview_approval=approved)

    fresh_ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains))
    for domain in domains:
        fresh_ledger.create(goal_id=f"approval-{domain}", task_digest=_digest(f"approval task {domain}"), domain=domain, now_ns=0)
    fresh_loop = AutonomousGoalControlLoop(AutonomousGoalWorker(fresh_ledger, resolver=resolve, executor=execute))
    result = fresh_loop.run(schedule_options=options, max_cycles=1, max_total_runs=len(domains), preview_approval=approved)
    assert result.stop_reason == "all_terminal"
    assert calls == {"resolve": len(domains), "execute": len(domains)}


def test_goal_control_loop_checks_approval_expiry_against_live_time_when_schedule_clock_is_pinned(monkeypatch: pytest.MonkeyPatch) -> None:
    now = 1_000
    monkeypatch.setattr("prism_sdk.autonomous_goal_control_loop.time.time_ns", lambda: now)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(goal_id="approval-expiry", task_digest=_digest("approval expiry task"), domain="coding", now_ns=0)
    calls = {"resolve": 0, "execute": 0}

    def resolve(_goal, _row):
        calls["resolve"] += 1
        return {"task": "approval expiry task"}

    def execute(_request):
        calls["execute"] += 1
        return {"status": "completed"}

    loop = AutonomousGoalControlLoop(AutonomousGoalWorker(ledger, resolver=resolve, executor=execute))
    schedule_options = {"now_ns": 100, "max_selected": 1, "max_concurrent": 1}
    preview = loop.preview(schedule_options=schedule_options)
    admissions = InMemoryAutonomousGoalPreviewAdmissionLedger()
    submitted = admissions.submit(preview, admission_id="approval-expiry", issued_at_ns=now, expires_at_ns=now + 100)
    approved = admissions.review("approval-expiry", approved=True, reviewer_digest=_digest("reviewer"), expected_record_digest=submitted["record_digest"])

    now = 1_200
    with pytest.raises(AutonomousGoalError, match="expired"):
        loop.run(schedule_options=schedule_options, max_cycles=1, max_total_runs=1, preview_approval=approved)
    assert calls == {"resolve": 0, "execute": 0}
    assert ledger.get("approval-expiry").status == "ready"


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
        ledger.create(goal_id=f"worker-{domain}", task_digest=_digest(f"private task for {domain}"), domain=domain, now_ns=0)
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


def test_goal_worker_persists_dispatch_intent_before_executor_and_fails_closed_on_refusal() -> None:
    store = _CasTextStore()
    ledger = AutonomousGoalLedger(clock=lambda: 10, max_goals=1)
    ledger.create(goal_id="persist-before-execute", task_digest=_digest("durable task"), domain="coding", now_ns=0)
    journal = AutonomousGoalWorkerJournal(clock=lambda: 11)
    journal_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        journal,
        JsonAutonomousGoalWorkerJournalPersistence(store),
    )
    order: list[str] = []

    def persist_intent(event):
        assert event.phase == "dispatch_started"
        journal_coordinator.flush()
        durable = json.loads(store.value)
        assert durable["events"][-1]["event_digest"] == event.event_digest
        order.append("persisted")

    def execute(_request):
        durable = json.loads(store.value)
        assert durable["events"][-1]["phase"] == "dispatch_started"
        order.append("executor")
        return {"status": "completed"}

    AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "durable task"},
        executor=execute,
        journal=journal,
        persist_dispatch_intent=persist_intent,
    ).run(
        schedule_options={"now_ns": 10, "max_selected": 1, "max_concurrent": 1},
        batch_id="persist-intent-batch",
    )
    assert order == ["persisted", "executor"]

    async_ledger = AutonomousGoalLedger(clock=lambda: 12, max_goals=1)
    async_ledger.create(goal_id="async-persist-before-execute", task_digest=_digest("async durable task"), domain="coding", now_ns=0)
    async_journal = AutonomousGoalWorkerJournal(clock=lambda: 13)
    async_store = _CasTextStore()
    async_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        async_journal,
        JsonAutonomousGoalWorkerJournalPersistence(async_store),
    )
    async_order: list[str] = []

    async def persist_async_intent(event):
        await asyncio.sleep(0)
        async_coordinator.flush()
        assert json.loads(async_store.value)["events"][-1]["event_digest"] == event.event_digest
        async_order.append("persisted")

    async def run_async_worker():
        await AutonomousGoalWorker(
            async_ledger,
            resolver=lambda _goal, _row: {"task": "async durable task"},
            executor=lambda _request: async_order.append("executor") or {"status": "completed"},
            journal=async_journal,
            persist_dispatch_intent=persist_async_intent,
        ).run_async(
            schedule_options={"now_ns": 12, "max_selected": 1, "max_concurrent": 1},
            batch_id="async-persist-intent-batch",
        )

    asyncio.run(run_async_worker())
    assert async_order == ["persisted", "executor"]

    parallel_ledger = AutonomousGoalLedger(clock=lambda: 30, max_goals=2)
    for goal_id in ("parallel-persist-a", "parallel-persist-b"):
        parallel_ledger.create(goal_id=goal_id, task_digest=_digest(goal_id), domain="coding", now_ns=0)
    parallel_journal = AutonomousGoalWorkerJournal(clock=lambda: 31)
    parallel_store = _CasTextStore()
    parallel_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        parallel_journal,
        JsonAutonomousGoalWorkerJournalPersistence(parallel_store),
    )
    stats_lock = Lock()
    active_persisters = 0
    max_active_persisters = 0
    persisted_goals: set[str] = set()

    def persist_parallel_intent(event):
        nonlocal active_persisters, max_active_persisters
        with stats_lock:
            active_persisters += 1
            max_active_persisters = max(max_active_persisters, active_persisters)
        time.sleep(0.01)
        parallel_coordinator.flush()
        persisted_goals.add(event.goal_id)
        with stats_lock:
            active_persisters -= 1

    def execute_parallel(request):
        assert request.goal.goal_id in persisted_goals
        return {"status": "completed"}

    AutonomousGoalWorker(
        parallel_ledger,
        resolver=lambda goal, _row: {"task": goal.goal_id},
        executor=execute_parallel,
        journal=parallel_journal,
        persist_dispatch_intent=persist_parallel_intent,
    ).run(
        schedule_options={"now_ns": 30, "max_selected": 2, "max_concurrent": 2},
        batch_id="parallel-persist-batch",
    )
    assert max_active_persisters == 1

    refused_ledger = AutonomousGoalLedger(clock=lambda: 20, max_goals=1)
    refused_ledger.create(goal_id="persist-refused", task_digest=_digest("durable refusal"), domain="coding", now_ns=0)
    refused_journal = AutonomousGoalWorkerJournal(clock=lambda: 21)
    refused_dispatches = 0

    def refuse_persistence(_event):
        raise RuntimeError("durable intent write refused")

    def execute_after_refusal(_request):
        nonlocal refused_dispatches
        refused_dispatches += 1
        return {"status": "completed"}

    refused_worker = AutonomousGoalWorker(
        refused_ledger,
        resolver=lambda _goal, _row: {"task": "durable refusal"},
        executor=execute_after_refusal,
        journal=refused_journal,
        persist_dispatch_intent=refuse_persistence,
    )
    with pytest.raises(RuntimeError, match="durable intent write refused"):
        refused_worker.run(
            schedule_options={"now_ns": 20, "max_selected": 1, "max_concurrent": 1},
            batch_id="refused-intent-batch",
        )
    assert refused_dispatches == 0
    assert refused_journal.active_for("persist-refused").phase == "dispatch_started"
    refused_journal.recover(refused_ledger, now_ns=22)
    assert refused_ledger.get("persist-refused").status == "blocked"
    for current_ledger in (ledger, async_ledger, parallel_ledger, refused_ledger):
        current_ledger.close()


def test_goal_worker_uses_shared_unicode_whitespace_for_tasks_and_executor_status() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(goal_id="unicode-worker-status", task_digest=goal_task_digest("task"), domain="coding", now_ns=0)
    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "task"},
        executor=lambda _request: {"status": "\u0085completed\u0085"},
    ).run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert ledger.get("unicode-worker-status").status == "completed"
    assert batch.runs[0].execution_status == "completed"

    blank_ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    blank_ledger.create(goal_id="unicode-blank-task", task_digest=goal_task_digest("placeholder"), domain="coding", now_ns=0)
    blank_worker = AutonomousGoalWorker(
        blank_ledger,
        resolver=lambda _goal, _row: {"task": "\u0085"},
        executor=lambda _request: {"status": "completed"},
    )
    with pytest.raises(AutonomousGoalError, match="resolved task"):
        blank_worker.run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert blank_ledger.get("unicode-blank-task").status == "ready"

    journal_worker = AutonomousGoalWorker(
        blank_ledger,
        resolver=lambda _goal, _row: {"task": "placeholder"},
        executor=lambda _request: {"status": "completed"},
        journal=AutonomousGoalWorkerJournal(),
    )
    with pytest.raises(AutonomousGoalError, match="batch_id"):
        journal_worker.run(batch_id="\u0085", schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert blank_ledger.get("unicode-blank-task").status == "ready"


def test_goal_worker_detaches_nested_parameters_before_binding_digest_and_dispatch() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    tasks = {"a-target": "target task", "z-mutator": "mutator task"}
    for goal_id, task in tasks.items():
        domain = "coding" if goal_id == "a-target" else "operations"
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain=domain, now_ns=0)

    resolver_parameters = {"nested": {"values": ["admitted"]}}

    def resolve(goal, _row):
        if goal.goal_id == "z-mutator":
            resolver_parameters["nested"]["values"][0] = "changed after admission"
        return {
            "task": tasks[goal.goal_id],
            "parameters": resolver_parameters if goal.goal_id == "a-target" else {},
        }

    observed: list[tuple[str, str]] = []
    executor_mutation_rejected: list[str] = []

    def execute(request):
        if request.goal.goal_id == "a-target":
            try:
                request.parameters["nested"]["values"][0] = "changed by executor"
            except TypeError:
                executor_mutation_rejected.append("item")
            try:
                request.parameters["nested"]["values"].append("changed by executor")
            except TypeError:
                executor_mutation_rejected.append("append")
            observed.append((
                request.parameters["nested"]["values"][0],
                content_digest({"parameters": request.parameters}),
            ))
        return {"status": "completed"}

    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    batch = AutonomousGoalWorker(
        ledger,
        resolver=resolve,
        executor=execute,
        journal=journal,
    ).run(
        batch_id="detached-parameters",
        schedule_options={
            "now_ns": 100,
            "max_selected": 2,
            "max_concurrent": 2,
            "required_domains": ["coding", "operations"],
            "signals": [
                {"goal_id": "a-target", "priority": 1.0},
                {"goal_id": "z-mutator", "priority": 0.0},
            ],
        },
    )

    assert batch.schedule.selected_goal_ids == ("a-target", "z-mutator")
    assert observed == [("admitted", journal.events(goal_id="a-target")[0].execution_binding_digest)]
    assert resolver_parameters["nested"]["values"] == ["changed after admission"]
    assert executor_mutation_rejected == ["item", "append"]
    assert all(run.goal_status == "completed" for run in batch.runs)


def test_goal_worker_runs_independent_goals_in_bounded_waves_and_preserves_schedule_order() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=3)
    tasks = {"prerequisite": "prerequisite task", "independent": "independent task", "dependent": "dependent task"}
    for goal_id, task in tasks.items():
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain="coding", now_ns=0)
    first_wave = Barrier(2, timeout=2)
    active_lock = Lock()
    active = 0
    maximum_active = 0
    dependency_observed = {"completed": False}

    def execute(request):
        nonlocal active, maximum_active
        if request.goal.goal_id in {"prerequisite", "independent"}:
            with active_lock:
                active += 1
                maximum_active = max(maximum_active, active)
            first_wave.wait()
            with active_lock:
                active -= 1
        else:
            dependency_observed["completed"] = ledger.get("prerequisite").status == "completed"
        return {"status": "completed"}

    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda goal, _row: {"task": tasks[goal.goal_id]},
        executor=execute,
        journal=journal,
    ).run(
        batch_id="bounded-parallel-wave",
        schedule_options={
            "now_ns": 100,
            "max_selected": 3,
            "max_concurrent": 3,
            "signals": [{"goal_id": "dependent", "dependencies": ["prerequisite"]}],
        }
    )

    assert maximum_active == 2
    assert dependency_observed["completed"] is True
    assert [run.goal_id for run in batch.runs] == list(batch.schedule.selected_goal_ids)
    assert all(run.goal_status == "completed" for run in batch.runs)
    for goal_id in ("prerequisite", "independent", "dependent"):
        assert [event.phase for event in journal.events(goal_id=goal_id)] == [
            "prepared",
            "claimed",
            "dispatch_started",
            "settled",
        ]
    journal_snapshot = journal.snapshot()
    restored_journal = AutonomousGoalWorkerJournal()
    assert restored_journal.restore(journal_snapshot)["head_digest"] == journal_snapshot["head_digest"]
    assert "prerequisite task" not in canonical_json(journal_snapshot)


def test_goal_worker_callback_snapshots_cannot_be_mutated_by_executor_callbacks() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    tasks = {"prerequisite": "prerequisite task", "dependent": "dependent task"}
    for goal_id, task in tasks.items():
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain="coding", now_ns=0)
    rows = {}
    goals = {}
    dispatched = []
    mutation_rejected = {"value": False}
    goal_mutation_rejected = {"value": False}
    request_mutation_rejected = {"value": False}

    def execute(request):
        dispatched.append(request.goal.goal_id)
        if request.goal.goal_id == "prerequisite":
            try:
                request.task = "changed after the binding digest was sealed"
            except AttributeError:
                request_mutation_rejected["value"] = True
            try:
                rows["dependent"].dependencies = ()
            except AttributeError:
                mutation_rejected["value"] = True
            try:
                goals["dependent"].domain = "biology"
            except AttributeError:
                goal_mutation_rejected["value"] = True
            return {"status": "blocked"}
        return {"status": "completed"}

    def resolve(goal, row):
        goals[goal.goal_id] = goal
        rows[goal.goal_id] = row
        return {"task": tasks[goal.goal_id]}

    batch = AutonomousGoalWorker(
        ledger,
        resolver=resolve,
        executor=execute,
    ).run(
        schedule_options={
            "now_ns": 100,
            "max_selected": 2,
            "max_concurrent": 2,
            "signals": [{"goal_id": "dependent", "dependencies": ["prerequisite"]}],
        }
    )

    assert mutation_rejected["value"] is True
    assert goal_mutation_rejected["value"] is True
    assert request_mutation_rejected["value"] is True
    assert goals["dependent"].domain == "coding"
    assert dispatched == ["prerequisite"]
    assert ledger.get("dependent").status == "paused"
    assert next(run for run in batch.runs if run.goal_id == "dependent").dispatched is False


def test_goal_worker_async_entry_awaits_callbacks_on_the_loop_and_preserves_schedule_order() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    tasks = {"async-a": "private async task a", "async-b": "private async task b"}
    for goal_id, task in tasks.items():
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain="coding", now_ns=0)

    async def exercise():
        loop_thread = get_ident()
        callback_threads: set[int] = set()
        active = 0
        maximum_active = 0

        async def resolve(goal, _row):
            callback_threads.add(get_ident())
            await asyncio.sleep(0)
            return {"task": tasks[goal.goal_id], "parameters": {"private": goal.goal_id}}

        async def execute(request):
            nonlocal active, maximum_active
            callback_threads.add(get_ident())
            active += 1
            maximum_active = max(maximum_active, active)
            await asyncio.sleep(0.01)
            active -= 1
            return {"status": "completed"}

        batch = await AutonomousGoalWorker(
            ledger,
            resolver=resolve,
            executor=execute,
        ).run_async(
            schedule_options={"now_ns": 100, "max_selected": 2, "max_concurrent": 2}
        )
        return batch, callback_threads, loop_thread, maximum_active

    batch, callback_threads, loop_thread, maximum_active = asyncio.run(exercise())

    assert callback_threads == {loop_thread}
    assert maximum_active == 2
    assert [run.goal_id for run in batch.runs] == list(batch.schedule.selected_goal_ids)
    assert all(run.goal_status == "completed" for run in batch.runs)
    assert "private async task" not in canonical_json(batch.to_dict())
    assert '"private"' not in canonical_json(batch.to_dict())


def test_goal_worker_async_cancellation_drains_in_flight_executor_before_returning() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(goal_id="async-cancel", task_digest=goal_task_digest("cancel task"), domain="coding", now_ns=0)

    async def exercise() -> None:
        started = asyncio.Event()
        release = asyncio.Event()

        async def execute(_request):
            started.set()
            await release.wait()
            return {"status": "completed"}

        worker = AutonomousGoalWorker(
            ledger,
            resolver=lambda _goal, _row: {"task": "cancel task"},
            executor=execute,
        )
        running = asyncio.create_task(
            worker.run_async(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
        )
        await asyncio.wait_for(started.wait(), timeout=2)
        running.cancel()
        await asyncio.sleep(0)
        assert not running.done(), "cancellation returned before the in-flight executor was fenced"
        release.set()
        with pytest.raises(asyncio.CancelledError):
            await running

    asyncio.run(exercise())
    assert ledger.get("async-cancel").status == "completed"


def test_sync_goal_worker_rejects_async_resolver_before_claiming_goal() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(goal_id="async-resolver", task_digest=goal_task_digest("async resolver task"), domain="coding", now_ns=0)

    async def resolve(_goal, _row):
        return {"task": "async resolver task"}

    worker = AutonomousGoalWorker(
        ledger,
        resolver=resolve,
        executor=lambda _request: {"status": "completed"},
    )
    with pytest.raises(AutonomousGoalError, match=r"async callbacks require run_async\(\)"):
        worker.run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert ledger.get("async-resolver").status == "ready"


def test_sync_goal_worker_rejects_async_executor_before_claiming_goal() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(goal_id="async-executor", task_digest=goal_task_digest("async executor task"), domain="coding", now_ns=0)

    async def execute(_request):
        return {"status": "completed"}

    worker = AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "async executor task"},
        executor=execute,
    )
    with pytest.raises(AutonomousGoalError, match=r"async callbacks require run_async\(\)"):
        worker.run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert ledger.get("async-executor").status == "ready"


def test_goal_control_loop_async_entry_awaits_every_callback_and_seals_valid_checkpoint() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    ledger.create(
        goal_id="async-control-loop",
        task_digest=goal_task_digest("private async control task"),
        domain="coding",
        now_ns=0,
    )
    callback_threads: set[int] = set()
    checkpoints: list[Mapping[str, object]] = []

    async def resolve(goal, _row):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"task": "private async control task", "parameters": {"secret": "transient"}}

    async def execute(_request):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"status": "completed"}

    async def options_factory(_context):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"max_selected": 1, "max_concurrent": 1}

    async def evaluate(cycle):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        run = cycle.batch.runs[0]
        return [{"goal_id": run.goal_id, "evaluator_id": "test.evaluator", "evaluator_version": "1", "reward": 1.0, "passed": True}]

    async def learn(_evaluations, _goals):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"learning_state_digest": content_digest({"learning": "settled"}), "signals": []}

    async def checkpoint(snapshot):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        checkpoints.append(snapshot)

    control_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(ledger, resolver=resolve, executor=execute),
        evaluator=evaluate,
        learner=learn,
    )

    with pytest.raises(AutonomousGoalError, match="async options_factory requires run_async"):
        control_loop.run(
            schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1},
            options_factory=options_factory,
        )
    assert ledger.get("async-control-loop").status == "ready"

    async def exercise():
        loop_thread = get_ident()
        result = await control_loop.run_async(
            schedule_options={"now_ns": 100},
            options_factory=options_factory,
            max_cycles=2,
            max_total_runs=1,
            checkpoint=checkpoint,
        )
        return loop_thread, result

    loop_thread, result = asyncio.run(exercise())

    assert result.stop_reason == "all_terminal"
    assert result.evaluation_count == 1
    assert result.cycles[0].evaluations[0].passed is True
    assert callback_threads == {loop_thread}
    assert len(checkpoints) == 1
    assert validate_autonomous_goal_control_loop_snapshot(checkpoints[0])["snapshot_digest"] == checkpoints[0]["snapshot_digest"]
    public = canonical_json({"result": result.to_dict(), "checkpoint": checkpoints[0]})
    assert "private async control task" not in public
    assert "transient" not in public


def test_goal_worker_drains_parallel_siblings_before_releasing_its_run_fence() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    tasks = {"a-broken": "broken task", "b-sibling": "sibling task"}
    for goal_id, task in tasks.items():
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain="coding", now_ns=0)

    journal = AutonomousGoalWorkerJournal(clock=lambda: 101)
    record = journal.record
    injected_failure = Event()
    fail_settlement_write = {"enabled": True}

    def record_with_one_failure(**kwargs):
        if fail_settlement_write["enabled"] and kwargs.get("goal_id") == "a-broken" and kwargs.get("phase") == "settled":
            fail_settlement_write["enabled"] = False
            injected_failure.set()
            raise RuntimeError("injected journal settlement failure")
        return record(**kwargs)

    journal.record = record_with_one_failure
    sibling_started = Event()
    release_sibling = Event()
    worker = AutonomousGoalWorker(
        ledger,
        resolver=lambda goal, _row: {"task": tasks[goal.goal_id]},
        executor=lambda request: _run_sibling_or_complete(request, sibling_started, release_sibling),
        journal=journal,
    )
    outcome: list[Exception] = []

    def run_worker() -> None:
        try:
            worker.run(
                batch_id="parallel-error-drain",
                schedule_options={"now_ns": 100, "max_selected": 2, "max_concurrent": 2},
            )
        except Exception as error:
            outcome.append(error)

    thread = Thread(target=run_worker)
    thread.start()
    try:
        assert injected_failure.wait(timeout=2)
        assert sibling_started.wait(timeout=2)
        assert thread.is_alive(), "worker returned while a parallel sibling was still running"
        with pytest.raises(AutonomousGoalError, match="another worker run is already active"):
            worker.run(
                batch_id="overlapping-parallel-error-drain",
                schedule_options={"now_ns": 100, "max_selected": 2, "max_concurrent": 2},
            )
    finally:
        release_sibling.set()
        thread.join(timeout=2)

    assert not thread.is_alive(), "worker did not finish after the sibling was released"
    assert len(outcome) == 1
    assert isinstance(outcome[0], RuntimeError)
    assert "injected journal settlement failure" in str(outcome[0])
    assert ledger.get("a-broken").status == "completed"
    assert ledger.get("b-sibling").status == "completed"
    assert journal.active_for("a-broken").phase == "dispatch_started"
    journal.recover(ledger)
    assert ledger.get("a-broken").status == "completed"
    assert journal.active_for("a-broken") is None


def _run_sibling_or_complete(request, sibling_started: Event, release_sibling: Event):
    if request.goal.goal_id == "b-sibling":
        sibling_started.set()
        if not release_sibling.wait(timeout=2):
            raise RuntimeError("test sibling release timed out")
    return {"status": "completed"}


def test_goal_worker_pauses_dependants_when_a_parallel_wave_prerequisite_does_not_complete() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=2)
    tasks = {"prerequisite": "prerequisite task", "dependent": "dependent task"}
    for goal_id, task in tasks.items():
        ledger.create(goal_id=goal_id, task_digest=goal_task_digest(task), domain="coding", now_ns=0)
    dispatched: list[str] = []

    def execute(request):
        dispatched.append(request.goal.goal_id)
        return {"status": "blocked" if request.goal.goal_id == "prerequisite" else "completed"}

    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda goal, _row: {"task": tasks[goal.goal_id]},
        executor=execute,
    ).run(
        schedule_options={
            "now_ns": 100,
            "max_selected": 2,
            "max_concurrent": 2,
            "signals": [{"goal_id": "dependent", "dependencies": ["prerequisite"]}],
        }
    )

    assert dispatched == ["prerequisite"]
    assert ledger.get("dependent").status == "paused"
    dependent_run = next(run for run in batch.runs if run.goal_id == "dependent")
    assert dependent_run.dispatched is False
    assert dependent_run.execution_status == "paused"


def test_goal_worker_single_attempt_digest_matches_typescript_reference() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100)
    ledger.create(goal_id="parity", task_digest=_digest("private"), domain="coding", now_ns=0)
    batch = AutonomousGoalWorker(
        ledger,
        resolver=lambda _goal, _row: {"task": "private"},
        executor=lambda _request: {"status": "completed"},
    ).run(schedule_options={"now_ns": "100", "max_selected": 1, "max_concurrent": 1})
    assert batch.worker_digest == "9506fc32a9c6101fd6187d9b3df12c1ffe679115e5b189512903ee83c4b3e2ab"


def test_goal_worker_rejects_unpaired_surrogates_before_hashing_or_claiming() -> None:
    with pytest.raises(AutonomousGoalError, match="Unicode scalar"):
        goal_task_digest("invalid task \ud800")
    with pytest.raises(AuthoringError, match="Unicode scalar"):
        canonical_json({"task": "invalid task \ud800"})
    with pytest.raises(AuthoringError, match="Unicode scalar"):
        canonical_json({"invalid key \ud800": "value"})

    task_ledger = AutonomousGoalLedger(clock=lambda: 100)
    task_ledger.create(goal_id="unicode-task", task_digest=_digest("valid task"), domain="coding", now_ns=0)
    task_executions = {"count": 0}

    def execute_task(_request):
        task_executions["count"] += 1
        return {"status": "completed"}

    task_worker = AutonomousGoalWorker(
        task_ledger,
        resolver=lambda _goal, _row: {"task": "invalid task \ud800"},
        executor=execute_task,
    )
    with pytest.raises(AutonomousGoalError, match="Unicode scalar"):
        task_worker.run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert task_executions["count"] == 0
    assert task_ledger.get("unicode-task").status == "ready"

    batch_ledger = AutonomousGoalLedger(clock=lambda: 100)
    batch_ledger.create(goal_id="unicode-batch", task_digest=_digest("valid batch task"), domain="coding", now_ns=0)
    resolutions = {"count": 0}

    def resolve_batch(_goal, _row):
        resolutions["count"] += 1
        return {"task": "valid batch task"}

    batch_worker = AutonomousGoalWorker(
        batch_ledger,
        resolver=resolve_batch,
        executor=lambda _request: {"status": "completed"},
        journal=AutonomousGoalWorkerJournal(clock=lambda: 101),
    )
    with pytest.raises(AutonomousGoalError, match="batch_id"):
        batch_worker.run(
            batch_id="invalid batch \ud800",
            schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1},
        )
    assert resolutions["count"] == 0
    assert batch_ledger.get("unicode-batch").status == "ready"


def test_goal_json_persistence_rejects_lone_surrogate_before_byte_counting() -> None:
    backend = _CasTextStore()
    backend.value = "\ud800"
    persistence = TransactionalJsonAutonomousGoalSnapshotPersistence(backend)
    with pytest.raises(AutonomousGoalError, match="byte bound"):
        persistence.read()


def test_goal_worker_blocks_unknown_executor_outcome_until_reconciliation() -> None:
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
    assert run.execution_status == "blocked"
    assert run.goal_status == "blocked"
    assert run.error_class == "RuntimeError"
    assert run.error_digest is not None
    assert "private provider response" not in json.dumps(batch.to_dict())
    assert ledger.get("failure").status == "blocked"
    assert ledger.get("failure").blockers == ("worker_dispatch_outcome_requires_reconciliation",)
    assert ledger.get("failure").next_action_digest == _digest("goal-reconciliation-review")


def test_goal_worker_refuses_task_rehydration_drift_before_claiming_or_dispatching() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100)
    ledger.create(goal_id="rehydration-drift", task_digest=_digest("immutable task"), domain="coding", now_ns=0)
    executions = {"count": 0}

    def execute(_request):
        executions["count"] += 1
        return {"status": "completed"}

    with pytest.raises(AutonomousGoalError, match="task digest"):
        AutonomousGoalWorker(
            ledger,
            resolver=lambda _goal, _row: {"task": "different task"},
            executor=execute,
        ).run(schedule_options={"now_ns": 100, "max_selected": 1, "max_concurrent": 1})
    assert executions["count"] == 0
    assert ledger.get("rehydration-drift").status == "ready"


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
            task_digest=_digest("pre task" if claim.goal_id == "pre" else "post task"),
            execution_binding_digest=("a" if claim.goal_id == "pre" else "b") * 64,
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
        task_digest=_digest("post task"),
        execution_binding_digest="b" * 64,
    )
    assert journal.active_for("post") is not None
    assert journal.active_for("post").execution_binding_digest == "b" * 64
    with pytest.raises(AutonomousGoalError, match="unreconciled"):
        journal.assert_no_active("post")
    recovery = journal.recover(ledger, now_ns=200)
    assert {row["goal_id"] for row in recovery["recovered"]} == {"pre", "post"}
    assert ledger.get("pre").status == "paused"
    assert ledger.get("pre").next_action_digest == _digest("goal-retry")
    assert ledger.get("post").status == "blocked"
    assert ledger.get("post").next_action_digest == _digest("goal-reconciliation-review")
    assert {event.goal_id for event in journal.active()} == {"post"}
    assert journal.active_for("post").phase == "reconciled"
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


def test_goal_worker_journal_revision_matches_javascript_safe_integer_boundary() -> None:
    journal = AutonomousGoalWorkerJournal(clock=lambda: 0)
    maximum_safe_integer = 2**53 - 1
    accepted = journal.record(
        batch_id="safe-revision-boundary",
        goal_id="maximum-safe-revision",
        phase="prepared",
        attempt=0,
        revision=maximum_safe_integer,
        schedule_digest="a" * 64,
        created_ns=0,
    )
    assert accepted.revision == maximum_safe_integer
    prior = journal.snapshot()
    assert accepted.event_digest == "3e7a90d87134f4d9ffafac06697e468b9f257acf22b1f56e39884a6391630dee"
    assert prior["snapshot_digest"] == "083bb651279e9b5b8d0f090bf02fbfe708f10b2c894c6a25df89dce58bcd9c9d"

    with pytest.raises(AutonomousGoalError, match="revision is outside its integer bounds"):
        journal.record(
            batch_id="safe-revision-boundary",
            goal_id="unsafe-revision",
            phase="prepared",
            attempt=0,
            revision=2**53,
            schedule_digest="b" * 64,
            created_ns=0,
        )
    assert journal.snapshot() == prior


def test_goal_worker_journal_rejects_non_string_genesis_chain_links_after_digest_restamping() -> None:
    journal = AutonomousGoalWorkerJournal(clock=lambda: 0)
    journal.record(
        batch_id="bad-chain",
        goal_id="bad-chain",
        phase="prepared",
        attempt=0,
        revision=0,
        schedule_digest="a" * 64,
    )
    def restamp(snapshot):
        event = snapshot["events"][0]
        event_body = {key: value for key, value in event.items() if key != "event_digest"}
        event["event_digest"] = content_digest(event_body)
        snapshot["head_digest"] = event["event_digest"]
        snapshot_body = {key: value for key, value in snapshot.items() if key != "snapshot_digest"}
        snapshot["snapshot_digest"] = content_digest(snapshot_body)
        return snapshot

    invalid_genesis = journal.snapshot()
    invalid_genesis["events"][0]["previous_digest"] = 0
    with pytest.raises(AutonomousGoalError, match="event.previous_digest must be a string"):
        AutonomousGoalWorkerJournal.validate_snapshot(restamp(invalid_genesis))

    missing_required = journal.snapshot()
    del missing_required["events"][0]["claim_digest"]
    with pytest.raises(AutonomousGoalError, match="event is missing required fields"):
        AutonomousGoalWorkerJournal.validate_snapshot(restamp(missing_required))

    null_optional = journal.snapshot()
    null_optional["events"][0]["task_digest"] = None
    with pytest.raises(AutonomousGoalError, match="event.task_digest must be omitted or a digest"):
        AutonomousGoalWorkerJournal.validate_snapshot(restamp(null_optional))


def test_goal_worker_journal_uses_exact_ns_and_explicitly_migrates_legacy_chains() -> None:
    import hashlib
    import hmac

    legacy_event_body = {
        "schema": GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01,
        "sequence": 1,
        "batch_id": "legacy-batch",
        "goal_id": "legacy-goal",
        "phase": "prepared",
        "attempt": 0,
        "revision": 0,
        "schedule_digest": "a" * 64,
        "claim_digest": None,
        "outcome_digest": None,
        "error_digest": None,
        "created_ns": 123,
        "previous_digest": "",
        "retention": GOAL_WORKER_JOURNAL_RETENTION,
        "secret_material": "never_returned",
    }
    legacy_event = {**legacy_event_body, "event_digest": content_digest(legacy_event_body)}
    legacy_snapshot_body = {
        "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
        "sequence": 1,
        "head_digest": legacy_event["event_digest"],
        "events": [legacy_event],
        "retention": GOAL_WORKER_JOURNAL_RETENTION,
        "secret_material": "never_returned",
    }
    legacy_snapshot = {**legacy_snapshot_body, "snapshot_digest": content_digest(legacy_snapshot_body)}

    migrated = migrate_legacy_autonomous_goal_worker_journal_snapshot(legacy_snapshot, "milliseconds")
    assert migrated["schema"].endswith("/0.2")
    assert migrated["events"][0]["created_ns"] == "123000000"
    assert migrated["migration"] == {
        "source_schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
        "source_timestamp_unit": "milliseconds",
        "source_snapshot_digest": legacy_snapshot["snapshot_digest"],
        "source_head_digest": legacy_event["event_digest"],
    }
    assert migrated["events"][0]["event_digest"] == "2603e5e32a5bb01498a652f143d2ed1bec2403298450f6a601dd8a68f1fc2253"
    assert migrated["snapshot_digest"] == "1ed7feef8b629d3bec289d036b8808b6adb673a4bc63ac0e978deec6a01c26dd"
    restored = AutonomousGoalWorkerJournal()
    restored.restore(migrated)
    assert restored.snapshot()["migration"] == migrated["migration"]
    exposed_snapshot = restored.snapshot()
    exposed_snapshot["migration"]["source_timestamp_unit"] = "nanoseconds"
    assert restored.snapshot()["migration"]["source_timestamp_unit"] == "milliseconds"
    tampered = json.loads(json.dumps(legacy_snapshot))
    tampered["events"][0]["created_ns"] += 1
    with pytest.raises(AutonomousGoalError, match="snapshot digest"):
        migrate_legacy_autonomous_goal_worker_journal_snapshot(tampered, "milliseconds")

    old_key = bytes(range(32))
    current_key = bytes(reversed(range(32)))
    old_auth_body = {
        "schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01,
        "key_id": "legacy-key",
        "snapshot": legacy_snapshot,
    }
    legacy_envelope = {
        **legacy_snapshot,
        "authentication": {
            "schema": AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA_V01,
            "key_id": "legacy-key",
            "tag": hmac.new(old_key, canonical_json(old_auth_body).encode("utf-8"), hashlib.sha256).hexdigest(),
        },
    }
    migrated_envelope = migrate_legacy_authenticated_autonomous_goal_worker_journal_envelope(
        legacy_envelope, "milliseconds", keys={"legacy-key": old_key, "current-key": current_key}, active_key_id="current-key",
    )
    assert migrated_envelope["authentication"]["schema"] == AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA
    assert migrated_envelope["authentication"]["key_id"] == "current-key"
    assert migrated_envelope["authentication"]["tag"] == "872839d10c95802edfd7a1f0b88e2eec7fed0de36a1dee857c28ee4f682bac86"
    assert migrated_envelope["events"][0]["created_ns"] == "123000000"
    tampered_envelope = json.loads(json.dumps(legacy_envelope))
    tampered_envelope["events"][0]["goal_id"] = "tampered"
    with pytest.raises(AutonomousGoalError, match="tag does not match"):
        migrate_legacy_authenticated_autonomous_goal_worker_journal_envelope(
            tampered_envelope, "milliseconds", keys={"legacy-key": old_key, "current-key": current_key}, active_key_id="current-key",
        )

    with pytest.raises(AutonomousGoalError, match="re-verification and re-issuance"):
        validate_goal_dispatch_resolution({
            "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA_V01,
            "goal_id": "legacy-goal",
            "attempt": 1,
            "dispatch_event_digest": "a" * 64,
            "execution_binding_digest": "b" * 64,
            "status": "completed",
            "evidence_digest": "c" * 64,
            "verifier_id": "deployment.status-adapter.v1",
            "observed_ns": 123,
            "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
            "secret_material": "never_returned",
        })


def test_authenticated_shared_goal_journal_rotates_keys_rejects_tampering_and_fences_stale_writers() -> None:
    import hashlib
    import hmac

    class SharedJournalStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest, value):
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.value = value
            return True

    def journal_with(goal_id: str) -> AutonomousGoalWorkerJournal:
        journal = AutonomousGoalWorkerJournal(clock=lambda: 123)
        journal.record(
            batch_id="shared-batch", goal_id=goal_id, phase="prepared", attempt=0, revision=0,
            schedule_digest="a" * 64, created_ns=123,
        )
        return journal

    assert hmac.new(b"\x0b" * 20, b"Hi There", hashlib.sha256).hexdigest() == "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    with pytest.raises(AutonomousGoalError, match="requires write_if_unchanged"):
        AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
            SimpleNamespace(read=lambda: None, write=lambda _value: None),
            keys={"journal-v1": bytes(range(32))}, active_key_id="journal-v1",
        )
    store = SharedJournalStore()
    old_key = bytes(range(32))
    new_key = bytes(reversed(range(32)))
    old_persistence = AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        store, keys={"journal-v1": old_key}, active_key_id="journal-v1",
    )
    first = AutonomousGoalWorkerJournalPersistenceCoordinator(journal_with("shared-one"), old_persistence)
    assert first.restore() is None
    old_snapshot = first.flush()
    raw_envelope = json.loads(store.value)
    assert raw_envelope["authentication"]["schema"] == AUTHENTICATED_GOAL_WORKER_JOURNAL_SCHEMA
    assert raw_envelope["authentication"]["key_id"] == "journal-v1"
    assert raw_envelope["authentication"]["tag"] == "063a74ad880fb8ed0ebe30989038abd6483a71bb3f568197a5de15113d1f7ffa"
    assert raw_envelope["snapshot_digest"] == old_snapshot["snapshot_digest"]
    assert old_key.hex() not in store.value

    rotating_persistence = AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        store, keys={"journal-v1": old_key, "journal-v2": new_key}, active_key_id="journal-v2",
    )
    rotating = AutonomousGoalWorkerJournalPersistenceCoordinator(AutonomousGoalWorkerJournal(), rotating_persistence)
    assert rotating.restore()["snapshot_digest"] == old_snapshot["snapshot_digest"]
    assert rotating.flush()["snapshot_digest"] == old_snapshot["snapshot_digest"]
    assert json.loads(store.value)["authentication"]["key_id"] == "journal-v1"
    rotating.journal.record(
        batch_id="shared-batch", goal_id="shared-one", phase="claimed", attempt=1, revision=1,
        schedule_digest="a" * 64, created_ns=124,
    )
    rotated_snapshot = rotating.flush()
    assert rotated_snapshot["snapshot_digest"] != old_snapshot["snapshot_digest"]
    assert json.loads(store.value)["authentication"]["key_id"] == "journal-v2"

    old_reader = AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        store, keys={"journal-v1": old_key}, active_key_id="journal-v1",
    )
    with pytest.raises(AutonomousGoalError, match="key id is not trusted"):
        old_reader.read()

    tampered = json.loads(store.value)
    tampered["authentication"]["tag"] = "0" * 64
    store.value = canonical_json(tampered)
    with pytest.raises(AutonomousGoalError, match="tag does not match"):
        rotating_persistence.read()

    shared = SharedJournalStore()
    persistence_a = AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        shared, keys={"journal-v1": old_key}, active_key_id="journal-v1",
    )
    persistence_b = AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        shared, keys={"journal-v1": old_key}, active_key_id="journal-v1",
    )
    journal_a, journal_b = journal_with("shared-a"), journal_with("shared-b")
    coordinator_a = AutonomousGoalWorkerJournalPersistenceCoordinator(journal_a, persistence_a)
    coordinator_b = AutonomousGoalWorkerJournalPersistenceCoordinator(journal_b, persistence_b)
    coordinator_a.restore()
    coordinator_b.restore()
    coordinator_a.flush()
    with pytest.raises(AutonomousGoalError, match="compare-and-swap conflict"):
        coordinator_b.flush()


def test_monotonic_journal_anchor_rejects_replay_of_an_older_valid_signed_snapshot() -> None:
    class SharedJournalStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest, value):
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.value = value
            return True

    class TrustedAnchorStore:
        def __init__(self):
            self.value = None

        def read(self):
            return None if self.value is None else dict(self.value)

        def write_if_unchanged(self, expected_anchor_digest, value):
            actual = None if self.value is None else self.value["anchor_digest"]
            if actual != expected_anchor_digest:
                return False
            self.value = dict(value)
            return True

    store, anchor = SharedJournalStore(), TrustedAnchorStore()
    persistence = MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence(
        store, anchor=anchor, keys={"journal-v1": b"a" * 32}, active_key_id="journal-v1",
    )
    journal = AutonomousGoalWorkerJournal(clock=lambda: 123)
    journal.record(batch_id="anchored-batch", goal_id="anchored-goal", phase="prepared", attempt=0, revision=0, schedule_digest="a" * 64, created_ns=123)
    coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(journal, persistence)
    assert coordinator.restore() is None
    old_snapshot = coordinator.flush()
    old_envelope = store.value
    assert anchor.value["snapshot_digest"] == old_snapshot["snapshot_digest"]

    journal.record(batch_id="anchored-batch", goal_id="anchored-goal", phase="claimed", attempt=1, revision=1, schedule_digest="a" * 64, created_ns=124)
    new_snapshot = coordinator.flush()
    assert new_snapshot["sequence"] > old_snapshot["sequence"]
    assert anchor.value["snapshot_digest"] == new_snapshot["snapshot_digest"]

    store.value = old_envelope
    with pytest.raises(AutonomousGoalError, match="rollback or incomplete commit"):
        persistence.read()
    with pytest.raises(AutonomousGoalError, match="does not match the trusted monotonic anchor"):
        persistence.roll_forward(old_snapshot)
    restored = persistence.roll_forward(new_snapshot)
    assert restored["snapshot_digest"] == new_snapshot["snapshot_digest"]


def test_verified_dispatch_outcomes_settle_exact_recovered_attempts_and_keep_uncertain_status_blocked() -> None:
    def make_recovered(goal_id: str, *, criteria=()):
        ledger = AutonomousGoalLedger(clock=lambda: 500)
        ledger.create(goal_id=goal_id, task_digest=_digest(f"{goal_id} task"), domain="coding", criteria=criteria, now_ns=0)
        ledger.transition(goal_id, "running", expected_revision=0, now_ns=1)
        journal = AutonomousGoalWorkerJournal(clock=lambda: 2)
        common = {
            "batch_id": f"{goal_id}-batch",
            "goal_id": goal_id,
            "attempt": 1,
            "revision": 1,
            "schedule_digest": _digest(f"{goal_id} schedule"),
            "claim_digest": _digest(f"{goal_id} claim"),
            "task_digest": _digest(f"{goal_id} task"),
            "execution_binding_digest": _digest(f"{goal_id} binding"),
        }
        journal.record(**common, phase="prepared")
        journal.record(**common, phase="claimed")
        dispatch = journal.record(**common, phase="dispatch_started")
        journal.recover(ledger, now_ns=100)
        return ledger, journal, dispatch

    def resolution(goal_id: str, dispatch, status: str, observed_ns: str) -> dict[str, object]:
        return {
            "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA,
            "goal_id": goal_id,
            "attempt": 1,
            "dispatch_event_digest": dispatch.event_digest,
            "execution_binding_digest": dispatch.execution_binding_digest,
            "status": status,
            "evidence_digest": _digest(f"{goal_id} status evidence {status}"),
            "verifier_id": "deployment.status-adapter.v1",
            "observed_ns": observed_ns,
            "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
            "secret_material": "never_returned",
        }

    ledger, journal, dispatch = make_recovered("status-uncertain")
    wrong_binding = resolution("status-uncertain", dispatch, "completed", "200")
    wrong_binding["execution_binding_digest"] = "f" * 64
    with pytest.raises(AutonomousGoalError, match="requires a deployment-owned verifier"):
        journal.reconcile_external_outcome(ledger, wrong_binding, None)
    with pytest.raises(AutonomousGoalError, match="does not match the configured status authority"):
        journal.reconcile_external_outcome(ledger, wrong_binding, _status_verifier(verifier_id="other.status-adapter.v1"))
    with pytest.raises(AutonomousGoalError, match="verifier rejected"):
        journal.reconcile_external_outcome(ledger, wrong_binding, _status_verifier(accepted=False))
    with pytest.raises(AutonomousGoalError, match="execution binding"):
        journal.reconcile_external_outcome(ledger, wrong_binding, _status_verifier())
    assert ledger.get("status-uncertain").status == "blocked"
    pending = resolution("status-uncertain", dispatch, "unknown", "200")
    assert journal.reconcile_external_outcome(ledger, pending, _status_verifier())["goal_status"] == "blocked"
    assert journal.active_for("status-uncertain").phase == "reconciled"
    assert journal.recover(ledger, now_ns=300)["recovered"][0]["goal_status"] == "blocked"
    assert journal.active_for("status-uncertain").phase == "reconciled"
    completed = resolution("status-uncertain", dispatch, "completed", "400")
    assert journal.reconcile_external_outcome(ledger, completed, _status_verifier())["goal_status"] == "completed"
    assert ledger.get("status-uncertain").status == "completed"
    assert journal.active_for("status-uncertain") is None
    assert journal.reconcile_external_outcome(ledger, completed, _status_verifier())["idempotent"] is True
    assert "status evidence completed" not in canonical_json(journal.snapshot())

    retry_ledger, retry_journal, retry_dispatch = make_recovered("status-not-applied")
    not_applied = resolution("status-not-applied", retry_dispatch, "not_applied", "200")
    assert retry_journal.reconcile_external_outcome(retry_ledger, not_applied, _status_verifier())["goal_status"] == "ready"
    assert retry_ledger.get("status-not-applied").next_action_digest == _digest("goal-retry")
    assert retry_journal.active_for("status-not-applied") is None

    failed_ledger, failed_journal, failed_dispatch = make_recovered("status-failed")
    terminal_failure = resolution("status-failed", failed_dispatch, "failed", "200")
    assert failed_journal.reconcile_external_outcome(failed_ledger, terminal_failure, _status_verifier())["goal_status"] == "failed"
    assert failed_journal.active_for("status-failed") is None

    criteria_ledger, criteria_journal, criteria_dispatch = make_recovered(
        "status-completed-criteria-open",
        criteria=[{"criterion_id": "evidence-reviewed", "criterion_digest": _digest("evidence-reviewed")}],
    )
    external_completion = resolution("status-completed-criteria-open", criteria_dispatch, "completed", "200")
    assert criteria_journal.reconcile_external_outcome(criteria_ledger, external_completion, _status_verifier())["goal_status"] == "paused"
    assert criteria_ledger.get("status-completed-criteria-open").blockers == ("required_goal_criteria_review",)
    assert criteria_ledger.get("status-completed-criteria-open").next_action_digest == _digest("goal-reconciliation-review")
    assert criteria_journal.active_for("status-completed-criteria-open") is None
    criteria_schedule = schedule_autonomous_goals(
        [criteria_ledger.get("status-completed-criteria-open")],
        {"now_ns": 300, "max_selected": 1, "max_concurrent": 1, "include_paused": True},
    )
    assert criteria_schedule.selected_goal_ids == ()

    normalized = validate_goal_dispatch_resolution(resolution("parity-goal", dispatch, "unknown", "500"))
    assert normalized["schema"] == GOAL_DISPATCH_RESOLUTION_SCHEMA
    with pytest.raises(AutonomousGoalError, match="unsupported or missing"):
        validate_goal_dispatch_resolution({**normalized, "external_status_body": "private"})
    parity_receipt = validate_goal_dispatch_resolution({
        "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA,
        "goal_id": "parity-goal",
        "attempt": 1,
        "dispatch_event_digest": "a" * 64,
        "execution_binding_digest": "b" * 64,
        "status": "unknown",
        "evidence_digest": "c" * 64,
        "verifier_id": "status-v1",
        "observed_ns": "1720000000000000",
        "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
        "secret_material": "never_returned",
    })
    assert content_digest(parity_receipt) == "ec699263c7c934dd1b405d2d9ac0667ee7c68f56f0f7a9f341f7c4226d57668d"
    with pytest.raises(AutonomousGoalError, match="canonical decimal-string wire format"):
        validate_goal_dispatch_resolution({**parity_receipt, "observed_ns": float(2**53 + 1)})


def test_recovery_coordinator_persists_verified_dispatch_status_and_refreshes_external_marker() -> None:
    goal_id = "coordinated-status-resolution"
    ledger = AutonomousGoalLedger(clock=lambda: 500)
    ledger.create(goal_id=goal_id, task_digest=_digest(f"{goal_id} task"), domain="coding", now_ns=0)
    ledger.transition(goal_id, "running", expected_revision=0, now_ns=1)
    source_journal = AutonomousGoalWorkerJournal(clock=lambda: 2)
    common = {
        "batch_id": f"{goal_id}-batch",
        "goal_id": goal_id,
        "attempt": 1,
        "revision": 1,
        "schedule_digest": _digest(f"{goal_id} schedule"),
        "claim_digest": _digest(f"{goal_id} claim"),
        "task_digest": _digest(f"{goal_id} task"),
        "execution_binding_digest": _digest(f"{goal_id} binding"),
    }
    source_journal.record(**common, phase="claimed")
    dispatch = source_journal.record(**common, phase="dispatch_started")

    class JournalStore:
        def __init__(self):
            self.value = canonical_json(source_journal.snapshot())

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

    class ControlStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

    journal_store = JournalStore()
    journal = AutonomousGoalWorkerJournalPersistenceCoordinator(
        AutonomousGoalWorkerJournal(),
        JsonAutonomousGoalWorkerJournalPersistence(journal_store),
    )
    recovery = AutonomousGoalRecoveryCoordinator(
        ledger,
        journal,
        AutonomousGoalControlLoopPersistenceCoordinator(ControlStore()),
    )
    assert recovery.restore(now_ns=100)["requires_external_reconciliation"] is True
    pending = recovery.reconcile_external_outcome({
        "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA,
        "goal_id": goal_id,
        "attempt": 1,
        "dispatch_event_digest": dispatch.event_digest,
        "execution_binding_digest": dispatch.execution_binding_digest,
        "status": "unknown",
        "evidence_digest": _digest(f"{goal_id} unknown external status"),
        "verifier_id": "deployment.status-adapter.v1",
        "observed_ns": "200",
        "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
        "secret_material": "never_returned",
    }, _status_verifier())
    assert pending["resolution"]["goal_status"] == "blocked"
    assert pending["recovery"]["requires_external_reconciliation"] is True
    repeated_restore = recovery.restore(now_ns=300)
    assert repeated_restore["requires_external_reconciliation"] is True
    assert repeated_restore["ready_to_resume"] is True
    assert journal.journal.active_for(goal_id).phase == "reconciled"
    pre_resolution_snapshot = ledger.snapshot()

    completed_receipt = {
        "schema": GOAL_DISPATCH_RESOLUTION_SCHEMA,
        "goal_id": goal_id,
        "attempt": 1,
        "dispatch_event_digest": dispatch.event_digest,
        "execution_binding_digest": dispatch.execution_binding_digest,
        "status": "completed",
        "evidence_digest": _digest(f"{goal_id} external status"),
        "verifier_id": "deployment.status-adapter.v1",
        "observed_ns": "400",
        "retention": GOAL_DISPATCH_RESOLUTION_RETENTION,
        "secret_material": "never_returned",
    }
    original_transition = ledger.transition
    failed_once = {"value": False}

    def fail_before_ledger_commit(*_args, **_kwargs):
        if not failed_once["value"]:
            failed_once["value"] = True
            raise RuntimeError("injected crash after durable journal stage")
        return original_transition(*_args, **_kwargs)

    ledger.transition = fail_before_ledger_commit
    with pytest.raises(RuntimeError, match="durable journal stage"):
        recovery.reconcile_external_outcome(completed_receipt, _status_verifier())
    assert json.loads(journal_store.value)["events"][-1]["resolution_goal_status"] == "completed"
    assert ledger.get(goal_id).status == "blocked"
    ledger.transition = original_transition

    replayed = recovery.restore(now_ns=500)
    assert replayed["status"] == "recovered"
    assert replayed["requires_external_reconciliation"] is False
    assert replayed["ready_to_resume"] is True
    assert ledger.get(goal_id).status == "completed"
    assert journal.journal.active_for(goal_id) is None
    assert json.loads(journal_store.value)["events"][-1]["phase"] == "settled"

    stale_ledger = AutonomousGoalLedger(clock=lambda: 900)
    stale_ledger.restore(pre_resolution_snapshot)
    stale_recovery = AutonomousGoalRecoveryCoordinator(
        stale_ledger,
        AutonomousGoalWorkerJournalPersistenceCoordinator(
            AutonomousGoalWorkerJournal(), JsonAutonomousGoalWorkerJournalPersistence(journal_store),
        ),
        AutonomousGoalControlLoopPersistenceCoordinator(ControlStore()),
    )
    stale_report = stale_recovery.restore(now_ns=600)
    assert stale_report["requires_external_reconciliation"] is False
    assert stale_ledger.get(goal_id).status == "completed"


def test_goal_recovery_reconciles_every_domain_before_exposing_a_resumable_loop() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=len(domains))
    for domain in domains:
        goal_id = f"recovery-{domain}"
        ledger.create(goal_id=goal_id, task_digest=_digest(f"private recovery task {domain}"), domain=domain, now_ns=0)
        ledger.transition(goal_id, "running", expected_revision=0, now_ns=1)

    source_journal = AutonomousGoalWorkerJournal(clock=lambda: 2)
    schedule_digest = _digest("recovery-schedule")
    for domain in domains:
        source_journal.record(
            batch_id="recovery-batch",
            goal_id=f"recovery-{domain}",
            phase="dispatch_started" if domain == "coding" else "claimed",
            attempt=1,
            revision=1,
            schedule_digest=schedule_digest,
            claim_digest=_digest("recovery-claim"),
            task_digest=_digest(f"private recovery task {domain}"),
            execution_binding_digest=_digest(f"private binding {domain}"),
        )

    order: list[str] = []

    class JournalStore:
        def __init__(self):
            self.value = canonical_json(source_journal.snapshot())

        def read(self):
            order.append("journal-read")
            return self.value

        def write(self, value):
            order.append("journal-write")
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest, value):
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            order.append("journal-write")
            self.value = value
            return True

    class ControlStore:
        def read(self):
            order.append("control-read")
            return None

        def write(self, _value):
            order.append("control-write")

    journal_store = JournalStore()
    journal_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        AutonomousGoalWorkerJournal(clock=lambda: 3),
        JsonAutonomousGoalWorkerJournalPersistence(journal_store),
    )
    control_coordinator = AutonomousGoalControlLoopPersistenceCoordinator(ControlStore())
    recovery = AutonomousGoalRecoveryCoordinator(ledger, journal_coordinator, control_coordinator)
    report = recovery.restore(now_ns=4)
    assert order == ["journal-read", "journal-write", "control-read"]
    assert report["status"] == "recovered"
    assert report["active_count_before_recovery"] == len(domains)
    assert len(report["recovered"]) == len(domains)
    assert report["requires_external_reconciliation"] is True
    assert report["ready_to_resume"] is True
    assert report["resume_snapshot"] is None
    assert validate_autonomous_goal_recovery_report(report)["report_digest"] == report["report_digest"]
    tampered_report = dict(report)
    tampered_report["report_digest"] = "0" * 64
    with pytest.raises(AutonomousGoalError, match="report digest"):
        validate_autonomous_goal_recovery_report(tampered_report)
    with pytest.raises(AutonomousGoalError, match="resume_snapshot is owned"):
        recovery.resume(
            AutonomousGoalControlLoop(
                AutonomousGoalWorker(
                    ledger,
                    resolver=lambda _goal, _row: {"task": "private recovery task coding"},
                    executor=lambda _request: {"status": "completed"},
                )
            ),
            options={"resume_snapshot": report["resume_snapshot"]},
        )
    public = json.dumps(report)
    assert "private recovery task" not in public
    assert "private binding" not in public
    assert [event.goal_id for event in journal_coordinator.journal.active()] == ["recovery-coding"]
    assert journal_coordinator.journal.active_for("recovery-coding").phase == "reconciled"
    assert ledger.get("recovery-coding").status == "blocked"
    assert "reconciled" in journal_store.value

    executed: list[str] = []
    loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            journal=journal_coordinator.journal,
            resolver=lambda goal, _row: {"task": f"private recovery task {goal.domain}"},
            executor=lambda request: (executed.append(request.goal.goal_id) or {"status": "completed"}),
        )
    )
    result = recovery.resume(
        loop,
        options={
            "schedule_options": {"now_ns": 5, "max_selected": len(domains), "max_concurrent": len(domains), "include_paused": True},
            "max_cycles": 2,
            "checkpoint": recovery.checkpoint,
        },
    )
    assert result.stop_reason == "no_admissible_work"
    assert len(executed) == len(domains) - 1
    assert ledger.get("recovery-coding").status == "blocked"
    assert all(ledger.get(f"recovery-{domain}").status == "completed" for domain in domains if domain != "coding")


def test_goal_agent_runtime_enforces_recovery_before_invoking_a_rehydrated_task() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 900)
    ledger.create(goal_id="runtime-recovery", task_digest=_digest("private runtime recovery task"), domain="coding", now_ns=0)
    ledger.transition("runtime-recovery", "running", expected_revision=0, now_ns=1)
    source_journal = AutonomousGoalWorkerJournal(clock=lambda: 2)
    source_journal.record(
        batch_id="runtime-recovery-batch",
        goal_id="runtime-recovery",
        phase="claimed",
        attempt=1,
        revision=1,
        schedule_digest=_digest("runtime-recovery-schedule"),
        claim_digest=_digest("runtime-recovery-claim"),
        task_digest=_digest("private runtime recovery task"),
        execution_binding_digest=_digest("private runtime binding"),
    )

    class JournalStore:
        def __init__(self):
            self.value = canonical_json(source_journal.snapshot())

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest, value):
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.value = value
            return True

    class ControlStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = canonical_json(value)

    journal_store = JournalStore()
    control_store = ControlStore()
    journal_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        AutonomousGoalWorkerJournal(clock=lambda: 3),
        JsonAutonomousGoalWorkerJournalPersistence(journal_store),
    )
    control_coordinator = AutonomousGoalControlLoopPersistenceCoordinator(control_store)
    recovery = AutonomousGoalRecoveryCoordinator(ledger, journal_coordinator, control_coordinator)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    orchestrator.run = lambda **_kwargs: SimpleNamespace(status="completed")
    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    agent.run = lambda **_kwargs: SimpleNamespace(status="completed")  # type: ignore[method-assign]
    runtime = agent.goal_agent_runtime(
        ledger,
        journal=journal_coordinator.journal,
        recovery=recovery,
        task_resolver=lambda _goal, _row: "private runtime recovery task",
    )
    with pytest.raises(AutonomousGoalError, match="restore"):
        runtime.run(schedule_options={"now_ns": 900, "max_selected": 1, "max_concurrent": 1, "include_paused": True})
    report = runtime.restore(now_ns=4)
    assert report["status"] == "recovered"
    result = runtime.run(schedule_options={"now_ns": 901, "max_selected": 1, "max_concurrent": 1, "include_paused": True}, max_cycles=1)
    assert result.stop_reason == "all_terminal"
    assert ledger.get("runtime-recovery").status == "completed"
    assert control_store.value is not None
    assert runtime.metadata()["recovery_execution"] == "ordered_journal_then_control_checkpoint"
    assert "private runtime recovery task" not in json.dumps(recovery.report)


def test_goal_control_loop_retries_paused_work_but_stops_on_uncertain_dispatch() -> None:
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
    assert failure_ledger.get("failed-loop").status == "blocked"
    assert failure_ledger.get("failed-loop").blockers == ("worker_dispatch_outcome_requires_reconciliation",)


def test_goal_control_loop_settles_explicit_evaluator_credit_and_adapts_all_domains() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 400, max_goals=len(domains))
    for domain in domains:
        ledger.create(goal_id=f"eval-{domain}", task_digest=_digest(f"private evaluator task {domain}"), domain=domain, now_ns=0)
    learner = AutonomousGoalBanditLearner(exploration=0.4)
    evaluator_cycles: list[int] = []

    def evaluate(cycle):
        evaluator_cycles.append(cycle.cycle)
        return [
            {
                "goal_id": run.goal_id,
                "evaluator_id": "domain-quality-evaluator",
                "evaluator_version": "2026.08",
                "reward": 1.0 if run.domain == "coding" else 0.25,
                "passed": True,
                "evidence_digest": _digest(f"private evidence {run.goal_id}"),
            }
            for run in cycle.batch.runs
        ]

    loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            resolver=lambda goal, _row: {"task": f"private evaluator task {goal.domain}"},
            executor=lambda _request: {"status": "completed"},
        ),
        evaluator=evaluate,
        learner=learner,
        batch_id_prefix="explicit-evaluator-loop",
    )
    result = loop.run(
        schedule_options={
            "now_ns": 400,
            "max_selected": len(domains),
            "max_concurrent": len(domains),
            "required_domains": list(domains),
        },
        max_cycles=2,
    )

    assert result.stop_reason == "all_terminal"
    assert evaluator_cycles == [1]
    assert result.evaluation_count == len(domains)
    assert result.evaluation_digest is not None
    assert result.learning_state_digest is not None
    assert learner.snapshot()["generation"] == 1
    assert all(record.evaluator_digest is not None for record in ledger.list(limit=len(domains)))
    assert all(record.learning_state_digest == result.learning_state_digest for record in ledger.list(limit=len(domains)))
    assert all(len(cycle.evaluations) == len(domains) for cycle in result.cycles)
    public = json.dumps(result.to_dict(), sort_keys=True)
    assert "private evaluator task" not in public
    assert "private evidence" not in public
    assert "domain-quality-evaluator" not in public
    assert ledger.verify_integrity()["goals"] == len(domains)

    tampered_reward = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            AutonomousGoalLedger(clock=lambda: 500),
            resolver=lambda _goal, _row: {"task": "private invalid evaluator task"},
            executor=lambda _request: {"status": "completed"},
        ),
        evaluator=lambda _cycle: [{"goal_id": "invalid-eval", "evaluator_id": "bad", "evaluator_version": "1", "reward": 2.0, "passed": True}],
    )
    tampered_reward.worker.ledger.create(goal_id="invalid-eval", task_digest=_digest("private invalid evaluator task"), domain="coding", now_ns=0)
    with pytest.raises(AutonomousGoalError, match="reward"):
        tampered_reward.run(schedule_options={"now_ns": 500, "max_selected": 1, "max_concurrent": 1})


def test_goal_control_loop_checkpoints_restart_bandit_and_fences_tampering_across_all_domains() -> None:
    domains = tuple(AUTONOMOUS_DOMAINS)
    ledger = AutonomousGoalLedger(clock=lambda: 550, max_goals=len(domains))
    for domain in domains:
        ledger.create(goal_id=f"checkpoint-{domain}", task_digest=_digest(f"private checkpoint task {domain}"), domain=domain, now_ns=0)
    phase = {"paused": True}
    snapshots: list[Mapping[str, object]] = []

    def evaluate(cycle):
        return [
            {
                "goal_id": run.goal_id,
                "evaluator_id": "checkpoint-evaluator",
                "evaluator_version": "1",
                "reward": 0.75,
                "passed": not phase["paused"],
            }
            for run in cycle.batch.runs
        ]

    first = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            resolver=lambda goal, _row: {"task": f"private checkpoint task {goal.domain}"},
            executor=lambda _request: {"status": "paused" if phase["paused"] else "completed"},
        ),
        evaluator=evaluate,
        batch_id_prefix="checkpoint-all-domains",
    ).run(
        run_id="checkpoint-all-domains",
        schedule_options={"now_ns": 550, "max_selected": len(domains), "max_concurrent": len(domains), "required_domains": list(domains)},
        max_cycles=1,
        checkpoint=snapshots.append,
    )
    assert first.stop_reason == "cycle_budget_exhausted"
    assert first.cycles[0].cycle == 1
    assert snapshots[0]["completed_cycles"] == 1
    assert snapshots[0]["learner_state"]["generation"] == 1
    encoded = json.dumps(snapshots[0], sort_keys=True)
    assert "private checkpoint task" not in encoded
    assert "checkpoint-evaluator" not in encoded
    assert "paused" in encoded

    phase["paused"] = False
    resumed = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            resolver=lambda goal, _row: {"task": f"private checkpoint task {goal.domain}"},
            executor=lambda _request: {"status": "completed"},
        ),
        evaluator=evaluate,
        batch_id_prefix="checkpoint-all-domains",
    ).run(
        run_id="checkpoint-all-domains",
        resume_snapshot=snapshots[-1],
        schedule_options={"now_ns": 551, "max_selected": len(domains), "max_concurrent": len(domains), "required_domains": list(domains)},
        max_cycles=3,
        checkpoint=snapshots.append,
    )
    assert resumed.stop_reason == "all_terminal"
    assert resumed.restored_cycle_count == 1
    assert resumed.cycles[0].cycle == 2
    assert resumed.evaluation_count == len(domains) * 2
    assert snapshots[-1]["generation"] == 2
    assert snapshots[-1]["previous_snapshot_digest"] == snapshots[0]["snapshot_digest"]
    assert snapshots[-1]["learner_state"]["generation"] == 2
    assert all(record.status == "completed" for record in ledger.list(limit=len(domains)))

    class Store:
        value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.value = value
            return True

    store = Store()
    persistence = TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence(store)
    coordinator = AutonomousGoalControlLoopPersistenceCoordinator(persistence)
    coordinator.flush(snapshots[0])
    restored = coordinator.restore()
    assert restored is not None
    assert restored["snapshot_digest"] == snapshots[0]["snapshot_digest"]
    tampered = dict(restored)
    tampered["total_runs"] = int(tampered["total_runs"]) + 1
    with pytest.raises(AutonomousGoalError, match="digest mismatch|aggregate counts"):
        validate_autonomous_goal_control_loop_snapshot(tampered)

    stale = AutonomousGoalControlLoopPersistenceCoordinator(persistence)
    assert stale.restore()["snapshot_digest"] == restored["snapshot_digest"]
    next_descriptor = dict(restored)
    next_descriptor.pop("snapshot_digest")
    next_descriptor["generation"] = 2
    next_descriptor["previous_snapshot_digest"] = restored["snapshot_digest"]
    next_descriptor["stop_reason"] = "cycle_budget_exhausted"
    next_snapshot = seal_autonomous_goal_control_loop_snapshot(next_descriptor)
    coordinator.flush(next_snapshot)
    with pytest.raises(AutonomousGoalError, match="compare-and-swap"):
        stale.flush(next_snapshot)


def test_goal_control_checkpoint_digest_matches_typescript_reference() -> None:
    snapshot = seal_autonomous_goal_control_loop_snapshot(
        {
            "schema": AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA,
            "run_id": "parity-fixture",
            "next_cycle": 1,
            "cycle_summaries": [],
            "previous_cycle": None,
            "completed_cycles": 0,
            "total_selected": 0,
            "total_claimed": 0,
            "total_runs": 0,
            "status_counts": {},
            "domain_counts": {},
            "evaluation_count": 0,
            "evaluation_digests": [],
            "learning_state_digest": None,
            "learned_signals": [],
            "learner_state": None,
            "stop_reason": "cycle_budget_exhausted",
            "generation": 1,
            "previous_snapshot_digest": None,
            "retention": "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
            "secret_material": "never_returned",
        }
    )
    assert snapshot["snapshot_digest"] == "043717b3e941042676eaf7cf1c2933fd1a50d09abf78a7bbad00f2608db97f2a"


def test_legacy_goal_control_checkpoint_verifies_before_deadline_conversion_and_keeps_provenance() -> None:
    legacy_body = {
        "schema": AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
        "run_id": "legacy-control",
        "next_cycle": 1,
        "cycle_summaries": [],
        "previous_cycle": None,
        "completed_cycles": 0,
        "total_selected": 0,
        "total_claimed": 0,
        "total_runs": 0,
        "status_counts": {},
        "domain_counts": {},
        "evaluation_count": 0,
        "evaluation_digests": [],
        "learning_state_digest": None,
        "learned_signals": [{"goal_id": "deadline-goal", "priority": 0.75, "urgency": 0.25, "deadline_ns": 123, "estimated_cost": 2, "dependencies": []}],
        "learner_state": None,
        "stop_reason": "cycle_budget_exhausted",
        "generation": 1,
        "previous_snapshot_digest": None,
        "retention": "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
        "secret_material": "never_returned",
    }
    legacy = {**legacy_body, "snapshot_digest": content_digest(legacy_body)}
    migrated = migrate_legacy_autonomous_goal_control_loop_snapshot(legacy, "milliseconds")
    assert migrated["schema"] == AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA
    assert migrated["learned_signals"][0]["deadline_ns"] == "123000000"
    assert migrated["migration"] == {
        "source_schema": AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
        "source_timestamp_unit": "milliseconds",
        "source_snapshot_digest": legacy["snapshot_digest"],
    }
    assert migrated["snapshot_digest"] == "3455302ab636f4f1b1c7293f5c5270ac512b6966396ae23d8c98061424359ca7"
    assert validate_autonomous_goal_control_loop_snapshot(migrated)["snapshot_digest"] == migrated["snapshot_digest"]
    tampered = json.loads(json.dumps(legacy))
    tampered["learned_signals"][0]["deadline_ns"] += 1
    with pytest.raises(AutonomousGoalError, match="snapshot digest mismatch"):
        migrate_legacy_autonomous_goal_control_loop_snapshot(tampered, "milliseconds")
    with pytest.raises(AutonomousGoalError, match="requires milliseconds or nanoseconds"):
        migrate_legacy_autonomous_goal_control_loop_snapshot(legacy, "guessed")

    next_descriptor = {key: value for key, value in migrated.items() if key != "snapshot_digest"}
    next_descriptor["generation"] += 1
    next_descriptor["previous_snapshot_digest"] = migrated["snapshot_digest"]
    next_snapshot = seal_autonomous_goal_control_loop_snapshot(next_descriptor)
    assert next_snapshot["migration"] == migrated["migration"]


def test_goal_agent_runtime_bridges_model_facade_across_every_domain_without_retaining_runtime_values() -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[tuple[str, dict[str, object]]] = []

    def run_single(**kwargs):
        calls.append(("single", kwargs))
        return SimpleNamespace(status="completed")

    def run_cross(**kwargs):
        calls.append(("cross", kwargs))
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = run_cross
    ledger = AutonomousGoalLedger(clock=lambda: 600, max_goals=len(AUTONOMOUS_DOMAINS))
    for domain in AUTONOMOUS_DOMAINS:
        ledger.create(
            goal_id=f"agent-{domain}",
            task_digest=_digest(f"private agent task {domain}"),
            domain=domain,
            capability=f"goal-{domain}-capability",
            risk_class=f"goal-{domain}-risk",
            now_ns=0,
        )

    def run_options(goal, _row):
        options = {"private_runtime_handle": object()}
        if goal.domain == "cross_domain":
            options["subtasks"] = ({"domain": "coding", "task": "private child task"},)
        return options

    runtime = AutonomousGoalAgentRuntime(
        orchestrator,
        ledger,
        task_resolver=lambda goal, _row: f"private agent task {goal.domain}",
        run_options_factory=run_options,
        evaluator=lambda cycle: [
            {"goal_id": run.goal_id, "evaluator_id": "agent-runtime-evaluator", "evaluator_version": "1", "reward": 0.75, "passed": True}
            for run in cycle.batch.runs
        ],
    )
    result = runtime.run(
        schedule_options={
            "now_ns": 600,
            "max_selected": len(AUTONOMOUS_DOMAINS),
            "max_concurrent": len(AUTONOMOUS_DOMAINS),
            "required_domains": list(AUTONOMOUS_DOMAINS),
        }
    )

    assert result.stop_reason == "all_terminal"
    assert result.evaluation_count == len(AUTONOMOUS_DOMAINS)
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert {kind for kind, _ in calls} == {"single", "cross"}
    cross_call = next(kwargs for kind, kwargs in calls if kind == "cross")
    assert cross_call["subtasks"][0]["task"] == "private child task"
    single_calls = [kwargs for kind, kwargs in calls if kind == "single"]
    assert len(single_calls) == len(AUTONOMOUS_DOMAINS) - 1
    assert all(call["capability"] == f"goal-{call['domain']}-capability" for call in single_calls)
    assert all(call["risk_class"] == f"goal-{call['domain']}-risk" for call in single_calls)
    assert cross_call["capability"] == "goal-cross_domain-capability"
    assert cross_call["risk_class"] == "goal-cross_domain-risk"
    assert all(record.status == "completed" for record in ledger.list(limit=len(AUTONOMOUS_DOMAINS)))
    serialized = json.dumps(result.to_dict(), sort_keys=True)
    assert "private agent task" not in serialized
    assert "private child task" not in serialized
    assert "private_runtime_handle" not in serialized
    assert runtime.metadata()["domain_count"] == len(AUTONOMOUS_DOMAINS)
    assert ledger.verify_integrity()["ok"] is True


def test_goal_agent_runtime_async_persists_dispatch_before_invoking_the_orchestrator() -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    ledger = AutonomousGoalLedger(clock=lambda: 610, max_goals=1)
    ledger.create(
        goal_id="async-runtime-persist-intent",
        task_digest=_digest("async runtime persist intent task"),
        domain="coding",
        now_ns=0,
    )
    journal = AutonomousGoalWorkerJournal(clock=lambda: 611)
    store = _CasTextStore()
    coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        journal,
        JsonAutonomousGoalWorkerJournalPersistence(store),
    )
    order: list[str] = []

    async def persist_dispatch_intent(event):
        assert event.phase == "dispatch_started"
        await asyncio.sleep(0)
        coordinator.flush()
        assert store.value is not None
        durable = json.loads(store.value)
        assert durable["events"][-1]["event_digest"] == event.event_digest
        order.append("persisted")

    def execute(*, task, domain, **_options):
        assert task == "async runtime persist intent task"
        assert domain == "coding"
        assert order == ["persisted"]
        assert journal.active_for("async-runtime-persist-intent").phase == "dispatch_started"
        assert store.value is not None
        assert json.loads(store.value)["events"][-1]["phase"] == "dispatch_started"
        order.append("executor")
        return SimpleNamespace(status="completed")

    orchestrator.run = execute
    orchestrator.run_cross_domain = lambda **_options: SimpleNamespace(status="completed")
    runtime = AutonomousGoalAgentRuntime(
        orchestrator,
        ledger,
        task_resolver=lambda _goal, _row: "async runtime persist intent task",
        journal=journal,
        persist_dispatch_intent=persist_dispatch_intent,
    )

    result = asyncio.run(
        runtime.run_async(
            schedule_options={"now_ns": 610, "max_selected": 1, "max_concurrent": 1}
        )
    )

    assert result.stop_reason == "all_terminal"
    assert order == ["persisted", "executor"]
    assert ledger.get("async-runtime-persist-intent").status == "completed"
    ledger.close()


def test_goal_agent_runtime_async_entry_awaits_application_callbacks_and_traced_runs() -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[dict[str, object]] = []

    def run_single(**kwargs):
        calls.append(kwargs)
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = lambda **kwargs: SimpleNamespace(status="completed")
    agent = SimpleNamespace(
        orchestrator=orchestrator,
        run=run_single,
        run_cross_domain=orchestrator.run_cross_domain,
        execute_action_handoff=lambda **kwargs: SimpleNamespace(status="completed"),
    )
    ledger = AutonomousGoalLedger(clock=lambda: 900, max_goals=2)
    ledger.create(
        goal_id="async-agent-a",
        task_digest=goal_task_digest("private async agent task async-agent-a"),
        domain="coding",
        capability="goal-capability",
        risk_class="goal-risk",
        now_ns=0,
    )
    callback_threads: set[int] = set()
    checkpoints: list[Mapping[str, object]] = []
    started = asyncio.Event()
    release_resolver = asyncio.Event()

    async def resolve(goal, _row):
        callback_threads.add(get_ident())
        if goal.goal_id == "async-agent-a":
            started.set()
            await release_resolver.wait()
        else:
            await asyncio.sleep(0)
        return f"private async agent task {goal.goal_id}"

    async def run_options(goal, _row):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"private_runtime_handle": "transient-handle"}

    async def handoff(_goal, _row, _task):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return None

    async def options_factory(_context):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"max_selected": 1, "max_concurrent": 1}

    async def evaluate(cycle):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return [
            {
                "goal_id": run.goal_id,
                "evaluator_id": "async-agent-evaluator",
                "evaluator_version": "1",
                "reward": 1.0,
                "passed": True,
            }
            for run in cycle.batch.runs
        ]

    async def learn(_evaluations, _goals):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        return {"learning_state_digest": content_digest({"learning": "async-agent"}), "signals": []}

    async def checkpoint(snapshot):
        callback_threads.add(get_ident())
        await asyncio.sleep(0)
        checkpoints.append(snapshot)

    runtime = AutonomousGoalAgentRuntime(
        orchestrator,
        ledger,
        agent=agent,
        task_resolver=resolve,
        run_options_factory=run_options,
        action_handoff_resolver=handoff,
        evaluator=evaluate,
        learner=learn,
    )

    with pytest.raises(AutonomousGoalError, match="async task_resolver requires run_async"):
        runtime.run(schedule_options={"now_ns": 900, "max_selected": 1, "max_concurrent": 1})
    assert ledger.get("async-agent-a").status == "ready"

    async def exercise():
        loop_thread = get_ident()
        running = asyncio.create_task(
            runtime.run_async(
                schedule_options={"now_ns": 900},
                options_factory=options_factory,
                max_cycles=2,
                max_total_runs=1,
                checkpoint=checkpoint,
            )
        )
        await asyncio.wait_for(started.wait(), timeout=2)
        heartbeat = 0
        for _ in range(3):
            await asyncio.sleep(0)
            heartbeat += 1
        release_resolver.set()
        result = await running
        return loop_thread, heartbeat, result

    loop_thread, heartbeat, result = asyncio.run(exercise())
    assert heartbeat == 3
    assert result.stop_reason == "all_terminal"
    assert result.evaluation_count == 1
    assert ledger.get("async-agent-a").status == "completed"

    ledger.create(
        goal_id="async-agent-b",
        task_digest=goal_task_digest("private async agent task async-agent-b"),
        domain="coding",
        capability="goal-capability",
        risk_class="goal-risk",
        now_ns=900,
    )
    trace_store = InMemoryAutonomousRunTraceStore()
    traced = asyncio.run(
        runtime.run_with_trace_async(
            trace_store=trace_store,
            run_id="async-agent-trace",
            schedule_options={"now_ns": 900},
            options_factory=options_factory,
            max_cycles=2,
            max_total_runs=1,
            checkpoint=checkpoint,
        )
    )

    assert traced.status == "completed"
    assert traced.result.stop_reason == "all_terminal"
    assert ledger.get("async-agent-b").status == "completed"
    assert callback_threads == {loop_thread}
    assert len(checkpoints) == 2
    assert all(
        validate_autonomous_goal_control_loop_snapshot(snapshot)["snapshot_digest"]
        == snapshot["snapshot_digest"]
        for snapshot in checkpoints
    )
    assert all(call["capability"] == "goal-capability" for call in calls)
    assert all(call["risk_class"] == "goal-risk" for call in calls)
    assert all(call["private_runtime_handle"] == "transient-handle" for call in calls)
    serialized = canonical_json({"result": result.to_dict(), "traced": traced.to_dict(), "checkpoints": checkpoints})
    assert "private async agent task" not in serialized
    assert "transient-handle" not in serialized


def test_goal_agent_async_cancellation_drains_and_releases_runtime_boundary() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 1_000, max_goals=2)
    ledger.create(
        goal_id="async-runtime-cancel-a",
        task_digest=goal_task_digest("private cancellation task a"),
        domain="coding",
        now_ns=0,
    )
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    calls: list[dict[str, object]] = []
    agent.run = lambda task, **options: calls.append({"task": task, **options}) or SimpleNamespace(status="completed")  # type: ignore[method-assign]
    started = asyncio.Event()
    release = asyncio.Event()

    async def resolve(goal, _row):
        if goal.goal_id == "async-runtime-cancel-a":
            started.set()
            await release.wait()
        return f"private cancellation task {goal.goal_id[-1]}"

    runtime = AutonomousGoalAgentRuntime(orchestrator, ledger, agent=agent, task_resolver=resolve)

    async def exercise() -> None:
        running = asyncio.create_task(
            runtime.run_async(
                schedule_options={"now_ns": 1_000, "max_selected": 1, "max_concurrent": 1},
                max_cycles=1,
                max_total_runs=1,
            )
        )
        await asyncio.wait_for(started.wait(), timeout=2)
        running.cancel()
        await asyncio.sleep(0)
        assert not running.done(), "runtime cancellation must wait for the admitted worker"
        running.cancel()
        await asyncio.sleep(0)
        assert not running.done(), "repeated cancellation must not release the runtime boundary"
        release.set()
        with pytest.raises(asyncio.CancelledError):
            await running

        assert ledger.get("async-runtime-cancel-a").status == "completed"
        ledger.create(
            goal_id="async-runtime-cancel-b",
            task_digest=goal_task_digest("private cancellation task b"),
            domain="coding",
            now_ns=0,
        )
        result = await runtime.run_async(
            schedule_options={"now_ns": 1_000, "max_selected": 1, "max_concurrent": 1},
            max_cycles=1,
            max_total_runs=1,
        )
        assert result.stop_reason == "all_terminal"

    asyncio.run(exercise())
    assert [call["task"] for call in calls] == [
        "private cancellation task a",
        "private cancellation task b",
    ]


def test_autonomous_agent_async_goal_control_convenience_keeps_async_rehydration_functional() -> None:
    task = "private async convenience task"
    ledger = AutonomousGoalLedger(clock=lambda: 950, max_goals=1)
    ledger.create(goal_id="async-convenience", task_digest=goal_task_digest(task), domain="science", now_ns=0)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[dict[str, object]] = []

    def run_single(**kwargs):
        calls.append(kwargs)
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = lambda **kwargs: SimpleNamespace(status="completed")
    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    agent.run = run_single  # type: ignore[method-assign]
    callback_threads: list[int] = []

    async def resolve(goal, _row):
        callback_threads.append(get_ident())
        await asyncio.sleep(0)
        return task

    async def options_factory(_context):
        callback_threads.append(get_ident())
        await asyncio.sleep(0)
        return {"max_selected": 1, "max_concurrent": 1}

    async def exercise():
        loop_thread = get_ident()
        result = await agent.run_goal_control_loop_async(
            ledger,
            task_resolver=resolve,
            schedule_options={"now_ns": 950},
            options_factory=options_factory,
            max_cycles=1,
            max_total_runs=1,
        )
        return loop_thread, result

    loop_thread, result = asyncio.run(exercise())

    assert result.stop_reason == "all_terminal"
    assert callback_threads == [loop_thread, loop_thread]
    assert ledger.get("async-convenience").status == "completed"
    assert len(calls) == 1
    assert calls[0]["task"] == task
    assert task not in canonical_json(result.to_dict())


def test_goal_agent_runtime_uses_protected_task_rehydration_across_every_domain() -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[tuple[str, dict[str, object]]] = []

    def run_single(**kwargs):
        calls.append(("single", kwargs))
        return SimpleNamespace(status="completed")

    def run_cross(**kwargs):
        calls.append(("cross", kwargs))
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = run_cross
    values: dict[str, str] = {}
    context = AutonomousProtectedRehydrationContext("tenant-a", "actor-a", "session-a", "a" * 64)
    boundary = AutonomousProtectedRehydrationBoundary(
        context,
        lambda reference, _context: values[reference.value_digest],
        authorizer=lambda _reference, _context: True,
        clock=lambda: 600.0,
    )
    protected_rehydration = AutonomousProtectedRehydrationAdapter(boundary)
    ledger = AutonomousGoalLedger(clock=lambda: 600, max_goals=len(AUTONOMOUS_DOMAINS))
    for domain in AUTONOMOUS_DOMAINS:
        task = f"protected agent task {domain}"
        values[goal_task_digest(task)] = task
        ledger.create(goal_id=f"protected-agent-{domain}", task_digest=_digest(task), domain=domain, now_ns=0)

    def run_options(goal, _row):
        options = {"private_runtime_handle": object()}
        if goal.domain == "cross_domain":
            options["subtasks"] = ({"domain": "coding", "task": "protected child task"},)
        return options

    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    agent.run = run_single  # type: ignore[method-assign]
    agent.run_cross_domain = run_cross  # type: ignore[method-assign]
    runtime = agent.goal_agent_runtime(
        ledger,
        protected_rehydration=protected_rehydration,
        run_options_factory=run_options,
        evaluator=lambda cycle: [
            {"goal_id": run.goal_id, "evaluator_id": "protected-agent-evaluator", "evaluator_version": "1", "reward": 0.75, "passed": True}
            for run in cycle.batch.runs
        ],
    )
    result = runtime.run(
        schedule_options={
            "now_ns": 600,
            "max_selected": len(AUTONOMOUS_DOMAINS),
            "max_concurrent": len(AUTONOMOUS_DOMAINS),
            "required_domains": list(AUTONOMOUS_DOMAINS),
        }
    )

    assert result.stop_reason == "all_terminal"
    assert result.evaluation_count == len(AUTONOMOUS_DOMAINS)
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert {kind for kind, _ in calls} == {"single", "cross"}
    assert runtime.metadata()["task_rehydration"] == "protected_receipt_adapter_fallback"
    serialized = json.dumps(result.to_dict(), sort_keys=True)
    assert "protected agent task" not in serialized
    assert "protected child task" not in serialized
    assert all(record.status == "completed" for record in ledger.list(limit=len(AUTONOMOUS_DOMAINS)))
    assert ledger.verify_integrity()["ok"] is True


def test_autonomous_agent_goal_control_convenience_accepts_protected_rehydration_without_plain_resolver() -> None:
    task = "protected convenience task"
    protected_values = {goal_task_digest(task): task}
    context = AutonomousProtectedRehydrationContext("tenant-convenience", "actor-convenience", "session-convenience", "b" * 64)
    boundary = AutonomousProtectedRehydrationBoundary(
        context,
        lambda reference, _context: protected_values[reference.value_digest],
        authorizer=lambda _reference, _context: True,
        clock=lambda: 700.0,
    )
    protected_rehydration = AutonomousProtectedRehydrationAdapter(boundary)
    ledger = AutonomousGoalLedger(clock=lambda: 700, max_goals=1)
    ledger.create(goal_id="protected-convenience", task_digest=_digest(task), domain="coding", now_ns=0)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[dict[str, object]] = []

    def run_single(**kwargs):
        calls.append(kwargs)
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = lambda **kwargs: SimpleNamespace(status="completed")
    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    agent.run = run_single  # type: ignore[method-assign]

    result = agent.run_goal_control_loop(
        ledger,
        protected_rehydration=protected_rehydration,
        schedule_options={"now_ns": 700, "max_selected": 1, "max_concurrent": 1},
    )

    assert result.stop_reason == "all_terminal"
    assert len(calls) == 1
    assert calls[0]["task"] == task
    assert calls[0]["domain"] == "coding"
    assert ledger.get("protected-convenience").status == "completed"
    assert task not in json.dumps(result.to_dict(), sort_keys=True)


def test_autonomous_agent_goal_control_convenience_forwards_checkpoint_and_resume_identity() -> None:
    task = "checkpointed convenience task"
    ledger = AutonomousGoalLedger(clock=lambda: 800, max_goals=1)
    ledger.create(goal_id="checkpointed-convenience", task_digest=_digest(task), domain="coding", now_ns=0)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[dict[str, object]] = []

    def run_single(**kwargs):
        calls.append(kwargs)
        return SimpleNamespace(status="completed")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = lambda **kwargs: SimpleNamespace(status="completed")
    agent = AutonomousAgent(None, LLMRuntime())
    agent.orchestrator = orchestrator
    agent.run = run_single  # type: ignore[method-assign]
    snapshots: list[dict[str, object]] = []

    result = agent.run_goal_control_loop(
        ledger,
        task_resolver=lambda _goal, _row: task,
        run_id="checkpointed-convenience-run",
        checkpoint=snapshots.append,
        schedule_options={"now_ns": 800, "max_selected": 1, "max_concurrent": 1},
    )

    assert result.stop_reason == "all_terminal"
    assert len(snapshots) == 1
    assert snapshots[0]["run_id"] == "checkpointed-convenience-run"
    assert task not in json.dumps(snapshots[0], sort_keys=True)
    with pytest.raises(AutonomousGoalError, match="run_id does not match the resume snapshot"):
        agent.run_goal_control_loop(
            ledger,
            task_resolver=lambda _goal, _row: task,
            run_id="different-run",
            resume_snapshot=snapshots[0],
            schedule_options={"now_ns": 801, "max_selected": 1, "max_concurrent": 1},
        )
    assert len(calls) == 1
    assert ledger.get("checkpointed-convenience").status == "completed"


def test_goal_agent_runtime_traces_the_complete_adaptive_loop_across_every_domain_without_payload_retention() -> None:
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    calls: list[tuple[str, dict[str, object]]] = []
    caller_observer_counts = {"before": 0, "after": 0}
    caller_selection_events = 0

    class CallerObserver:
        def before(self, _metadata):
            caller_observer_counts["before"] += 1

        def after(self, _metadata, _response, _error, _latency_ms):
            caller_observer_counts["after"] += 1

    caller_observer = CallerObserver()

    def caller_trace_callback(**_event):
        nonlocal caller_selection_events
        caller_selection_events += 1

    def emit_lifecycle(kwargs: dict[str, object]) -> None:
        observer = kwargs.get("invocation_observer")
        metadata = ProviderInvocationMetadata(
            provider="local",
            model="trace-fixture",
            kind="invoke",
            input_tokens=3,
            requested_output_tokens=2,
            tool_count=0,
        )
        if observer is not None:
            observer.before(metadata)  # type: ignore[union-attr]
        callback = kwargs.get("trace_event_callback")
        if callback is not None:
            callback(
                phase="model_selection_started",
                status="running",
                attempt=1,
                failover=False,
                candidate_count=1,
                eligible_candidate_count=1,
                strategy="deterministic_health_utility",
                selected_provider=None,
                selected_model=None,
                selection_digest=None,
                detail_digest=None,
                failure_code=None,
            )  # type: ignore[operator]
            callback(
                phase="model_selection_finished",
                status="selected",
                attempt=1,
                failover=False,
                candidate_count=1,
                eligible_candidate_count=1,
                strategy="deterministic_health_utility",
                selected_provider="local",
                selected_model="trace-fixture",
                selection_digest="a" * 64,
                detail_digest=None,
                failure_code=None,
            )  # type: ignore[operator]
        if observer is not None:
            observer.after(  # type: ignore[union-attr]
                metadata,
                ProviderResponse(
                    provider="local",
                    model="trace-fixture",
                    text="private provider output",
                    status_code=200,
                    request_id="private-provider-request-id",
                    usage={"input_tokens": 3, "output_tokens": 2},
                    raw={"private": "provider payload"},
                ),
                None,
                1.0,
            )

    def run_single(**kwargs: object):
        calls.append(("single", dict(kwargs)))
        emit_lifecycle(dict(kwargs))
        return SimpleNamespace(status="completed", output="private provider output")

    def run_cross(**kwargs: object):
        calls.append(("cross", dict(kwargs)))
        emit_lifecycle(dict(kwargs))
        return SimpleNamespace(status="completed", output="private cross-domain output")

    orchestrator.run = run_single
    orchestrator.run_cross_domain = run_cross
    ledger = AutonomousGoalLedger(clock=lambda: 650, max_goals=len(AUTONOMOUS_DOMAINS))
    for domain in AUTONOMOUS_DOMAINS:
        ledger.create(goal_id=f"trace-agent-{domain}", task_digest=_digest(f"private trace task {domain}"), domain=domain, now_ns=0)

    runtime = AutonomousGoalAgentRuntime(
        orchestrator,
        ledger,
        task_resolver=lambda goal, _row: f"private trace task {goal.domain}",
        run_options_factory=lambda goal, _row: {
            "invocation_observer": caller_observer,
            "trace_event_callback": caller_trace_callback,
            **({"subtasks": ({"domain": "coding", "task": "private child trace task"},)} if goal.domain == "cross_domain" else {}),
        },
        evaluator=lambda cycle: [
            {"goal_id": run.goal_id, "evaluator_id": "trace-evaluator", "evaluator_version": "1", "reward": 1, "passed": True}
            for run in cycle.batch.runs
        ],
    )
    trace_store = InMemoryAutonomousRunTraceStore(clock=lambda: 650)
    trace_registry = AutonomousRunTraceRegistry({"max_runs": 4_096, "max_events": 20_000, "max_bytes": 2_000_000})
    traced = runtime.run_with_trace(
        trace_store=trace_store,
        run_id="goal-trace-every-domain",
        trace_registry=trace_registry,
        schedule_options={"now_ns": 650, "max_selected": len(AUTONOMOUS_DOMAINS), "max_concurrent": len(AUTONOMOUS_DOMAINS), "required_domains": list(AUTONOMOUS_DOMAINS)},
        max_cycles=2,
        max_total_runs=len(AUTONOMOUS_DOMAINS),
    )
    assert traced.result.stop_reason == "all_terminal"
    assert traced.trace.status == "completed"
    assert traced.trace_registry is not None
    assert traced.trace_registry.status == "published"
    assert traced.trace_registry.run_import_state == "imported"
    assert trace_registry.query({"run_id": "goal-trace-every-domain"}).total_matches == 1
    assert trace_registry.query({"domain": "neuroscience"}).total_matches == 1
    assert traced.trace.provider_invocations == len(AUTONOMOUS_DOMAINS)
    assert set(traced.trace.domains) == set(AUTONOMOUS_DOMAINS)
    events = trace_store.events({"run_id": "goal-trace-every-domain"})
    assert sum(event.phase == "plan_compiled" for event in events) >= len(AUTONOMOUS_DOMAINS) + 1
    assert any(event.phase == "model_selection_finished" and event.selection_digest == "a" * 64 for event in events)
    assert any(event.phase == "evaluation_settled" for event in events)
    assert any(event.phase == "learning_prepared" for event in events)
    serialized = json.dumps(traced.to_dict(), sort_keys=True)
    assert "private trace task" not in serialized
    assert "private child trace task" not in serialized
    assert "private provider output" not in serialized
    assert "private provider output" not in json.dumps(trace_store.snapshot().to_dict(), sort_keys=True)
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert caller_observer_counts == {"before": len(AUTONOMOUS_DOMAINS), "after": len(AUTONOMOUS_DOMAINS)}
    assert caller_selection_events == len(AUTONOMOUS_DOMAINS) * 2
    assert trace_store.verify_integrity()["verified"] is True
    assert trace_registry.verify_integrity()["verified"] is True


def test_goal_agent_trace_records_failure_when_plan_trace_write_fails() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 700, max_goals=1)
    ledger.create(goal_id="trace-startup-failure", task_digest=_digest("trace startup task"), domain="coding", now_ns=0)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    orchestrator.run = lambda **_kwargs: SimpleNamespace(status="completed")
    runtime = AutonomousGoalAgentRuntime(
        orchestrator,
        ledger,
        task_resolver=lambda _goal, _row: "trace startup task",
    )
    trace_store = InMemoryAutonomousRunTraceStore(clock=lambda: 700)
    append = trace_store.append
    failed_once = {"value": False}

    def append_with_one_transient_failure(event: object):
        if isinstance(event, dict) and event.get("phase") == "plan_compiled" and not failed_once["value"]:
            failed_once["value"] = True
            raise OSError("injected transient trace-store write failure")
        return append(event)  # type: ignore[arg-type]

    trace_store.append = append_with_one_transient_failure  # type: ignore[method-assign]
    with pytest.raises(OSError, match="injected transient trace-store write failure"):
        runtime.run_with_trace(trace_store=trace_store, run_id="trace-startup-failure", max_cycles=1)

    events = trace_store.events({"run_id": "trace-startup-failure"})
    assert [event.phase for event in events] == ["started", "failed"]
    assert events[-1].failure_code == "goal_control_loop_error"
    assert events[-1].failure_class == "OSError"
    assert trace_store.verify_integrity()["verified"] is True


def test_goal_agent_rejects_an_untraced_loop_while_a_traced_loop_owns_runtime_state() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 710, max_goals=1)
    ledger.create(goal_id="trace-exclusive-run", task_digest=_digest("trace exclusive task"), domain="coding", now_ns=0)
    orchestrator = object.__new__(AutonomousTaskOrchestrator)
    orchestrator.run = lambda **_kwargs: SimpleNamespace(status="completed")
    entered_resolver = Event()
    release_resolver = Event()

    def resolve(_goal, _row):
        entered_resolver.set()
        if not release_resolver.wait(timeout=5):
            raise TimeoutError("test did not release the task resolver")
        return "trace exclusive task"

    runtime = AutonomousGoalAgentRuntime(orchestrator, ledger, task_resolver=resolve)
    trace_store = InMemoryAutonomousRunTraceStore(clock=lambda: 710)
    results: list[object] = []
    failures: list[BaseException] = []

    def traced_run() -> None:
        try:
            results.append(runtime.run_with_trace(trace_store=trace_store, run_id="trace-exclusive-run", max_cycles=1))
        except BaseException as error:
            failures.append(error)

    thread = Thread(target=traced_run)
    thread.start()
    try:
        assert entered_resolver.wait(timeout=5)
        with pytest.raises(AutonomousGoalError, match="another goal runtime operation is already active"):
            runtime.run(max_cycles=1)
    finally:
        release_resolver.set()
        thread.join(timeout=5)

    assert not thread.is_alive()
    assert failures == []
    assert len(results) == 1
    assert results[0].trace.status == "completed"  # type: ignore[union-attr]


def test_goal_agent_runtime_replays_caller_owned_action_handoffs_before_run_boundary() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    calls: list[dict[str, object]] = []
    original_execute_handoff = agent.execute_action_handoff

    def execute_handoff(**kwargs: object):
        calls.append(dict(kwargs))
        try:
            return original_execute_handoff(**kwargs)
        except Exception as error:
            calls[-1]["error"] = str(error)
            raise

    agent.execute_action_handoff = execute_handoff  # type: ignore[method-assign]
    agent.run_auto = lambda **kwargs: SimpleNamespace(execution_status="completed")  # type: ignore[method-assign]
    controller = AutonomousActionAdmissionController(
        InMemoryAutonomousActionAdmissionLedger(max_records=len(AUTONOMOUS_DOMAINS) + 1)
    )
    ledger = AutonomousGoalLedger(clock=lambda: 800, max_goals=len(AUTONOMOUS_DOMAINS))
    for domain in AUTONOMOUS_DOMAINS:
        ledger.create(
            goal_id=f"handoff-goal-{domain}",
            task_digest=_digest(f"private handoff task {domain}"),
            domain=domain,
            now_ns=0,
        )

    def resolve_handoff(goal, _row, task):
        if goal.domain == "cross_domain":
            plan = agent.action_plan(task=task, domain="cross_domain", allow_cross_domain=False)
            request = {"domain": "cross_domain", "allow_cross_domain": False}
        else:
            plan = agent.action_plan(task=task, domain=goal.domain, allow_cross_domain=False)
            request = None
        action_id = f"goal-handoff-{goal.domain}"
        controller.submit(
            action_id,
            plan,
            approvals={gate: True for gate in plan["required_approvals"]},
            reviewed=True,
            authorization_digest="c" * 64,
        )
        handoff = controller.dispatch_handoff(action_id)
        return {"handoff": handoff, "request": request} if request is not None else handoff

    runtime = AutonomousGoalAgentRuntime(
        agent.orchestrator,
        ledger,
        agent=agent,
        task_resolver=lambda goal, _row: f"private handoff task {goal.domain}",
        action_handoff_resolver=resolve_handoff,
        run_options_factory=lambda goal, _row: {
            "credentials": {},
            **({"subtasks": ({"domain": "coding", "task": "private child task"},)} if goal.domain == "cross_domain" else {}),
        },
        evaluator=lambda cycle: [
            {"goal_id": run.goal_id, "evaluator_id": "handoff-evaluator", "evaluator_version": "1", "reward": 1, "passed": True}
            for run in cycle.batch.runs
        ],
    )
    result = runtime.run(
        schedule_options={
            "now_ns": 800,
            "max_selected": len(AUTONOMOUS_DOMAINS),
            "max_concurrent": len(AUTONOMOUS_DOMAINS),
            "required_domains": list(AUTONOMOUS_DOMAINS),
        }
    )

    assert result.stop_reason == "all_terminal"
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert all(call["credentials"] == {} for call in calls)
    assert runtime.metadata()["execution_surface"] == "autonomous_goal_action_handoff_facade"
    assert runtime.metadata()["action_handoff_execution"] == "verified_handoff_replay_before_run_boundary"
    serialized = json.dumps(result.to_dict(), sort_keys=True)
    assert "private handoff task" not in serialized
    assert "private child task" not in serialized
    assert all(record.status == "completed" for record in ledger.list(limit=len(AUTONOMOUS_DOMAINS)))
    assert ledger.verify_integrity()["ok"] is True


def test_autonomous_agent_exposes_facade_backed_goal_control_across_every_domain() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    calls: list[tuple[str, dict[str, object]]] = []

    def run_single(**kwargs: object):
        calls.append(("single", kwargs))
        return SimpleNamespace(status="completed")

    def run_cross(**kwargs: object):
        calls.append(("cross", kwargs))
        return SimpleNamespace(status="completed")

    agent.run = run_single  # type: ignore[method-assign]
    agent.run_cross_domain = run_cross  # type: ignore[method-assign]
    ledger = AutonomousGoalLedger(clock=lambda: 700, max_goals=len(AUTONOMOUS_DOMAINS))
    for domain in AUTONOMOUS_DOMAINS:
        ledger.create(
            goal_id=f"facade-{domain}",
            task_digest=_digest(f"private facade task {domain}"),
            domain=domain,
            now_ns=0,
        )

    def run_options(goal, _row):
        options: dict[str, object] = {"credentials": {}, "model_candidates": ()}
        if goal.domain == "cross_domain":
            options["subtasks"] = ({"domain": "coding", "task": "private child"},)
        return options

    result = agent.run_goal_control_loop(
        ledger,
        task_resolver=lambda goal, _row: f"private facade task {goal.domain}",
        run_options_factory=run_options,
        evaluator=lambda cycle: [
            {
                "goal_id": run.goal_id,
                "evaluator_id": "facade-evaluator",
                "evaluator_version": "1",
                "reward": 0.9,
                "passed": True,
            }
            for run in cycle.batch.runs
        ],
        schedule_options={
            "now_ns": 700,
            "max_selected": len(AUTONOMOUS_DOMAINS),
            "max_concurrent": len(AUTONOMOUS_DOMAINS),
            "required_domains": list(AUTONOMOUS_DOMAINS),
        },
    )

    assert result.stop_reason == "all_terminal"
    assert result.evaluation_count == len(AUTONOMOUS_DOMAINS)
    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert {kind for kind, _ in calls} == {"single", "cross"}
    assert all(record.status == "completed" for record in ledger.list(limit=len(AUTONOMOUS_DOMAINS)))
    assert agent.goal_agent_runtime(
        ledger,
        task_resolver=lambda goal, _row: f"private facade task {goal.domain}",
    ).metadata()["execution_surface"] == "autonomous_agent_facade"
    serialized = json.dumps(result.to_dict(), sort_keys=True)
    assert "private facade task" not in serialized
    assert "private child" not in serialized
    assert ledger.verify_integrity()["ok"] is True


def test_autonomous_agent_goal_control_rechecks_live_preview_revocation_before_task_rehydration() -> None:
    ledger = AutonomousGoalLedger(clock=lambda: 100, max_goals=1)
    task = "private facade approval task"
    ledger.create(goal_id="facade-preview", task_digest=_digest(task), domain="coding", now_ns=0)
    admissions = InMemoryAutonomousGoalPreviewAdmissionLedger()
    calls = {"resolve": 0, "execute": 0}
    agent = AutonomousAgent(None, LLMRuntime())

    def resolve(_goal, _row):
        calls["resolve"] += 1
        return task

    def execute(**_kwargs):
        calls["execute"] += 1
        return SimpleNamespace(status="completed")

    agent.run = execute  # type: ignore[method-assign]
    options = {"now_ns": 100, "max_selected": 1, "max_concurrent": 1}
    runtime = agent.goal_agent_runtime(
        ledger,
        task_resolver=resolve,
        preview_admission_ledger=admissions,
    )
    preview = runtime.preview(schedule_options=options)
    submitted = admissions.submit(
        preview,
        admission_id="facade-preview-review",
        issued_at_ns=100,
        expires_at_ns=1_000,
        requested_by_digest=content_digest("facade requester"),
    )
    approved = admissions.review(
        "facade-preview-review",
        approved=True,
        reviewer_digest=content_digest("facade reviewer"),
        expected_record_digest=submitted["record_digest"],
    )
    admissions.revoke(
        "facade-preview-review",
        reviewer_digest=content_digest("facade reviewer"),
        reason="approval withdrawn before dispatch",
        expected_record_digest=approved["record_digest"],
    )

    with pytest.raises(AutonomousGoalError, match="stale relative to the live admission ledger"):
        agent.run_goal_control_loop(
            ledger,
            task_resolver=resolve,
            preview_admission_ledger=admissions,
            schedule_options=options,
            max_cycles=1,
            max_total_runs=1,
            expected_preview_digest=preview.preview_digest,
            preview_approval=approved,
        )

    assert calls == {"resolve": 0, "execute": 0}
    assert ledger.get("facade-preview").status == "ready"


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


def test_authenticated_goal_snapshots_share_a_key_rotatable_hmac_contract_with_typescript() -> None:
    with pytest.raises(AutonomousGoalError, match="requires write_if_unchanged"):
        AuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence(
            SimpleNamespace(read=lambda: None, write=lambda _value: None),
            keys={"goal-v1": bytes(range(32))},
            active_key_id="goal-v1",
        )

    store = _CasTextStore()
    old_key = bytes(range(32))
    new_key = bytes(255 - index for index in range(32))
    old_persistence = AuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence(
        store, keys={"goal-v1": old_key}, active_key_id="goal-v1",
    )
    source = AutonomousGoalLedger(clock=lambda: 7)
    source.create(
        goal_id="auth-vector",
        task_digest=goal_task_digest("authenticated goal snapshot"),
        domain="coding",
        now_ns=7,
    )
    source_coordinator = AutonomousGoalPersistenceCoordinator(source, old_persistence)
    source_coordinator.flush()
    first_snapshot = source.snapshot()
    first_envelope = json.loads(store.value)
    body = {"schema": GOAL_AUTH_SCHEMA, "key_id": "goal-v1", "snapshot": first_snapshot}
    expected_tag = hmac.new(old_key, canonical_json(body).encode("utf-8"), hashlib.sha256).hexdigest()
    assert first_envelope["authentication"] == {
        "schema": GOAL_AUTH_SCHEMA,
        "key_id": "goal-v1",
        "tag": "56eebea964e1be47083d9e57bd035c2566042eba1bdc998067b8be258488a047",
    }
    assert first_snapshot["snapshot_digest"] == "56ee95b1d328789bdf36017c47e03db3737f6345ffada26979b5e4e74c197ad9"
    assert first_envelope["authentication"]["tag"] == expected_tag
    assert len(store.value.encode("utf-8")) <= MAX_AUTHENTICATED_GOAL_SNAPSHOT_BYTES
    assert old_key.hex() not in store.value

    rotating_persistence = AuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence(
        store,
        keys={"goal-v1": old_key, "goal-v2": new_key},
        active_key_id="goal-v2",
    )
    rotating_ledger = AutonomousGoalLedger(clock=lambda: 8)
    rotating_coordinator = AutonomousGoalPersistenceCoordinator(rotating_ledger, rotating_persistence)
    assert rotating_coordinator.restore()["snapshot_digest"] == first_snapshot["snapshot_digest"]
    rotating_coordinator.flush()
    assert json.loads(store.value)["authentication"]["key_id"] == "goal-v1"
    rotating_ledger.transition("auth-vector", "running", expected_revision=0, now_ns=8)
    rotating_coordinator.flush()
    assert json.loads(store.value)["authentication"]["key_id"] == "goal-v2"

    stale_ledger = AutonomousGoalLedger(clock=lambda: 9)
    stale_coordinator = AutonomousGoalPersistenceCoordinator(stale_ledger, rotating_persistence)
    stale_coordinator.restore()
    rotating_ledger.transition("auth-vector", "blocked", expected_revision=1, now_ns=10)
    rotating_coordinator.flush()
    with pytest.raises(AutonomousGoalError, match="compare-and-swap conflict"):
        stale_coordinator.flush()

    old_reader = AuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence(
        store, keys={"goal-v1": old_key}, active_key_id="goal-v1",
    )
    with pytest.raises(AutonomousGoalError, match="key id is not trusted"):
        old_reader.read()
    tampered = json.loads(store.value)
    tampered["authentication"]["tag"] = "0" * 64
    store.value = canonical_json(tampered)
    with pytest.raises(AutonomousGoalError, match="tag does not match"):
        rotating_persistence.read()
    for ledger in (source, rotating_ledger, stale_ledger):
        ledger.close()


def test_monotonic_goal_anchor_rejects_replay_of_an_older_valid_signed_snapshot() -> None:
    class SharedGoalStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

        def write_if_unchanged(self, expected_snapshot_digest, value):
            actual = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if actual != expected_snapshot_digest:
                return False
            self.value = value
            return True

    class TrustedAnchorStore:
        def __init__(self):
            self.value = None

        def read(self):
            return None if self.value is None else dict(self.value)

        def write_if_unchanged(self, expected_anchor_digest, value):
            actual = None if self.value is None else self.value["anchor_digest"]
            if actual != expected_anchor_digest:
                return False
            self.value = dict(value)
            return True

    store, anchor = SharedGoalStore(), TrustedAnchorStore()
    persistence = MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalSnapshotPersistence(
        store, anchor=anchor, keys={"goal-v1": b"g" * 32}, active_key_id="goal-v1",
    )
    ledger = AutonomousGoalLedger(clock=lambda: 123)
    ledger.create(goal_id="anchored-goal", task_digest=_digest("anchored goal"), domain="coding", now_ns=123)
    coordinator = AutonomousGoalPersistenceCoordinator(ledger, persistence)
    assert coordinator.restore() is None
    old_snapshot = coordinator.flush()
    old_envelope = store.value

    ledger.transition("anchored-goal", "running", expected_revision=0, now_ns=124)
    new_snapshot = coordinator.flush()
    assert new_snapshot["sequence"] > old_snapshot["sequence"]
    assert anchor.value["snapshot_digest"] == new_snapshot["snapshot_digest"]

    store.value = old_envelope
    with pytest.raises(AutonomousGoalError, match="rollback or incomplete commit"):
        persistence.read()
    with pytest.raises(AutonomousGoalError, match="does not match the trusted monotonic anchor"):
        persistence.roll_forward(old_snapshot)
    restored = persistence.roll_forward(new_snapshot)
    assert restored["snapshot_digest"] == new_snapshot["snapshot_digest"]
    ledger.close()


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
        with pytest.raises(AutonomousGoalError, match="expected_revision must be a non-negative safe integer"):
            ledger.transition("bounded-goal", "running", expected_revision=2**53)
        assert ledger.get("bounded-goal").revision == 0
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


def test_goal_record_rejects_revisions_outside_the_javascript_safe_integer_range() -> None:
    with pytest.raises(AutonomousGoalError, match="goal.revision must be a non-negative safe integer"):
        AutonomousGoalRecord(
            goal_id="unsafe-revision",
            task_digest=_digest("unsafe revision task"),
            domain="coding",
            capability=None,
            risk_class=None,
            status="ready",
            attempt=0,
            max_attempts=1,
            revision=2**53,
            created_ns=0,
            updated_ns=0,
        )


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
    assert record.state_digest == "c433d1780c63522fbf3a8fee6476b2617a6421df1879b6331b1c44ea5d515fc3"
    assert record.to_dict()["created_ns"] == "100"


def test_goal_snapshot_v02_preserves_exact_cross_sdk_nanoseconds() -> None:
    with AutonomousGoalLedger(":memory:") as ledger:
        record = ledger.create(
            goal_id="exact-time",
            task_digest=goal_task_digest("exact-time task"),
            domain="coding",
            now_ns="1780000000123456789",
        )
        snapshot = ledger.snapshot()
    assert record.to_dict()["schema"] == "bioprism-autonomous-goal/0.2"
    assert record.to_dict()["created_ns"] == "1780000000123456789"
    assert snapshot["schema"] == "bioprism-autonomous-goal-snapshot/0.2"
    assert snapshot["events"][0]["created_ns"] == "1780000000123456789"
    assert record.state_digest == "56e996525cf194c079e3599c933a6811b7de27ee5e9b69949dfd6929c2e032db"
    assert snapshot["snapshot_digest"] == "83740f2f1f02a369914601bbbdca389bf50bdce4a2eaf5ff8d92f8f2254f73b9"


def test_goal_scheduler_scores_exact_nanosecond_ages_and_deadlines() -> None:
    created_ns = "1780000000123456789"
    now_ns = "1780043200123456789"
    deadline_ns = "1780086400123456789"
    with AutonomousGoalLedger(":memory:") as ledger:
        goal = ledger.create(goal_id="exact-schedule", task_digest=goal_task_digest("exact schedule"), domain="coding", now_ns=created_ns)
        schedule = schedule_autonomous_goals(
            [goal],
            {
                "now_ns": now_ns,
                "aging_window_ns": "86400000000000",
                "signals": [{"goal_id": goal.goal_id, "deadline_ns": deadline_ns}],
            },
        )
    row = schedule.rows[0].to_dict()
    assert row["age_score"] == 0.5
    assert row["deadline_score"] == 0.6667
    assert schedule.to_dict()["now_ns"] == now_ns
    assert schedule.schedule_digest == "4b43220e5a07d16d0fda20c9b7fb4d9e99d884598ba7a20d4bb6e43df1aab0c9"


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


def _canonical_digest(value: object) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def _legacy_goal_snapshot_v01(timestamp_ms: int = 123) -> dict:
    with AutonomousGoalLedger(clock=lambda: timestamp_ms * 1_000_000) as ledger:
        ledger.create(goal_id="legacy-ms", task_digest=_digest("legacy goal"), domain="coding")
        current = ledger.snapshot()
    current_record = current["goals"][0]
    legacy_state = {
        key: value
        for key, value in current_record.items()
        if key not in {"state_digest", "retention", "secret_material"}
    }
    legacy_state["schema"] = "bioprism-autonomous-goal/0.1"
    legacy_state["created_ns"] = timestamp_ms
    legacy_state["updated_ns"] = timestamp_ms
    legacy_record = {
        **legacy_state,
        "state_digest": _canonical_digest(legacy_state),
        "retention": current_record["retention"],
        "secret_material": current_record["secret_material"],
    }
    legacy_event_body = {
        **current["events"][0],
        "schema": "bioprism-autonomous-goal-event/0.1",
        "payload": legacy_record,
        "created_ns": timestamp_ms,
        "previous_digest": "",
    }
    legacy_event_body.pop("event_digest")
    legacy_event = {**legacy_event_body, "event_digest": _canonical_digest(legacy_event_body)}
    legacy_body = {
        "schema": "bioprism-autonomous-goal-snapshot/0.1",
        "sequence": 1,
        "head_digest": legacy_event["event_digest"],
        "goals": [legacy_record],
        "events": [legacy_event],
        "retention": current["retention"],
        "secret_material": current["secret_material"],
    }
    return {**legacy_body, "snapshot_digest": _canonical_digest(legacy_body)}


def test_goal_legacy_snapshot_migration_verifies_and_preserves_source_identity() -> None:
    legacy = _legacy_goal_snapshot_v01()
    migrated = migrate_legacy_goal_snapshot(legacy, source_unit="milliseconds")
    assert migrated["schema"] == "bioprism-autonomous-goal-snapshot/0.2"
    assert migrated["snapshot_digest"] == "63e476b1b6afc29bf262a901d3e86c25593b7cc6f0f05c1c747412ad91e07163"
    assert migrated["goals"][0]["created_ns"] == "123000000"
    assert migrated["events"][0]["created_ns"] == "123000000"
    assert migrated["migration"] == {
        "source_schema": "bioprism-autonomous-goal-snapshot/0.1",
        "source_timestamp_unit": "milliseconds",
        "source_snapshot_digest": legacy["snapshot_digest"],
        "source_head_digest": legacy["head_digest"],
    }
    with AutonomousGoalLedger(clock=lambda: 999) as restored:
        restored.restore(migrated)
        assert restored.snapshot() == migrated
        assert restored.verify_integrity()["ok"] is True

    tampered = json.loads(json.dumps(legacy))
    tampered["events"][0]["created_ns"] += 1
    with pytest.raises(AutonomousGoalError, match="snapshot digest mismatch"):
        migrate_legacy_goal_snapshot(tampered, source_unit="milliseconds")

    legacy_nanoseconds = _legacy_goal_snapshot_v01(timestamp_ms=123_000_000)
    migrated_nanoseconds = migrate_legacy_goal_snapshot(legacy_nanoseconds, source_unit="nanoseconds")
    assert migrated_nanoseconds["goals"][0]["created_ns"] == "123000000"
    assert migrated_nanoseconds["migration"]["source_timestamp_unit"] == "nanoseconds"


def test_goal_legacy_sqlite_migration_requires_unit_and_is_durable(tmp_path: Path) -> None:
    path = tmp_path / "legacy-goal.sqlite3"
    legacy = _legacy_goal_snapshot_v01()
    record = legacy["goals"][0]
    event = legacy["events"][0]
    with AutonomousGoalLedger(str(path)) as ledger:
        ledger.create(goal_id="legacy-ms", task_digest=_digest("legacy goal"), domain="coding", now_ns=123_000_000)
    connection = sqlite3.connect(path)
    connection.execute(
        "UPDATE autonomous_goals SET state_json = ?, state_digest = ?, created_ns = 123, updated_ns = 123 "
        "WHERE goal_id = 'legacy-ms'",
        (json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":")), record["state_digest"]),
    )
    connection.execute(
        "UPDATE autonomous_goal_events SET payload_json = ?, previous_digest = ?, event_digest = ?, created_ns = 123 "
        "WHERE sequence = 1",
        (
            json.dumps(event["payload"], ensure_ascii=False, sort_keys=True, separators=(",", ":")),
            event["previous_digest"],
            event["event_digest"],
        ),
    )
    connection.execute("DELETE FROM autonomous_goal_metadata WHERE key = 'migration'")
    connection.commit()
    connection.close()

    with pytest.raises(AutonomousGoalError, match="legacy_timestamp_unit"):
        AutonomousGoalLedger(str(path))
    with AutonomousGoalLedger(str(path), legacy_timestamp_unit="milliseconds") as migrated:
        assert migrated.get("legacy-ms").created_ns == 123_000_000
        migration = migrated.snapshot()["migration"]
        assert migration["source_snapshot_digest"] is None
        assert migration["source_head_digest"] == legacy["head_digest"]
        verified = migrated.snapshot()
    with AutonomousGoalLedger(str(path)) as reopened:
        assert reopened.snapshot() == verified


def test_migrated_goal_journal_and_checkpoint_snapshots_survive_persistence_and_live_restart_cycle() -> None:
    goal_id = "legacy-ms"
    task = "legacy goal"
    migrated_goal = migrate_legacy_goal_snapshot(_legacy_goal_snapshot_v01(), source_unit="milliseconds")
    legacy_journal_body = {
        "schema": GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
        "sequence": 0,
        "head_digest": "",
        "events": [],
        "retention": GOAL_WORKER_JOURNAL_RETENTION,
        "secret_material": "never_returned",
    }
    migrated_journal = migrate_legacy_autonomous_goal_worker_journal_snapshot(
        {**legacy_journal_body, "snapshot_digest": content_digest(legacy_journal_body)}, "milliseconds",
    )
    legacy_checkpoint_body = {
        "schema": AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
        "run_id": "migrated-live-cycle",
        "next_cycle": 1,
        "cycle_summaries": [],
        "previous_cycle": None,
        "completed_cycles": 0,
        "total_selected": 0,
        "total_claimed": 0,
        "total_runs": 0,
        "status_counts": {},
        "domain_counts": {},
        "evaluation_count": 0,
        "evaluation_digests": [],
        "learning_state_digest": None,
        "learned_signals": [{"goal_id": goal_id, "priority": 0.75, "urgency": 0.25, "deadline_ns": 200, "estimated_cost": 1, "dependencies": []}],
        "learner_state": None,
        "stop_reason": "cycle_budget_exhausted",
        "generation": 1,
        "previous_snapshot_digest": None,
        "retention": "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
        "secret_material": "never_returned",
    }
    migrated_checkpoint = migrate_legacy_autonomous_goal_control_loop_snapshot(
        {**legacy_checkpoint_body, "snapshot_digest": content_digest(legacy_checkpoint_body)}, "milliseconds",
    )

    class TextStore:
        def __init__(self, value: str):
            self.value = value

        def read(self):
            return self.value

        def write(self, value: str):
            self.value = value

    goal_store = TextStore(canonical_json(migrated_goal))
    journal_store = TextStore(canonical_json(migrated_journal))
    checkpoint_store = TextStore(canonical_json(migrated_checkpoint))
    ledger = AutonomousGoalLedger(clock=lambda: 600_000_000)
    ledger_persistence = AutonomousGoalPersistenceCoordinator(ledger, JsonAutonomousGoalSnapshotPersistence(goal_store))
    ledger_persistence.restore()
    journal_coordinator = AutonomousGoalWorkerJournalPersistenceCoordinator(
        AutonomousGoalWorkerJournal(clock=lambda: 700_000_000),
        JsonAutonomousGoalWorkerJournalPersistence(journal_store),
    )
    control_coordinator = AutonomousGoalControlLoopPersistenceCoordinator(
        JsonAutonomousGoalControlLoopSnapshotPersistence(checkpoint_store),
    )
    recovery = AutonomousGoalRecoveryCoordinator(ledger, journal_coordinator, control_coordinator)
    report = recovery.restore(now_ns="800000000")
    assert report["status"] == "restored"
    assert report["resume_snapshot"]["migration"]["source_snapshot_digest"] == migrated_checkpoint["migration"]["source_snapshot_digest"]

    dispatches = 0

    def execute(_request):
        nonlocal dispatches
        dispatches += 1
        return {"status": "completed"}

    loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            ledger,
            journal=journal_coordinator.journal,
            resolver=lambda _goal, _row: {"task": task},
            executor=execute,
        ),
        batch_id_prefix="migrated-live-cycle",
    )
    recovery.resume(
        loop,
        options={
            "run_id": "migrated-live-cycle",
            "schedule_options": {"now_ns": "500000000", "max_selected": 1, "max_concurrent": 1},
            "max_cycles": 1,
            "checkpoint": recovery.checkpoint,
        },
    )
    ledger_persistence.flush()
    assert ledger.snapshot()["snapshot_digest"] == "45f6015def5e0df33906c3beae15ae1ac90b2da4b3433b845fdc66f280f204c4"
    assert journal_coordinator.journal.snapshot()["snapshot_digest"] == "2bac65b5053f0fbe7a4caee81d7fcc59b3d913713f6f63c7532ba60bb7a44376"
    assert json.loads(checkpoint_store.value)["snapshot_digest"] == "348616cce9e5ec46b6600c27d393668189c2bcea16e4166f843182c07952da7f"
    assert dispatches == 1
    assert ledger.get(goal_id).status == "completed"
    assert ledger.snapshot()["migration"] == migrated_goal["migration"]
    assert journal_coordinator.journal.snapshot()["migration"] == migrated_journal["migration"]
    assert json.loads(checkpoint_store.value)["migration"] == migrated_checkpoint["migration"]
    assert task not in "".join((goal_store.value, journal_store.value, checkpoint_store.value))

    restarted_ledger = AutonomousGoalLedger(clock=lambda: 900_000_000)
    AutonomousGoalPersistenceCoordinator(restarted_ledger, JsonAutonomousGoalSnapshotPersistence(goal_store)).restore()
    restarted_journal = AutonomousGoalWorkerJournalPersistenceCoordinator(
        AutonomousGoalWorkerJournal(clock=lambda: 900_000_000),
        JsonAutonomousGoalWorkerJournalPersistence(journal_store),
    )
    restarted_control = AutonomousGoalControlLoopPersistenceCoordinator(
        JsonAutonomousGoalControlLoopSnapshotPersistence(checkpoint_store),
    )
    restarted_recovery = AutonomousGoalRecoveryCoordinator(restarted_ledger, restarted_journal, restarted_control)
    assert restarted_recovery.restore(now_ns="900000000")["status"] == "restored"
    terminal_loop = AutonomousGoalControlLoop(
        AutonomousGoalWorker(
            restarted_ledger,
            journal=restarted_journal.journal,
            resolver=lambda _goal, _row: (_ for _ in ()).throw(AssertionError("completed migrated work must not be rehydrated")),
            executor=execute,
        ),
        batch_id_prefix="migrated-live-cycle",
    )
    terminal = restarted_recovery.resume(terminal_loop, options={"run_id": "migrated-live-cycle", "max_cycles": 2})
    assert terminal.stop_reason == "all_terminal"
    assert dispatches == 1
