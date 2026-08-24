from __future__ import annotations

import json

import pytest

from test_autonomy import _Workspace

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousEvidencePlan,
    AutonomousEvidenceRequirement,
    AutonomousEvidenceRuntime,
    AutonomousLLMEvidenceAdapter,
    AutonomousLLMEvidenceAdapterHealthPersistenceCoordinator,
    AutonomousLLMEvidenceAdapterRegistry,
    AutonomousLLMEvidenceAdapterSelector,
    AutonomousLLMEvidenceAdapterSelectionPlan,
    AutonomousLLMEvidenceFailoverPolicy,
    InMemoryAutonomousLLMEvidenceAdapterHealthStore,
    JsonAutonomousLLMEvidenceAdapterHealthPersistence,
    LLMRuntime,
    ProviderError,
    TransactionalJsonAutonomousLLMEvidenceAdapterHealthPersistence,
    content_digest,
    create_autonomous_llm_evidence_adapter,
    create_autonomous_llm_evidence_adapter_failover_acquirer,
)
from prism_sdk.errors import ArgumentError


class _AcceptAll:
    evaluator_id = "orchestration-fixture-evaluator"
    evaluator_version = "v1"

    def evaluate(self, _input: object) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


def _plan(domain: str) -> tuple[AutonomousEvidencePlan, AutonomousEvidenceRequirement]:
    workflow_digest = content_digest({"workflow": domain, "version": 1})
    requirement = AutonomousEvidenceRequirement(
        requirement_id=f"{domain}:answer:answer",
        domain=domain,
        workflow_id=f"{domain}:answer",
        workflow_digest=workflow_digest,
        stage_id="answer",
        label="answer",
        objective=f"Produce a bounded answer for {domain}.",
        required_capabilities=("llm_evidence",),
        evaluator_signals=("grounded",),
    )
    return (
        AutonomousEvidencePlan(
            domains=(domain,),
            workflow_ids=(requirement.workflow_id,),
            workflow_digests=(workflow_digest,),
            requirements=(requirement,),
            missing_requirement_ids=(requirement.requirement_id,),
            coverage_status="not_evaluated",
        ),
        requirement,
    )


def _request(requirement: AutonomousEvidenceRequirement) -> dict[str, object]:
    return {
        "requirement_id": requirement.requirement_id,
        "source_id": "orchestration-fixture",
        "request_id": f"request-{requirement.domain}",
        "metadata": {"fixture": "offline", "domain": requirement.domain},
    }


def _adapter(runtime: LLMRuntime, domain: str, adapter_id: str, provider: str, model: str) -> AutonomousLLMEvidenceAdapter:
    return create_autonomous_llm_evidence_adapter(
        adapter_id=adapter_id,
        version="v1",
        domain=domain,
        provider=provider,
        runtime=runtime,
        capabilities=("llm_evidence",),
        model=model,
        prompt_for_context=lambda context: [
            {"role": "user", "content": context["requirement"].objective}  # type: ignore[index]
        ],
        project=lambda value, context: [
            {
                "label": context["requirement"].label,  # type: ignore[index]
                "kind": "fact",
                "status": "observed",
                "value_digest": content_digest(value),
            }
        ],
        require_json=True,
    )


def _registry(runtime: LLMRuntime) -> AutonomousLLMEvidenceAdapterRegistry:
    return AutonomousLLMEvidenceAdapterRegistry(
        [
            adapter
            for domain in AUTONOMOUS_DOMAINS
            for adapter in (
                _adapter(runtime, domain, f"a-primary-{domain}", "orchestration-primary", "fixture-primary"),
                _adapter(runtime, domain, f"b-secondary-{domain}", "orchestration-secondary", "fixture-secondary"),
            )
        ]
    )


def test_selection_and_failover_execute_all_domains_and_record_only_metadata() -> None:
    runtime = LLMRuntime()
    primary_calls = 0
    secondary_calls = 0

    def primary(_request: object) -> object:
        nonlocal primary_calls
        primary_calls += 1
        raise ProviderError("transient fixture outage", retryable=True)

    def secondary(request: object) -> dict[str, object]:
        nonlocal secondary_calls
        secondary_calls += 1
        return {"text": json.dumps({"answer": request.model})}  # type: ignore[attr-defined]

    runtime.register_in_memory_provider("orchestration-primary", primary)
    runtime.register_in_memory_provider("orchestration-secondary", secondary)
    registry = _registry(runtime)
    selector = AutonomousLLMEvidenceAdapterSelector(registry)
    plan = selector.select_for_domains(AUTONOMOUS_DOMAINS, capability="llm_evidence")
    assert plan.complete is True
    assert all(row.adapter_id and row.adapter_id.startswith("a-primary-") for row in plan.rows)

    events: list[dict[str, object]] = []
    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    failover = create_autonomous_llm_evidence_adapter_failover_acquirer(
        registry,
        plan,
        policy=AutonomousLLMEvidenceFailoverPolicy(max_failovers=1),
        health_store=health,
        observe_failover=lambda event: events.append(event.to_dict()),
    )

    for domain in AUTONOMOUS_DOMAINS:
        evidence_plan, requirement = _plan(domain)
        result = AutonomousEvidenceRuntime(evidence_plan).execute(
            [_request(requirement)],
            acquirer=failover,
            projector=failover,
            evaluator=_AcceptAll(),
        )
        assert result.status == "completed", domain
        assert result.receipts[0].status == "observed", domain
        assert result.assessments[0].verdict == "accepted", domain
        failover.record_evaluation(
            {
                "plan_digest": evidence_plan.plan_digest,
                "requirement": requirement,
                "request": _request(requirement),
            },
            status="accepted",
            evaluator_reward=1.0,
            evaluator_passed=True,
            evaluator_id=_AcceptAll.evaluator_id,
            evaluator_version=_AcceptAll.evaluator_version,
            evidence_digest=result.receipts[0].receipt_digest,
        )

    # The provider runtime opens its circuit after three failed transports; later domains still
    # fail over, but do not re-enter the transport boundary.
    assert 1 <= primary_calls <= len(AUTONOMOUS_DOMAINS)
    assert secondary_calls == len(AUTONOMOUS_DOMAINS)
    assert len(events) == len(AUTONOMOUS_DOMAINS) * 2
    assert all(event["status"] in {"fallback_started", "candidate_succeeded"} for event in events)
    serialized = json.dumps(health.snapshot())
    assert "transient fixture outage" not in serialized
    assert "fixture-primary" not in serialized
    assert "private_payload" not in serialized


def test_health_signals_promote_successful_adapters_and_reject_tampered_selection() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime)
    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    for domain in AUTONOMOUS_DOMAINS:
        primary = registry.manifest_for(domain, f"a-primary-{domain}")
        secondary = registry.manifest_for(domain, f"b-secondary-{domain}")
        for _ in range(3):
            health.record_acquisition(
                adapter_id=primary.adapter_id,
                manifest_digest=primary.manifest_digest,
                domain=domain,
                outcome="failure",
                status="failed",
                latency_ms=25,
                failure_class="provider_retryable",
            )
            health.record_acquisition(
                adapter_id=secondary.adapter_id,
                manifest_digest=secondary.manifest_digest,
                domain=domain,
                outcome="success",
                status="observed",
                latency_ms=5,
            )
        health.record_evaluation(
            adapter_id=secondary.adapter_id,
            manifest_digest=secondary.manifest_digest,
            domain=domain,
            status="accepted",
            evaluator_reward=1.0,
            evaluator_passed=True,
        )

    signals = health.selection_signals(
        manifest_digests={manifest.adapter_id: manifest.manifest_digest for manifest in registry.manifests()}
    )
    assert signals[f"a-primary-{AUTONOMOUS_DOMAINS[0]}"]["eligible"] is False
    adaptive = AutonomousLLMEvidenceAdapterSelector(registry).select_adaptive_for_domains(
        AUTONOMOUS_DOMAINS,
        signals,
        capability="llm_evidence",
    )
    assert all(row.adapter_id and row.adapter_id.startswith("b-secondary-") for row in adaptive.rows)
    assert adaptive.signal_digest is not None

    changed = _adapter(runtime, AUTONOMOUS_DOMAINS[0], f"a-primary-{AUTONOMOUS_DOMAINS[0]}", "orchestration-primary", "fixture-changed")
    registry.register(changed, replace=True)
    with pytest.raises(ArgumentError, match="registry digest is stale"):
        registry.verify_selection(adaptive)


def test_health_snapshot_json_round_trip_cas_and_chain_tamper_refusal() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime)
    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    manifest = registry.manifest_for("science", "b-secondary-science")
    health.record_acquisition(
        adapter_id=manifest.adapter_id,
        manifest_digest=manifest.manifest_digest,
        domain="science",
        outcome="success",
        status="observed",
        latency_ms=3,
    )
    snapshot = health.snapshot()

    class TextStore:
        def __init__(self) -> None:
            self.value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if observed != expected:
                return False
            self.value = value
            return True

    store = TextStore()
    persistence = JsonAutonomousLLMEvidenceAdapterHealthPersistence(store)
    persistence.write(snapshot)
    assert persistence.read() == snapshot

    transactional = TransactionalJsonAutonomousLLMEvidenceAdapterHealthPersistence(store)
    assert transactional.write_if_unchanged(snapshot["snapshot_digest"], snapshot) is True
    assert transactional.write_if_unchanged(None, snapshot) is False

    restored = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    coordinator = AutonomousLLMEvidenceAdapterHealthPersistenceCoordinator(restored, persistence)
    assert coordinator.restore()["events"] == 1
    assert restored.verify_integrity()["verified"] is True

    tampered = json.loads(json.dumps(snapshot))
    tampered["events"][0]["observation"]["status"] = "tampered"
    with pytest.raises(ArgumentError):
        InMemoryAutonomousLLMEvidenceAdapterHealthStore().restore(tampered)


def test_failover_does_not_retry_non_retryable_prompt_or_credential_failures() -> None:
    runtime = LLMRuntime()
    primary_calls = 0
    secondary_calls = 0

    def primary_prompt(_context: object) -> list[dict[str, str]]:
        nonlocal primary_calls
        primary_calls += 1
        raise ArgumentError("reviewed prompt is malformed")

    def secondary(request: object) -> dict[str, object]:
        nonlocal secondary_calls
        secondary_calls += 1
        return {"text": json.dumps({"answer": request.model})}  # type: ignore[attr-defined]

    runtime.register_in_memory_provider("orchestration-primary", lambda _request: {"text": "unused"})
    runtime.register_in_memory_provider("orchestration-secondary", secondary)
    domain = "coding"
    primary = create_autonomous_llm_evidence_adapter(
        adapter_id="a-primary-coding",
        version="v1",
        domain=domain,
        provider="orchestration-primary",
        runtime=runtime,
        capabilities=("llm_evidence",),
        model="fixture-primary",
        prompt_for_context=primary_prompt,
        require_json=True,
    )
    secondary_adapter = _adapter(runtime, domain, "b-secondary-coding", "orchestration-secondary", "fixture-secondary")
    registry = AutonomousLLMEvidenceAdapterRegistry((primary, secondary_adapter))
    plan = AutonomousLLMEvidenceAdapterSelector(registry).select_for_domains((domain,), capability="llm_evidence")
    failover = create_autonomous_llm_evidence_adapter_failover_acquirer(
        registry,
        plan,
        policy=AutonomousLLMEvidenceFailoverPolicy(max_failovers=1),
    )
    evidence_plan, requirement = _plan(domain)
    result = AutonomousEvidenceRuntime(evidence_plan).execute([_request(requirement)], acquirer=failover)
    assert result.status == "failed"
    assert result.receipts[0].error_class == "ArgumentError"
    assert primary_calls == 1
    assert secondary_calls == 0


def test_failover_adapter_composes_with_autonomous_agent_evidence_entrypoint() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "orchestration-secondary",
        lambda request: {"text": json.dumps({"answer": request.model})},  # type: ignore[attr-defined]
    )
    domain = "coding"
    adapter = _adapter(runtime, domain, "coding-adapter", "orchestration-secondary", "fixture-coding")
    registry = AutonomousLLMEvidenceAdapterRegistry((adapter,))
    selection = AutonomousLLMEvidenceAdapterSelector(registry).select_for_domains(
        (domain,), capability="llm_evidence"
    )
    failover = create_autonomous_llm_evidence_adapter_failover_acquirer(registry, selection)
    agent = AutonomousAgent(_Workspace(), runtime)
    plan = agent.evidence_plan((domain,))
    result = agent.acquire_evidence(
        (domain,),
        [
            {
                "requirement_id": requirement.requirement_id,
                "source_id": "agent-entrypoint-fixture",
                "request_id": f"agent-entrypoint-request-{index}",
                "metadata": {"fixture": "offline", "index": index},
            }
            for index, requirement in enumerate(plan.requirements)
        ],
        acquirer=failover,
        projector=failover,
        evaluator=_AcceptAll(),
    )
    assert result.status == "completed"
    assert result.receipts[0].status == "observed"
    assert result.assessments[0].verdict == "accepted"
