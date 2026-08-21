from __future__ import annotations

import json
from types import SimpleNamespace

import pytest

from prism_sdk import (
    AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
    AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorObservation,
    AutonomousConnectorRegistration,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    ApiClient,
    DomainEvidenceSourceExecutionRequest,
    DomainEvidenceSourcePlanRequest,
    DomainEvidenceProviderConnectorManifest,
    builtin_autonomous_domain_tool_profiles,
    content_digest,
    create_autonomous_api_source_connector_executor,
)
from prism_sdk.errors import ArgumentError


def _registration(domain: str, executor, *, approval_required: bool = True) -> AutonomousConnectorRegistration:
    manifest = DomainEvidenceProviderConnectorManifest(
        connector_id=f"connector-{domain}",
        version="v1",
        provider="caller-managed",
        connector_kind="provider_api",
        domains=(domain,),
        capabilities=("evidence_read",),
    )
    return AutonomousConnectorRegistration(manifest, executor, approval_required=approval_required)


def _request(domain: str, *, approved: bool = True, capability: str = "evidence_read") -> AutonomousConnectorDispatchRequest:
    return AutonomousConnectorDispatchRequest(
        dispatch_id=f"dispatch-{domain}",
        execution_id=f"execution-{domain}",
        call_id=f"call-{domain}",
        connector_id=f"connector-{domain}",
        domains=(domain,),
        capability=capability,
        request={"query": domain, "limit": 3},
        parent_digests=(content_digest({"parent": domain}),),
        attempt_id=f"attempt-{domain}",
        approved=approved,
    )


def test_connector_registry_plans_and_dispatches_every_builtin_domain() -> None:
    profiles = builtin_autonomous_domain_tool_profiles()
    observed: list[str] = []
    registry = AutonomousConnectorRegistry()
    for profile in profiles:
        domain = profile.domain

        def execute(manifest, request, domain=domain):
            observed.append(domain)
            return {"domain": domain, "query": request["query"], "count": 1}

        registry.register(_registration(domain, execute))

    plan = registry.plan_for_domains(AUTONOMOUS_DOMAINS, capability="evidence_read")
    assert plan["schema"] == AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA
    assert plan["plan_digest"] == content_digest({key: value for key, value in plan.items() if key != "plan_digest"})
    assert all(row["status"] == "selected" for row in plan["coverage"].values())

    published = []
    runtime = AutonomousConnectorRuntime(registry, receipt_sink=published.append)
    results = [runtime.dispatch(_request(domain)) for domain in AUTONOMOUS_DOMAINS]

    assert len(results) == len(AUTONOMOUS_DOMAINS)
    assert observed == list(AUTONOMOUS_DOMAINS)
    assert all(result.receipt.status == "observed" for result in results)
    assert all(result.value["count"] == 1 for result in results)
    assert published == [result.receipt for result in results]
    assert all(result.receipt.to_dict()["schema"] == "bioprism-python-autonomous-connector-receipt/0.1" for result in results)
    encoded = json.dumps(results[0].to_dict())
    assert '"request":' not in encoded
    assert '"value":' not in encoded


def test_connector_runtime_keeps_approval_scope_and_executor_errors_explicit() -> None:
    calls: list[str] = []

    def execute(_manifest, _request):
        calls.append("executed")
        raise RuntimeError("private provider response must not escape")

    registry = AutonomousConnectorRegistry([_registration("coding", execute)])
    runtime = AutonomousConnectorRuntime(registry)

    refused = runtime.dispatch(_request("coding", approved=False))
    assert refused.receipt.status == "refused"
    assert refused.receipt.failure_class == "approval_required"
    assert refused.value is None
    assert calls == []

    errored = runtime.dispatch(_request("coding"))
    assert errored.receipt.status == "error"
    assert errored.receipt.failure_class == "executor_error"
    assert errored.value is None
    assert "private provider response" not in json.dumps(errored.to_dict())

    out_of_scope = AutonomousConnectorDispatchRequest(
        dispatch_id="dispatch-scope",
        execution_id="execution-scope",
        call_id="call-scope",
        connector_id="connector-coding",
        domains=("data",),
        capability="evidence_read",
        request={"query": "data", "limit": 3},
        approved=True,
    )
    scoped = runtime.dispatch(out_of_scope)
    assert scoped.receipt.status == "refused"
    assert scoped.receipt.failure_class == "domain_scope"

    wrong_capability = runtime.dispatch(_request("coding", capability="evidence_write"))
    assert wrong_capability.receipt.failure_class == "capability_scope"


def test_connector_request_and_registration_reject_secrets_and_unsupported_domains() -> None:
    with pytest.raises(ArgumentError):
        AutonomousConnectorDispatchRequest(
            dispatch_id="secret-dispatch",
            execution_id="secret-execution",
            call_id="secret-call",
            connector_id="connector-coding",
            domains=("coding",),
            capability="evidence_read",
            request={"api_key": "must-not-enter"},
            approved=True,
        )

    with pytest.raises(ArgumentError, match="unsupported domain"):
        _registration("not-a-domain", lambda _manifest, _request: {})

    registry = AutonomousConnectorRegistry([_registration("coding", lambda _manifest, _request: {})])
    with pytest.raises(ArgumentError):
        registry.plan_for_domains(("not-a-domain",))


def test_connector_observation_preserves_partial_status_and_rejects_invalid_values() -> None:
    partial = AutonomousConnectorObservation(
        {"records": [{"id": "record-1"}]},
        status="partial",
        failure_class="source_partial",
    )
    assert partial.status == "partial"
    assert partial.value["records"][0]["id"] == "record-1"
    with pytest.raises(ArgumentError):
        AutonomousConnectorObservation({"token": "private"})
    with pytest.raises(ArgumentError):
        AutonomousConnectorObservation({}, status="not-a-status")


def test_connector_plan_is_review_only_and_does_not_dispatch() -> None:
    called = []
    registry = AutonomousConnectorRegistry(
        [_registration("evaluation", lambda _manifest, _request: called.append(True) or {"ok": True})]
    )
    plan = registry.plan_for_domains(("evaluation",))
    assert plan["execution"] == "planning_only;no_dispatch;no_authorization"
    assert called == []
    assert AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA in str(plan) or "dispatch" in str(plan)


def test_api_source_connector_binds_execution_to_the_returned_plan_digest(monkeypatch) -> None:
    client = ApiClient("https://prism.test")
    plan_calls: list[DomainEvidenceSourcePlanRequest] = []
    execution_calls: list[DomainEvidenceSourceExecutionRequest] = []
    plan_digest = content_digest({"plan": "science"})

    plan_payload = {
        "group_id": "group-science",
        "domains": ["science"],
        "subject_id": "subject-1",
        "connector_kind": "provider_api",
        "locator_kind": "opaque",
        "locator": "caller-source-reference",
        "retrieval_mode": "metadata_only",
        "source_tool": "caller-source",
        "parent_digests": [],
        "retrieval_policy": {
            "network": "caller_managed",
            "max_bytes": 4096,
            "timeout_ms": 1000,
            "cache": "content_addressed",
            "allowed_hosts": [],
        },
        "does_not_claim": ["not a truth claim"],
    }

    def plan(request):
        assert isinstance(request, DomainEvidenceSourcePlanRequest)
        plan_calls.append(request)
        return SimpleNamespace(plan_digest=plan_digest)

    def execute(request):
        assert isinstance(request, DomainEvidenceSourceExecutionRequest)
        execution_calls.append(request)
        return SimpleNamespace(
            outcome="observed",
            to_dict=lambda: {"workflow": "domain_evidence_source_execute", "response_digest": content_digest({"ok": True})},
        )

    monkeypatch.setattr(client, "domain_evidence_source_plan_tool", plan)
    monkeypatch.setattr(client, "domain_evidence_source_execute_tool", execute)
    monkeypatch.setattr(client, "tools", lambda: (_ for _ in ()).throw(AssertionError("discovery is forbidden")))

    registration = _registration("science", lambda _manifest, _request: {})
    executor = create_autonomous_api_source_connector_executor(client)
    registry = AutonomousConnectorRegistry(
        [AutonomousConnectorRegistration(registration.manifest, executor, approval_required=True)]
    )
    runtime = AutonomousConnectorRuntime(registry)
    request = AutonomousConnectorDispatchRequest(
        dispatch_id="source-dispatch",
        execution_id="source-execution",
        call_id="source-call",
        connector_id=registration.manifest.connector_id,
        domains=("science",),
        capability="evidence_read",
        request={
            "plan": plan_payload,
            "execution": {"source_tool": "caller-source", "request": {"query": "transient"}},
        },
        approved=True,
    )

    result = runtime.dispatch(request)

    assert result.receipt.status == "observed"
    assert result.value["workflow"] == "domain_evidence_source_execute"
    assert plan_calls[0].source_tool == "caller-source"
    assert execution_calls[0].source_plan_digest == plan_digest
    assert execution_calls[0].request == {"query": "transient"}
    assert request.request_digest in result.receipt.request_digest
    assert "transient" not in json.dumps(result.receipt.to_dict())


def test_api_source_connector_rejects_wrong_manifest_kind_without_network_discovery() -> None:
    client = ApiClient("https://prism.test")
    executor = create_autonomous_api_source_connector_executor(client)
    request = {
        "plan": {
            "group_id": "group",
            "domains": ["science"],
            "subject_id": "subject",
            "connector_kind": "literature",
            "locator_kind": "opaque",
            "locator": "reference",
            "retrieval_mode": "reference_only",
            "does_not_claim": ["not a truth claim"],
        },
        "execution": {},
    }
    manifest = DomainEvidenceProviderConnectorManifest(
        connector_id="science-provider-api",
        version="v1",
        provider="caller-managed",
        connector_kind="provider_api",
        domains=("science",),
        capabilities=("evidence_read",),
    )
    with pytest.raises(ArgumentError, match="kind"):
        executor(manifest, request)
