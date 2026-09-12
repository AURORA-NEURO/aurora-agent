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
    CredentialStore,
    InMemoryAutonomousEvidenceExecutionCheckpointStore,
    InMemoryAutonomousEvidenceRuntimeJournal,
    JsonAutonomousEvidenceExecutionCheckpointPersistence,
    LLMRuntime,
    TransactionalJsonAutonomousEvidenceExecutionCheckpointPersistence,
    ArgumentError,
    canonical_json,
    content_digest,
    create_autonomous_evidence_execution_reconciliation_receipt,
    evidence_execution_reconciliation_request_digest,
    register_autonomous_evidence_adapters_for_all_domains,
    validate_autonomous_evidence_execution_checkpoint,
    validate_autonomous_evidence_execution_reconciliation_receipt,
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
    projector_id = "fixture-projector"
    projector_version = "1"

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


def _execute_options(
    journal=None,
    values=None,
    *,
    approve: bool = True,
    resumable: bool = False,
) -> dict[str, object]:
    options: dict[str, object] = {
        "approve_source_dispatch": approve,
        "projector": _Projector(),
        "evaluator": _Evaluator(),
        "sleep": lambda _delay: None,
    }
    if resumable:
        options["resumable_policy_identity"] = {
            "journal": {
                "id": "fixture-runtime-journal",
                "version": "1",
                "config_digest": content_digest({"store": "fixture-journal-v1"}),
            },
            "value_rehydrator": {
                "id": "fixture-value-rehydrator",
                "version": "1",
                "config_digest": content_digest({"store": "fixture-values-v1"}),
            }
        }
    if journal is not None:
        options["journal"] = journal
    if values is not None:
        options["rehydrate_value"] = lambda receipt: values.get(receipt["value_digest"])
    return options


def _resumable(controller, store, job_id: str):
    return AutonomousEvidenceExecutionResumableController(
        controller,
        store,
        job_id,
        reconciliation_authority_id="source-audit",
        reconciliation_authority_version="1",
        reconciliation_authority_config_digest=content_digest(
            {"journal": "fixture-source-audit-v1"}
        ),
    )


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
    requests.append(
        {
            **requests[0],
            "source_id": "resumable-source-secondary",
            "request_id": "resumable-request-secondary",
        }
    )
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    first = _resumable(controller, store, "all-domains-job")
    gated = first.run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, approve=False, resumable=True),
    )
    assert gated.status == "approval_required"
    assert gated.checkpoint.completed_request_count == 0
    assert calls == []

    restarted = _resumable(controller, store, "all-domains-job")
    assert restarted.restore()["status"] == "restored"
    completed = restarted.run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, resumable=True),
    )
    assert completed.status == "completed"
    assert completed.checkpoint.completed_request_count == len(plan.requirements)
    assert completed.checkpoint.accepted_request_count == len(requests)
    assert completed.replayed is False
    assert completed.checkpoint.checkpoint_generation == 3
    assert completed.checkpoint.required_requirement_count == len(plan.requirements)
    completed_digest = completed.checkpoint.checkpoint_digest
    assert len(calls) == len(requests)

    replayed = _resumable(controller, store, "all-domains-job").run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, values=values, resumable=True),
    )
    assert replayed.status == "completed"
    assert replayed.replayed is True
    assert replayed.checkpoint.checkpoint_generation == 5
    assert replayed.checkpoint.checkpoint_digest != completed_digest
    assert replayed.checkpoint.previous_checkpoint_digest is not None
    assert all(receipt.replay == "replayed" for receipt in replayed.result.runtime.receipts)
    assert len(calls) == len(requests)
    assert "sequence" not in json.dumps(replayed.to_dict())


def test_resumable_execution_binds_policy_authority_and_cas_before_source_calls() -> None:
    calls: list[int] = []
    values: dict[str, object] = {}
    registry = _registry(calls, values)
    plan = _agent().evidence_plan(("coding",))
    controller = AutonomousEvidenceExecutionController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    requests = _requests(plan, "fenced")
    journal = InMemoryAutonomousEvidenceRuntimeJournal()

    class RecordingStore(InMemoryAutonomousEvidenceExecutionCheckpointStore):
        def __init__(self) -> None:
            super().__init__()
            self.commits = []

        def write_if_unchanged(self, expected, checkpoint):
            written = super().write_if_unchanged(expected, checkpoint)
            if written:
                self.commits.append(checkpoint)
            return written

    store = RecordingStore()
    first = _resumable(controller, store, "fenced-workers-job")
    stale = _resumable(controller, store, "fenced-workers-job")
    assert first.restore()["status"] == "empty"
    assert stale.restore()["status"] == "empty"
    completed = first.run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, resumable=True),
    )
    calls_after_first = len(calls)
    assert completed.status == "completed"
    assert [item.checkpoint_generation for item in store.commits] == [1, 2]
    first_pending_digest = store.commits[0].checkpoint_digest
    with pytest.raises(ArgumentError, match="another worker committed"):
        stale.run(
            execution_plan,
            plan,
            requests,
            **_execute_options(journal=journal, resumable=True),
        )
    assert len(calls) == calls_after_first

    replayed = _resumable(controller, store, "fenced-workers-job").run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=journal, values=values, resumable=True),
    )
    assert replayed.status == "completed"
    assert [item.checkpoint_generation for item in store.commits] == [1, 2, 3, 4]
    assert store.commits[2].checkpoint_digest != first_pending_digest
    assert store.commits[2].previous_checkpoint_digest == completed.checkpoint.checkpoint_digest
    assert len(calls) == calls_after_first

    policy_store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    gated = _resumable(controller, policy_store, "policy-bound-job").run(
        execution_plan,
        plan,
        requests,
        **_execute_options(journal=InMemoryAutonomousEvidenceRuntimeJournal(), approve=False, resumable=True),
    )
    assert gated.status == "approval_required"
    calls_before_drift = len(calls)
    drifted = _execute_options(
        journal=InMemoryAutonomousEvidenceRuntimeJournal(),
        resumable=True,
    )
    drifted["parent_evidence_digests"] = ["a" * 64]
    with pytest.raises(ArgumentError, match="execution policy"):
        _resumable(controller, policy_store, "policy-bound-job").run(
            execution_plan,
            plan,
            requests,
            **drifted,
        )
    assert len(calls) == calls_before_drift
    journal_drift = _execute_options(
        journal=InMemoryAutonomousEvidenceRuntimeJournal(),
        resumable=True,
    )
    journal_drift["resumable_policy_identity"] = {
        **journal_drift["resumable_policy_identity"],
        "journal": {
            "id": "different-runtime-journal",
            "version": "1",
            "config_digest": content_digest({"store": "different-journal"}),
        },
    }
    with pytest.raises(ArgumentError, match="execution policy"):
        _resumable(controller, policy_store, "policy-bound-job").run(
            execution_plan,
            plan,
            requests,
            **journal_drift,
        )
    assert len(calls) == calls_before_drift
    callback_drift = _execute_options(
        journal=InMemoryAutonomousEvidenceRuntimeJournal(),
        resumable=True,
    )

    class ProjectorV2(_Projector):
        projector_version = "2"

    callback_drift["projector"] = ProjectorV2()
    with pytest.raises(ArgumentError, match="execution policy"):
        _resumable(controller, policy_store, "policy-bound-job").run(
            execution_plan,
            plan,
            requests,
            **callback_drift,
        )
    assert len(calls) == calls_before_drift
    with pytest.raises(ArgumentError, match="trust root"):
        AutonomousEvidenceExecutionResumableController(
            controller,
            policy_store,
            "policy-bound-job",
            reconciliation_authority_id="other-audit",
            reconciliation_authority_version="1",
            reconciliation_authority_config_digest=content_digest(
                {"journal": "fixture-source-audit-v1"}
            ),
        ).restore()
    with pytest.raises(ArgumentError, match="trust root"):
        AutonomousEvidenceExecutionResumableController(
            controller,
            policy_store,
            "policy-bound-job",
            reconciliation_authority_id="source-audit",
            reconciliation_authority_version="1",
            reconciliation_authority_config_digest=content_digest(
                {"journal": "different-source-audit-config"}
            ),
        ).restore()

    class PlainStore:
        checkpoint = None

        def read(self):
            return self.checkpoint

        def write(self, checkpoint):
            self.checkpoint = checkpoint

    with pytest.raises(ArgumentError, match="compare-and-swap"):
        _resumable(controller, PlainStore(), "unfenced-job").run(
            execution_plan,
            plan,
            requests,
            **_execute_options(
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                resumable=True,
            ),
        )
    assert len(calls) == calls_before_drift
    with pytest.raises(ArgumentError, match="runtime journal"):
        _resumable(
            controller,
            InMemoryAutonomousEvidenceExecutionCheckpointStore(),
            "journal-required-job",
        ).run(
            execution_plan,
            plan,
            requests,
            **_execute_options(resumable=True),
        )
    with pytest.raises(ArgumentError, match="configured reconciliation authority"):
        AutonomousEvidenceExecutionResumableController(
            controller,
            InMemoryAutonomousEvidenceExecutionCheckpointStore(),
            "authority-required-job",
        ).run(
            execution_plan,
            plan,
            requests,
            **_execute_options(
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                resumable=True,
            ),
        )
    assert len(calls) == calls_before_drift


def test_uncertain_dispatch_requires_per_request_reconciliation_and_never_blindly_retries() -> None:
    calls: list[int] = []
    values: dict[str, object] = {}
    registry = _registry(calls, values)
    agent = _agent()
    plan = agent.evidence_plan(("coding",))

    class AmbiguousController(AutonomousEvidenceExecutionController):
        fail_after_first_dispatch = True

        def execute(self, execution_plan, evidence_plan, requests, **options):
            if self.fail_after_first_dispatch:
                self.fail_after_first_dispatch = False
                super().execute(execution_plan, evidence_plan, requests[:1], **options)
                raise RuntimeError("transport acknowledgement was lost")
            return super().execute(execution_plan, evidence_plan, requests, **options)

    controller = AmbiguousController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    requests = _requests(plan, "uncertain")
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    resumable = _resumable(controller, store, "uncertain-dispatch-job")
    with pytest.raises(RuntimeError, match="acknowledgement"):
        resumable.run(
            execution_plan,
            plan,
            requests,
            **_execute_options(journal=journal, resumable=True),
        )
    checkpoint = store.read()
    assert checkpoint.status == "reconciliation_required"
    assert len(calls) == 1

    with pytest.raises(ArgumentError, match="cannot authorize source redispatch"):
        _resumable(controller, store, "uncertain-dispatch-job").run(
            execution_plan,
            plan,
            requests,
            resume_after_reconciliation=True,
            **_execute_options(journal=journal, values=values, resumable=True),
        )
    assert len(calls) == 1

    request_digests = [evidence_execution_reconciliation_request_digest(plan, request) for request in requests]
    first_receipt = next(
        entry.receipt for entry in journal.records()
        if entry.receipt.request_digest == request_digests[0]
    )

    def outcomes(second_outcome: str) -> list[dict[str, object]]:
        return [
            {
                "request_digest": request_digest,
                "outcome": "succeeded" if index == 0 else second_outcome if index == 1 else "not_executed",
                "evidence_digest": content_digest({"request_digest": request_digest, "attestation": second_outcome}),
                "evidence_kind": "source_dispatch_audit",
                "effect_absent": index != 0 and (index != 1 or second_outcome == "not_executed"),
                "runtime_receipt_digest": first_receipt.receipt_digest if index == 0 else None,
            }
            for index, request_digest in enumerate(request_digests)
        ]

    unknown = create_autonomous_evidence_execution_reconciliation_receipt(
        checkpoint,
        execution_plan,
        plan,
        requests,
        authority_id="source-audit",
        authority_version="1",
        outcomes=outcomes("unknown"),
    )
    with pytest.raises(ArgumentError, match="trust root"):
        create_autonomous_evidence_execution_reconciliation_receipt(
            checkpoint,
            execution_plan,
            plan,
            requests,
            authority_id="other-source-audit",
            authority_version="1",
            outcomes=outcomes("unknown"),
        )
    held = _resumable(controller, store, "uncertain-dispatch-job").run(
        execution_plan,
        plan,
        requests,
        reconciliation_receipt=unknown,
        **_execute_options(journal=journal, values=values, resumable=True),
    )
    assert held.status == "reconciliation_required"
    assert held.checkpoint.reconciliation_receipt_digest == unknown.receipt_digest
    assert held.checkpoint.checkpoint_digest != checkpoint.checkpoint_digest
    assert len(calls) == 1

    tampered = {**unknown.to_dict(), "authority_id": "forged-source-audit"}
    with pytest.raises(ArgumentError, match="receipt digest"):
        validate_autonomous_evidence_execution_reconciliation_receipt(tampered)

    reconciled = create_autonomous_evidence_execution_reconciliation_receipt(
        held.checkpoint,
        execution_plan,
        plan,
        requests,
        authority_id="source-audit",
        authority_version="1",
        outcomes=outcomes("not_executed"),
    )
    completed = _resumable(controller, store, "uncertain-dispatch-job").run(
        execution_plan,
        plan,
        requests,
        reconciliation_receipt=reconciled,
        **_execute_options(journal=journal, values=values, resumable=True),
    )
    assert completed.status == "completed"
    assert completed.replayed is True
    assert completed.checkpoint.reconciliation_receipt_digest == reconciled.receipt_digest
    assert len(calls) == len(requests)
    with pytest.raises(ArgumentError, match="outside a reconciliation boundary"):
        _resumable(controller, store, "uncertain-dispatch-job").run(
            execution_plan,
            plan,
            requests,
            reconciliation_receipt=reconciled,
            **_execute_options(journal=journal, values=values, resumable=True),
        )


def test_reconciliation_prevalidates_later_success_before_any_fresh_dispatch() -> None:
    calls: list[int] = []
    values: dict[str, object] = {}
    registry = _registry(calls, values)
    agent = _agent()
    plan = agent.evidence_plan(("coding",))

    class LaterSuccessController(AutonomousEvidenceExecutionController):
        fail_once = True

        def execute(self, execution_plan, evidence_plan, requests, **options):
            if self.fail_once:
                self.fail_once = False
                super().execute(execution_plan, evidence_plan, requests[1:2], **options)
                raise RuntimeError("later source acknowledgement was lost")
            return super().execute(execution_plan, evidence_plan, requests, **options)

    controller = LaterSuccessController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    requests = _requests(plan, "prevalidate")
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    with pytest.raises(RuntimeError, match="acknowledgement"):
        _resumable(controller, store, "prevalidate-reconciliation-job").run(
            execution_plan,
            plan,
            requests,
            **_execute_options(journal=journal, resumable=True),
        )
    assert len(calls) == 1
    checkpoint = store.read()
    request_digests = [
        evidence_execution_reconciliation_request_digest(plan, request)
        for request in requests
    ]
    succeeded = next(entry.receipt for entry in journal.records())
    receipt = create_autonomous_evidence_execution_reconciliation_receipt(
        checkpoint,
        execution_plan,
        plan,
        requests,
        authority_id="source-audit",
        authority_version="1",
        outcomes=[
            {
                "request_digest": request_digest,
                "outcome": "succeeded" if index == 1 else "not_executed",
                "evidence_digest": content_digest(
                    {"request_digest": request_digest, "attestation": "prevalidated"}
                ),
                "evidence_kind": "source_dispatch_audit",
                "effect_absent": index != 1,
                "runtime_receipt_digest": succeeded.receipt_digest if index == 1 else None,
            }
            for index, request_digest in enumerate(request_digests)
        ],
    )

    with pytest.raises(ArgumentError, match="value does not match"):
        _resumable(controller, store, "prevalidate-reconciliation-job").run(
            execution_plan,
            plan,
            requests,
            reconciliation_receipt=receipt,
            **_execute_options(journal=journal, values={}, resumable=True),
        )
    assert len(calls) == 1


def test_reconciliation_rejects_a_restamped_receipt_inside_an_unrestamped_journal() -> None:
    calls: list[int] = []
    values: dict[str, object] = {}
    registry = _registry(calls, values)
    plan = _agent().evidence_plan(("coding",))

    class AmbiguousController(AutonomousEvidenceExecutionController):
        fail_once = True

        def execute(self, execution_plan, evidence_plan, requests, **options):
            if self.fail_once:
                self.fail_once = False
                super().execute(execution_plan, evidence_plan, requests[:1], **options)
                raise RuntimeError("journal acknowledgement was lost")
            return super().execute(execution_plan, evidence_plan, requests, **options)

    controller = AmbiguousController(registry)
    execution_plan = controller.prepare(
        plan,
        readiness_policy=AutonomousEvidenceReadinessPolicy(require_health=False),
        allow_degraded_dispatch=True,
    )
    requests = _requests(plan, "journal-tamper")
    store = InMemoryAutonomousEvidenceExecutionCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    with pytest.raises(RuntimeError, match="acknowledgement"):
        _resumable(controller, store, "journal-tamper-job").run(
            execution_plan,
            plan,
            requests,
            **_execute_options(journal=journal, resumable=True),
        )
    checkpoint = store.read()
    entry = journal.records()[0]
    forged_entry = entry.to_dict()
    forged_receipt = dict(forged_entry["receipt"])
    forged_receipt["source_id"] = "forged-source"
    forged_receipt["receipt_digest"] = content_digest(
        {key: value for key, value in forged_receipt.items() if key != "receipt_digest"}
    )
    forged_entry["receipt"] = forged_receipt

    class ForgedEntry:
        entry_digest = forged_entry["entry_digest"]

        def to_dict(self):
            return forged_entry

    class ForgedJournal:
        def records(self):
            return (ForgedEntry(),)

        def append(self, _entry):
            raise AssertionError("invalid journal must fail before append")

    request_digests = [
        evidence_execution_reconciliation_request_digest(plan, request)
        for request in requests
    ]
    receipt = create_autonomous_evidence_execution_reconciliation_receipt(
        checkpoint,
        execution_plan,
        plan,
        requests,
        authority_id="source-audit",
        authority_version="1",
        outcomes=[
            {
                "request_digest": request_digest,
                "outcome": "succeeded" if index == 0 else "not_executed",
                "evidence_digest": content_digest(
                    {"request_digest": request_digest, "attestation": "journal-audit"}
                ),
                "evidence_kind": "source_dispatch_audit",
                "effect_absent": index != 0,
                "runtime_receipt_digest": (
                    forged_receipt["receipt_digest"] if index == 0 else None
                ),
            }
            for index, request_digest in enumerate(request_digests)
        ],
    )
    calls_before_resume = len(calls)
    with pytest.raises(ArgumentError, match="entry digest"):
        _resumable(controller, store, "journal-tamper-job").run(
            execution_plan,
            plan,
            requests,
            reconciliation_receipt=receipt,
            **_execute_options(journal=ForgedJournal(), values=values, resumable=True),
        )
    assert len(calls) == calls_before_resume


def test_checkpoint_persistence_is_canonical_tamper_evident_and_cas_fenced() -> None:
    checkpoint_payload = {
        "schema": "bioprism-python-autonomous-evidence-execution-checkpoint/0.2",
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
        "required_requirement_count": 1,
        "execution_policy_digest": "e" * 64,
        "reconciliation_authority_id": "source-audit",
        "reconciliation_authority_version": "1",
        "reconciliation_authority_config_digest": content_digest(
            {"journal": "fixture-source-audit-v1"}
        ),
        "reconciliation_receipt_digest": None,
        "checkpoint_generation": 1,
        "previous_checkpoint_digest": None,
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
    forged_payload = {
        **checkpoint_payload,
        "status": "completed",
        "runtime_status": "completed",
        "runtime_result_digest": "e" * 64,
        "completed_request_count": 0,
        "pending_request_count": 17,
        "accepted_request_count": 0,
    }
    forged = {
        **forged_payload,
        "checkpoint_digest": content_digest(forged_payload),
        "retention": checkpoint["retention"],
        "secret_material": checkpoint["secret_material"],
    }
    with pytest.raises(ArgumentError, match="incomplete request counts"):
        validate_autonomous_evidence_execution_checkpoint(forged)

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
        execute_options=_execute_options(journal=journal, approve=False, resumable=True),
        reconciliation_authority_id="source-audit",
        reconciliation_authority_version="1",
        reconciliation_authority_config_digest=content_digest(
            {"journal": "fixture-source-audit-v1"}
        ),
    )
    assert gated.status == "approval_required"
    completed = agent.execute_reviewed_evidence_resumable(
        registry,
        ("coding",),
        requests,
        job_id="facade-job",
        checkpoint_store=store,
        prepare_options={"readiness_policy": AutonomousEvidenceReadinessPolicy(require_health=False), "allow_degraded_dispatch": True},
        execute_options={**_execute_options(journal=journal, resumable=True)},
        reconciliation_authority_id="source-audit",
        reconciliation_authority_version="1",
        reconciliation_authority_config_digest=content_digest(
            {"journal": "fixture-source-audit-v1"}
        ),
    )
    assert completed.status == "completed"
    assert len(calls) == len(requests)
