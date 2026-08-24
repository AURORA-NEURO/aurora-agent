from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousEvidencePlan,
    AutonomousEvidenceProviderContractRegistry,
    AutonomousEvidenceRequirement,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceSourceAdmissionError,
    AutonomousEvidenceSourceLedger,
    AutonomousEvidenceSourceLedgerPersistenceCoordinator,
    AutonomousEvidenceSourcePolicy,
    AutonomousLLMEvidenceAdapter,
    AutonomousLLMEvidenceAdapterRegistry,
    AutonomousLLMEvidenceAdapterSelector,
    AutonomousLLMEvidenceFailoverPolicy,
    JsonAutonomousEvidenceSourceLedgerPersistence,
    LLMRuntime,
    ProviderError,
    TransactionalJsonAutonomousEvidenceSourceLedgerPersistence,
    content_digest,
    create_autonomous_evidence_source_acquirer,
    create_autonomous_llm_evidence_adapter,
    create_autonomous_llm_evidence_adapter_failover_acquirer,
)
from prism_sdk.errors import ArgumentError


def _plan(domain: str) -> tuple[AutonomousEvidencePlan, AutonomousEvidenceRequirement]:
    workflow_digest = content_digest({"workflow": domain, "version": 1})
    requirement = AutonomousEvidenceRequirement(
        requirement_id=f"{domain}:answer:answer",
        domain=domain,
        workflow_id=f"{domain}:answer",
        workflow_digest=workflow_digest,
        stage_id="answer",
        label="answer",
        objective=f"Produce a bounded evidence answer for {domain}.",
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


def _adapter(runtime: LLMRuntime, domain: str, adapter_id: str, provider: str = "contract-fixture") -> AutonomousLLMEvidenceAdapter:
    return create_autonomous_llm_evidence_adapter(
        adapter_id=adapter_id,
        version="v1",
        domain=domain,
        provider=provider,
        runtime=runtime,
        capabilities=("llm_evidence",),
        model=f"fixture-{domain}",
        prompt_for_context=lambda context: [{"role": "user", "content": context["requirement"].objective}],  # type: ignore[index]
        project=lambda value, context: [{
            "label": context["requirement"].label,  # type: ignore[index]
            "kind": "fact",
            "status": "observed",
            "value_digest": content_digest(value),
        }],
        require_json=True,
    )


def _request(requirement: AutonomousEvidenceRequirement, *, request_id: str | None = None, metadata: dict[str, object] | None = None) -> dict[str, object]:
    return {
        "requirement_id": requirement.requirement_id,
        "source_id": f"source-{requirement.domain}",
        "source_digest": content_digest({"source": requirement.domain}),
        "request_id": request_id or f"request-{requirement.domain}",
        "metadata": {"operation": "lookup", "domain": requirement.domain, **(metadata or {})},
    }


def _context(domain: str, *, request_id: str | None = None, metadata: dict[str, object] | None = None) -> dict[str, object]:
    plan, requirement = _plan(domain)
    return {"plan_digest": plan.plan_digest, "requirement": requirement, "request": _request(requirement, request_id=request_id, metadata=metadata)}


def _contracts(runtime: LLMRuntime) -> tuple[AutonomousLLMEvidenceAdapterRegistry, AutonomousEvidenceProviderContractRegistry]:
    adapters = tuple(_adapter(runtime, domain, f"contract-{domain}") for domain in AUTONOMOUS_DOMAINS)
    registry = AutonomousLLMEvidenceAdapterRegistry(adapters)
    contracts = AutonomousEvidenceProviderContractRegistry(registry)
    for domain in AUTONOMOUS_DOMAINS:
        contracts.register_for_adapter(
            contract_id=f"contract-{domain}",
            version="v1",
            provider="contract-fixture",
            protocol="caller_defined",
            operations=("lookup",),
            domains=(domain,),
            capabilities=("llm_evidence",),
            source_kinds=("llm_structured",),
            auth_mode="none",
            freshness="realtime",
            pagination="none",
            adapter_id=f"contract-{domain}",
            required_metadata=("operation",),
            operation_metadata_key="operation",
        )
    return registry, contracts


def _describe_source(payload: dict[str, object]) -> dict[str, object]:
    context = payload["context"]
    request = context["request"]  # type: ignore[index]
    domain = context["requirement"].domain  # type: ignore[index]
    return {
        "source_id": request["source_id"],  # type: ignore[index]
        "source_digest": content_digest({"source": domain}),
        "authority": "provider_observed",
        "status": "observed",
        "observed_at_ms": payload["now_ms"],
        "citation_digest": content_digest({"citation": domain}),
        "limitations": (),
    }


def test_provider_contracts_and_source_receipts_cover_every_domain_without_raw_values() -> None:
    runtime = LLMRuntime()
    calls: list[object] = []

    def handler(request: object) -> dict[str, object]:
        calls.append(request)
        return {"text": json.dumps({"answer": f"grounded-{request.model}"})}  # type: ignore[attr-defined]

    runtime.register_in_memory_provider("contract-fixture", handler)
    adapter_registry, contract_registry = _contracts(runtime)
    assert all(row.state == "complete" for row in contract_registry.coverage())
    assert contract_registry.verify() is contract_registry
    projection = json.dumps(contract_registry.to_dict())
    assert "grounded-" not in projection
    assert "secret_material" in projection

    ledger = AutonomousEvidenceSourceLedger()
    for domain in AUTONOMOUS_DOMAINS:
        plan, requirement = _plan(domain)
        acquirer = create_autonomous_evidence_source_acquirer(
            contract_registry,
            adapter_id=f"contract-{domain}",
            domain=domain,
            policy=AutonomousEvidenceSourcePolicy(clock=lambda: 1.0),
            ledger=ledger,
            describe_source=_describe_source,
        )
        value = acquirer.acquire({"plan_digest": plan.plan_digest, "requirement": requirement, "request": _request(requirement)})
        assert value == {"answer": f"grounded-fixture-{domain}"}

    assert len(calls) == len(AUTONOMOUS_DOMAINS)
    assert len(ledger.records()) == len(AUTONOMOUS_DOMAINS)
    assert all(entry.receipt.decision == "accepted" for entry in ledger.records())
    serialized_ledger = json.dumps(ledger.snapshot())
    assert "grounded-" not in serialized_ledger
    assert "contract-fixture" in serialized_ledger
    assert adapter_registry.registry_digest == contract_registry.to_dict()["adapter_registry_digest"]


def test_contract_metadata_is_checked_before_provider_invocation_and_failover_preserves_it() -> None:
    runtime = LLMRuntime()
    calls: list[object] = []
    primary_calls = 0

    def handler(request: object) -> dict[str, object]:
        calls.append(request)
        return {"text": json.dumps({"answer": "secondary"})}

    def primary_handler(_request: object) -> object:
        nonlocal primary_calls
        primary_calls += 1
        raise ProviderError("primary fixture failure", retryable=False)

    runtime.register_in_memory_provider("contract-primary", primary_handler)
    runtime.register_in_memory_provider("contract-secondary", handler)
    primary = _adapter(runtime, "science", "primary-science", "contract-primary")
    secondary = _adapter(runtime, "science", "secondary-science", "contract-secondary")
    registry = AutonomousLLMEvidenceAdapterRegistry((primary, secondary))
    contracts = AutonomousEvidenceProviderContractRegistry(registry)
    for adapter, provider in ((primary, "contract-primary"), (secondary, "contract-secondary")):
        contracts.register_for_adapter(
            contract_id=f"{adapter.adapter_id}-contract",
            version="v1",
            provider=provider,
            protocol="caller_defined",
            operations=("lookup",),
            domains=("science",),
            capabilities=("llm_evidence",),
            source_kinds=("llm_structured",),
            auth_mode="none",
            freshness="realtime",
            pagination="none",
            adapter_id=adapter.adapter_id,
            required_metadata=("operation",),
            operation_metadata_key="operation",
        )
    missing_metadata = _context("science", metadata={"operation": None})
    with pytest.raises(ArgumentError, match="operation"):
        contracts.create_acquirer_for_adapter("secondary-science", "science").acquire(missing_metadata)
    assert calls == []

    selection = AutonomousLLMEvidenceAdapterSelector(registry).select_for_domains(("science",), capability="llm_evidence")
    failover = create_autonomous_llm_evidence_adapter_failover_acquirer(
        registry,
        selection,
        policy=AutonomousLLMEvidenceFailoverPolicy(max_failovers=1),
        provider_contracts=contracts,
    )
    context = _context("science")
    # The primary is selected first by score; its transport is deliberately non-retryable, so
    # this verifies that the contract boundary runs before the selected adapter invocation.
    with pytest.raises(ProviderError):
        failover.acquire(context)
    assert primary_calls == 1
    assert calls == []
    assert failover.to_dict()["provider_contracts_enabled"] is True
    assert failover.to_dict()["provider_contract_registry_digest"] == contracts.registry_digest


def test_source_admission_records_refusals_and_rejects_secret_values() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("contract-fixture", lambda _request: {"text": json.dumps({"answer": "ok"})})
    _adapter_registry, contracts = _contracts(runtime)
    ledger = AutonomousEvidenceSourceLedger()
    state = {"status": "stale", "observed_at_ms": 1}

    def describe(payload: dict[str, object]) -> dict[str, object]:
        context = payload["context"]
        domain = context["requirement"].domain  # type: ignore[index]
        return {
            "source_id": context["request"]["source_id"],  # type: ignore[index]
            "source_digest": content_digest({"source": domain}),
            "authority": "provider_observed",
            "status": state["status"],
            "observed_at_ms": state["observed_at_ms"],
            "citation_digest": content_digest({"citation": domain}),
            "limitations": (),
        }

    acquirer = create_autonomous_evidence_source_acquirer(
        contracts,
        adapter_id="contract-science",
        domain="science",
        policy=AutonomousEvidenceSourcePolicy(max_age_ms=10, clock=lambda: 1000.0),
        ledger=ledger,
        describe_source=describe,
    )
    with pytest.raises(AutonomousEvidenceSourceAdmissionError) as refused:
        acquirer.acquire(_context("science", request_id="stale-request"))
    assert refused.value.decision == "stale"
    assert ledger.records()[-1].receipt.decision == "stale"

    secret_runtime = LLMRuntime()
    secret_runtime.register_in_memory_provider("secret-contract-fixture", lambda _request: {"text": json.dumps({"token": "must-not-cross"})})
    secret_adapter = _adapter(secret_runtime, "science", "secret-science", "secret-contract-fixture")
    secret_registry = AutonomousLLMEvidenceAdapterRegistry((secret_adapter,))
    secret_contracts = AutonomousEvidenceProviderContractRegistry(secret_registry)
    secret_contracts.register_for_adapter(
        contract_id="secret-science-contract",
        version="v1",
        provider="secret-contract-fixture",
        protocol="caller_defined",
        operations=("lookup",),
        domains=("science",),
        capabilities=("llm_evidence",),
        source_kinds=("llm_structured",),
        auth_mode="none",
        freshness="realtime",
        pagination="none",
        adapter_id="secret-science",
        required_metadata=("operation",),
        operation_metadata_key="operation",
    )
    secret_acquirer = create_autonomous_evidence_source_acquirer(
        secret_contracts,
        adapter_id="secret-science",
        domain="science",
        policy=AutonomousEvidenceSourcePolicy(clock=lambda: 1000.0),
        ledger=ledger,
        describe_source=_describe_source,
    )
    with pytest.raises(ArgumentError, match="credential-shaped"):
        secret_acquirer.acquire(_context("science", request_id="secret-request"))
    assert len(ledger.records()) == 1


class _TextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value


class _TransactionalTextStore(_TextStore):
    def write_if_unchanged(self, expected: str | None, value: str) -> bool:
        current = None if self.value is None else json.loads(self.value)["ledger_digest"]
        if current != expected:
            return False
        self.value = value
        return True


def test_source_ledger_json_and_compare_and_swap_persistence_round_trip() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("contract-fixture", lambda _request: {"text": json.dumps({"answer": "ok"})})
    _adapter_registry, contracts = _contracts(runtime)
    ledger = AutonomousEvidenceSourceLedger()
    acquirer = create_autonomous_evidence_source_acquirer(
        contracts,
        adapter_id="contract-science",
        domain="science",
        policy=AutonomousEvidenceSourcePolicy(clock=lambda: 1000.0),
        ledger=ledger,
        describe_source=_describe_source,
    )
    acquirer.acquire(_context("science", request_id="persist-request"))
    snapshot = ledger.snapshot()

    store = _TextStore()
    persistence = JsonAutonomousEvidenceSourceLedgerPersistence(store)
    persistence.write(snapshot)
    restored = AutonomousEvidenceSourceLedger()
    coordinator = AutonomousEvidenceSourceLedgerPersistenceCoordinator(restored, persistence)
    assert coordinator.restore()["verified"] is True
    assert restored.snapshot() == snapshot
    assert len(persistence.records()) == 1

    transactional_store = _TransactionalTextStore()
    transactional = TransactionalJsonAutonomousEvidenceSourceLedgerPersistence(transactional_store)
    assert transactional.write_if_unchanged(None, snapshot) is True
    assert transactional.write_if_unchanged(None, snapshot) is False
    tx_ledger = AutonomousEvidenceSourceLedger(persistence=transactional)
    tx_ledger.append(ledger.records()[0].receipt)
    assert len(transactional.records()) == 1
