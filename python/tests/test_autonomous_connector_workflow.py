import json

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousConnectorOperationRegistry,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorRegistration,
    InMemoryAutonomousEvidenceRuntimeJournal,
    AutonomousWorkflowCheckpoint,
    LLMRuntime,
    content_digest,
)
from prism_sdk.autonomous_builtin_connectors import _RECOMMENDED_FIELDS


class _EvidenceProjector:
    def project(self, value, context):
        return [{
            "label": context["requirement"].label,
            "kind": "fact",
            "status": "observed",
            "confidence": 1,
        }]


class _AcceptingEvaluator:
    evaluator_id = "local-stage-evaluator"
    evaluator_version = "1"

    def evaluate(self, input_value):
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1,
            "evidence_digest": content_digest(input_value["value"]),
        }


def _agent(tmp_path):
    journal = AutonomousConnectorReceiptJournal(tmp_path / "workflow-receipts.jsonl")
    agent = AutonomousAgent(object(), LLMRuntime())
    registrations = agent.register_builtin_domain_connectors(receipt_store=journal)
    return agent, registrations, journal


def _request_for_stage(context):
    operation = AutonomousConnectorOperationRegistry().for_domain(context.blueprint.spec.domain)[0]
    return {
        "subject_digest": content_digest({"domain": context.blueprint.spec.domain}),
        **{field: {"fixture": context.stage.id} for field in _RECOMMENDED_FIELDS[operation.operation_id]},
    }


def test_connector_workflow_executes_every_domain_without_model_credentials(tmp_path) -> None:
    agent, registrations, journal = _agent(tmp_path)
    assert len(registrations) == len(AUTONOMOUS_DOMAINS)
    assert agent.connector_catalogue()["connector_count"] == len(AUTONOMOUS_DOMAINS)
    total_stages = 0

    for domain in AUTONOMOUS_DOMAINS:
        blueprint = agent.prepare(task=f"offline fixture for {domain}", domain=domain)
        result = agent.run_connector_workflow(
            blueprint=blueprint,
            approved=True,
            request_for_stage=_request_for_stage,
        )
        total_stages += len(blueprint.workflow.stages)
        assert result.status == "completed", domain
        assert result.next_stage_ids == (), domain
        assert len(result.stage_results) == len(blueprint.workflow.stages), domain
        assert all(stage.execution_status == "completed" for stage in result.stage_results), domain
        assert all(stage.declared_status == "completed" for stage in result.stage_results), domain
        assert all(stage.result is None for stage in result.stage_results), domain
        assert len(result.checkpoint.completed_stage_ids) == len(blueprint.workflow.stages), domain
        serialized = json.dumps(result.to_dict())
        assert "offline fixture" not in serialized
        assert "fixture" not in serialized
        assert "field_digests" not in serialized
        assert "subject_digest" not in serialized

    assert journal.verify_integrity()["entries"] == total_stages


def test_connector_workflow_requires_approval_and_preserves_partial_evidence(tmp_path) -> None:
    agent, _registrations, journal = _agent(tmp_path)
    blueprint = agent.prepare(task="approval-bound offline coding review", domain="coding")

    refused = agent.run_connector_workflow(
        blueprint=blueprint,
        approved=False,
        request_for_stage=_request_for_stage,
    )
    assert refused.status == "approval_required"
    assert refused.stage_results[0].execution_status == "approval_required"
    assert refused.stage_results[0].result is None
    assert refused.checkpoint.completed_stage_ids == ()

    partial = agent.run_connector_workflow(
        blueprint=blueprint,
        run_id="partial-coding-workflow",
        approved=True,
        request_for_stage=lambda _context: {},
    )
    assert partial.status == "stage_proposed"
    assert partial.stage_results[0].execution_status == "completed"
    assert partial.stage_results[0].declared_status == "proposed"
    assert partial.stage_results[0].uncertainty
    assert partial.checkpoint.stages[0]["status"] == "proposed"
    retried = agent.run_connector_workflow(
        blueprint=blueprint,
        checkpoint=partial.checkpoint,
        approved=True,
        retry_blocked=True,
        max_stage_calls=1,
        request_for_stage=_request_for_stage,
    )
    assert retried.status == "paused"
    assert retried.stage_results[0].execution_status == "completed"
    assert retried.stage_results[0].declared_status == "completed"
    assert retried.stage_results[0].attempt == 2
    assert journal.verify_integrity()["entries"] == 3


def test_connector_workflow_callback_flows_through_agent_facade(tmp_path) -> None:
    agent, _registrations, _journal = _agent(tmp_path)
    blueprint = agent.prepare(task="callback-bound offline coding review", domain="coding")
    events: list[dict[str, object]] = []
    result = agent.run_connector_workflow(
        blueprint=blueprint,
        approved=True,
        request_for_stage=_request_for_stage,
        trace_event_callback=lambda **event: events.append(event),
    )
    assert result.status == "completed"
    assert [event["phase"] for event in events].count("connector_started") == len(result.stage_results)
    assert [event["phase"] for event in events].count("connector_finished") == len(result.stage_results)


def test_connector_workflow_evidence_binding_requires_explicit_acceptance_across_domains(tmp_path) -> None:
    agent, _registrations, journal = _agent(tmp_path)
    total_stages = 0
    for domain in AUTONOMOUS_DOMAINS:
        blueprint = agent.prepare(task=f"evaluated offline fixture for {domain}", domain=domain)
        evidence_journal = InMemoryAutonomousEvidenceRuntimeJournal()
        evidence_runtime = agent.evidence_runtime((domain,), journal=evidence_journal)
        result = agent.run_connector_workflow(
            blueprint=blueprint,
            approved=True,
            request_for_stage=_request_for_stage,
            evidence_runtime=evidence_runtime,
            evidence_projector=_EvidenceProjector(),
            evidence_evaluator=_AcceptingEvaluator(),
        )
        total_stages += len(blueprint.workflow.stages)
        assert result.status == "completed", domain
        assert all(stage.execution_status == "completed" and stage.declared_status == "completed" for stage in result.stage_results), domain
        assert all("evidence_runtime" in stage.structured for stage in result.stage_results), domain
        assert len(evidence_journal.records()) == sum(len(stage.evidence_outputs) for stage in blueprint.workflow.stages), domain
    assert journal.verify_integrity()["entries"] == total_stages


def test_connector_workflow_replay_requires_digest_verified_rehydration(tmp_path) -> None:
    agent, registrations, journal = _agent(tmp_path)
    coding = next(item for item in registrations if item.manifest.domains == ("coding",))
    stored_payloads = []
    calls = []
    original = coding.executor

    def counted(manifest, request):
        calls.append(request["operation_id"])
        observation = original(manifest, request)
        stored_payloads.append(observation.value)
        return observation

    agent.connector_registry.register(
        AutonomousConnectorRegistration(coding.manifest, counted, approval_required=True),
        replace=True,
    )
    blueprint = agent.prepare(task="restart-safe offline coding review", domain="coding")
    first = agent.run_connector_workflow(
        blueprint=blueprint,
        run_id="replay-coding-workflow",
        approved=True,
        max_stage_calls=1,
        request_for_stage=_request_for_stage,
    )
    assert first.status == "paused"
    assert first.stage_results[0].execution_status == "completed"
    assert calls == ["coding.repository_change_analysis"]

    empty = AutonomousWorkflowCheckpoint(
        run_id=first.checkpoint.run_id,
        task_digest=first.checkpoint.task_digest,
        workflow_id=first.checkpoint.workflow_id,
        workflow_digest=first.checkpoint.workflow_digest,
    )
    missing = agent.run_connector_workflow(
        blueprint=blueprint,
        checkpoint=empty,
        approved=True,
        max_stage_calls=1,
        request_for_stage=_request_for_stage,
    )
    assert missing.status == "paused"
    assert missing.stage_results[0].execution_status == "paused"
    assert missing.stage_results[0].uncertainty == ("connector_payload_rehydration_required",)
    assert calls == ["coding.repository_change_analysis"]

    restored = agent.run_connector_workflow(
        blueprint=blueprint,
        checkpoint=empty,
        approved=True,
        max_stage_calls=1,
        request_for_stage=_request_for_stage,
        rehydrate_payload=lambda _receipt: stored_payloads[0],
    )
    assert restored.status == "paused"
    assert restored.stage_results[0].execution_status == "completed"
    assert restored.stage_results[0].declared_status == "completed"
    assert "caller-rehydrated" in " ".join(restored.stage_results[0].uncertainty)
    assert calls == ["coding.repository_change_analysis"]
    assert journal.verify_integrity()["entries"] == 1


def test_connector_workflow_rejects_changed_blueprint_before_dispatch(tmp_path) -> None:
    agent, _registrations, journal = _agent(tmp_path)
    first_blueprint = agent.prepare(task="first offline task", domain="science")
    first = agent.run_connector_workflow(
        blueprint=first_blueprint,
        approved=True,
        max_stage_calls=1,
        request_for_stage=_request_for_stage,
    )
    changed_blueprint = agent.prepare(task="different offline task", domain="science")
    try:
        agent.run_connector_workflow(
            blueprint=changed_blueprint,
            checkpoint=first.checkpoint,
            approved=True,
            request_for_stage=_request_for_stage,
        )
    except Exception as error:
        assert "checkpoint task" in str(error)
    else:
        raise AssertionError("changed blueprint was accepted by connector workflow resume")
    assert journal.verify_integrity()["entries"] == 1
