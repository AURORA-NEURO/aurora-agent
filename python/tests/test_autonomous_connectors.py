from __future__ import annotations

import json
from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from types import SimpleNamespace
import threading

import pytest

from prism_sdk import (
    AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA,
    AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA,
    AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
    AutonomousAgent,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorObservation,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorRegistration,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
    AutonomousRunTraceSession,
    InMemoryAutonomousRunTraceStore,
    ApiClient,
    DomainEvidenceSourceExecutionRequest,
    DomainEvidenceSourcePlanRequest,
    DomainEvidenceProviderConnectorManifest,
    LLMRuntime,
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
    selection = registry.select_for_domains(AUTONOMOUS_DOMAINS, capability="evidence_read")
    assert selection.complete is True
    assert tuple(row.domain for row in selection.rows) == tuple(AUTONOMOUS_DOMAINS)
    assert all(row.connector_id == f"connector-{domain}" for row, domain in zip(selection.rows, AUTONOMOUS_DOMAINS))
    adaptive = registry.select_adaptive_for_domains(
        AUTONOMOUS_DOMAINS,
        capability="evidence_read",
        selection_signals={
            f"connector-{domain}": {"health": 0.75, "success_rate": 0.8, "evaluator_reward": 0.2}
            for domain in AUTONOMOUS_DOMAINS
        },
    )
    assert adaptive.complete is True
    assert adaptive.signal_digest is not None

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


def test_connector_dispatch_trace_bridge_covers_every_builtin_domain() -> None:
    registry = AutonomousConnectorRegistry()
    for domain in AUTONOMOUS_DOMAINS:
        registry.register(_registration(domain, lambda _manifest, request: {"domain": request["query"]}, approval_required=False))
    runtime = AutonomousConnectorRuntime(registry)
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 1)
    session = AutonomousRunTraceSession(
        store,
        run_id="connector-trace",
        task_digest="a" * 64,
        domains=AUTONOMOUS_DOMAINS,
    )
    session.started()
    for domain in AUTONOMOUS_DOMAINS:
        runtime.dispatch(_request(domain), trace_event_callback=session.record)
    session.complete(status="completed")
    events = store.events({"run_id": "connector-trace"})
    assert sum(event.phase == "connector_started" for event in events) == len(AUTONOMOUS_DOMAINS)
    assert sum(event.phase == "connector_finished" for event in events) == len(AUTONOMOUS_DOMAINS)
    assert all(event.status == "completed" for event in events if event.phase == "connector_finished")
    assert session.summary().domains == tuple(sorted(AUTONOMOUS_DOMAINS))


def test_connector_dispatch_trace_bridge_covers_replay_and_inflight_waiters() -> None:
    entered = threading.Event()
    release = threading.Event()
    events: list[dict[str, object]] = []

    def execute(_manifest, _request):
        entered.set()
        assert release.wait(timeout=5)
        return {"ok": True}

    registry = AutonomousConnectorRegistry([_registration("coding", execute, approval_required=False)])
    runtime = AutonomousConnectorRuntime(registry)
    request = _request("coding")

    with ThreadPoolExecutor(max_workers=2) as pool:
        callback = lambda **event: events.append(event)
        first = pool.submit(runtime.dispatch, request, trace_event_callback=callback)
        assert entered.wait(timeout=5)
        second = pool.submit(runtime.dispatch, request, trace_event_callback=callback)
        release.set()
        first_result = first.result(timeout=5)
        second_result = second.result(timeout=5)

    assert {first_result.replay, second_result.replay} == {"fresh", "replayed"}
    assert [event["phase"] for event in events].count("connector_started") == 2
    assert [event["phase"] for event in events].count("connector_finished") == 2


def test_connector_selection_plan_is_deterministic_reviewable_and_bound_to_dispatch() -> None:
    registry = AutonomousConnectorRegistry([_registration("coding", lambda _manifest, _request: {"ok": True})])
    alternative_manifest = DomainEvidenceProviderConnectorManifest(
        connector_id="connector-coding-z",
        version="v1",
        provider="caller-managed-secondary",
        connector_kind="provider_api",
        domains=("coding",),
        capabilities=("evidence_read",),
    )
    registry.register(AutonomousConnectorRegistration(alternative_manifest, lambda _manifest, _request: {"secondary": True}))
    plan = registry.select_for_domains(("coding",), capability="evidence_read")

    assert plan.complete is True
    assert plan.rows[0].connector_id == "connector-coding"
    assert plan.rows[0].candidate_ids == ("connector-coding", "connector-coding-z")
    assert plan.rows[0].reason == "lexicographic_connector_id"
    assert plan.to_dict()["plan_digest"] == plan.plan_digest
    assert plan.to_dict()["schema"].endswith("selection-plan/0.1")
    restored = AutonomousConnectorSelectionPlan.from_mapping(plan.to_dict())
    assert restored == plan
    assert registry.plan_for_domains(("coding",), capability="evidence_read")["selection_plan_digest"] == plan.plan_digest

    request = replace(_request("coding"), selection_plan_digest=plan.plan_digest)
    result = AutonomousConnectorRuntime(registry).dispatch_from_plan(plan, request)
    assert result.receipt.request_digest == request.request_digest
    assert result.value == {"ok": True}

    with pytest.raises(ArgumentError, match="not bound"):
        AutonomousConnectorRuntime(registry).dispatch_from_plan(plan, _request("coding"))

    tampered = plan.to_dict()
    tampered["rows"][0]["reason"] = "tampered"
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousConnectorSelectionPlan.from_mapping(tampered)

    replacement_manifest = DomainEvidenceProviderConnectorManifest(
        connector_id="connector-coding",
        version="v2",
        provider="caller-managed",
        connector_kind="provider_api",
        domains=("coding",),
        capabilities=("evidence_read",),
    )
    registry.register(
        AutonomousConnectorRegistration(replacement_manifest, lambda _manifest, _request: {"v2": True}),
        replace=True,
    )
    with pytest.raises(ArgumentError, match="stale"):
        plan.verify(registry)


def test_connector_adaptive_selection_uses_explicit_evidence_and_deterministic_ties() -> None:
    primary = _registration("coding", lambda _manifest, _request: {"selected": "primary"})
    secondary_manifest = DomainEvidenceProviderConnectorManifest(
        connector_id="connector-coding-z",
        version="v1",
        provider="caller-managed-secondary",
        connector_kind="provider_api",
        domains=("coding",),
        capabilities=("evidence_read",),
    )
    secondary = AutonomousConnectorRegistration(
        secondary_manifest,
        lambda _manifest, _request: {"selected": "secondary"},
    )
    registry = AutonomousConnectorRegistry([primary, secondary])
    signals = {
        "connector-coding": {
            "health": 0.2,
            "success_rate": 0.2,
            "evaluator_reward": -1.0,
            "latency_ms": 10_000,
            "cost_per_million_tokens": 1_000,
        },
        "connector-coding-z": {
            "health": 0.9,
            "success_rate": 0.95,
            "evaluator_reward": 0.8,
            "latency_ms": 10,
            "cost_per_million_tokens": 1,
        },
    }
    plan = registry.select_adaptive_for_domains(
        ("coding",),
        capability="evidence_read",
        selection_signals=signals,
    )

    row = plan.rows[0]
    assert plan.strategy == "weighted_evidence"
    assert plan.signal_digest is not None
    assert row.connector_id == "connector-coding-z"
    assert row.candidate_scores[1] > row.candidate_scores[0]
    assert row.candidate_eligible == (True, True)
    assert AutonomousConnectorSelectionPlan.from_mapping(plan.to_dict()) == plan
    assert plan.verify(registry) is plan

    request = AutonomousConnectorDispatchRequest(
        dispatch_id="adaptive-dispatch",
        execution_id="adaptive-execution",
        call_id="adaptive-call",
        connector_id="connector-coding-z",
        domains=("coding",),
        capability="evidence_read",
        request={"query": "adaptive"},
        selection_plan_digest=plan.plan_digest,
        approved=True,
    )
    result = AutonomousConnectorRuntime(registry).dispatch_from_plan(plan, request)
    assert result.value == {"selected": "secondary"}

    ineligible_plan = registry.select_adaptive_for_domains(
        ("coding",),
        capability="evidence_read",
        selection_signals={
            "connector-coding": {"eligible": False, "health": 1.0},
            "connector-coding-z": {"eligible": True, "health": 0.1},
        },
    )
    assert ineligible_plan.rows[0].connector_id == "connector-coding-z"
    assert ineligible_plan.rows[0].candidate_eligible == (False, True)

    with pytest.raises(ArgumentError, match="credential"):
        registry.select_adaptive_for_domains(
            ("coding",),
            capability="evidence_read",
            selection_signals={"connector-coding": {"api_key": "must-not-enter"}},
        )
    with pytest.raises(ArgumentError, match="between"):
        registry.select_adaptive_for_domains(
            ("coding",),
            capability="evidence_read",
            selection_signals={"connector-coding": {"evaluator_reward": 2}},
        )


def test_connector_selection_plan_reports_missing_domains_without_authorizing_dispatch() -> None:
    registry = AutonomousConnectorRegistry([_registration("coding", lambda _manifest, _request: {"ok": True})])
    plan = registry.select_for_domains(("coding", "science"), capability="evidence_read")

    assert plan.complete is False
    assert plan.rows[0].status == "selected"
    assert plan.rows[1].status == "missing"
    request = replace(
        _request("coding"),
        domains=("science",),
        selection_plan_digest=plan.plan_digest,
    )
    with pytest.raises(ArgumentError, match="does not select"):
        AutonomousConnectorRuntime(registry).dispatch_from_plan(plan, request)


def test_autonomous_agent_exposes_connector_planning_and_plan_bound_dispatch() -> None:
    registry = AutonomousConnectorRegistry([_registration("coding", lambda _manifest, _request: {"agent": True})])
    runtime = AutonomousConnectorRuntime(registry)
    agent = AutonomousAgent(
        object(),
        LLMRuntime(),
        connector_registry=registry,
        connector_runtime=runtime,
    )

    assert agent.connector_catalogue()["connector_count"] == 1
    plan = agent.connector_selection_plan(("coding",), capability="evidence_read")
    request = replace(_request("coding"), selection_plan_digest=plan.plan_digest)
    result = agent.dispatch_connector(plan, request)
    assert result.value == {"agent": True}

    adaptive = agent.connector_selection_plan(
        ("coding",),
        capability="evidence_read",
        selection_signals={"connector-coding": {"health": 0.9, "evaluator_reward": 0.75}},
    )
    assert adaptive.strategy == "weighted_evidence"
    assert agent.connector_catalogue()["secret_material"] == "never_returned"


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


def test_connector_runtime_enforces_context_before_executor_across_all_domains() -> None:
    calls: list[str] = []
    registry = AutonomousConnectorRegistry(
        [_registration(domain, lambda _manifest, _request, domain=domain: calls.append(domain) or {"domain": domain}, approval_required=False) for domain in AUTONOMOUS_DOMAINS]
    )
    runtime = AutonomousConnectorRuntime(registry)
    ledger = AutonomousAuthorizationLedger(max_grants=4, max_events=64)
    grant = ledger.issue(
        grant_id="connector-runtime-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=AUTONOMOUS_DOMAINS,
        allowed_operations=("connector_dispatch",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=len(AUTONOMOUS_DOMAINS),
    )
    context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=grant.authorization_digest,
        domains=AUTONOMOUS_DOMAINS,
        clock=lambda: 1_200,
    )

    for domain in AUTONOMOUS_DOMAINS:
        result = runtime.dispatch(_request(domain), authorization_context=context)
        assert result.receipt.status == "observed", domain
    assert calls == list(AUTONOMOUS_DOMAINS)
    assert ledger.get(grant.grant_id).used_count == len(AUTONOMOUS_DOMAINS)  # type: ignore[union-attr]

    blocked_ledger = AutonomousAuthorizationLedger(max_grants=2, max_events=8)
    blocked = blocked_ledger.issue(
        grant_id="blocked-connector-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=("coding",),
        allowed_operations=("tool_execution",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=1,
    )
    blocked_context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(blocked_ledger),
        grant_id=blocked.grant_id,
        tenant_id=blocked.tenant_id,
        actor_id=blocked.actor_id,
        session_id=blocked.session_id,
        authorization_digest=blocked.authorization_digest,
        domains=("coding",),
        clock=lambda: 1_200,
    )
    with pytest.raises(ArgumentError, match="authorization was refused"):
        runtime.dispatch(replace(_request("coding"), dispatch_id="dispatch-blocked", execution_id="execution-blocked", call_id="call-blocked"), authorization_context=blocked_context)
    assert calls == list(AUTONOMOUS_DOMAINS)


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


def test_connector_receipt_journal_rehydrates_without_reinvoking_external_connector(tmp_path) -> None:
    calls: list[str] = []

    def execute(_manifest, request):
        calls.append(request["query"])
        return {"observed": request["query"]}

    path = tmp_path / "connector-receipts.jsonl"
    registry = AutonomousConnectorRegistry([_registration("coding", execute)])
    journal = AutonomousConnectorReceiptJournal(path)
    runtime = AutonomousConnectorRuntime(registry, receipt_store=journal)
    request = _request("coding")

    first = runtime.dispatch(request)
    replayed = runtime.dispatch(request)

    assert first.replay == "fresh"
    assert first.value == {"observed": "coding"}
    assert replayed.replay == "replayed"
    assert replayed.value is None
    assert calls == ["coding"]
    assert journal.verify_integrity()["entries"] == 1

    reopened = AutonomousConnectorReceiptJournal(path)

    def should_not_run(_manifest, _request):
        raise AssertionError("rehydrated connector dispatch must not invoke the executor")

    restarted = AutonomousConnectorRuntime(
        AutonomousConnectorRegistry([_registration("coding", should_not_run)]),
        receipt_store=reopened,
    )
    restored = restarted.dispatch(request)
    assert restored.replay == "replayed"
    assert restored.receipt == first.receipt
    assert restored.value is None

    retry = replace(
        request,
        dispatch_id="dispatch-coding-retry",
        execution_id="execution-coding-retry",
        call_id="call-coding-retry",
        attempt_id="attempt-coding-retry",
    )
    retried = AutonomousConnectorRuntime(registry, receipt_store=reopened).dispatch(retry)
    assert retried.replay == "fresh"
    assert retried.value == {"observed": "coding"}
    assert calls == ["coding", "coding"]
    assert reopened.verify_integrity()["entries"] == 2


def test_connector_receipt_journal_is_all_domain_bounded_and_tamper_evident(tmp_path) -> None:
    journal = AutonomousConnectorReceiptJournal(tmp_path / "all-domains.jsonl")
    registry = AutonomousConnectorRegistry(
        [
            _registration(
                domain,
                lambda _manifest, request, domain=domain: {"domain": domain, "query": request["query"]},
            )
            for domain in AUTONOMOUS_DOMAINS
        ]
    )
    runtime = AutonomousConnectorRuntime(registry, receipt_store=journal)
    results = [runtime.dispatch(_request(domain)) for domain in AUTONOMOUS_DOMAINS]

    assert len(journal.receipts(limit=len(AUTONOMOUS_DOMAINS))) == len(AUTONOMOUS_DOMAINS)
    assert journal.verify_integrity()["verified"] is True
    assert all(row.to_dict()["schema"] == AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA for row in journal.receipts(limit=256))
    assert all('"request":' not in json.dumps(row.to_dict()) for row in journal.receipts(limit=256))
    assert all('"value":' not in json.dumps(row.to_dict()) for row in journal.receipts(limit=256))

    duplicate = journal.append(results[0].receipt)
    assert duplicate.receipt == results[0].receipt
    with pytest.raises(ArgumentError, match="identity conflict"):
        journal.append(replace(results[0].receipt, status="error", failure_class="executor_error"))

    path = tmp_path / "all-domains.jsonl"
    original = path.read_text(encoding="utf-8")
    path.write_text(original.replace('"entry_digest":"', '"entry_digest":"0', 1), encoding="utf-8")
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousConnectorReceiptJournal(path)
