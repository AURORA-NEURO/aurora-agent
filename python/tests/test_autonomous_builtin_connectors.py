import json
from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorRegistration,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorOperationRegistry,
    AutonomousConnectorOperationFacade,
    AutonomousConnectorOperationInput,
    AutonomousConnectorIntentJobController,
    AutonomousConnectorIntentPlan,
    InMemoryAutonomousConnectorWorkQueue,
    LLMRuntime,
    builtin_autonomous_connector_registration,
    content_digest,
    register_builtin_autonomous_connectors,
)
from prism_sdk.errors import ArgumentError


class _SnapshotStore:
    def __init__(self):
        self.snapshot = None

    def read(self):
        return self.snapshot

    def write(self, snapshot):
        self.snapshot = snapshot


def _request(operation_id: str, domain: str, connector_id: str, plan_digest: str, *, approved: bool = True):
    fields = {
        "coding": {"repository_digest": "repo", "changed_files": ["src/app.py"], "test_results": {"passed": 3}},
        "browser": {"source_digests": ["source-a"], "retrieved_at": "2026-08-21T00:00:00Z", "citation_metadata": {"count": 1}},
        "data": {"schema": {"columns": 3}, "row_count": 12, "column_count": 3, "lineage": ["fixture"]},
        "science": {"hypothesis": "fixture", "evidence_digests": ["evidence-a"], "analysis_digest": "analysis"},
        "biomedical": {"provenance": "fixture", "cohort_digest": "cohort", "review_questions": ["scope"]},
        "neuroscience": {"signal_digest": "signal", "sampling_rate": 100, "study_design": "fixture"},
        "operations": {"incident_digest": "incident", "telemetry_digest": "telemetry", "runbook_digest": "runbook"},
        "enterprise": {"workflow_digest": "workflow", "record_type": "fixture", "policy_digest": "policy"},
        "multi_agent": {"delegation_digest": "delegation", "agent_digests": ["agent-a"], "conflicts": []},
        "multimodal": {"modalities": ["document", "image"], "asset_digests": ["asset-a"], "alignment_digest": "alignment"},
        "cross_domain": {"domain_digests": ["domain-a"], "evidence_digests": ["evidence-a"], "route_digest": "route"},
        "evaluation": {"benchmark_digest": "benchmark", "case_count": 2, "replay_digest": "replay"},
    }[domain]
    return AutonomousConnectorDispatchRequest(
        dispatch_id=f"dispatch-{domain}",
        execution_id=f"execution-{domain}",
        call_id=f"call-{domain}",
        connector_id=connector_id,
        domains=(domain,),
        capability="review" if domain == "coding" else "reproducibility" if domain in {"science", "neuroscience"} else "governance" if domain == "enterprise" else "routing" if domain == "cross_domain" else "rubric" if domain == "evaluation" else "evidence_read",
        request={"operation_id": operation_id, "subject_digest": content_digest({"domain": domain}), **fields},
        parent_digests=(content_digest({"parent": domain}),),
        selection_plan_digest=plan_digest,
        approved=approved,
    )


def test_builtin_adapter_covers_and_dispatches_every_domain_without_raw_values(tmp_path) -> None:
    operation_registry = AutonomousConnectorOperationRegistry()
    registration = builtin_autonomous_connector_registration(operation_registry)
    registry = AutonomousConnectorRegistry([registration])
    journal = AutonomousConnectorReceiptJournal(tmp_path / "builtin-receipts.jsonl")
    calls: list[str] = []
    original = registration.executor

    def counted(manifest, request):
        calls.append(request["operation_id"])
        return original(manifest, request)

    registry = AutonomousConnectorRegistry([
        AutonomousConnectorRegistration(registration.manifest, counted, approval_required=True),
    ])
    runtime = AutonomousConnectorRuntime(registry, receipt_store=journal)

    operations = {operation.domain: operation for operation in operation_registry.operations()}
    for domain in AUTONOMOUS_DOMAINS:
        operation = operations[domain]
        capability = "review" if domain == "coding" else "reproducibility" if domain in {"science", "neuroscience"} else "governance" if domain == "enterprise" else "routing" if domain == "cross_domain" else "rubric" if domain == "evaluation" else "evidence_read"
        # Use a capability declared by the operation; the compact fixture above intentionally
        # exercises the ordinary all-domain route and the operation itself remains authoritative.
        if capability not in operation.capabilities:
            capability = operation.capabilities[0]
        plan = registry.select_for_domains((domain,), capability=capability)
        request = _request(operation.operation_id, domain, registration.connector_id, plan.plan_digest)
        request = replace(request, capability=capability)
        result = runtime.dispatch_from_plan(plan, request)
        assert result.receipt.status == "observed"
        assert result.value["operation_id"] == operation.operation_id
        assert result.value["domain"] == domain
        assert "repo" not in json.dumps(result.receipt.to_dict())
        assert "src/app.py" not in json.dumps(result.receipt.to_dict())
        assert result.value["secret_material"] == "never_accepted_or_returned"

    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert journal.verify_integrity()["entries"] == len(AUTONOMOUS_DOMAINS)


def test_builtin_adapter_requires_approval_and_replays_without_invocation(tmp_path) -> None:
    registration = builtin_autonomous_connector_registration()
    registry = AutonomousConnectorRegistry([registration])
    journal = AutonomousConnectorReceiptJournal(tmp_path / "replay.jsonl")
    calls: list[str] = []
    original = registration.executor

    def counted(manifest, request):
        calls.append(request["operation_id"])
        return original(manifest, request)

    registry.register(
        AutonomousConnectorRegistration(registration.manifest, counted, approval_required=True),
        replace=True,
    )
    plan = registry.select_for_domains(("coding",), capability="review")
    refused = AutonomousConnectorRuntime(registry).dispatch_from_plan(
        plan,
        _request("coding.repository_change_analysis", "coding", registration.connector_id, plan.plan_digest, approved=False),
    )
    assert refused.receipt.status == "refused"
    assert refused.receipt.failure_class == "approval_required"
    assert calls == []

    first = AutonomousConnectorRuntime(registry, receipt_store=journal).dispatch_from_plan(
        plan,
        _request("coding.repository_change_analysis", "coding", registration.connector_id, plan.plan_digest),
    )
    assert first.replay == "fresh"
    assert calls == ["coding.repository_change_analysis"]
    replay = AutonomousConnectorRuntime(registry, receipt_store=journal).dispatch_from_plan(
        plan,
        _request("coding.repository_change_analysis", "coding", registration.connector_id, plan.plan_digest),
    )
    assert replay.replay == "replayed"
    assert replay.value is None
    assert calls == ["coding.repository_change_analysis"]


def test_builtin_registration_is_agent_ready_and_rejects_secret_shaped_input() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    registration = agent.register_builtin_connectors()
    assert registration.connector_id == "builtin.offline-evidence"
    assert agent.connector_catalogue()["connector_count"] == 1
    plan = agent.connector_selection_plan(("coding",), capability="review")
    request = _request("coding.repository_change_analysis", "coding", registration.connector_id, plan.plan_digest)
    result = agent.dispatch_connector(plan, request)
    assert result.receipt.status == "observed"
    assert result.value["evidence_posture"].startswith("caller_supplied_metadata")

    with pytest.raises(ArgumentError, match="credential-shaped"):
        AutonomousConnectorDispatchRequest(
            dispatch_id="secret-dispatch",
            execution_id="secret-execution",
            call_id="secret-call",
            connector_id=registration.connector_id,
            domains=("coding",),
            capability="review",
            request={
                "operation_id": "coding.repository_change_analysis",
                "subject_digest": "a" * 64,
                "api_key": "must-never-enter-the-dispatch-boundary",
            },
            selection_plan_digest=plan.plan_digest,
            approved=True,
        )


def test_intent_job_controller_restores_flushes_and_rolls_back_partial_submission() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    agent.register_builtin_connectors(approval_required=True)
    intent = agent.connector_intent_facade()
    request_by_domain = {
        "data": {"schema": {"columns": ["id"]}, "fixture_value": "controller-private-data"},
        "science": {"hypothesis": "controller-private-science"},
    }
    transient = {
        "task": "Profile a dataset schema and reproduce the scientific evidence.",
        "hints": ("data", "science"),
        "max_domains": 2,
        "allow_cross_domain": True,
        "request_by_domain": request_by_domain,
        "approved": True,
    }
    plan = intent.plan(**transient)
    queue = InMemoryAutonomousConnectorWorkQueue()
    store = _SnapshotStore()
    controller = AutonomousConnectorIntentJobController(intent, queue, store)
    with pytest.raises(ArgumentError, match="restore before"):
        controller.enqueue(plan, {"job_id": "controller-job-1", **transient})

    restored = controller.restore()
    assert restored["status"] == "empty"
    submitted = controller.enqueue(plan, {"job_id": "controller-job-1", **transient, "now": 1_000})
    serialized = json.dumps(submitted)
    assert submitted["status"] == "submitted"
    assert submitted["items"] == 2
    assert transient["task"] not in serialized
    assert "controller-private-data" not in serialized
    assert "controller-private-science" not in serialized
    assert transient["task"] not in json.dumps(store.snapshot)
    assert "controller-private-data" not in json.dumps(store.snapshot)

    restarted_queue = InMemoryAutonomousConnectorWorkQueue()
    restarted_controller = AutonomousConnectorIntentJobController(intent, restarted_queue, store)
    assert restarted_controller.restore()["status"] == "restored"
    executed = restarted_controller.run_queued(
        plan,
        {"job_id": "controller-job-1", **transient, "worker_id": "controller-worker-1", "now": 1_000},
    )
    assert executed["status"] == "executed"
    assert executed["worker"]["completed"] == 2
    assert all(row["value_retained"] is False for row in executed["worker"]["rows"])
    assert all(item["status"] == "completed" for item in store.snapshot["items"])
    assert transient["task"] not in json.dumps(store.snapshot)

    bounded_queue = InMemoryAutonomousConnectorWorkQueue(max_items=1)
    bounded_store = _SnapshotStore()
    bounded_controller = AutonomousConnectorIntentJobController(intent, bounded_queue, bounded_store)
    bounded_controller.restore()
    with pytest.raises(ArgumentError, match="queue is full"):
        bounded_controller.enqueue(plan, {"job_id": "controller-overflow", **transient})
    assert bounded_queue.rows() == ()
    assert bounded_store.snapshot["items"] == []


def test_builtin_adapter_reports_sparse_fixtures_as_partial() -> None:
    registration = builtin_autonomous_connector_registration()
    observation = registration.executor(
        registration.manifest,
        {
            "operation_id": "browser.web_evidence_retrieval",
            "subject_digest": "b" * 64,
            "source_digests": ["source-only"],
        },
    )
    assert observation.status == "partial"
    assert observation.failure_class == "incomplete_local_fixture"
    assert observation.value["available_fields"] == ["source_digests"]
    assert "source-only" not in json.dumps(observation.value)


def test_operation_facade_covers_every_domain_and_replays_without_request_values(tmp_path) -> None:
    operation_registry = AutonomousConnectorOperationRegistry()
    registration = builtin_autonomous_connector_registration(
        operation_registry,
        approval_required=True,
    )
    registry = AutonomousConnectorRegistry([registration])
    journal = AutonomousConnectorReceiptJournal(tmp_path / "operation-facade.jsonl")
    runtime = AutonomousConnectorRuntime(registry, receipt_store=journal)
    facade = AutonomousConnectorOperationFacade(registry, runtime, operation_registry)

    inputs = tuple(
        AutonomousConnectorOperationInput(
            domain=operation.domain,
            capability=operation.capabilities[0],
            operation_id=operation.operation_id,
            request={"fixture_digest": "a" * 64, "raw_note": "must remain transient"},
            approved=True,
        )
        for operation in operation_registry.operations()
    )
    plans = tuple(facade.plan(value) for value in inputs)
    assert len(plans) == len(AUTONOMOUS_DOMAINS)
    assert all(plan.status == "ready" for plan in plans)
    assert all("raw_note" not in json.dumps(plan.to_dict()) for plan in plans)

    events: list[dict[str, object]] = []
    executions = tuple(
        facade.execute_planned(
            plan,
            value,
            trace_event_callback=(lambda **event: events.append(event)) if index == 0 else None,
        )
        for index, (plan, value) in enumerate(zip(plans, inputs))
    )
    assert {execution.operation_plan.domain for execution in executions} == set(AUTONOMOUS_DOMAINS)
    assert all(execution.status == "partial" for execution in executions)
    assert all(execution.replay == "fresh" for execution in executions)
    assert [event["phase"] for event in events] == ["connector_started", "connector_finished"]

    replay = facade.execute(inputs[0])
    assert replay.replay == "replayed"
    assert replay.dispatch.value is None
    assert len(journal.receipts()) == len(AUTONOMOUS_DOMAINS)

    tampered = AutonomousConnectorOperationInput(
        domain=inputs[0].domain,
        capability=inputs[0].capability,
        operation_id=inputs[0].operation_id,
        request={"fixture_digest": "b" * 64, "raw_note": "tampered"},
        approved=True,
    )
    with pytest.raises(ArgumentError, match="does not match"):
        facade.execute_planned(plans[0], tampered)


def test_operation_facade_batch_preserves_order_and_omits_after_stop(tmp_path) -> None:
    operation_registry = AutonomousConnectorOperationRegistry()
    registration = builtin_autonomous_connector_registration(
        operation_registry,
        approval_required=True,
    )
    registry = AutonomousConnectorRegistry([registration])
    runtime = AutonomousConnectorRuntime(
        registry,
        receipt_store=AutonomousConnectorReceiptJournal(tmp_path / "operation-batch.jsonl"),
    )
    facade = AutonomousConnectorOperationFacade(registry, runtime, operation_registry)
    coding = operation_registry.resolve("coding.repository_change_analysis")
    values = [
        AutonomousConnectorOperationInput(
            domain="coding",
            capability=coding.capabilities[0],
            operation_id=coding.operation_id,
            request={"fixture": "first"},
            approved=False,
        ),
        AutonomousConnectorOperationInput(
            domain="coding",
            capability=coding.capabilities[0],
            operation_id=coding.operation_id,
            request={"fixture": "second"},
            approved=True,
        ),
        AutonomousConnectorOperationInput(
            domain="coding",
            capability=coding.capabilities[0],
            operation_id=coding.operation_id,
            request={"fixture": "third"},
            approved=True,
        ),
    ]
    result = facade.execute_batch(values, max_parallelism=1, stop_on_error=True)
    assert result.status == "failed"
    assert [item["index"] for item in result.items] == [0, 1, 2]
    assert [item["status"] for item in result.items] == ["refused", "omitted", "omitted"]
    assert result.completed_count == 0
    assert result.failed_count == 1
    assert result.omitted_count == 2
    assert "second" not in json.dumps(result.to_dict())


def test_intent_facade_routes_and_executes_single_and_cross_domain_tasks_without_raw_task_retention() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    agent.register_builtin_connectors(approval_required=True)
    intent = agent.connector_intent_facade()

    coding_plan = intent.plan(
        task="Review changed files and verify testing results.",
        hints=("coding",),
        allow_cross_domain=False,
        request_by_domain={"coding": {"repository_digest": "a" * 64}},
        approved=True,
    )
    assert coding_plan.status == "ready"
    assert coding_plan.selected_domains == ("coding",)
    assert coding_plan.selections[0].operation_id == "coding.repository_change_analysis"
    assert "Review changed files" not in json.dumps(coding_plan.to_dict())
    events: list[dict[str, object]] = []
    coding_result = intent.execute(
        coding_plan,
        task="Review changed files and verify testing results.",
        hints=("coding",),
        allow_cross_domain=False,
        request_by_domain={"coding": {"repository_digest": "a" * 64}},
        approved=True,
        trace_event_callback=lambda **event: events.append(event),
    )
    assert coding_result.status == "completed"
    assert coding_result.executions[0].status == "partial"
    assert [event["phase"] for event in events] == ["connector_started", "connector_finished"]
    restored_coding_plan = AutonomousConnectorIntentPlan.from_mapping(coding_plan.to_dict())
    assert restored_coding_plan.plan_digest == coding_plan.plan_digest

    cross_domain_plan = intent.plan(
        task="Profile a dataset schema and reproduce the scientific evidence.",
        hints=("data", "science"),
        max_domains=2,
        allow_cross_domain=True,
        request_by_domain={
            "data": {"schema": {"columns": ["id"]}},
            "science": {"hypothesis": "fixture"},
        },
        approved=True,
    )
    assert cross_domain_plan.cross_domain is True
    assert set(cross_domain_plan.selected_domains) == {"data", "science"}
    assert all(selection.operation_plan.status == "ready" for selection in cross_domain_plan.selections)

    refused_route = intent.plan(
        task="unclassifiable fixture",
        hints=(),
        min_confidence=1.0,
        allow_cross_domain=False,
        approved=True,
    )
    assert refused_route.status == "route_review_required"
    assert refused_route.selections == ()


def test_intent_facade_enqueues_and_recovers_cross_domain_jobs_without_persisting_transient_values() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    agent.register_builtin_connectors(approval_required=True)
    intent = agent.connector_intent_facade()
    request_by_domain = {
        "data": {"schema": {"columns": ["id"]}, "fixture_value": "data-private-transient"},
        "science": {"hypothesis": "science-private-transient"},
    }
    task = "Profile a dataset schema and reproduce the scientific evidence."
    plan = intent.plan(
        task=task,
        hints=("data", "science"),
        max_domains=2,
        allow_cross_domain=True,
        request_by_domain=request_by_domain,
        approved=True,
    )
    queue = InMemoryAutonomousConnectorWorkQueue()
    job = intent.enqueue(
        plan,
        job_id="intent-job-1",
        queue=queue,
        task=task,
        hints=("data", "science"),
        max_domains=2,
        allow_cross_domain=True,
        request_by_domain=request_by_domain,
        approved=True,
        now=1_000,
    )
    assert job.status == "queued"
    assert job.enqueued_count == 2
    assert job.omitted_count == 0
    serialized_job = json.dumps(job.to_dict())
    assert task not in serialized_job
    assert "data-private-transient" not in serialized_job
    assert "science-private-transient" not in serialized_job
    assert all("request_digest" not in item for item in job.items)
    assert all("subject_digest" not in item for item in job.items)
    assert all("raw" not in json.dumps(item) for item in job.items)
    other_job = intent.enqueue(
        plan,
        job_id="intent-job-2",
        queue=queue,
        task=task,
        hints=("data", "science"),
        max_domains=2,
        allow_cross_domain=True,
        request_by_domain=request_by_domain,
        approved=True,
        now=1_000,
    )

    worker_result = intent.run_queued(
        plan,
        job_id="intent-job-1",
        queue=queue,
        task=task,
        hints=("data", "science"),
        max_domains=2,
        allow_cross_domain=True,
        request_by_domain=request_by_domain,
        approved=True,
        worker_id="intent-worker-1",
        now=1_000,
    )
    assert worker_result["completed"] == 2
    assert worker_result["reconciled"] == 0
    assert "data-private-transient" not in json.dumps(worker_result)
    assert "science-private-transient" not in json.dumps(worker_result)
    assert all(row["value_retained"] is False for row in worker_result["rows"])
    assert all(queue.get(item["work_id"]).status == "completed" for item in job.items)
    assert all(queue.get(item["work_id"]).status == "queued" for item in other_job.items)

    with pytest.raises(ArgumentError, match="does not match"):
        intent.run_queued(
            plan,
            job_id="intent-job-1",
            queue=queue,
            task=task,
            hints=("data", "science"),
            max_domains=2,
            allow_cross_domain=True,
            request_by_domain={"data": {"fixture_value": "tampered"}, "science": request_by_domain["science"]},
            approved=True,
            worker_id="intent-worker-2",
            now=1_001,
        )
