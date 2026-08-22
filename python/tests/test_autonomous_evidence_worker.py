from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    ArgumentError,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceWorker,
    AutonomousEvidenceWorkQueuePersistenceCoordinator,
    InMemoryAutonomousEvidenceRuntimeJournal,
    InMemoryAutonomousEvidenceWorkQueue,
    build_autonomous_evidence_plan,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
)


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
