from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousEvidenceAdapterRegistry,
    AutonomousEvidenceExecutionController,
    AutonomousEvidenceExecutionResumableController,
    AutonomousEvidenceReadinessPolicy,
    AutonomousEvidenceProviderContractRegistry,
    AutonomousEvidenceRetryPolicy,
    CredentialStore,
    InMemoryAutonomousEvidenceExecutionCheckpointStore,
    InMemoryAutonomousEvidenceRuntimeJournal,
    JsonAutonomousEvidenceExecutionCheckpointPersistence,
    LLMRuntime,
    TransactionalJsonAutonomousEvidenceExecutionCheckpointPersistence,
    ArgumentError,
    canonical_json,
    content_digest,
    register_autonomous_evidence_adapters_for_all_domains,
    validate_autonomous_evidence_execution_checkpoint,
)


def _agent() -> AutonomousAgent:
    return AutonomousAgent(None, LLMRuntime(CredentialStore()))


def _registry(calls: list[int] | None = None, values: dict[str, object] | None = None) -> AutonomousEvidenceAdapterRegistry:
    observed_calls = calls if calls is not None else []
    observed_values = values if values is not None else {}
    registry = AutonomousEvidenceAdapterRegistry()

    def factory(domain: str) -> dict[str, object]:
        def acquire(context: dict[str, object]) -> dict[str, object]:
            observed_calls.append(1)
            requirement = context["requirement"]
            label = getattr(requirement, "label", requirement.get("label") if isinstance(requirement, dict) else "evidence")
            value = {"domain": domain, "label": label, "sequence": len(observed_calls)}
            observed_values[content_digest(value)] = value
            return value

        return {
            "adapter_id": f"fixture_{domain}",
            "version": "1",
            "capabilities": ("debugging", "evidence", "implementation", "review", "testing"),
            "source_kinds": ("fixture",),
            "acquire": acquire,
        }

    register_autonomous_evidence_adapters_for_all_domains(registry, factory)
    return registry


def _requests(plan, prefix: str = "request") -> list[dict[str, object]]:
    return [
        {
            "requirement_id": requirement.requirement_id,
            "source_id": f"{prefix}-{index}",
            "source_digest": "c" * 64,
            "request_id": f"{prefix}-id-{index}",
            "metadata": {"operation": "observe"},
        }
        for index, requirement in enumerate(plan.requirements)
    ]


class _Projector:
    def project(self, value: dict[str, object], context: dict[str, object]) -> list[dict[str, object]]:
        requirement = context["requirement"]
        label = getattr(requirement, "label", "evidence")
        return [{"label": label, "kind": "fact", "status": "observed", "value_digest": content_digest(value)}]


class _Evaluator:
    evaluator_id = "fixture-evaluator"
    evaluator_version = "1"

    def evaluate(self, _input: dict[str, object]) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
            "evidence_digest": "d" * 64,
        }


def _execute_options(journal=None, values=None) -> dict[str, object]:
    options: dict[str, object] = {
        "approve_source_dispatch": True,
        "projector": _Projector(),
        "evaluator": _Evaluator(),
        "sleep": lambda _delay: None,
    }
    if journal is not None:
        options["journal"] = journal
    if values is not None:
        options["rehydrate_value"] = lambda receipt: values.get(receipt["value_digest"])
    return options


def test_prepare_is_side_effect_free_and_execute_covers_all_domains() -> None:
    calls: list[int] = []
    registry = _registry(calls)
    agent = _agent()
    evidence_plan = agent.evidence_plan(AUTONOMOUS_DOMAIN_NAMES)
    controller = AutonomousEvidenceExecutionController(registry)
    execution_plan = controller.prepare(
        evidence_plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    assert execution_plan.status == "ready_for_review"
    assert execution_plan.readiness.degraded_count == len(AUTONOMOUS_DOMAIN_NAMES)
    assert calls == []
    with pytest.raises(ArgumentError, match="explicit approval"):
        controller.execute(execution_plan, evidence_plan, _requests(evidence_plan), projector=_Projector())
    assert calls == []

    result = controller.execute(execution_plan, evidence_plan, _requests(evidence_plan), **_execute_options())
    assert result.status == "completed"
    assert len(result.runtime.completed_requirement_ids) == len(evidence_plan.requirements)
    assert len(calls) == len(evidence_plan.requirements)
    assert result.to_dict()["result_digest"] == result.result_digest
    assert "sequence" not in json.dumps(result.to_dict())


def test_readiness_drift_requires_review_before_dispatch() -> None:
    registry = _registry()
    agent = _agent()
    plan = agent.evidence_plan(("coding",))
    controller = AutonomousEvidenceExecutionController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    # Add an observation after preparation.  The health snapshot digest changes, so the old
    # reviewed image cannot be silently reused.
    manifest = registry.resolve("coding")
    health = __import__("prism_sdk").InMemoryAutonomousEvidenceAdapterHealthStore()
    health.record_acquisition(adapter_id=manifest.adapter_id, manifest_digest=manifest.manifest_digest, domain="coding", outcome="success", status="success", latency_ms=1)
    drifted_controller = AutonomousEvidenceExecutionController(registry, health)
    with pytest.raises(ArgumentError, match="readiness changed"):
        drifted_controller.execute(execution_plan, plan, _requests(plan), **_execute_options())


def test_provider_contracts_bind_generic_adapters_and_source_gate_is_enforced() -> None:
    registry = _registry()
    contracts = AutonomousEvidenceProviderContractRegistry(registry)
    contract = contracts.register_for_adapter(
        contract_id="fixture-coding-contract",
        version="1",
        provider="caller_owned",
        protocol="caller_defined",
        operations=("observe",),
        domains=("coding",),
        capabilities=("debugging", "evidence", "implementation", "review", "testing"),
        source_kinds=("fixture",),
        auth_mode="caller_managed_credential",
        freshness="caller_declared",
        pagination="none",
        adapter_id=registry.resolve("coding").adapter_id,
        required_metadata=("operation",),
        operation_metadata_key="operation",
    )
    assert contract["contract_id"] == "fixture-coding-contract"
    plan = _agent().evidence_plan(("coding",))
    controller = AutonomousEvidenceExecutionController(registry)
    execution_plan = controller.prepare(
        plan,
        provider_contracts=contracts,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    result = controller.execute(execution_plan, plan, _requests(plan), provider_contracts=contracts, **_execute_options())
    assert result.status == "completed"

    with pytest.raises(ArgumentError, match="describe_source"):
        controller.prepare(
            plan,
            provider_contracts=contracts,
            source_boundary={"policy": object()},
            readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        )


def test_resumable_execution_gates_restart_and_replays_without_source_calls() -> None:
    calls: list[int] = []
    values: dict[str, object] = {}
    registry = _registry(calls, values)
    agent = _agent()
    plan = agent.evidence_plan(AUTONOMOUS_DOMAIN_NAMES)
    controller = AutonomousEvidenceExecutionController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    requests = _requests(plan, "resumable")
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    first = AutonomousEvidenceExecutionResumableController(controller, store, "all-domains-job")
    gated = first.run(execution_plan, plan, requests, journal=journal)
    assert gated.status == "approval_required"
    assert gated.checkpoint.completed_request_count == 0
    assert calls == []

    restarted = AutonomousEvidenceExecutionResumableController(controller, store, "all-domains-job")
    assert restarted.restore()["status"] == "restored"
    completed = restarted.run(execution_plan, plan, requests, **_execute_options(journal=journal))
    assert completed.status == "completed"
    assert completed.checkpoint.accepted_request_count == len(requests)
    assert completed.replayed is False
    assert len(calls) == len(requests)

    replayed = AutonomousEvidenceExecutionResumableController(controller, store, "all-domains-job").run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, values=values),
    )
    assert replayed.status == "completed"
    assert replayed.replayed is True
    assert all(receipt.replay == "replayed" for receipt in replayed.result.runtime.receipts)
    assert len(calls) == len(requests)
    assert "sequence" not in json.dumps(replayed.to_dict())


def test_checkpoint_persistence_is_canonical_tamper_evident_and_cas_fenced() -> None:
    checkpoint_payload = {
        "schema": "bioprism-python-autonomous-evidence-execution-checkpoint/0.1",
        "job_id": "checkpoint-job",
        "evidence_plan_digest": "a" * 64,
        "execution_plan_digest": "b" * 64,
        "request_digest": "c" * 64,
        "readiness_report_digest": "d" * 64,
        "status": "approval_required",
        "runtime_status": None,
        "runtime_result_digest": None,
        "completed_request_count": 0,
        "pending_request_count": 0,
        "accepted_request_count": 0,
    }
    checkpoint = {
        **checkpoint_payload,
        "checkpoint_digest": content_digest(checkpoint_payload),
        "retention": "metadata_only;requests_readiness_and_source_values_caller_owned",
        "secret_material": "never_returned",
    }

    class TextStore:
        value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

    class CasTextStore(TextStore):
        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            current = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
            if current != expected:
                return False
            self.value = value
            return True

    plain = TextStore()
    persistence = JsonAutonomousEvidenceExecutionCheckpointPersistence(plain)
    persistence.write(validate_autonomous_evidence_execution_checkpoint(checkpoint))
    assert plain.value == canonical_json(json.loads(plain.value))
    assert persistence.read().checkpoint_digest == checkpoint["checkpoint_digest"]
    with pytest.raises(ArgumentError, match="digest is invalid"):
        validate_autonomous_evidence_execution_checkpoint({**checkpoint, "checkpoint_digest": "e" * 64})

    cas = CasTextStore()
    transactional = TransactionalJsonAutonomousEvidenceExecutionCheckpointPersistence(cas)
    typed = validate_autonomous_evidence_execution_checkpoint(checkpoint)
    assert transactional.write_if_unchanged(None, typed) is True
    assert transactional.write_if_unchanged(None, typed) is False
    assert transactional.write_if_unchanged(typed.checkpoint_digest, typed) is True


def test_agent_facade_exposes_preparation_and_resumable_execution() -> None:
    calls: list[int] = []
    registry = _registry(calls)
    agent = _agent()
    plan = agent.evidence_plan(("coding",))
    requests = _requests(plan, "facade")
    prepared = agent.prepare_reviewed_evidence(
        registry,
        ("coding",),
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    assert prepared.status == "ready_for_review"
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    gated = agent.execute_reviewed_evidence_resumable(
        registry,
        ("coding",),
        requests,
        job_id="facade-job",
        checkpoint_store=store,
        prepare_options={"readiness_policy": AutonomousEvidenceReadinessPolicy(require_health=False), "allow_degraded_dispatch": True},
        execute_options={"journal": journal},
    )
    assert gated.status == "approval_required"
    completed = agent.execute_reviewed_evidence_resumable(
        registry,
        ("coding",),
        requests,
        job_id="facade-job",
        checkpoint_store=store,
        prepare_options={"readiness_policy": AutonomousEvidenceReadinessPolicy(require_health=False), "allow_degraded_dispatch": True},
        execute_options={**_execute_options(journal=journal)},
    )
    assert completed.status == "completed"
    assert len(calls) == len(requests)
