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
    LLMRuntime,
    builtin_autonomous_connector_registration,
    content_digest,
    register_builtin_autonomous_connectors,
)
from prism_sdk.errors import ArgumentError


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
