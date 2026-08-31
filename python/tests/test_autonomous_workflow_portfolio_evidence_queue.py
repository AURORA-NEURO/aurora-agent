from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    ArgumentError,
    AutonomousAgent,
    AutonomousWorkflowPortfolioExecutionCheckpoint,
    AutonomousWorkflowPortfolioExecutionItem,
    AutonomousWorkflowPortfolioExecutionResult,
    AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker,
    AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator,
    AutonomousWorkflowPortfolioEvidenceWorkWorker,
    InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
    InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
    JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
    LLMRuntime,
    SQLiteAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
    admit_autonomous_workflow_portfolio_evidence_work_items,
    autonomous_workflow_portfolio_provider_execution_digest,
    content_digest,
    validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot,
)
from prism_sdk.autonomous_workflow_portfolio import AutonomousWorkflowPortfolioPlan


def _agent() -> AutonomousAgent:
    return AutonomousAgent(object(), LLMRuntime())


def _execution(agent: AutonomousAgent, domains=AUTONOMOUS_DOMAINS, *, chain: bool = False):
    requests = []
    for index, domain in enumerate(domains):
        item = {
            "id": f"item-{index}-{domain}",
            "task": f"perform a bounded {domain} portfolio operation",
            "domain": domain,
        }
        if chain and index:
            item["depends_on"] = [f"item-{index - 1}-{domains[index - 1]}"]
        requests.append(item)
    plan = AutonomousWorkflowPortfolioPlan.from_dict(
        agent.plan_workflow_portfolio(requests, require_all_domains=False)
    )
    providers = tuple(
        AutonomousWorkflowPortfolioExecutionItem(
            item_id=item.item_id,
            domain=item.domain,
            depends_on=item.depends_on,
            status="succeeded",
            result_digest=content_digest({"provider": item.item_id}),
            result_bytes=32,
        )
        for item in plan.items
    )
    item_ids = tuple(item.item_id for item in plan.items)
    sorted_providers = tuple(sorted(providers, key=lambda item: item.item_id))
    checkpoint = AutonomousWorkflowPortfolioExecutionCheckpoint.create(
        job_id="provider-job",
        plan_digest=plan.portfolio_digest,
        portfolio_input_digest=content_digest(requests),
        item_ids=item_ids,
        request_digests=tuple(item.request_digest for item in plan.items),
        task_digests=tuple(item.task_digest for item in plan.items),
        settled_item_ids=tuple(sorted(item_ids)),
        settled_result_digests=tuple(item.result_digest for item in sorted_providers),
        max_parallelism=4,
        stop_on_error=False,
        status="completed",
    )
    return AutonomousWorkflowPortfolioExecutionResult(
        status="completed",
        plan=plan,
        items=providers,
        executed_waves=plan.dependency_graph.waves,
        completed_count=len(providers),
        failed_count=0,
        blocked_count=0,
        approval_required_count=0,
        next_action="complete",
        checkpoint=checkpoint,
    )


def _admit(queue, execution, agent, *, job_id="queue-job", now=1_000):
    evidence_plan = agent.evidence_plan(tuple(item.domain for item in execution.plan.items))
    request_digests = tuple(
        content_digest({"evidence_request": item.item_id})
        for item in execution.plan.items
    )
    return admit_autonomous_workflow_portfolio_evidence_work_items(
        queue,
        job_id=job_id,
        execution=execution,
        evidence_plan_digest=evidence_plan.plan_digest,
        item_request_digests=request_digests,
        now=now,
    )


def test_queue_admits_every_builtin_domain_and_binds_all_execution_identities():
    agent = _agent()
    execution = _execution(agent)
    queue = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    admitted = _admit(queue, execution, agent)

    assert len(admitted) == len(AUTONOMOUS_DOMAINS)
    assert {item.domain for item in admitted} == set(AUTONOMOUS_DOMAINS)
    assert len(queue.pending(now=1_000)) == len(AUTONOMOUS_DOMAINS)
    assert all(item.provider_execution_digest == autonomous_workflow_portfolio_provider_execution_digest(execution) for item in admitted)
    assert all(item.portfolio_plan_digest == execution.plan.portfolio_digest for item in admitted)
    assert all(item.work_id.startswith("queue-job:") for item in admitted)
    wire = json.dumps(queue.snapshot(), sort_keys=True)
    assert "perform a bounded" not in wire
    assert "provider_result" not in wire


def test_queue_enforces_dependency_waves_leases_and_expiry_reconciliation():
    agent = _agent()
    execution = _execution(agent, ("coding", "data", "science"), chain=True)
    queue = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    admitted = _admit(queue, execution, agent)
    root, child, grandchild = admitted

    assert [item.work_id for item in queue.pending(now=1_000)] == [root.work_id]
    claimed = queue.claim(root.work_id, "worker-a", lease_ms=10, now=1_000)
    assert claimed is not None and claimed.attempts == 1
    assert queue.claim(root.work_id, "worker-b", now=1_001) is None
    with pytest.raises(ArgumentError):
        queue.renew(root.work_id, "worker-b", now=1_002)
    with pytest.raises(ArgumentError):
        queue.complete(
            root.work_id,
            "worker-b",
            status="completed",
            result_digest=content_digest({"root": True}),
            now=1_003,
        )

    expired = queue.reclaim_expired(now=1_010)
    assert expired[0].failure_class == "lease_expired"
    assert queue.get(root.work_id).status == "reconciliation_required"
    # Explicit requeue makes the root runnable again; dependents remain held until it settles.
    assert queue.requeue(root.work_id, now=1_011).status == "queued"
    assert [item.work_id for item in queue.pending(now=1_011)] == [root.work_id]
    claimed_again = queue.claim(root.work_id, "worker-a", now=1_011)
    assert claimed_again is not None
    queue.complete(
        root.work_id,
        "worker-a",
        status="completed",
        result_digest=content_digest({"root": "settled"}),
        now=1_012,
    )
    assert [item.work_id for item in queue.pending(now=1_012)] == [child.work_id]
    queue.claim(child.work_id, "worker-a", now=1_012)
    queue.complete(
        child.work_id,
        "worker-a",
        status="completed",
        result_digest=content_digest({"child": "settled"}),
        now=1_013,
    )
    assert [item.work_id for item in queue.pending(now=1_013)] == [grandchild.work_id]


def test_worker_retries_are_bounded_and_settlement_requires_explicit_result_digest():
    agent = _agent()
    execution = _execution(agent, ("coding",))
    queue = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    admitted = _admit(queue, execution, agent, now=1_000)
    calls = []

    def execute(item, _context):
        calls.append(item.work_id)
        if len(calls) == 1:
            return {"status": "failed", "error_class": "transport_error", "retryable": True}
        return {"status": "completed", "result_digest": content_digest({"work": item.work_id})}

    worker = AutonomousWorkflowPortfolioEvidenceWorkWorker(queue, execute)
    first = worker.run(worker_id="worker-a", now=1_000)
    assert first["retried"] == 1
    assert queue.get(admitted[0].work_id).status == "queued"
    second = worker.run(worker_id="worker-a", now=2_000)
    assert second["completed"] == 1
    assert queue.get(admitted[0].work_id).status == "completed"

    with pytest.raises(ArgumentError):
        queue.complete(
            admitted[0].work_id,
            "worker-a",
            status="completed",
            result_digest="not-a-digest",
            now=2_001,
        )


def test_json_sqlite_and_atomic_coordinators_fence_stale_workers_and_tampering():
    agent = _agent()
    execution = _execution(agent, ("coding", "science"))
    queue = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    _admit(queue, execution, agent)
    snapshot = queue.snapshot()

    persistence = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence()
    persistence.write(snapshot)
    assert persistence.read() == snapshot
    queue_two = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    coordinator = AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator(queue_two, persistence)
    assert coordinator.restore()["status"] == "restored"
    work_id = snapshot["items"][0]["work_id"]
    claimed = coordinator.claim(work_id, "worker-a", now=1_001)
    assert claimed is not None
    assert persistence.write_if_unchanged(snapshot["snapshot_digest"], queue.snapshot()) is False

    class _TextStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

    text_store = _TextStore()
    json_persistence = JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence(text_store)
    json_persistence.write(queue.snapshot())
    assert json_persistence.read() == queue.snapshot()

    with SQLiteAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence(":memory:") as sqlite_persistence:
        sqlite_persistence.write(queue.snapshot())
        assert sqlite_persistence.read() == queue.snapshot()
        assert sqlite_persistence.write_if_unchanged(
            queue.snapshot()["snapshot_digest"], queue.snapshot()
        ) is True

    tampered_item = dict(snapshot["items"][0])
    tampered_item["item_digest"] = "0" * 64
    tampered = dict(snapshot)
    tampered["items"] = [tampered_item, *snapshot["items"][1:]]
    with pytest.raises(ArgumentError):
        validate_autonomous_workflow_portfolio_evidence_work_queue_snapshot(tampered)


def test_atomic_worker_runs_all_domains_without_persisting_executor_values():
    agent = _agent()
    execution = _execution(agent)
    source = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence()
    queue = InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue()
    _admit(queue, execution, agent)
    source.write(queue.snapshot())
    coordinator = AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator(
        InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue(), source
    )
    observed = []

    def execute(item, context):
        observed.append(item.domain)
        context["renew"](now=1_001)
        return {
            "status": "completed",
            "result_digest": content_digest({"result": item.work_id}),
        }

    worker = AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker(coordinator, execute)
    result = worker.run(worker_id="atomic-worker", now=1_000)
    assert result["completed"] == len(AUTONOMOUS_DOMAINS)
    assert set(observed) == set(AUTONOMOUS_DOMAINS)
    final = coordinator.snapshot()
    assert all(item["status"] == "completed" for item in final["items"])
    assert "transient" not in json.dumps(final)
