from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    ArgumentError,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceWorker,
    AutonomousEvidenceWorkQueuePersistenceCoordinator,
    TransactionalJsonAutonomousEvidenceWorkQueueSnapshotPersistence,
    InMemoryAutonomousEvidenceRuntimeJournal,
    InMemoryAutonomousEvidenceWorkQueue,
    build_autonomous_evidence_plan,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
    SQLiteAutonomousEvidenceWorkQueuePersistence,
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


class _Evaluator:
    evaluator_id = "worker-evaluator"
    evaluator_version = "1"

    def evaluate(self, value):
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
            "evidence_digest": value["requirement"].workflow_digest,
        }


def _request(requirement, index=0):
    return {
        "requirement_id": requirement.requirement_id,
        "source_id": f"worker-source-{index}",
        "request_id": f"worker-request-{index}",
        "metadata": {"fixture": True, "domain": requirement.domain},
    }


def _execute(calls, evaluator=True):
    def acquire(context):
        calls.append(context["requirement"].requirement_id)
        return {"fixture": "transient-evidence", "requirement": context["requirement"].requirement_id}

    def project(_value, context):
        return ({"label": context["requirement"].label, "status": "observed", "kind": "fact", "confidence": 1.0},)

    result = {"acquirer": acquire, "projector": project}
    if evaluator:
        result["evaluator"] = _Evaluator()
    return result


def _single_domain_plan(domain):
    profile = next(profile for profile in builtin_autonomous_domain_profiles() if profile.domain == domain)
    workflow = next(workflow for workflow in builtin_autonomous_workflow_strategies() if workflow.domain == profile.domain)
    baseline = build_autonomous_evidence_plan([workflow])
    return build_autonomous_evidence_plan(
        [workflow],
        available_evidence=tuple(item.requirement_id for item in baseline.requirements[1:]),
    )


def test_evidence_worker_executes_one_accepted_request_for_every_autonomous_domain():
    queue = InMemoryAutonomousEvidenceWorkQueue()
    contexts = {}
    for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES):
        plan = _single_domain_plan(domain)
        request = _request(plan.requirements[0], index)
        journal = InMemoryAutonomousEvidenceRuntimeJournal()
        calls = []
        item = queue.enqueue(work_id=f"evidence-work-{domain}", plan=plan, request=request, now=1_000)
        contexts[item.work_id] = {
            "plan": plan,
            "request": request,
            "runtime": AutonomousEvidenceRuntime(plan=plan, journal=journal),
            "execute": _execute(calls),
        }

    run = AutonomousEvidenceWorker(queue, lambda item: contexts[item.work_id]).run(worker_id="worker-a", limit=len(AUTONOMOUS_DOMAIN_NAMES), now=1_000)
    assert run["completed"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert run["failed"] == 0
    assert run["reconciled"] == 0
    assert all(item.status == "completed" for item in queue.rows())
    assert all(row["value_retained"] is False for row in run["rows"])
    assert all(row["result_digest"] and row["receipt_digest"] for row in run["rows"])


def test_worker_handoff_is_explicit_and_restart_does_not_reacquire():
    plan = _single_domain_plan("science")
    request = _request(plan.requirements[0])
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    queue = InMemoryAutonomousEvidenceWorkQueue()
    item = queue.enqueue(work_id="pending-evidence", plan=plan, request=request, now=2_000)
    calls = []
    first = AutonomousEvidenceWorker(
        queue,
        lambda _item: {"plan": plan, "request": request, "runtime": AutonomousEvidenceRuntime(plan=plan, journal=journal), "execute": _execute(calls, evaluator=False)},
    )
    pending = first.run(worker_id="worker-a", now=2_000)
    assert pending["awaiting_evaluation"] == 1
    assert queue.get(item.work_id).status == "awaiting_evaluation"
    assert len(calls) == 1

    snapshot = queue.snapshot()
    restarted_queue = InMemoryAutonomousEvidenceWorkQueue()
    persistence = AutonomousEvidenceWorkQueuePersistenceCoordinator(restarted_queue, type("Persistence", (), {"read": lambda self: snapshot, "write": lambda self, _snapshot: None})())
    assert persistence.restore()["status"] == "restored"
    restarted_queue.requeue(item.work_id, now=3_000)
    restarted_runtime = AutonomousEvidenceRuntime(plan=plan, journal=journal)
    restarted_runtime.rehydrate()
    restarted = AutonomousEvidenceWorker(
        restarted_queue,
        lambda _item: {
            "plan": plan,
            "request": request,
            "runtime": restarted_runtime,
            "execute": {
                **_execute(calls),
                "rehydrate_value": lambda _receipt: {"fixture": "transient-evidence", "requirement": plan.requirements[0].requirement_id},
                "reevaluate_pending": True,
            },
        },
    )
    accepted = restarted.run(worker_id="worker-b", now=3_000)
    assert accepted["completed"] == 1
    assert restarted_queue.get(item.work_id).status == "completed"
    assert len(calls) == 1


def test_worker_leases_are_fenced_and_snapshot_tampering_is_rejected():
    plan = _single_domain_plan("coding")
    request = _request(plan.requirements[0])
    queue = InMemoryAutonomousEvidenceWorkQueue()
    item = queue.enqueue(work_id="fenced-evidence", plan=plan, request=request, now=4_000)
    assert queue.claim(item.work_id, "worker-a", lease_ms=100, now=4_000) is not None
    assert queue.claim(item.work_id, "worker-b", lease_ms=100, now=4_050) is None
    with pytest.raises(ArgumentError, match="cannot be renewed"):
        queue.renew(item.work_id, "worker-b", lease_ms=100, now=4_060)
    snapshot = queue.snapshot()
    tampered = {**snapshot, "items": [{**snapshot["items"][0], "source_id": "tampered"}]}
    with pytest.raises(ArgumentError, match="snapshot digest is invalid"):
        InMemoryAutonomousEvidenceWorkQueue().restore(tampered)
    with pytest.raises(ArgumentError, match="not waiting"):
        queue.requeue(item.work_id, now=4_070)


def test_expired_evidence_leases_distinguish_pre_dispatch_from_in_flight_execution():
    plan = _single_domain_plan("science")
    queue = InMemoryAutonomousEvidenceWorkQueue()

    pre_dispatch = queue.enqueue(work_id="pre-dispatch-expired", plan=plan, request=_request(plan.requirements[0], 1), now=4_100)
    assert queue.claim(pre_dispatch.work_id, "worker-a", lease_ms=10, now=4_100) is not None
    reclaimed = queue.reclaim_expired(now=4_110)
    assert reclaimed[0].status == "queued"
    assert reclaimed[0].execution_phase == "not_started"
    assert pre_dispatch.work_id in {item.work_id for item in queue.pending(now=4_110)}

    in_flight = queue.enqueue(work_id="in-flight-expired", plan=plan, request=_request(plan.requirements[0], 2), now=4_100)
    assert queue.claim(in_flight.work_id, "worker-a", lease_ms=10, now=4_100) is not None
    queue.begin_execution(in_flight.work_id, "worker-a", now=4_105)
    expired = queue.reclaim_expired(now=4_115)
    quarantined = next(item for item in expired if item.work_id == in_flight.work_id)
    assert quarantined.status == "reconciliation_required"
    assert quarantined.execution_phase == "running"
    assert quarantined.reconciliation_digest is None
    with pytest.raises(ArgumentError, match="no-effect reconciliation"):
        queue.requeue(in_flight.work_id, now=4_120)
    with pytest.raises(ArgumentError, match="active or uncertain"):
        queue.cancel(in_flight.work_id, now=4_120)


def test_evidence_reconciliation_receipts_are_idempotent_and_bound_safe_requeue():
    plan = _single_domain_plan("coding")
    queue = InMemoryAutonomousEvidenceWorkQueue()

    successful = queue.enqueue(work_id="reconcile-success", plan=plan, request=_request(plan.requirements[0], 3), now=4_200)
    queue.claim(successful.work_id, "worker-a", lease_ms=100, now=4_200)
    queue.begin_execution(successful.work_id, "worker-a", now=4_201)
    queue.reclaim_expired(now=4_301)
    settled = queue.settle_reconciliation(successful.work_id, outcome="succeeded", evidence_digest="a" * 64, now=4_302)
    assert settled.status == "completed"
    assert settled.execution_phase == "settled"
    assert settled.result_digest == settled.reconciliation_digest
    assert queue.settle_reconciliation(successful.work_id, outcome="succeeded", evidence_digest="a" * 64, now=4_303) == settled
    with pytest.raises(ArgumentError, match="conflicts"):
        queue.settle_reconciliation(successful.work_id, outcome="failed", evidence_digest="b" * 64, now=4_304)

    no_effect = queue.enqueue(work_id="reconcile-no-effect", plan=plan, request=_request(plan.requirements[0], 4), now=4_200)
    queue.claim(no_effect.work_id, "worker-a", lease_ms=100, now=4_200)
    queue.begin_execution(no_effect.work_id, "worker-a", now=4_201)
    queue.reclaim_expired(now=4_301)
    observed = queue.settle_reconciliation(no_effect.work_id, outcome="not_executed", evidence_digest="c" * 64, now=4_302)
    assert observed.reconciliation_effect_absent is True
    with pytest.raises(ArgumentError, match="matching reconciliation digest"):
        queue.requeue(no_effect.work_id, reconciliation_digest="d" * 64, now=4_303)
    queued = queue.requeue(no_effect.work_id, reconciliation_digest=observed.reconciliation_digest, now=4_304)
    assert queued.status == "queued"
    assert queued.execution_phase == "not_started"
    assert queued.reconciliation_digest is None
    assert queued.reconciliation_history == (observed.reconciliation_digest,)
    queue.claim(no_effect.work_id, "worker-a", lease_ms=100, now=4_305)
    queue.begin_execution(no_effect.work_id, "worker-a", now=4_306)
    queue.reclaim_expired(now=4_406)
    second_observed = queue.settle_reconciliation(no_effect.work_id, outcome="not_executed", evidence_digest="e" * 64, now=4_407)
    assert second_observed.reconciliation_digest != observed.reconciliation_digest
    assert second_observed.reconciliation_history == (observed.reconciliation_digest,)
    assert queue.settle_reconciliation(no_effect.work_id, outcome="not_executed", evidence_digest="e" * 64, now=4_408) == second_observed
    restored = InMemoryAutonomousEvidenceWorkQueue()
    restored.restore(queue.snapshot())
    assert restored.get(no_effect.work_id).reconciliation_digest == second_observed.reconciliation_digest
    assert restored.get(no_effect.work_id).reconciliation_history == (observed.reconciliation_digest,)


def test_worker_never_retries_a_runtime_failure_after_execution_begins():
    plan = _single_domain_plan("operations")
    request = _request(plan.requirements[0], 5)
    queue = InMemoryAutonomousEvidenceWorkQueue()

    def acquire(_context):
        raise RuntimeError("caller transport failed after dispatch")

    item = queue.enqueue(work_id="post-dispatch-failure", plan=plan, request=request, now=4_400)
    worker = AutonomousEvidenceWorker(
        queue,
        lambda _item: {
            "plan": plan,
            "request": request,
            "runtime": AutonomousEvidenceRuntime(plan=plan),
            "execute": {"acquirer": acquire},
        },
    )
    result = worker.run(worker_id="worker-a", now=4_400)
    assert result["reconciled"] == 1
    assert result["retried"] == 0
    assert queue.get(item.work_id).status == "reconciliation_required"
    assert queue.get(item.work_id).execution_phase == "running"


def test_worker_identity_mismatch_quarantines_work():
    plan = _single_domain_plan("biomedical")
    request = _request(plan.requirements[0])
    queue = InMemoryAutonomousEvidenceWorkQueue()
    item = queue.enqueue(work_id="identity-evidence", plan=plan, request=request, now=5_000)
    worker = AutonomousEvidenceWorker(
        queue,
        lambda _item: {
            "plan": plan,
            "request": {**request, "source_id": "different-source"},
            "runtime": AutonomousEvidenceRuntime(plan=plan),
            "execute": _execute([]),
        },
    )
    result = worker.run(worker_id="worker-a", now=5_000)
    assert result["reconciled"] == 1
    assert result["rows"][0]["error_class"] == "identity_conflict"
    assert queue.get(item.work_id).status == "reconciliation_required"


def test_work_queue_rejects_credential_shaped_metadata_before_persistence():
    plan = _single_domain_plan("operations")
    requirement = plan.requirements[0]
    queue = InMemoryAutonomousEvidenceWorkQueue()
    with pytest.raises(ArgumentError, match="credential-shaped metadata"):
        queue.enqueue(
            work_id="secret-metadata-evidence",
            plan=plan,
            request={
                **_request(requirement),
                "metadata": {"api_key": "caller-secret-must-never-enter-the-queue"},
            },
            now=6_000,
        )


def test_sqlite_persistence_restores_metadata_only_queue_across_process_objects(tmp_path):
    plan = _single_domain_plan("science")
    request = _request(plan.requirements[0])
    path = tmp_path / "evidence-work-queue.sqlite3"
    queue = InMemoryAutonomousEvidenceWorkQueue()
    with SQLiteAutonomousEvidenceWorkQueuePersistence(path) as persistence:
        coordinator = AutonomousEvidenceWorkQueuePersistenceCoordinator(queue, persistence)
        assert coordinator.restore()["status"] == "empty"
        queue.enqueue(work_id="sqlite-evidence", plan=plan, request=request, now=7_000)
        snapshot = coordinator.flush()
        assert snapshot["snapshot_digest"]

    reopened = InMemoryAutonomousEvidenceWorkQueue()
    with SQLiteAutonomousEvidenceWorkQueuePersistence(path) as persistence:
        coordinator = AutonomousEvidenceWorkQueuePersistenceCoordinator(reopened, persistence)
        restored = coordinator.restore()
        assert restored["status"] == "restored"
        assert restored["items"] == 1
        assert reopened.get("sqlite-evidence").request_digest == queue.get("sqlite-evidence").request_digest

    assert b"transient-evidence" not in path.read_bytes()
    assert b"caller-secret" not in path.read_bytes()


def test_text_persistence_is_canonical_plan_safe_and_stale_writer_fenced():
    plan = _single_domain_plan("science")
    queue = InMemoryAutonomousEvidenceWorkQueue()
    queue.enqueue(work_id="text-evidence", plan=plan, request=_request(plan.requirements[0]), now=8_000)
    backend = _CasTextStore()
    persistence = TransactionalJsonAutonomousEvidenceWorkQueueSnapshotPersistence(backend)
    source = AutonomousEvidenceWorkQueuePersistenceCoordinator(queue, persistence)
    flushed = source.flush()
    restarted = InMemoryAutonomousEvidenceWorkQueue()
    restarted_coordinator = AutonomousEvidenceWorkQueuePersistenceCoordinator(restarted, persistence)
    assert restarted_coordinator.restore()["snapshot_digest"] == flushed["snapshot_digest"]
    backend.value = json.dumps(json.loads(backend.value), indent=2)
    with pytest.raises(ArgumentError, match="not canonical"):
        persistence.read()
    persistence.write(queue.snapshot())
    persistence.write(InMemoryAutonomousEvidenceWorkQueue().snapshot())
    with pytest.raises(ArgumentError, match="compare-and-swap conflict"):
        restarted_coordinator.flush()
