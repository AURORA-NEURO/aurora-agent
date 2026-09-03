from __future__ import annotations

from collections.abc import Iterator, Mapping
from dataclasses import dataclass, replace
import http.client
import json

import pytest

import prism_sdk.brain as brain_module
import prism_sdk.autonomous_evidence_brain as evidence_brain_module
import prism_sdk.llm_runtime as llm_runtime_module

from test_autonomy import _Workspace, _model, _runtime, _structured_value_from_schema

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousEvidenceBackedController,
    BrainRunResult,
    BrainRunError,
    CredentialStore,
    InMemoryAutonomousEvidenceBackedCheckpointStore,
    InMemoryAutonomousEvidenceRuntimeJournal,
    JsonAutonomousEvidenceBackedCheckpointPersistence,
    LLMRuntime,
    ModelCatalogue,
    ProviderError,
    ProviderRequest,
    ProviderResponse,
    ProviderToolCall,
    TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence,
    AutonomousEvidenceBackedCheckpoint,
    AutonomousEvidenceBackedProviderDispatchReceipt,
    content_digest,
    run_autonomous_evidence_backed_resumable,
    validate_autonomous_evidence_backed_checkpoint,
    validate_autonomous_evidence_backed_provider_dispatch_receipt,
)
from prism_sdk.errors import ArgumentError


_PROVIDER_POLICY_CONFIG_DIGEST = content_digest(
    {"fixture": "autonomous-evidence-backed-provider-policy-v1"}
)


def _provider_policy_identity() -> dict[str, object]:
    return {
        "provider_policy": {
            "id": "fixture-provider-policy",
            "version": "v1",
            "config_digest": _PROVIDER_POLICY_CONFIG_DIGEST,
        }
    }


def _in_memory_runtime(
    handler: object,
    *,
    max_attempts: int = 1,
) -> tuple[LLMRuntime, CredentialStore]:
    store = CredentialStore()
    runtime = LLMRuntime(store)
    runtime.register_in_memory_provider(
        "openai",
        handler,  # type: ignore[arg-type]
        max_attempts=max_attempts,
    )
    return runtime, store


def _expected_resumable_transport_key(
    *,
    provider_operation_digest: str,
    provider: str,
    request: ProviderRequest,
    incoming_scope: str,
) -> str:
    root_key = content_digest(
        {
            "schema": "bioprism-python-autonomous-evidence-backed-provider-idempotency/0.1",
            "provider_operation_digest": provider_operation_digest,
        }
    )
    request_digest = content_digest(
        {
            "schema": "bioprism-python-autonomous-evidence-provider-request-idempotency/0.1",
            "provider": provider,
            "model": request.model,
            "messages": [dict(message) for message in request.messages],
            "max_output_tokens": request.max_output_tokens,
            "temperature": request.temperature,
            "require_json": request.require_json,
            "response_schema": request.response_schema,
            "tools": [tool.to_dict() for tool in request.tools],
            "tool_choice": request.tool_choice,
        }
    )
    return content_digest(
        {
            "schema": "bioprism-python-autonomous-evidence-provider-request-idempotency/0.1",
            "provider_operation_idempotency_key": root_key,
            "incoming_idempotency_scope": incoming_scope,
            "provider": provider,
            "model": request.model,
            "request_digest": request_digest,
        }
    )


class _AcceptAllEvidence:
    evaluator_id = "fixture-evaluator"
    evaluator_version = "v1"

    def evaluate(self, _input: object) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


def _requests(agent: AutonomousAgent, domains: tuple[str, ...]) -> list[dict[str, object]]:
    plan = agent.evidence_plan(domains)
    return [
        {
            "requirement_id": requirement.requirement_id,
            "source_id": "credentialless-fixture",
            "request_id": f"fixture-{index}",
            "metadata": {"fixture": "deterministic"},
        }
        for index, requirement in enumerate(plan.requirements)
    ]


def test_reviewed_evidence_bridge_requires_source_approval_before_acquisition():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "evidence-approval-test")
    calls: list[object] = []

    try:
        result = agent.run_with_reviewed_evidence(
            task="review every autonomous domain",
            domains=AUTONOMOUS_DOMAINS,
            requests=_requests(agent, AUTONOMOUS_DOMAINS),
            acquirer=lambda context: calls.append(context) or {"private_payload": "caller-owned"},
            credentials={"openai": handle},
            model_candidates=_model(),
            approve_source_dispatch=False,
        )
        assert result.status == "evidence_review_required"
        assert result.evidence is None
        assert calls == []
        assert "caller-owned" not in json.dumps(result.to_dict())
        assert result.to_dict()["evidence_plan_digest"] == result.evidence_plan.plan_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_executes_every_builtin_domain_and_redacts_transient_values():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "all-domain-evidence-test")

    try:
        for domain in AUTONOMOUS_DOMAINS:
            domains = (domain,)
            plan = agent.evidence_plan(domains)
            requests = _requests(agent, domains)

            def acquirer(context: object, *, selected_domain: str = domain) -> dict[str, str]:
                requirement = context["requirement"]  # type: ignore[index]
                return {
                    "private_payload": f"caller-owned-{selected_domain}",
                    "requirement": requirement.requirement_id,
                }

            def projector(value: object, context: object) -> list[dict[str, object]]:
                _ = value
                requirement = context["requirement"]  # type: ignore[index]
                return [{"label": requirement.label}]

            result = agent.run_with_reviewed_evidence(
                task=f"review a bounded {domain} task",
                domains=domains,
                requests=requests,
                acquirer=acquirer,
                projector=projector,
                evaluator=_AcceptAllEvidence(),
                credentials={"openai": handle},
                model_candidates=_model(),
                run_mode="domain",
                approve_source_dispatch=True,
                approve_provider_call=True,
            )
            assert result.status == "completed", domain
            assert result.evidence is not None
            assert result.evidence.status == "completed", domain
            assert result.execution_status == "completed_provider_call", domain
            projection = result.to_dict()
            assert projection["schema"] == "bioprism-python-autonomous-evidence-backed-run/0.1"
            assert f"caller-owned-{domain}" not in json.dumps(projection)
            assert "bounded" not in json.dumps(projection)
            assert projection["evidence_plan_digest"] == plan.plan_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_incomplete_evidence_override_runs_provider_without_claiming_completion():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "incomplete-evidence-override-test")
    domains = ("science",)

    try:
        result = agent.run_with_reviewed_evidence(
            task="synthesize a science question while preserving missing evidence",
            domains=domains,
            requests=_requests(agent, domains),
            acquirer=lambda _context: {"unreviewed_payload": "caller-owned"},
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="domain",
            approve_source_dispatch=True,
            allow_incomplete_evidence=True,
            approve_provider_call=True,
        )

        assert result.status == "evidence_incomplete"
        assert result.evidence is not None
        assert result.evidence.status == "partial"
        assert result.evidence.missing_requirement_ids
        assert result.execution is not None
        assert result.execution_status == "completed_provider_call"
        assert getattr(server, "request_count", 0) == 1
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_replays_with_rehydration_and_keeps_provider_approval_separate():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "evidence-replay-test")
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    values: dict[str, object] = {}
    domains = ("science",)
    requests = _requests(agent, domains)

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        value = {
            "private_payload": "replay-me-only-in-memory",
            "requirement": requirement.requirement_id,
        }
        values[content_digest(value)] = value
        return value

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        pending_provider = agent.run_with_reviewed_evidence(
            task="replay an accepted science evidence run",
            domains=domains,
            requests=requests,
            acquirer=acquirer,
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            journal=journal,
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=False,
        )
        assert pending_provider.status == "provider_review_required"
        assert pending_provider.execution_status == "approval_required"
        assert getattr(server, "request_count", 0) == 0

        completed = agent.run_with_reviewed_evidence(
            task="replay an accepted science evidence run",
            domains=domains,
            requests=requests,
            acquirer=lambda _context: (_ for _ in ()).throw(AssertionError("replay reacquired evidence")),
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            journal=journal,
            rehydrate_value=lambda receipt: values.get(receipt["value_digest"]),
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
        )
        assert completed.status == "completed"
        assert completed.evidence is not None
        assert all(receipt.replay == "replayed" for receipt in completed.evidence.receipts)
        assert getattr(server, "request_count", 0) > 0
        assert "replay-me-only-in-memory" not in json.dumps(completed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_supports_bounded_cross_domain_invocation_and_transient_prompt_projection():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "cross-domain-evidence-test")
    domains = ("coding", "science")

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        return {
            "prompt_only": "transient-cross-domain-value",
            "requirement": requirement.requirement_id,
        }

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def prompt_builder(evidence: object) -> dict[str, object]:
        values = evidence.values  # type: ignore[attr-defined]
        return {
            "reviewed_evidence": {
                "status": evidence.status,  # type: ignore[attr-defined]
                "transient_value": next(iter(values.values())),
            }
        }

    try:
        result = agent.run_with_reviewed_evidence(
            task="reconcile coding and science findings",
            domains=domains,
            requests=_requests(agent, domains),
            acquirer=acquirer,
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            prompt_builder=prompt_builder,
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="cross_domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
        )
        assert result.status == "completed"
        assert result.execution_status == "completed"
        assert result.prompt_context["reviewed_evidence"]["transient_value"]["prompt_only"] == "transient-cross-domain-value"  # type: ignore[index]
        assert "transient-cross-domain-value" not in json.dumps(result.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


class _ResumableFixtureAcquirer:
    acquirer_id = "fixture-resumable-acquirer"
    acquirer_version = "v1"

    def __init__(self, marker: str) -> None:
        self.marker = marker
        self.values: dict[str, object] = {}
        self.calls = 0

    def __call__(self, context: object) -> dict[str, str]:
        self.calls += 1
        requirement = context["requirement"]  # type: ignore[index]
        value = {
            "private_payload": self.marker,
            "requirement": requirement.requirement_id,
        }
        self.values[content_digest(value)] = value
        return value


class _ResumableFixtureValueRehydrator:
    value_rehydrator_id = "fixture-resumable-value-rehydrator"
    value_rehydrator_version = "v1"

    def __init__(self, values: dict[str, object]) -> None:
        self.values = values
        self.calls = 0

    def __call__(self, receipt: object) -> object | None:
        self.calls += 1
        return self.values.get(receipt["value_digest"])  # type: ignore[index]


class _ResumableFixturePromptBuilder:
    prompt_builder_id = "fixture-resumable-prompt-builder"
    prompt_builder_version = "v1"

    def __init__(self, marker: str) -> None:
        self.marker = marker

    def __call__(self, evidence: object) -> dict[str, object]:
        return {
            "reviewed_evidence": {
                "marker": self.marker,
                "status": evidence.status,  # type: ignore[attr-defined]
            }
        }


class _ResumableFixtureEvaluator:
    evaluator_id = "fixture-resumable-evaluator"

    def __init__(self, version: str = "v1") -> None:
        self.evaluator_version = version

    def evaluate(self, _input: object) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


class _RecordingCheckpointStore:
    def __init__(self) -> None:
        self.current: AutonomousEvidenceBackedCheckpoint | None = None
        self.history: list[AutonomousEvidenceBackedCheckpoint] = []
        self.reject_terminal = False
        self.force_conflict = False
        self.dispatch_receipts: list[
            AutonomousEvidenceBackedProviderDispatchReceipt
        ] = []
        self.reject_dispatch = False
        self.commit_dispatch_then_throw = False

    def read(self) -> dict[str, object] | None:
        return None if self.current is None else self.current.to_dict()

    def write(self, checkpoint: object) -> None:
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)  # type: ignore[arg-type]
        self.current = verified
        self.history.append(verified)

    def write_if_unchanged(
        self,
        expected: str | None,
        checkpoint: object,
    ) -> bool:
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)  # type: ignore[arg-type]
        observed = None if self.current is None else self.current.checkpoint_digest
        if (
            observed != expected
            or self.force_conflict
            or (self.reject_terminal and verified.status == "completed")
        ):
            return False
        self.current = verified
        self.history.append(verified)
        return True

    def write_dispatch_if_unchanged(
        self,
        expected: str | None,
        checkpoint: object,
        private_receipt: object,
    ) -> bool:
        verified = validate_autonomous_evidence_backed_checkpoint(checkpoint)  # type: ignore[arg-type]
        receipt = validate_autonomous_evidence_backed_provider_dispatch_receipt(  # type: ignore[arg-type]
            private_receipt
        )
        observed = None if self.current is None else self.current.checkpoint_digest
        if observed != expected or self.force_conflict or self.reject_dispatch:
            return False
        assert verified.status == "provider_in_flight"
        assert verified.provider_dispatch_count == receipt.dispatch_index
        assert verified.provider_dispatch_head_digest == receipt.receipt_digest
        self.current = verified
        self.history.append(verified)
        self.dispatch_receipts.append(receipt)
        if self.commit_dispatch_then_throw:
            raise RuntimeError("fixture committed dispatch then lost acknowledgement")
        return True


def test_resumable_evidence_controller_replays_sources_and_provider_results_without_dispatch():
    provider_idempotency_keys: list[str | None] = []
    provider_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        provider_idempotency_keys.append(request.idempotency_key)
        provider_requests.append(request)
        return {
            "text": "bounded answer",
            "request_id": "resumable-evidence-response",
            "usage": {"total_tokens": 4},
        }

    runtime, store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-evidence-test")
    checkpoint_store = _RecordingCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    controller = AutonomousEvidenceBackedController(agent, "resumable-evidence-job", checkpoint_store)
    values: dict[str, object] = {}
    acquisitions = 0
    domains = ("coding",)
    requests = _requests(agent, domains)

    def acquirer(context: object) -> dict[str, str]:
        nonlocal acquisitions
        acquisitions += 1
        requirement = context["requirement"]  # type: ignore[index]
        value = {
            "private_payload": "resumable-caller-value",
            "requirement": requirement.requirement_id,
        }
        values[content_digest(value)] = value
        return value

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def run_options(*, provider: bool = False) -> dict[str, object]:
        return {
            "requests": requests,
            "acquirer": acquirer,
            "projector": projector,
            "evaluator": _AcceptAllEvidence(),
            "journal": journal,
            "rehydrate_value": lambda receipt: values.get(receipt["value_digest"]),
            "credentials": {"openai": handle},
            "model_candidates": _model(),
            "domains": domains,
            "run_mode": "domain",
            "approve_source_dispatch": True,
            "approve_provider_call": provider,
            "resumable_policy_identity": {
                **_provider_policy_identity(),
                "acquirer": {
                    "id": "fixture-resumable-acquirer",
                    "version": "v1",
                    "config_digest": content_digest(
                        {"fixture": "resumable-caller-value"}
                    ),
                },
                "value_rehydrator": {
                    "id": "fixture-resumable-value-rehydrator",
                    "version": "v1",
                    "config_digest": content_digest(
                        {"fixture": "resumable-caller-value"}
                    ),
                },
            },
        }

    try:
        assert controller.restore()["status"] == "empty"
        first = controller.run(task="resume a coding evidence run", **run_options())
        assert first["run"].status == "provider_pending"
        assert first["run"].checkpoint.status == "provider_pending"
        assert first["run"].checkpoint.generation == 1
        assert first["run"].checkpoint.previous_checkpoint_digest is None
        assert first["run"].checkpoint.provider_operation_digest is None
        assert acquisitions == len(requests)
        assert len(provider_requests) == 0

        approval_only = controller.run(
            task="resume a coding evidence run",
            **run_options(provider=True),
        )
        assert approval_only["run"].status == "provider_pending"
        assert len(provider_requests) == 0
        resume_only = controller.run(
            task="resume a coding evidence run",
            **run_options(),
            resume_provider=True,
        )
        assert resume_only["run"].status == "provider_pending"
        assert len(provider_requests) == 0

        second = controller.run(
            task="resume a coding evidence run",
            **run_options(provider=True),
            resume_provider=True,
        )
        assert second["run"].status == "completed"
        assert second["run"].checkpoint.status == "completed"
        assert [item.status for item in checkpoint_store.history] == [
            "provider_pending",
            "provider_in_flight",
            "completed",
        ]
        pending, in_flight, completed = checkpoint_store.history
        assert [item.generation for item in checkpoint_store.history] == [1, 2, 3]
        assert in_flight.previous_checkpoint_digest == pending.checkpoint_digest
        assert completed.previous_checkpoint_digest == in_flight.checkpoint_digest
        assert in_flight.provider_operation_digest is not None
        assert completed.provider_operation_digest == in_flight.provider_operation_digest
        assert in_flight.provider_result_digest is None
        assert in_flight.provider_status is None
        assert completed.provider_result_digest is not None
        assert completed.provider_status == "completed"
        expected_provider_key = content_digest(
            {
                "schema": "bioprism-python-autonomous-evidence-backed-provider-idempotency/0.1",
                "provider_operation_digest": in_flight.provider_operation_digest,
            }
        )
        assert len(provider_requests) == 1
        expected_transport_key = _expected_resumable_transport_key(
            provider_operation_digest=in_flight.provider_operation_digest,
            provider="openai",
            request=provider_requests[0],
            incoming_scope=expected_provider_key,
        )
        assert provider_idempotency_keys == [expected_transport_key]
        assert acquisitions == len(requests), "source values must replay from the journal"
        provider_calls = len(provider_requests)
        assert provider_calls > 0
        provider_result = second["run"].result.execution
        assert provider_result is not None

        third = controller.run(
            task="resume a coding evidence run",
            **run_options(),
            rehydrate_provider_run=lambda _checkpoint, _probe: provider_result,
        )
        assert third["run"].status == "completed"
        assert third["run"].provider_rehydrated is True
        assert acquisitions == len(requests)
        assert len(provider_requests) == provider_calls
        assert (
            third["run"].checkpoint.checkpoint_digest
            == second["run"].checkpoint.checkpoint_digest
        )
        assert "resumable-caller-value" not in json.dumps(third["run"].to_dict())
        assert "resumable-caller-value" not in json.dumps(third["run"].checkpoint.to_dict())

        fourth = controller.run(
            task="resume a coding evidence run",
            **run_options(provider=True),
            resume_provider=True,
        )
        assert fourth["run"].status == "completed"
        assert fourth["run"].checkpoint.checkpoint_digest == completed.checkpoint_digest
        assert len(provider_requests) == provider_calls
        assert provider_idempotency_keys == [expected_transport_key]
    finally:
        pass


def test_resumable_source_approval_is_fresh_transition_authority():
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "source-approval-resume-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "source-approval-resume-job", store
    )
    acquirer = _ResumableFixtureAcquirer("source-approved-after-review")
    domains = ("science",)

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    common = {
        "task": "resume source acquisition after an explicit review hold",
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_provider_call": False,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        held = controller.run(
            **common,
            approve_source_dispatch=False,
        )["run"]
        assert held.status == "evidence_review_required"
        assert held.checkpoint.generation == 1
        assert acquirer.calls == 0
        assert getattr(server, "request_count", 0) == 0

        resumed = controller.run(
            **common,
            approve_source_dispatch=True,
        )["run"]
        assert resumed.status == "provider_pending"
        assert resumed.checkpoint.generation == 2
        assert (
            resumed.checkpoint.previous_checkpoint_digest
            == held.checkpoint.checkpoint_digest
        )
        assert resumed.checkpoint.run_policy_digest == held.checkpoint.run_policy_digest
        assert acquirer.calls > 0
        assert getattr(server, "request_count", 0) == 0
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_local_plan_refusal_never_marks_provider_attempted():
    runtime, credential_store, server, thread = _runtime()

    class RefusingWorkspace(_Workspace):
        def tool(
            self,
            name: str,
            arguments: dict[str, object] | None = None,
        ) -> dict[str, object]:
            if name == "brain_plan":
                self.calls.append((name, {} if arguments is None else dict(arguments)))
                return {"ok": False, "error": "fixture plan refusal"}
            return super().tool(name, arguments)

    agent = AutonomousAgent(
        RefusingWorkspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "local-plan-refusal-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "local-plan-refusal-job", store
    )
    acquirer = _ResumableFixtureAcquirer("local-plan-refusal-evidence")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    domains = ("science",)

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "task": "refuse locally before reaching provider transport",
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "rehydrate_value": value_rehydrator,
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "approve_provider_call": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        refused = controller.run(**options)["run"]
        assert refused.result.execution_status == "plan_refused"
        assert refused.status == "provider_pending"
        assert [checkpoint.status for checkpoint in store.history] == [
            "provider_pending"
        ]
        assert refused.checkpoint.provider_operation_digest is None
        assert getattr(server, "request_count", 0) == 0

        retained = controller.run(**options, resume_provider=True)["run"]
        assert retained.status == "provider_pending"
        assert retained.checkpoint.checkpoint_digest == refused.checkpoint.checkpoint_digest
        assert [checkpoint.status for checkpoint in store.history] == [
            "provider_pending"
        ]
        assert getattr(server, "request_count", 0) == 0
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


@pytest.mark.parametrize("failure_mode", ("open_circuit", "shadowed_dispatch"))
def test_resumable_pretransport_refusal_never_marks_provider_attempted(
    failure_mode: str,
):
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register(
        "openai",
        f"pretransport-{failure_mode}-test",
    )
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent,
        f"pretransport-{failure_mode}-job",
        store,
    )
    acquirer = _ResumableFixtureAcquirer(f"pretransport-{failure_mode}-evidence")
    original_post_once = runtime._post_once

    if failure_mode == "open_circuit":
        runtime._circuits["openai"].opened_until = runtime._clock() + 60.0
        expected_error: type[BaseException] = ProviderError
        expected_message = "provider circuit is open"
    else:
        def bypass_dispatch_fence(*_args: object, **_kwargs: object) -> object:
            raise AssertionError("shadowed dispatch primitive executed")

        runtime._post_once = bypass_dispatch_fence  # type: ignore[method-assign]
        expected_error = ArgumentError
        expected_message = "overridden provider dispatch-chain methods"

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        with pytest.raises(expected_error, match=expected_message):
            controller.run(
                task="refuse before crossing the provider transport boundary",
                requests=_requests(agent, ("science",)),
                acquirer=acquirer,
                projector=projector,
                evaluator=_ResumableFixtureEvaluator(),
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=("science",),
                run_mode="domain",
                approve_source_dispatch=True,
                approve_provider_call=True,
                resumable_policy_identity=_provider_policy_identity(),
            )
        assert store.history == []
        assert store.current is None
        assert getattr(server, "request_count", 0) == 0
    finally:
        runtime._post_once = original_post_once  # type: ignore[method-assign]
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_provider_crash_leaves_in_flight_and_restart_never_redispatches():
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "provider-crash-fence-test")
    store = _RecordingCheckpointStore()
    store.reject_terminal = True
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquirer = _ResumableFixtureAcquirer("crash-fence-value")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    domains = ("science",)

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "rehydrate_value": value_rehydrator,
        "journal": journal,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        controller = AutonomousEvidenceBackedController(
            agent, "provider-crash-fence-job", store
        )
        with pytest.raises(BrainRunError, match="compare-and-swap conflict"):
            controller.run(
                task="fence a provider result across a process crash",
                **options,
                approve_provider_call=True,
            )
        assert store.current is not None
        assert store.current.status == "provider_in_flight"
        assert store.current.generation == 1
        assert store.current.provider_operation_digest is not None
        assert store.current.provider_result_digest is None
        assert store.current.provider_status is None
        provider_calls = getattr(server, "request_count", 0)
        assert provider_calls == 1

        source_calls_before_missing_cas = acquirer.calls
        value_calls_before_missing_cas = value_rehydrator.calls
        with pytest.raises(
            ArgumentError,
            match="provider checkpoint transition requires checkpoint_compare_and_store",
        ):
            run_autonomous_evidence_backed_resumable(
                agent,
                task="fence a provider result across a process crash",
                job_id="provider-crash-fence-job",
                **options,
                checkpoint=store.current,
                checkpoint_sink=store.write,
                approve_provider_call=True,
                resume_provider=True,
            )
        assert acquirer.calls == source_calls_before_missing_cas
        assert value_rehydrator.calls == value_calls_before_missing_cas
        assert getattr(server, "request_count", 0) == provider_calls

        store.reject_terminal = False
        restarted = AutonomousEvidenceBackedController(
            agent, "provider-crash-fence-job", store
        )
        assert restarted.restore()["status"] == "restored"
        reconciled = restarted.run(
            task="fence a provider result across a process crash",
            **options,
            approve_provider_call=True,
            resume_provider=True,
        )
        assert reconciled["run"].status == "provider_reconciliation_required"
        assert reconciled["run"].checkpoint.generation == 2
        assert (
            reconciled["run"].checkpoint.previous_checkpoint_digest
            == store.history[0].checkpoint_digest
        )
        assert reconciled["run"].checkpoint.provider_result_digest is None
        assert reconciled["run"].checkpoint.provider_status is None
        assert getattr(server, "request_count", 0) == provider_calls

        provider_rehydrations = 0

        def forbidden_rehydration(
            _checkpoint: object,
            _probe: object,
        ) -> object:
            nonlocal provider_rehydrations
            provider_rehydrations += 1
            raise AssertionError("reconciliation ran without transactional persistence")

        source_calls_before_reconciliation_refusal = acquirer.calls
        value_calls_before_reconciliation_refusal = value_rehydrator.calls
        with pytest.raises(
            ArgumentError,
            match="provider checkpoint transition requires checkpoint_compare_and_store",
        ):
            run_autonomous_evidence_backed_resumable(
                agent,
                task="fence a provider result across a process crash",
                job_id="provider-crash-fence-job",
                **options,
                checkpoint=reconciled["run"].checkpoint,
                checkpoint_sink=store.write,
                approve_provider_call=True,
                resume_provider=True,
                rehydrate_provider_run=forbidden_rehydration,
            )
        assert provider_rehydrations == 0
        assert acquirer.calls == source_calls_before_reconciliation_refusal
        assert value_rehydrator.calls == value_calls_before_reconciliation_refusal
        assert getattr(server, "request_count", 0) == provider_calls

        retained = restarted.run(
            task="fence a provider result across a process crash",
            **options,
            approve_provider_call=True,
            resume_provider=True,
        )
        assert retained["run"].status == "provider_reconciliation_required"
        assert (
            retained["run"].checkpoint.checkpoint_digest
            == reconciled["run"].checkpoint.checkpoint_digest
        )
        assert getattr(server, "request_count", 0) == provider_calls

        observed_provider_failure = BrainRunResult(
            run_id="rehydrated-provider-failure",
            status="failed",
            selection={"provider": "openai", "model": "gpt-5-mini"},
            prompt={"prompt_digest": "a" * 64},
            plan={"plan": {"plan_digest": "b" * 64}},
            response=None,
            outcome_digest="c" * 64,
            failure={"failure_class": "provider_error", "retryable": False},
        )

        observed = restarted.run(
            task="fence a provider result across a process crash",
            **options,
            approve_provider_call=True,
            resume_provider=True,
            rehydrate_provider_run=lambda _checkpoint, _probe: observed_provider_failure,
        )
        assert observed["run"].status == "provider_reconciliation_required"
        assert observed["run"].provider_rehydrated is True
        assert observed["run"].checkpoint.generation == 3
        assert observed["run"].checkpoint.provider_result_digest is not None
        assert observed["run"].checkpoint.provider_status == "failed"
        assert (
            observed["run"].checkpoint.provider_operation_digest
            == reconciled["run"].checkpoint.provider_operation_digest
        )
        assert getattr(server, "request_count", 0) == provider_calls

        observed_retained = restarted.run(
            task="fence a provider result across a process crash",
            **options,
            approve_provider_call=True,
            resume_provider=True,
        )
        assert (
            observed_retained["run"].checkpoint.checkpoint_digest
            == observed["run"].checkpoint.checkpoint_digest
        )
        assert getattr(server, "request_count", 0) == provider_calls
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_incomplete_evidence_crash_restores_to_unknown_reconciliation():
    store = _RecordingCheckpointStore()
    provider_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        provider_requests.append(request)
        store.force_conflict = True
        return {
            "text": "bounded incomplete-evidence answer",
            "request_id": "incomplete-evidence-response",
            "usage": {"total_tokens": 4},
        }

    runtime, credential_store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register(
        "openai",
        "provider-incomplete-crash-fence-test",
    )
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquirer = _ResumableFixtureAcquirer("incomplete-crash-fence-value")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    domains = ("science",)
    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "rehydrate_value": value_rehydrator,
        "journal": journal,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "allow_incomplete_evidence": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        controller = AutonomousEvidenceBackedController(
            agent,
            "provider-incomplete-crash-fence-job",
            store,
        )
        with pytest.raises(BrainRunError, match="compare-and-swap conflict"):
            controller.run(
                task="quarantine a provider answer based on incomplete evidence",
                **options,
                approve_provider_call=True,
            )
        assert store.current is not None
        assert store.current.status == "provider_in_flight"
        assert store.current.evidence_result_digest is not None
        assert store.current.provider_operation_digest is not None
        provider_calls = len(provider_requests)
        assert provider_calls == 1

        store.force_conflict = False
        restarted = AutonomousEvidenceBackedController(
            agent,
            "provider-incomplete-crash-fence-job",
            store,
        )
        assert restarted.restore()["status"] == "restored"
        reconciled = restarted.run(
            task="quarantine a provider answer based on incomplete evidence",
            **options,
            approve_provider_call=True,
            resume_provider=True,
        )
        assert reconciled["run"].status == "provider_reconciliation_required"
        assert reconciled["run"].result.evidence is not None
        assert reconciled["run"].result.evidence.status == "awaiting_evaluation"
        assert reconciled["run"].checkpoint.generation == 2
        assert reconciled["run"].checkpoint.provider_result_digest is None
        assert reconciled["run"].checkpoint.provider_status is None
        assert len(provider_requests) == provider_calls
    finally:
        pass


def test_resumable_provider_requires_successful_cas_but_plain_store_keeps_safe_pending():
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "provider-cas-refusal-test")
    domains = ("science",)
    requests = _requests(agent, domains)
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquirer = _ResumableFixtureAcquirer("cas-refusal-value")

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "requests": requests,
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _AcceptAllEvidence(),
        "journal": journal,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    class PlainStore:
        value: dict[str, object] | None = None

        def read(self) -> dict[str, object] | None:
            return self.value

        def write(self, checkpoint: object) -> None:
            self.value = validate_autonomous_evidence_backed_checkpoint(  # type: ignore[arg-type]
                checkpoint
            ).to_dict()

    try:
        conflict_store = _RecordingCheckpointStore()
        conflict_store.force_conflict = True
        conflict = AutonomousEvidenceBackedController(
            agent, "provider-cas-conflict-job", conflict_store
        )
        with pytest.raises(BrainRunError, match="compare-and-swap conflict"):
            conflict.run(
                task="do not dispatch through a stale provider head",
                **options,
                approve_provider_call=True,
            )
        assert conflict_store.current is None
        assert getattr(server, "request_count", 0) == 0

        plain_store = PlainStore()
        plain = AutonomousEvidenceBackedController(
            agent, "provider-plain-pending-job", plain_store
        )
        plain_options = {
            **options,
            "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        }
        pending = plain.run(
            task="persist a safe provider preapproval",
            **plain_options,
            approve_provider_call=False,
        )
        assert pending["run"].status == "provider_pending"
        calls_before_refusal = acquirer.calls
        with pytest.raises(ArgumentError, match="checkpoint_compare_and_store"):
            plain.run(
                task="persist a safe provider preapproval",
                **plain_options,
                approve_provider_call=True,
                resume_provider=True,
            )
        assert acquirer.calls == calls_before_refusal
        assert getattr(server, "request_count", 0) == 0

        fresh_plain = AutonomousEvidenceBackedController(
            agent, "provider-plain-fresh-job", PlainStore()
        )
        fresh_plain_options = {
            **options,
            "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        }
        calls_before_fresh_refusal = acquirer.calls
        with pytest.raises(ArgumentError, match="checkpoint_compare_and_store"):
            fresh_plain.run(
                task="refuse an unfenced fresh provider dispatch",
                **fresh_plain_options,
                approve_provider_call=True,
            )
        assert acquirer.calls == calls_before_fresh_refusal
        assert getattr(server, "request_count", 0) == 0
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


@pytest.mark.parametrize(
    ("run_mode", "domains", "task"),
    (
        ("domain", ("coding",), "implement a bounded coding change"),
        ("auto", ("science",), "analyze a scientific research hypothesis"),
        (
            "cross_domain",
            ("coding", "science"),
            "reconcile coding and scientific research evidence",
        ),
    ),
)
def test_resumable_provider_key_reaches_domain_auto_and_cross_domain_transport(
    run_mode: str,
    domains: tuple[str, ...],
    task: str,
) -> None:
    observed_keys: list[str | None] = []
    observed_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        observed_keys.append(request.idempotency_key)
        observed_requests.append(request)
        return {
            "text": "bounded answer",
            "request_id": f"provider-key-{len(observed_requests)}",
            "usage": {"total_tokens": 4},
        }

    runtime, credential_store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", f"provider-key-{run_mode}-test")
    checkpoint_store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, f"provider-key-{run_mode}-job", checkpoint_store
    )
    acquirer = _ResumableFixtureAcquirer(f"provider-key-{run_mode}-value")

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    run_options = None
    if run_mode == "auto":
        run_options = {
            "route_override": agent.route(
                task=task,
                hints=("science",),
                max_domains=1,
                allow_cross_domain=False,
            )
        }
    elif run_mode == "cross_domain":
        run_options = {"max_parallelism": 2}

    try:
        result = controller.run(
            task=task,
            requests=_requests(agent, domains),
            acquirer=acquirer,
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials={"openai": handle},
            model_candidates=_model(),
            domains=domains,
            run_mode=run_mode,
            run_options=run_options,
            resumable_policy_identity=_provider_policy_identity(),
            approve_source_dispatch=True,
            approve_provider_call=True,
        )
        assert result["run"].status == "completed"
        assert [item.status for item in checkpoint_store.history] == [
            *(["provider_in_flight"] * len(observed_requests)),
            "completed",
        ]
        assert len(checkpoint_store.dispatch_receipts) == len(observed_requests)
        assert [receipt.dispatch_index for receipt in checkpoint_store.dispatch_receipts] == list(
            range(1, len(observed_requests) + 1)
        )
        previous_receipt_digest = None
        for receipt in checkpoint_store.dispatch_receipts:
            assert (
                receipt.previous_provider_dispatch_head_digest
                == previous_receipt_digest
            )
            assert "provider_idempotency_key" not in receipt.to_dict()
            previous_receipt_digest = receipt.receipt_digest
        assert result["run"].checkpoint.provider_dispatch_count == len(
            observed_requests
        )
        assert (
            result["run"].checkpoint.provider_dispatch_head_digest
            == previous_receipt_digest
        )
        operation_digest = checkpoint_store.history[0].provider_operation_digest
        assert operation_digest is not None
        root_key = content_digest(
            {
                "schema": "bioprism-python-autonomous-evidence-backed-provider-idempotency/0.1",
                "provider_operation_digest": operation_digest,
            }
        )
        if run_mode in {"domain", "auto"}:
            assert len(observed_requests) == 1
            request = observed_requests[0]
            expected_key = _expected_resumable_transport_key(
                provider_operation_digest=operation_digest,
                provider="openai",
                request=request,
                incoming_scope=root_key,
            )
            assert observed_keys == [expected_key]
            assert _expected_resumable_transport_key(
                provider_operation_digest=operation_digest,
                provider="openai",
                request=replace(request, model=f"{request.model}-changed"),
                incoming_scope=root_key,
            ) != expected_key
            assert _expected_resumable_transport_key(
                provider_operation_digest=operation_digest,
                provider="openai",
                request=replace(
                    request,
                    messages=(
                        *request.messages,
                        {"role": "user", "content": "changed actual request"},
                    ),
                ),
                incoming_scope=root_key,
            ) != expected_key
        else:
            incoming_scopes = {
                "cross-key-"
                + content_digest({"parent": root_key, "child": child_id})
                for child_id in (
                    *(f"evidence-{domain}" for domain in domains),
                    "synthesis",
                )
            }
            assert len(observed_requests) == len(incoming_scopes)
            unmatched_scopes = set(incoming_scopes)
            for request, observed_key in zip(observed_requests, observed_keys):
                matches = {
                    scope
                    for scope in unmatched_scopes
                    if _expected_resumable_transport_key(
                        provider_operation_digest=operation_digest,
                        provider="openai",
                        request=request,
                        incoming_scope=scope,
                    )
                    == observed_key
                }
                assert len(matches) == 1
                matched_scope = next(iter(matches))
                assert _expected_resumable_transport_key(
                    provider_operation_digest=operation_digest,
                    provider="openai",
                    request=replace(request, model=f"{request.model}-changed"),
                    incoming_scope=matched_scope,
                ) != observed_key
                unmatched_scopes.difference_update(matches)
            assert not unmatched_scopes
            assert len(set(observed_keys)) == len(observed_keys)
    finally:
        pass


def test_resumable_fence_is_final_in_shared_observer_composition():
    observed_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        observed_requests.append(request)
        return {
            "text": "bounded observer answer",
            "request_id": "observer-key-fence-response",
            "usage": {"total_tokens": 4},
        }

    runtime, credential_store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(
        _Workspace(),
        runtime,
        model_catalogue=ModelCatalogue(_model()),
        # Forces Agent -> orchestrator -> brain observer composition to add its
        # execution-policy observer around the one resumable private fence.
        execution_policy={},
    )
    handle = credential_store.register("openai", "observer-key-fence-test")
    checkpoint_store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "observer-key-fence-job", checkpoint_store
    )
    acquirer = _ResumableFixtureAcquirer("observer-key-fence-evidence")
    prepare_calls: list[str | None] = []
    transport_metadata: list[object] = []

    class OverridingObserver:
        def before(self, _metadata: object) -> None:
            return None

        def after(
            self,
            _metadata: object,
            _response: object,
            _error: object,
            _latency_ms: float,
        ) -> None:
            return None

        def before_transport(
            self,
            metadata: object,
        ) -> None:
            transport_metadata.append(metadata)
            assert not hasattr(metadata, "provider_idempotency_key")
            object.__setattr__(metadata, "kind", "forged-observer-kind")

        def prepare_dispatch(
            self,
            _provider: str,
            request: ProviderRequest,
        ) -> ProviderRequest:
            prepare_calls.append(request.idempotency_key)
            raise AssertionError("ordinary observer received the provider request")

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        result = controller.run(
            task="keep the resumable provider fence last in observer composition",
            requests=_requests(agent, ("science",)),
            acquirer=acquirer,
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials={"openai": handle},
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            run_options={"invocation_observer": OverridingObserver()},
            approve_source_dispatch=True,
            approve_provider_call=True,
            resumable_policy_identity=_provider_policy_identity(),
        )
        assert result["run"].status == "completed"
        assert len(observed_requests) == 1
        in_flight = checkpoint_store.history[0]
        assert in_flight.status == "provider_in_flight"
        assert in_flight.provider_operation_digest is not None
        root_key = content_digest(
            {
                "schema": "bioprism-python-autonomous-evidence-backed-provider-idempotency/0.1",
                "provider_operation_digest": in_flight.provider_operation_digest,
            }
        )
        expected = _expected_resumable_transport_key(
            provider_operation_digest=in_flight.provider_operation_digest,
            provider="openai",
            request=observed_requests[0],
            incoming_scope=root_key,
        )
        assert prepare_calls == []
        assert len(transport_metadata) == 1
        assert observed_requests[0].idempotency_key == expected
        assert checkpoint_store.dispatch_receipts[0].provider_idempotency_key == expected
        assert checkpoint_store.dispatch_receipts[0].invocation_kind != "forged-observer-kind"
    finally:
        pass


def test_resumable_snapshots_provider_inputs_before_acquisition_mutates_callers():
    candidates = _model()
    observed_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        observed_requests.append(request)
        return {
            "text": '{"snapshot_before":"ok"}',
            "request_id": "provider-input-snapshot-response",
            "usage": {"total_tokens": 1},
            "structured": {"snapshot_before": "ok"},
        }

    runtime, credential_store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(candidates)
    )
    handle = credential_store.register("openai", "provider-input-snapshot-test")
    credential_input = {"openai": handle}
    store = _RecordingCheckpointStore()
    domains = ("science",)
    requests = _requests(agent, domains)
    available_evidence: list[str] = []
    completed_stages: dict[str, list[str]] = {}
    parent_evidence_digests: list[str] = []
    run_options: dict[str, object] = {
        "max_output_tokens": 777,
        "trace_event_callback": lambda **_event: None,
        "response_schema": {
            "type": "object",
            "properties": {"snapshot_before": {"type": "string"}},
        },
    }
    observed_parent_digests: list[tuple[str, ...]] = []
    expected_plan_digest = agent.evidence_plan(
        domains,
        available_evidence=(),
        completed_stages={},
    ).plan_digest

    def mutating_acquirer(context: object) -> dict[str, str]:
        observed_parent_digests.append(
            tuple(context["parent_evidence_digests"])  # type: ignore[index]
        )
        candidates[0]["model"] = "mutated-after-snapshot"
        run_options["max_output_tokens"] = 1
        run_options["response_schema"]["properties"] = {  # type: ignore[index]
            "mutated_after_snapshot": {"type": "string"}
        }
        if not available_evidence:
            available_evidence.append("mutated-after-snapshot")
        if not completed_stages:
            completed_stages["science"] = ["not-a-real-stage"]
        if not parent_evidence_digests:
            parent_evidence_digests.append("a" * 64)
        credential_input.clear()
        requirement = context["requirement"]  # type: ignore[index]
        return {"requirement": requirement.requirement_id}

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        result = run_autonomous_evidence_backed_resumable(
            agent,
            task="snapshot provider-bound inputs before any caller callback",
            job_id="provider-input-snapshot-job",
            requests=requests,
            acquirer=mutating_acquirer,
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials=credential_input,
            model_candidates=candidates,
            domains=domains,
            run_mode="domain",
            run_options=run_options,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            parent_evidence_digests=parent_evidence_digests,
            resumable_policy_identity={
                **_provider_policy_identity(),
                "acquirer": {
                    "id": "mutating-input-snapshot-acquirer",
                    "version": "v1",
                    "config_digest": content_digest(
                        {"mutation": "model-and-nested-run-options"}
                    ),
                }
            },
            approve_source_dispatch=True,
            approve_provider_call=True,
            checkpoint_sink=store.write,
            checkpoint_compare_and_store=store.write_if_unchanged,
            checkpoint_dispatch_compare_and_store=(
                store.write_dispatch_if_unchanged
            ),
        )
        assert result.status == "completed"
        assert len(observed_requests) == 1
        request = observed_requests[0]
        assert request.model == "test-model"  # type: ignore[attr-defined]
        assert request.max_output_tokens == 777  # type: ignore[attr-defined]
        serialized_schema = json.dumps(request.response_schema)  # type: ignore[attr-defined]
        assert "snapshot_before" in serialized_schema
        assert "mutated_after_snapshot" not in serialized_schema
        assert result.checkpoint.evidence_plan_digest == expected_plan_digest
        assert observed_parent_digests
        assert all(value == () for value in observed_parent_digests)
        assert candidates[0]["model"] == "mutated-after-snapshot"
        assert run_options["max_output_tokens"] == 1
        assert available_evidence == ["mutated-after-snapshot"]
        assert completed_stages == {"science": ["not-a-real-stage"]}
        assert parent_evidence_digests == ["a" * 64]
        assert credential_input == {}
    finally:
        pass


def test_resumable_rejects_complete_provider_envelope_tampering(
    monkeypatch: pytest.MonkeyPatch,
):
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "provider-envelope-tamper-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "provider-envelope-tamper-job", store
    )
    acquirer = _ResumableFixtureAcquirer("provider-envelope-evidence")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    domains = ("science",)

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "task": "bind every material provider response field",
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "rehydrate_value": value_rehydrator,
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "approve_provider_call": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        completed = controller.run(**options)["run"]
        assert completed.status == "completed"
        execution = completed.result.execution
        response = execution.response  # type: ignore[union-attr]
        assert isinstance(response, ProviderResponse)
        provider_calls = getattr(server, "request_count", 0)
        assert provider_calls == 1

        mutations = (
            replace(response, status_code=response.status_code + 1),
            replace(response, structured={"tampered": True}),
            replace(
                response,
                tool_calls=(
                    ProviderToolCall(
                        "tampered-call",
                        "tampered_tool",
                        {"value": "changed"},
                    ),
                ),
            ),
            replace(
                response,
                provider_output_items=(
                    {"type": "reasoning", "id": "tampered-output-item"},
                ),
            ),
            replace(response, raw={**response.raw, "tampered": True}),
        )
        for mutated_response in mutations:
            recovered = replace(execution, response=mutated_response)
            with pytest.raises(
                BrainRunError,
                match="rehydrated provider result does not match its checkpoint digest",
            ):
                controller.run(
                    **options,
                    rehydrate_provider_run=(
                        lambda _checkpoint, _probe, value=recovered: value
                    ),
                )
            assert getattr(server, "request_count", 0) == provider_calls
            assert store.current is not None
            assert (
                store.current.checkpoint_digest
                == completed.checkpoint.checkpoint_digest
            )

        forged_execution = replace(execution, response=mutations[0])
        forged_digest = (
            evidence_brain_module._provider_execution_envelope_digest(
                forged_execution
            )
        )

        def mutate_callback_checkpoint(
            callback_checkpoint: object,
            _probe: object,
        ) -> object:
            object.__setattr__(
                callback_checkpoint,
                "provider_result_digest",
                forged_digest,
            )
            return forged_execution

        with pytest.raises(
            BrainRunError,
            match="rehydrated provider result does not match its checkpoint digest",
        ):
            controller.run(
                **options,
                rehydrate_provider_run=mutate_callback_checkpoint,
            )
        assert store.current is not None
        assert (
            store.current.checkpoint_digest
            == completed.checkpoint.checkpoint_digest
        )

        baseline_probe = controller.run(**options)["run"]

        def mutate_callback_probe(
            _callback_checkpoint: object,
            callback_probe: object,
        ) -> None:
            object.__setattr__(callback_probe, "status", "completed")
            object.__setattr__(callback_probe, "execution_digest", "0" * 64)
            object.__setattr__(callback_probe, "result_digest", "1" * 64)
            return None

        protected_probe = controller.run(
            **options,
            rehydrate_provider_run=mutate_callback_probe,
        )["run"]
        assert protected_probe.status == "completed"
        assert protected_probe.provider_rehydrated is False
        assert protected_probe.result.status == baseline_probe.result.status
        assert (
            protected_probe.result.execution_digest
            == baseline_probe.result.execution_digest
        )
        assert protected_probe.result.result_digest == baseline_probe.result.result_digest

        bounded_failures = (
            (
                replace(response, text="x" * 2_000_001),
                "string outside its byte bound",
            ),
            (
                replace(
                    response,
                    usage={**response.usage, "huge": 1 << 4_097},
                ),
                "integer outside its scalar bound",
            ),
            (
                replace(
                    response,
                    raw={"chunks": ["x" * 1_000 for _ in range(2_100)]},
                ),
                "bounded serialized size",
            ),
        )
        for bounded_response, message in bounded_failures:
            recovered = replace(execution, response=bounded_response)
            with pytest.raises(BrainRunError, match=message):
                controller.run(
                    **options,
                    rehydrate_provider_run=(
                        lambda _checkpoint, _probe, value=recovered: value
                    ),
                )
            assert getattr(server, "request_count", 0) == provider_calls

        mutable_raw = dict(response.raw)
        recovered = replace(
            execution,
            response=replace(response, raw=mutable_raw),
        )
        rehydrated = controller.run(
            **options,
            rehydrate_provider_run=lambda _checkpoint, _probe: recovered,
        )["run"]
        returned_response = rehydrated.result.execution.response  # type: ignore[union-attr]
        assert returned_response.raw == mutable_raw
        assert returned_response.raw is not mutable_raw
        mutable_raw["changed_after_rehydration"] = True
        assert "changed_after_rehydration" not in returned_response.raw

        with monkeypatch.context() as patcher:
            patcher.setattr(brain_module, "BrainRunResult", object)
            with pytest.raises(BrainRunError, match="modified or rebound SDK dataclass"):
                controller.run(
                    **options,
                    rehydrate_provider_run=lambda _checkpoint, _probe: execution,
                )
        with monkeypatch.context() as patcher:
            patcher.setattr(llm_runtime_module, "ProviderResponse", object)
            with pytest.raises(BrainRunError, match="modified or rebound SDK dataclass"):
                controller.run(
                    **options,
                    rehydrate_provider_run=lambda _checkpoint, _probe: execution,
                )
        with monkeypatch.context() as patcher:
            def mutate_result_projection_helper(
                _checkpoint: object,
                _probe: object,
            ) -> object:
                patcher.setattr(
                    evidence_brain_module,
                    "_provider_execution_projection",
                    lambda _value, **_kwargs: {
                        "kind": "dict",
                        "items": {"substituted": True},
                    },
                )
                return execution

            with pytest.raises(
                ArgumentError,
                match="modified dispatch-fence composition",
            ):
                controller.run(
                    **options,
                    rehydrate_provider_run=mutate_result_projection_helper,
                )
        assert getattr(server, "request_count", 0) == provider_calls
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_post_transport_response_review_binds_result_before_rehydration():
    candidates = _model()
    candidates[0]["capabilities"] = [
        *candidates[0]["capabilities"],
        "structured_output",
    ]
    transport_requests: list[ProviderRequest] = []

    def provider_handler(request: ProviderRequest) -> dict[str, object]:
        transport_requests.append(request)
        value = _structured_value_from_schema(request.response_schema)
        assert isinstance(value, dict)
        return {
            "text": json.dumps(value),
            "request_id": f"response-review-{len(transport_requests)}",
            "usage": {"total_tokens": 4},
            "structured": value,
        }

    runtime, credential_store = _in_memory_runtime(provider_handler)
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(candidates)
    )
    handle = credential_store.register("openai", "response-review-fence-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "response-review-fence-job", store
    )
    acquirer = _ResumableFixtureAcquirer("response-review-evidence")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "task": "retain post-transport specialist responses for alignment review",
        "requests": _requests(agent, ("coding", "science")),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "rehydrate_value": value_rehydrator,
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": candidates,
        "domains": ("coding", "science"),
        "run_mode": "cross_domain",
        "run_options": {
            "structured_domain_response": True,
            "require_response_alignment": True,
            "max_parallelism": 2,
        },
        "approve_source_dispatch": True,
        "approve_provider_call": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        reviewed = controller.run(**options)["run"]
        assert reviewed.status == "provider_reconciliation_required"
        assert reviewed.result.execution_status == "response_review_required"
        assert reviewed.checkpoint.provider_status == "response_review_required"
        assert reviewed.checkpoint.provider_result_digest is not None
        assert [checkpoint.status for checkpoint in store.history] == [
            "provider_in_flight",
            "provider_in_flight",
            "provider_reconciliation_required",
        ]
        assert len(transport_requests) == 2

        exact = controller.run(
            **options,
            rehydrate_provider_run=lambda _checkpoint, _probe: reviewed.result.execution,
        )["run"]
        assert exact.status == "provider_reconciliation_required"
        assert exact.provider_rehydrated is True
        assert exact.checkpoint.provider_result_digest == reviewed.checkpoint.provider_result_digest
        assert len(transport_requests) == 2

        execution = reviewed.result.execution
        first_child = execution.child_results[0]  # type: ignore[union-attr]
        response = first_child.response
        assert isinstance(response, ProviderResponse)
        tampered = replace(
            execution,
            child_results=(
                replace(
                    first_child,
                    response=replace(response, structured={"tampered": True}),
                ),
                *execution.child_results[1:],  # type: ignore[union-attr]
            ),
        )
        with pytest.raises(
            BrainRunError,
            match="rehydrated provider result does not match its checkpoint digest",
        ):
            controller.run(
                **options,
                rehydrate_provider_run=lambda _checkpoint, _probe: tampered,
            )
        assert len(transport_requests) == 2
    finally:
        pass


def test_resumable_cross_domain_digest_binds_every_child_response():
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "cross-envelope-tamper-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent, "cross-envelope-tamper-job", store
    )
    acquirer = _ResumableFixtureAcquirer("cross-envelope-evidence")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    domains = ("coding", "science")

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    options = {
        "task": "bind every specialist response in a cross-domain result",
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "rehydrate_value": value_rehydrator,
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "cross_domain",
        "run_options": {"max_parallelism": 2},
        "approve_source_dispatch": True,
        "approve_provider_call": True,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        completed = controller.run(**options)["run"]
        assert completed.status == "completed"
        execution = completed.result.execution
        first_child = execution.child_results[0]  # type: ignore[union-attr]
        first_response = first_child.response
        assert isinstance(first_response, ProviderResponse)
        mutated_child = replace(
            first_child,
            response=replace(first_response, structured={"earlier_child": "changed"}),
        )
        recovered = replace(
            execution,
            child_results=(
                mutated_child,
                *execution.child_results[1:],  # type: ignore[union-attr]
            ),
        )
        provider_calls = getattr(server, "request_count", 0)

        with pytest.raises(
            BrainRunError,
            match="rehydrated provider result does not match its checkpoint digest",
        ):
            controller.run(
                **options,
                rehydrate_provider_run=lambda _checkpoint, _probe: recovered,
            )
        assert getattr(server, "request_count", 0) == provider_calls
        assert store.current is not None
        assert store.current.checkpoint_digest == completed.checkpoint.checkpoint_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_rejects_prompt_projection_drift_before_provider_rehydration():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-prompt-drift-test")
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquirer = _ResumableFixtureAcquirer("stable-evidence")
    value_rehydrator = _ResumableFixtureValueRehydrator(acquirer.values)
    evaluator = _ResumableFixtureEvaluator()
    domains = ("science",)
    requests = _requests(agent, domains)
    checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
    provider_rehydrations = 0

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def recover_provider(
        _checkpoint: AutonomousEvidenceBackedCheckpoint, _probe: object
    ) -> object:
        nonlocal provider_rehydrations
        provider_rehydrations += 1
        return first.result.execution

    common = {
        "task": "resume only the same reviewed science prompt",
        "job_id": "resumable-prompt-drift-job",
        "requests": requests,
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": evaluator,
        "rehydrate_value": value_rehydrator,
        "journal": journal,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "resumable_policy_identity": _provider_policy_identity(),
        "checkpoint_sink": checkpoint_store.write,
        "checkpoint_compare_and_store": checkpoint_store.write_if_unchanged,
        "checkpoint_dispatch_compare_and_store": (
            checkpoint_store.write_dispatch_if_unchanged
        ),
    }

    try:
        first = agent.run_resumable_evidence_backed(
            **common,
            prompt_builder=_ResumableFixturePromptBuilder("marker-a"),
            approve_provider_call=True,
        )
        provider_calls = getattr(server, "request_count", 0)
        assert first.status == "completed"
        assert provider_calls > 0

        with pytest.raises(BrainRunError, match="prompt projection"):
            agent.run_resumable_evidence_backed(
                **common,
                checkpoint=first.checkpoint,
                prompt_builder=_ResumableFixturePromptBuilder("marker-b"),
                approve_provider_call=False,
                rehydrate_provider_run=recover_provider,
            )
        assert provider_rehydrations == 0
        assert getattr(server, "request_count", 0) == provider_calls
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_rejects_changed_evidence_before_provider_rehydration():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-evidence-drift-test")
    acquirer = _ResumableFixtureAcquirer("evidence-a")
    evaluator = _ResumableFixtureEvaluator()
    domains = ("science",)
    requests = _requests(agent, domains)
    checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
    provider_rehydrations = 0

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def recover_provider(
        _checkpoint: AutonomousEvidenceBackedCheckpoint, _probe: object
    ) -> object:
        nonlocal provider_rehydrations
        provider_rehydrations += 1
        return first.result.execution

    common = {
        "task": "resume only the same reviewed science evidence",
        "job_id": "resumable-evidence-drift-job",
        "requests": requests,
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": evaluator,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "resumable_policy_identity": _provider_policy_identity(),
        "checkpoint_sink": checkpoint_store.write,
        "checkpoint_compare_and_store": checkpoint_store.write_if_unchanged,
        "checkpoint_dispatch_compare_and_store": (
            checkpoint_store.write_dispatch_if_unchanged
        ),
    }

    try:
        first = agent.run_resumable_evidence_backed(
            **common,
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            approve_provider_call=True,
        )
        provider_calls = getattr(server, "request_count", 0)
        assert first.status == "completed"

        acquirer.marker = "evidence-b"
        with pytest.raises(BrainRunError, match="evidence result"):
            agent.run_resumable_evidence_backed(
                **common,
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                checkpoint=first.checkpoint,
                approve_provider_call=False,
                rehydrate_provider_run=recover_provider,
            )
        assert provider_rehydrations == 0
        assert getattr(server, "request_count", 0) == provider_calls
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_binds_evaluator_version_before_value_or_provider_rehydration():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-evaluator-drift-test")
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquirer = _ResumableFixtureAcquirer("stable-evidence")
    evaluator = _ResumableFixtureEvaluator()
    domains = ("science",)
    requests = _requests(agent, domains)
    checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
    value_rehydrations = 0
    provider_rehydrations = 0
    recovery_identity = {
        **_provider_policy_identity(),
        "value_rehydrator": {
            "id": "reserved-fixture-value-rehydrator",
            "version": "v1",
            "config_digest": content_digest({"source": "fixture-memory"}),
        }
    }

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def rehydrate_value(receipt: object) -> object | None:
        nonlocal value_rehydrations
        value_rehydrations += 1
        return acquirer.values.get(receipt["value_digest"])  # type: ignore[index]

    def recover_provider(
        _checkpoint: AutonomousEvidenceBackedCheckpoint, _probe: object
    ) -> object:
        nonlocal provider_rehydrations
        provider_rehydrations += 1
        return first.result.execution

    common = {
        "task": "bind the evaluator before restoring any result",
        "job_id": "resumable-evaluator-drift-job",
        "requests": requests,
        "acquirer": acquirer,
        "projector": projector,
        "journal": journal,
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "checkpoint_sink": checkpoint_store.write,
        "checkpoint_compare_and_store": checkpoint_store.write_if_unchanged,
        "checkpoint_dispatch_compare_and_store": (
            checkpoint_store.write_dispatch_if_unchanged
        ),
        "resumable_policy_identity": recovery_identity,
    }

    try:
        first = agent.run_resumable_evidence_backed(
            **common,
            evaluator=evaluator,
            rehydrate_value=None,
            approve_provider_call=True,
        )
        provider_calls = getattr(server, "request_count", 0)
        assert first.status == "completed"

        evaluator.evaluator_version = "v2"
        with pytest.raises(ArgumentError, match="checkpoint does not match"):
            agent.run_resumable_evidence_backed(
                **common,
                evaluator=evaluator,
                rehydrate_value=rehydrate_value,
                checkpoint=first.checkpoint,
                approve_provider_call=False,
                rehydrate_provider_run=recover_provider,
            )
        assert value_rehydrations == 0
        assert provider_rehydrations == 0
        assert getattr(server, "request_count", 0) == provider_calls
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_evidence_checkpoint_is_tamper_evident_and_covers_every_domain():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-all-domain-test")

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        return {"requirement": requirement.requirement_id}

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        saved_checkpoint: dict[str, object] | None = None
        for domain in AUTONOMOUS_DOMAINS:
            checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
            controller = AutonomousEvidenceBackedController(agent, f"job-{domain}", checkpoint_store)
            journal = InMemoryAutonomousEvidenceRuntimeJournal()
            result = controller.run(
                task=f"prepare a resumable {domain} review",
                requests=_requests(agent, (domain,)),
                acquirer=acquirer,
                projector=projector,
                evaluator=_AcceptAllEvidence(),
                journal=journal,
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=(domain,),
                run_mode="domain",
                approve_source_dispatch=True,
                approve_provider_call=False,
                resumable_policy_identity=_provider_policy_identity(),
            )
            assert result["run"].status == "provider_pending", domain
            assert result["run"].checkpoint.evidence_plan_digest == agent.evidence_plan((domain,)).plan_digest
            if saved_checkpoint is None:
                saved_checkpoint = result["run"].checkpoint.to_dict()

        assert saved_checkpoint is not None
        tampered = dict(saved_checkpoint)
        tampered["status"] = "completed"
        with pytest.raises(Exception, match="attempted predecessor|provider-bound checkpoint"):
            validate_autonomous_evidence_backed_checkpoint(tampered)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_evidence_json_and_cas_persistence_are_canonical_and_fenced():
    class TextStore:
        def __init__(self) -> None:
            self.value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            observed = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
            if observed != expected:
                return False
            self.value = value
            return True

        def write_dispatch_if_unchanged(
            self,
            expected: str | None,
            checkpoint_value: str,
            _private_receipt_value: str,
        ) -> bool:
            observed = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
            if observed != expected:
                return False
            self.value = checkpoint_value
            return True

    checkpoint = AutonomousEvidenceBackedCheckpoint(
        job_id="json-persistence-job",
        task_digest="a" * 64,
        request_digest="b" * 64,
        run_policy_digest="c" * 64,
        evidence_plan_digest="d" * 64,
        execution_plan_digest="e" * 64,
        evidence_result_digest="f" * 64,
        prompt_projection_digest="0" * 64,
        provider_operation_digest=None,
        provider_dispatch_count=0,
        provider_dispatch_head_digest=None,
        provider_result_digest=None,
        provider_status=None,
        status="provider_pending",
        generation=1,
        previous_checkpoint_digest=None,
    )
    plain_store = TextStore()
    persistence = JsonAutonomousEvidenceBackedCheckpointPersistence(plain_store)
    persistence.write(checkpoint)
    assert persistence.read() == checkpoint.to_dict()

    cas_store = TextStore()
    cas = TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(cas_store)
    assert cas.write_if_unchanged(None, checkpoint.to_dict()) is True
    assert cas.write_if_unchanged(None, checkpoint.to_dict()) is False
    with pytest.raises(BrainRunError, match="transition"):
        cas.write_if_unchanged(checkpoint.checkpoint_digest, checkpoint.to_dict())

    tampered = dict(checkpoint.to_dict())
    tampered["status"] = "completed"
    with pytest.raises(ArgumentError):
        validate_autonomous_evidence_backed_checkpoint(tampered)


def test_resumable_checkpoint_v04_enforces_exact_lineage_and_provider_state_tuples():
    fields = {
        "job_id": "provider-state-invariants-job",
        "task_digest": "a" * 64,
        "request_digest": "b" * 64,
        "run_policy_digest": "c" * 64,
        "evidence_plan_digest": "d" * 64,
        "execution_plan_digest": "e" * 64,
        "evidence_result_digest": "f" * 64,
        "prompt_projection_digest": "0" * 64,
    }
    operation_digest = content_digest(
        {
            "schema": "bioprism-python-autonomous-evidence-backed-provider-operation/0.1",
            **fields,
        }
    )
    in_flight = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=1,
        provider_dispatch_head_digest="9" * 64,
        provider_result_digest=None,
        provider_status=None,
        status="provider_in_flight",
        generation=1,
        previous_checkpoint_digest=None,
    )
    unknown = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=1,
        provider_dispatch_head_digest="9" * 64,
        provider_result_digest=None,
        provider_status=None,
        status="provider_reconciliation_required",
        generation=2,
        previous_checkpoint_digest=in_flight.checkpoint_digest,
    )
    observed = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=1,
        provider_dispatch_head_digest="9" * 64,
        provider_result_digest="1" * 64,
        provider_status="completed",
        status="provider_reconciliation_required",
        generation=2,
        previous_checkpoint_digest=in_flight.checkpoint_digest,
    )
    assert validate_autonomous_evidence_backed_checkpoint(unknown).to_dict() == unknown.to_dict()
    assert validate_autonomous_evidence_backed_checkpoint(observed).to_dict() == observed.to_dict()

    def resign(checkpoint: dict[str, object]) -> dict[str, object]:
        payload = {
            key: value
            for key, value in checkpoint.items()
            if key not in {"checkpoint_digest", "retention", "secret_material"}
        }
        return {**checkpoint, "checkpoint_digest": content_digest(payload)}

    forged_operation = resign(
        {**in_flight.to_dict(), "provider_operation_digest": "2" * 64}
    )
    with pytest.raises(ArgumentError, match="provider operation digest"):
        validate_autonomous_evidence_backed_checkpoint(forged_operation)

    invented_provider_status = resign(
        {
            **observed.to_dict(),
            "provider_status": "completed_but_failed",
        }
    )
    with pytest.raises(ArgumentError, match="provider_status is invalid"):
        validate_autonomous_evidence_backed_checkpoint(invented_provider_status)

    pending_with_operation = resign(
        {
            **in_flight.to_dict(),
            "status": "provider_pending",
        }
    )
    with pytest.raises(ArgumentError, match="provider-pending checkpoint"):
        validate_autonomous_evidence_backed_checkpoint(pending_with_operation)

    impossible_terminal = resign(
        {
            **observed.to_dict(),
            "status": "completed",
            "provider_status": "completed",
            "generation": 1,
            "previous_checkpoint_digest": None,
        }
    )
    with pytest.raises(ArgumentError, match="attempted predecessor"):
        validate_autonomous_evidence_backed_checkpoint(impossible_terminal)

    boolean_generation = resign({**in_flight.to_dict(), "generation": True})
    with pytest.raises(ArgumentError, match="generation is outside"):
        validate_autonomous_evidence_backed_checkpoint(boolean_generation)
    exhausted_generation = resign(
        {
            **in_flight.to_dict(),
            "generation": 2_147_483_648,
            "previous_checkpoint_digest": "3" * 64,
        }
    )
    with pytest.raises(ArgumentError, match="generation is outside"):
        validate_autonomous_evidence_backed_checkpoint(exhausted_generation)

    old_field = in_flight.to_dict()
    old_field["checkpoint_generation"] = old_field.pop("generation")
    with pytest.raises(ArgumentError, match="unsupported or missing fields"):
        validate_autonomous_evidence_backed_checkpoint(old_field)

    source_calls = 0

    def forbidden_acquirer(_context: object) -> object:
        nonlocal source_calls
        source_calls += 1
        raise AssertionError("legacy checkpoint reached source dispatch")

    class ForbiddenAgent:
        def evidence_plan(self, *_args: object, **_kwargs: object) -> object:
            raise AssertionError("legacy checkpoint reached planning")

    for legacy_schema in (
        "bioprism-python-autonomous-evidence-backed-checkpoint/0.3",
        "bioprism-python-autonomous-evidence-backed-checkpoint/0.2",
    ):
        legacy = resign(
            {
                **in_flight.to_dict(),
                "schema": legacy_schema,
            }
        )
        with pytest.raises(ArgumentError, match="unsupported or missing fields"):
            run_autonomous_evidence_backed_resumable(
                ForbiddenAgent(),
                task="legacy checkpoints fail before work",
                job_id="legacy-provider-checkpoint-job",
                requests=({},),
                acquirer=forbidden_acquirer,
                credentials={},
                checkpoint_sink=lambda _checkpoint: None,
                checkpoint=legacy,
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                approve_source_dispatch=True,
                approve_provider_call=True,
            )
    assert source_calls == 0


@pytest.mark.parametrize(
    ("identity", "run_options"),
    (
        (None, None),
        (
            {
                "provider_policy": {
                    "id": "missing-config-provider-policy",
                    "version": "v1",
                }
            },
            None,
        ),
        (None, {"memory": object()}),
    ),
)
def test_resumable_requires_provider_policy_trust_root_before_any_work(
    identity: object,
    run_options: object,
) -> None:
    plan_calls = 0
    provider_calls = 0

    class ForbiddenAgent:
        def evidence_plan(self, *_args: object, **_kwargs: object) -> object:
            nonlocal plan_calls
            plan_calls += 1
            raise AssertionError("missing provider policy reached planning")

        def run(self, *_args: object, **_kwargs: object) -> object:
            nonlocal provider_calls
            provider_calls += 1
            raise AssertionError("missing provider policy reached provider dispatch")

    class ForbiddenAcquirer:
        acquirer_id = "missing-provider-policy-acquirer"
        acquirer_version = "v1"

        def __init__(self) -> None:
            self.calls = 0

        def __call__(self, _context: object) -> object:
            self.calls += 1
            raise AssertionError("missing provider policy reached source dispatch")

    acquirer = ForbiddenAcquirer()

    with pytest.raises(ArgumentError, match="requires provider_policy identity with config_digest"):
        run_autonomous_evidence_backed_resumable(
            ForbiddenAgent(),
            task="require an explicit provider policy trust root",
            job_id="missing-provider-policy-job",
            requests=({},),
            acquirer=acquirer,
            credentials={},
            checkpoint_sink=lambda _checkpoint: None,
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            model_candidates=_model(),
            domains=("science",),
            approve_source_dispatch=True,
            approve_provider_call=False,
            resumable_policy_identity=identity,  # type: ignore[arg-type]
            run_options=run_options,  # type: ignore[arg-type]
        )
    assert plan_calls == 0
    assert acquirer.calls == 0
    assert provider_calls == 0


def test_resumable_rejects_changed_provider_policy_before_plan_source_or_provider():
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "provider-policy-drift-test")
    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent,
        "provider-policy-drift-job",
        store,
    )
    acquirer = _ResumableFixtureAcquirer("provider-policy-drift-value")
    domains = ("science",)

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    common = {
        "task": "bind caller and agent provider policy across restart",
        "requests": _requests(agent, domains),
        "acquirer": acquirer,
        "projector": projector,
        "evaluator": _ResumableFixtureEvaluator(),
        "journal": InMemoryAutonomousEvidenceRuntimeJournal(),
        "credentials": {"openai": handle},
        "model_candidates": _model(),
        "domains": domains,
        "run_mode": "domain",
        "approve_source_dispatch": True,
        "approve_provider_call": False,
        "resumable_policy_identity": _provider_policy_identity(),
    }

    try:
        pending = controller.run(**common)
        assert pending["run"].status == "provider_pending"
        checkpoint_digest = pending["run"].checkpoint.checkpoint_digest
        source_calls = acquirer.calls
        provider_calls = getattr(server, "request_count", 0)
        assert provider_calls == 0
        plan_calls = 0

        def forbidden_plan(*_args: object, **_kwargs: object) -> object:
            nonlocal plan_calls
            plan_calls += 1
            raise AssertionError("provider policy drift reached planning")

        agent.evidence_plan = forbidden_plan  # type: ignore[method-assign]
        changed = {
            **common,
            "resumable_policy_identity": {
                "provider_policy": {
                    "id": "fixture-provider-policy",
                    "version": "v1",
                    "config_digest": content_digest(
                        {"fixture": "changed-provider-policy"}
                    ),
                }
            },
        }
        with pytest.raises(ArgumentError, match="does not match the current task, requests, policy, or job"):
            controller.run(**changed)
        assert plan_calls == 0
        assert acquirer.calls == source_calls
        assert getattr(server, "request_count", 0) == provider_calls
        assert store.current is not None
        assert store.current.checkpoint_digest == checkpoint_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


@pytest.mark.parametrize(
    "unsafe_options",
    (
        [],
        {"execution_mode": "tool_loop"},
        {"execution_mode": "mission"},
        {"execution_mode": {}},
        {"execution_controller": object()},
        {"child_execution_mode": "tool_loop"},
        {"synthesis_execution_mode": "tool_loop"},
        {"semantic_routing": True},
        {"semantic_routing": 1},
        {"semantic_routing": "yes"},
        {"planning_mode": "provider"},
        {"workflow_execution": True},
        {"workflow_execution": "yes"},
        {"learn": 1},
        {"learning_mode": "online"},
    ),
)
def test_resumable_provider_fence_rejects_unbound_multi_provider_modes_before_work(
    unsafe_options: object,
) -> None:
    plan_calls = 0
    provider_calls = 0

    class ForbiddenAgent:
        def evidence_plan(self, *_args: object, **_kwargs: object) -> object:
            nonlocal plan_calls
            plan_calls += 1
            raise AssertionError("unsafe provider mode reached planning")

        def run(self, *_args: object, **_kwargs: object) -> object:
            nonlocal provider_calls
            provider_calls += 1
            raise AssertionError("unsafe provider mode reached provider dispatch")

    source_calls = 0

    def forbidden_acquirer(_context: object) -> object:
        nonlocal source_calls
        source_calls += 1
        raise AssertionError("unsafe provider mode reached source dispatch")

    with pytest.raises(ArgumentError, match="resumable evidence-backed"):
        run_autonomous_evidence_backed_resumable(
            ForbiddenAgent(),
            task="reject an unbound multi-provider mode",
            job_id="unsafe-provider-mode-job",
            requests=({},),
            acquirer=forbidden_acquirer,
            credentials={},
            checkpoint_sink=lambda _checkpoint: None,
            checkpoint_compare_and_store=lambda _expected, _checkpoint: True,
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            approve_source_dispatch=True,
            approve_provider_call=True,
            run_options=unsafe_options,
        )
    assert plan_calls == 0
    assert source_calls == 0
    assert provider_calls == 0


def test_resumable_provider_fence_rejects_adaptive_model_failover_before_work() -> None:
    class ForbiddenAgent:
        def evidence_plan(self, *_args: object, **_kwargs: object) -> object:
            raise AssertionError("adaptive provider candidates reached planning")

    candidates = [*_model(), {**_model()[0], "model": "second-model"}]
    with pytest.raises(ArgumentError, match="exactly one explicit model candidate"):
        run_autonomous_evidence_backed_resumable(
            ForbiddenAgent(),
            task="reject ambiguous adaptive provider retries",
            job_id="adaptive-provider-candidates-job",
            requests=({},),
            acquirer=lambda _context: (_ for _ in ()).throw(
                AssertionError("adaptive provider candidates reached source dispatch")
            ),
            credentials={},
            checkpoint_sink=lambda _checkpoint: None,
            checkpoint_compare_and_store=lambda _expected, _checkpoint: True,
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            approve_source_dispatch=True,
            approve_provider_call=True,
            model_candidates=candidates,
        )


def test_resumable_rejects_new_higher_mro_getattribute_before_lookup_or_work(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    provider_calls: list[ProviderRequest] = []
    runtime, credential_store = _in_memory_runtime(
        lambda request: provider_calls.append(request) or "forbidden"
    )
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "mro-shadow-test")
    requests = _requests(agent, ("science",))
    hostile_lookups = 0
    checkpoint_calls: list[object] = []

    def hostile_getattribute(self: object, name: str) -> object:
        nonlocal hostile_lookups
        hostile_lookups += 1
        return object.__getattribute__(self, name)

    monkeypatch.setattr(
        AutonomousAgent,
        "__getattribute__",
        hostile_getattribute,
        raising=False,
    )
    with pytest.raises(ArgumentError, match="overridden AutonomousAgent"):
        run_autonomous_evidence_backed_resumable(
            agent,
            task="reject a newly shadowed MRO lookup path",
            job_id="mro-shadow-job",
            requests=requests,
            acquirer=_ResumableFixtureAcquirer("mro-shadow-evidence"),
            evaluator=_ResumableFixtureEvaluator(),
            credentials={"openai": handle},
            checkpoint_sink=checkpoint_calls.append,
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=False,
            resumable_policy_identity=_provider_policy_identity(),
        )
    assert hostile_lookups == 0
    assert checkpoint_calls == []
    assert provider_calls == []


@pytest.mark.parametrize("timing", ("prompt", "dispatch_cas"))
@pytest.mark.parametrize(
    "mutation",
    (
        "registry",
        "handler",
        "http_factory",
        "http_request_method",
        "json_encoder",
        "body_code",
        "socket_create_connection",
        "socket_getaddrinfo",
        "https_context_factory",
        "ssl_default_context",
        "request_snapshot_helper",
    ),
)
def test_resumable_transport_graph_mutation_never_reaches_transport(
    monkeypatch: pytest.MonkeyPatch,
    timing: str,
    mutation: str,
) -> None:
    provider_calls: list[ProviderRequest] = []
    factory_calls: list[object] = []
    server = None
    thread = None
    if mutation in {
        "http_factory",
        "http_request_method",
        "json_encoder",
        "body_code",
        "socket_create_connection",
        "socket_getaddrinfo",
        "https_context_factory",
        "ssl_default_context",
    }:
        runtime, credential_store, server, thread = _runtime()
    else:
        runtime, credential_store = _in_memory_runtime(
            lambda request: provider_calls.append(request) or "forbidden"
        )
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", f"{timing}-{mutation}-test")
    changed = False

    def mutate_transport() -> None:
        nonlocal changed
        if changed:
            return
        changed = True
        if mutation == "registry":
            monkeypatch.setitem(
                runtime._providers,
                "shadow-provider",
                runtime._providers["openai"],
            )
        elif mutation == "handler":
            transport = runtime._providers["openai"].transport
            assert transport is not None
            monkeypatch.setattr(
                transport,
                "_handler",
                lambda request: provider_calls.append(request) or "wrong",
            )
        elif mutation == "http_factory":
            def forbidden_https_connection(*args: object, **kwargs: object) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("mutated HTTP constructor executed")

            monkeypatch.setattr(
                http.client,
                "HTTPConnection",
                forbidden_https_connection,
            )
        elif mutation == "http_request_method":
            def forbidden_request(*args: object, **kwargs: object) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("mutated HTTP request method executed")

            monkeypatch.setattr(
                http.client.HTTPConnection,
                "request",
                forbidden_request,
            )
        elif mutation == "json_encoder":
            original_encoder = json.JSONEncoder

            class MutatedJsonEncoder(original_encoder):
                pass

            monkeypatch.setattr(json, "JSONEncoder", MutatedJsonEncoder)
        elif mutation == "body_code":
            def substituted_body(
                _config: object,
                _request: ProviderRequest,
            ) -> dict[str, object]:
                return {"model": "substituted", "messages": []}

            monkeypatch.setattr(
                LLMRuntime._body,
                "__code__",
                substituted_body.__code__,
            )
        elif mutation == "socket_create_connection":
            def forbidden_socket_connection(
                *args: object,
                **kwargs: object,
            ) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("substituted socket connector executed")

            monkeypatch.setattr(
                http.client.socket,
                "create_connection",
                forbidden_socket_connection,
            )
        elif mutation == "socket_getaddrinfo":
            def forbidden_getaddrinfo(
                *args: object,
                **kwargs: object,
            ) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("substituted address resolver executed")

            monkeypatch.setattr(
                http.client.socket,
                "getaddrinfo",
                forbidden_getaddrinfo,
            )
        elif mutation == "https_context_factory":
            def forbidden_https_context(
                *args: object,
                **kwargs: object,
            ) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("substituted HTTPS context executed")

            monkeypatch.setattr(
                http.client,
                "_create_https_context",
                forbidden_https_context,
            )
        elif mutation == "ssl_default_context":
            def forbidden_default_context(
                *args: object,
                **kwargs: object,
            ) -> object:
                factory_calls.append((args, kwargs))
                raise AssertionError("substituted default HTTPS context executed")

            monkeypatch.setattr(
                http.client.ssl,
                "_create_default_https_context",
                forbidden_default_context,
            )
        if mutation == "request_snapshot_helper":
            original_snapshot = evidence_brain_module._provider_request_snapshot

            def substituted_snapshot(
                selected_provider: object,
                selected_request: object,
            ) -> tuple[ProviderRequest, str]:
                snapshot, digest = original_snapshot(
                    selected_provider,
                    selected_request,
                )
                return (
                    replace(
                        snapshot,
                        messages=(
                            {
                                "role": "user",
                                "content": "substituted after durable CAS",
                            },
                        ),
                    ),
                    digest,
                )

            monkeypatch.setattr(
                evidence_brain_module,
                "_provider_request_snapshot",
                substituted_snapshot,
            )

    class MutatingDispatchStore(_RecordingCheckpointStore):
        def write_dispatch_if_unchanged(
            self,
            expected: str | None,
            checkpoint: object,
            private_receipt: object,
        ) -> bool:
            stored = super().write_dispatch_if_unchanged(
                expected,
                checkpoint,
                private_receipt,
            )
            if stored:
                mutate_transport()
            return stored

    store = MutatingDispatchStore()
    controller = AutonomousEvidenceBackedController(
        agent,
        f"transport-graph-{timing}-{mutation}-job",
        store,
    )
    acquirer = _ResumableFixtureAcquirer(
        f"transport-graph-{timing}-{mutation}-evidence"
    )

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def mutating_prompt(evidence: object) -> dict[str, object]:
        mutate_transport()
        return {
            "transport_graph_test": {
                "status": evidence.status,  # type: ignore[attr-defined]
            }
        }

    identity = _provider_policy_identity()
    prompt_builder = None
    if timing == "prompt":
        prompt_builder = mutating_prompt
        identity = {
            **identity,
            "prompt_builder": {
                "id": "mutating-transport-prompt",
                "version": "v1",
                "config_digest": content_digest(
                    {"timing": timing, "mutation": mutation}
                ),
            },
        }
    try:
        with pytest.raises(
            ArgumentError,
            match="provider (transport graph|registry|HTTP transport factories|dispatch-chain|fencing rejects modified dispatch-fence)",
        ):
            controller.run(
                task="reject provider transport mutation across every callback seam",
                requests=_requests(agent, ("science",)),
                acquirer=acquirer,
                projector=projector,
                evaluator=_ResumableFixtureEvaluator(),
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=("science",),
                run_mode="domain",
                prompt_builder=prompt_builder,
                approve_source_dispatch=True,
                approve_provider_call=True,
                resumable_policy_identity=identity,
            )
        assert changed is True
        assert provider_calls == []
        assert factory_calls == []
        if server is not None:
            assert getattr(server, "request_count", 0) == 0
        if timing == "prompt":
            assert store.current is None
            assert store.history == []
        else:
            assert store.current is not None
            assert store.current.status == "provider_in_flight"
            assert controller.flush()["status"] == "reload_required"
    finally:
        if server is not None and thread is not None:
            server.shutdown()
            thread.join(timeout=2)
            server.server_close()


def test_resumable_rechecks_selected_credential_expiry_after_dispatch_cas() -> None:
    clock_state = {"now": 0.0}
    provider_calls: list[ProviderRequest] = []

    def clock() -> float:
        return clock_state["now"]

    credential_store = CredentialStore(clock=clock)
    runtime = LLMRuntime(credential_store)
    local_config = runtime.register_in_memory_provider(
        "openai",
        lambda request: provider_calls.append(request) or "forbidden",
    )
    runtime.register_provider(replace(local_config, requires_credential=True))
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register(
        "openai",
        "expires-during-dispatch-cas",
        ttl_seconds=1.0,
    )

    class ExpiringDispatchStore(_RecordingCheckpointStore):
        def write_dispatch_if_unchanged(
            self,
            expected: str | None,
            checkpoint: object,
            private_receipt: object,
        ) -> bool:
            stored = super().write_dispatch_if_unchanged(
                expected,
                checkpoint,
                private_receipt,
            )
            if stored:
                clock_state["now"] = 2.0
            return stored

    store = ExpiringDispatchStore()
    controller = AutonomousEvidenceBackedController(
        agent,
        "credential-expiry-final-fence-job",
        store,
    )

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    with pytest.raises(BrainRunError, match="credential expired"):
        controller.run(
            task="refuse credentials that expire while dispatch CAS is running",
            requests=_requests(agent, ("science",)),
            acquirer=_ResumableFixtureAcquirer(
                "credential-expiry-final-fence-evidence"
            ),
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials={"openai": handle},
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
            resumable_policy_identity=_provider_policy_identity(),
        )
    assert provider_calls == []
    assert store.current is not None
    assert store.current.status == "provider_in_flight"
    assert controller.flush()["status"] == "reload_required"


def test_resumable_rejects_selected_provider_config_after_registry_swap_restore() -> None:
    original_calls: list[ProviderRequest] = []
    substituted_calls: list[ProviderRequest] = []
    runtime, credential_store = _in_memory_runtime(
        lambda request: original_calls.append(request) or "original"
    )
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "config-swap-restore-test")
    original_config = runtime._providers["openai"]
    substituted_transport = llm_runtime_module.InMemoryProvider(
        "openai",
        lambda request: substituted_calls.append(request) or "substituted",
    )
    substituted_config = replace(
        original_config,
        transport=substituted_transport,
    )
    registry_swapped = False
    registry_restored = False

    def swap_during_model_selection(**_event: object) -> None:
        nonlocal registry_swapped
        if not registry_swapped:
            runtime._providers["openai"] = substituted_config
            registry_swapped = True

    class RestoringObserver:
        def before(self, _metadata: object) -> None:
            nonlocal registry_restored
            # LLMRuntime has already selected its local config before invoking this hook.
            runtime._providers["openai"] = original_config
            registry_restored = True

        def before_transport(self, _metadata: object) -> None:
            return None

        def after(
            self,
            _metadata: object,
            _response: object,
            _error: object,
            _latency_ms: float,
        ) -> None:
            return None

    store = _RecordingCheckpointStore()
    controller = AutonomousEvidenceBackedController(
        agent,
        "config-swap-restore-job",
        store,
    )

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    with pytest.raises(
        BrainRunError,
        match="configuration outside its snapshotted registry",
    ):
        controller.run(
            task="reject a locally retained substituted provider configuration",
            requests=_requests(agent, ("science",)),
            acquirer=_ResumableFixtureAcquirer("config-swap-restore-evidence"),
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials={"openai": handle},
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            run_options={
                "trace_event_callback": swap_during_model_selection,
                "invocation_observer": RestoringObserver(),
            },
            approve_source_dispatch=True,
            approve_provider_call=True,
            resumable_policy_identity=_provider_policy_identity(),
        )
    assert registry_swapped is True
    assert registry_restored is True
    assert runtime._providers["openai"] is original_config
    assert original_calls == []
    assert substituted_calls == []
    assert store.current is None
    assert store.history == []


def test_resumable_rejects_coordinated_http_config_and_factory_swap_restore(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "coordinated-http-swap-test")
    config = runtime._providers["openai"]
    original_base_url = config.base_url
    original_api_key_header = config.api_key_header
    original_factory = http.client.HTTPConnection
    malicious_factory_calls: list[object] = []
    swapped = False
    restored = False

    def malicious_factory(*args: object, **kwargs: object) -> object:
        malicious_factory_calls.append((args, kwargs))
        raise AssertionError("captured substituted HTTP factory executed")

    def swap_before_runtime_entry(**_event: object) -> None:
        nonlocal swapped
        if swapped:
            return
        swapped = True
        object.__setattr__(config, "base_url", "http://malicious.invalid:9")
        object.__setattr__(config, "api_key_header", "X-Malicious-Key")
        monkeypatch.setattr(http.client, "HTTPConnection", malicious_factory)

    class RestoringObserver:
        def before(self, _metadata: object) -> None:
            return None

        def before_transport(self, _metadata: object) -> None:
            nonlocal restored
            object.__setattr__(config, "base_url", original_base_url)
            object.__setattr__(config, "api_key_header", original_api_key_header)
            monkeypatch.setattr(http.client, "HTTPConnection", original_factory)
            restored = True

        def after(
            self,
            _metadata: object,
            _response: object,
            _error: object,
            _latency_ms: float,
        ) -> None:
            return None

    store = _RecordingCheckpointStore()

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        with pytest.raises(BrainRunError, match="configuration outside its snapshotted registry"):
            AutonomousEvidenceBackedController(
                agent,
                "coordinated-http-swap-job",
                store,
            ).run(
                task="reject restored HTTP state paired with stale derived dispatch values",
                requests=_requests(agent, ("science",)),
                acquirer=_ResumableFixtureAcquirer("coordinated-http-swap-evidence"),
                projector=projector,
                evaluator=_ResumableFixtureEvaluator(),
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=("science",),
                run_mode="domain",
                run_options={
                    "trace_event_callback": swap_before_runtime_entry,
                    "invocation_observer": RestoringObserver(),
                },
                approve_source_dispatch=True,
                approve_provider_call=True,
                resumable_policy_identity=_provider_policy_identity(),
            )
        assert swapped is True
        assert restored is True
        assert malicious_factory_calls == []
        assert getattr(server, "request_count", 0) == 0
        assert store.current is None
        assert store.history == []
    finally:
        object.__setattr__(config, "base_url", original_base_url)
        object.__setattr__(config, "api_key_header", original_api_key_header)
        monkeypatch.setattr(http.client, "HTTPConnection", original_factory)
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


@pytest.mark.parametrize(
    "mutation",
    (
        "credential_value",
        "credential_expose",
        "request_descriptor",
        "urlsplit_helper",
        "wire_messages_helper",
    ),
)
def test_resumable_final_transport_fence_rejects_transitive_mutation(
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
) -> None:
    runtime, credential_store, server, thread = _runtime()
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", f"{mutation}-fence-test")
    entry = credential_store._entries[handle.credential_id]
    mutated = False

    class MutatingObserver:
        def before(self, _metadata: object) -> None:
            return None

        def before_transport(self, _metadata: object) -> None:
            nonlocal mutated
            mutated = True
            if mutation == "credential_value":
                object.__setattr__(entry.secret, "_value", "substituted-secret")
            elif mutation == "credential_expose":
                monkeypatch.setattr(
                    llm_runtime_module.SecretValue,
                    "expose",
                    lambda _self: "substituted-secret",
                )
            elif mutation == "request_descriptor":
                monkeypatch.setattr(
                    ProviderRequest,
                    "messages",
                    property(
                        lambda _self: (
                            {"role": "user", "content": "substituted prompt"},
                        )
                    ),
                )
            elif mutation == "urlsplit_helper":
                monkeypatch.setattr(
                    llm_runtime_module,
                    "urlsplit",
                    lambda _value: (_ for _ in ()).throw(
                        AssertionError("substituted urlsplit executed")
                    ),
                )
            else:
                monkeypatch.setattr(
                    llm_runtime_module,
                    "_wire_messages",
                    lambda *_args: [
                        {"role": "user", "content": "substituted prompt"}
                    ],
                )

        def after(
            self,
            _metadata: object,
            _response: object,
            _error: object,
            _latency_ms: float,
        ) -> None:
            return None

    store = _RecordingCheckpointStore()

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        with pytest.raises(
            (ArgumentError, BrainRunError),
            match="request|ProviderRequest|credential|dispatch dependencies|transport graph|HTTP transport",
        ):
            AutonomousEvidenceBackedController(
                agent,
                f"transitive-{mutation}-job",
                store,
            ).run(
                task="reject transitive state replacement at the final provider seam",
                requests=_requests(agent, ("science",)),
                acquirer=_ResumableFixtureAcquirer(
                    f"transitive-{mutation}-evidence"
                ),
                projector=projector,
                evaluator=_ResumableFixtureEvaluator(),
                journal=InMemoryAutonomousEvidenceRuntimeJournal(),
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=("science",),
                run_mode="domain",
                run_options={"invocation_observer": MutatingObserver()},
                approve_source_dispatch=True,
                approve_provider_call=True,
                resumable_policy_identity=_provider_policy_identity(),
            )
        assert mutated is True
        assert getattr(server, "request_count", 0) == 0
        assert store.current is None
        assert store.history == []
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_quota_callback_mutation_runs_before_final_private_fence() -> None:
    original_calls: list[ProviderRequest] = []
    substituted_calls: list[ProviderRequest] = []
    releases = 0
    settlements = 0
    transport_holder: list[object] = []

    class Reservation:
        def __init__(self) -> None:
            self.dispatched = False

        @property
        def is_dispatched(self) -> bool:
            return self.dispatched

        def mark_dispatched(self) -> None:
            self.dispatched = True
            transport = transport_holder[0]
            object.__setattr__(
                transport,
                "_handler",
                lambda request: substituted_calls.append(request) or "substituted",
            )

        def release(self) -> None:
            nonlocal releases
            releases += 1

        def settle(self, _actual: object = None) -> dict[str, object]:
            nonlocal settlements
            settlements += 1
            return {}

    class Quota:
        def reserve(self, _estimate: object) -> Reservation:
            return Reservation()

    credential_store = CredentialStore()
    runtime = LLMRuntime(credential_store, provider_quota=Quota())
    runtime.register_in_memory_provider(
        "openai",
        lambda request: original_calls.append(request) or "original",
    )
    transport = runtime._providers["openai"].transport
    assert transport is not None
    transport_holder.append(transport)
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "quota-fence-test")
    store = _RecordingCheckpointStore()

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    with pytest.raises(ArgumentError, match="provider transport graph"):
        AutonomousEvidenceBackedController(
            agent,
            "quota-final-fence-job",
            store,
        ).run(
            task="run quota accounting before the final private provider fence",
            requests=_requests(agent, ("science",)),
            acquirer=_ResumableFixtureAcquirer("quota-final-fence-evidence"),
            projector=projector,
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            credentials={"openai": handle},
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
            resumable_policy_identity=_provider_policy_identity(),
        )
    assert original_calls == []
    assert substituted_calls == []
    assert releases == 1
    assert settlements == 0
    assert store.current is None


def test_resumable_request_snapshot_rejects_alias_preserving_mapping() -> None:
    class StickyMapping(Mapping[str, str]):
        def __init__(self) -> None:
            self.values = {"role": "user", "content": "approved"}

        def __getitem__(self, key: str) -> str:
            return self.values[key]

        def __iter__(self) -> Iterator[str]:
            return iter(self.values)

        def __len__(self) -> int:
            return len(self.values)

        def __deepcopy__(self, _memo: object) -> "StickyMapping":
            return self

    request = ProviderRequest(
        model="test-model",
        messages=(StickyMapping(),),
    )
    fence = evidence_brain_module._ProviderDispatchFenceObserver(
        "a" * 64,
        lambda _attestation: None,
    )
    with pytest.raises(BrainRunError, match="exact built-in mappings"):
        fence.prepare_dispatch("openai", request)


@pytest.mark.parametrize("callback_kind", ("sink", "cas"))
def test_resumable_rejects_checkpoint_mutation_by_persistence_callback(
    callback_kind: str,
) -> None:
    provider_calls: list[ProviderRequest] = []
    runtime, credential_store = _in_memory_runtime(
        lambda request: provider_calls.append(request) or "forbidden"
    )
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", f"checkpoint-{callback_kind}-test")
    callback_values: list[AutonomousEvidenceBackedCheckpoint] = []

    def mutate(checkpoint: AutonomousEvidenceBackedCheckpoint) -> None:
        callback_values.append(checkpoint)
        object.__setattr__(checkpoint, "status", "completed")

    def sink(checkpoint: AutonomousEvidenceBackedCheckpoint) -> None:
        mutate(checkpoint)

    def compare_and_store(
        _expected: str | None,
        checkpoint: AutonomousEvidenceBackedCheckpoint,
    ) -> bool:
        mutate(checkpoint)
        return True

    with pytest.raises(
        BrainRunError,
        match="checkpoint changed during persistence callback",
    ):
        run_autonomous_evidence_backed_resumable(
            agent,
            task="reject mutation of a detached checkpoint commit value",
            job_id=f"checkpoint-{callback_kind}-mutation-job",
            requests=_requests(agent, ("science",)),
            acquirer=_ResumableFixtureAcquirer(
                f"checkpoint-{callback_kind}-mutation-evidence"
            ),
            credentials={"openai": handle},
            checkpoint_sink=sink,
            checkpoint_compare_and_store=(
                compare_and_store if callback_kind == "cas" else None
            ),
            evaluator=_ResumableFixtureEvaluator(),
            journal=InMemoryAutonomousEvidenceRuntimeJournal(),
            model_candidates=_model(),
            domains=("science",),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=False,
            resumable_policy_identity=_provider_policy_identity(),
        )
    assert len(callback_values) == 1
    assert callback_values[0].status == "completed"
    assert provider_calls == []


def test_checkpoint_store_cas_rejects_completed_to_forged_pending_lifecycle_reset() -> None:
    fields = {
        "job_id": "cas-lifecycle-job",
        "task_digest": "a" * 64,
        "request_digest": "b" * 64,
        "run_policy_digest": "c" * 64,
        "evidence_plan_digest": "d" * 64,
        "execution_plan_digest": "e" * 64,
        "evidence_result_digest": "f" * 64,
        "prompt_projection_digest": "0" * 64,
    }
    operation_digest = content_digest(
        {
            "schema": "bioprism-python-autonomous-evidence-backed-provider-operation/0.1",
            **fields,
        }
    )
    completed = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=1,
        provider_dispatch_head_digest="9" * 64,
        provider_result_digest="8" * 64,
        provider_status="completed",
        status="completed",
        generation=2,
        previous_checkpoint_digest="7" * 64,
    )
    forged_pending = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=None,
        provider_dispatch_count=0,
        provider_dispatch_head_digest=None,
        provider_result_digest=None,
        provider_status=None,
        status="provider_pending",
        generation=3,
        previous_checkpoint_digest=completed.checkpoint_digest,
    )
    memory_store = InMemoryAutonomousEvidenceBackedCheckpointStore(completed)
    with pytest.raises(BrainRunError, match="invalid.*completed -> provider_pending"):
        memory_store.write_if_unchanged(
            completed.checkpoint_digest,
            forged_pending,
        )

    class TextStore:
        def __init__(self) -> None:
            self.value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            observed = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
            if observed != expected:
                return False
            self.value = value
            return True

        def write_dispatch_if_unchanged(
            self,
            expected: str | None,
            value: str,
            _private_receipt: str,
        ) -> bool:
            return self.write_if_unchanged(expected, value)

    text_store = TextStore()
    persistence = TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(
        text_store
    )
    persistence.write(completed)
    with pytest.raises(BrainRunError, match="invalid.*completed -> provider_pending"):
        persistence.write_if_unchanged(
            completed.checkpoint_digest,
            forged_pending,
        )

    pending = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=None,
        provider_dispatch_count=0,
        provider_dispatch_head_digest=None,
        provider_result_digest=None,
        provider_status=None,
        status="provider_pending",
        generation=1,
        previous_checkpoint_digest=None,
    )
    forged_in_flight = AutonomousEvidenceBackedCheckpoint(
        **fields,
        provider_operation_digest=operation_digest,
        provider_dispatch_count=1,
        provider_dispatch_head_digest="6" * 64,
        provider_result_digest=None,
        provider_status=None,
        status="provider_in_flight",
        generation=2,
        previous_checkpoint_digest=pending.checkpoint_digest,
    )
    pending_memory_store = InMemoryAutonomousEvidenceBackedCheckpointStore(
        pending
    )
    with pytest.raises(BrainRunError, match="atomic private receipt transaction"):
        pending_memory_store.write_if_unchanged(
            pending.checkpoint_digest,
            forged_in_flight,
        )

    pending_text_store = TextStore()
    pending_persistence = (
        TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(
            pending_text_store
        )
    )
    pending_persistence.write(pending)
    with pytest.raises(BrainRunError, match="atomic private receipt transaction"):
        pending_persistence.write_if_unchanged(
            pending.checkpoint_digest,
            forged_in_flight,
        )


def test_in_memory_dispatch_receipt_enumeration_is_public_and_lookup_is_defensive() -> None:
    runtime, credential_store = _in_memory_runtime(lambda _request: "completed")
    agent = AutonomousAgent(
        _Workspace(), runtime, model_catalogue=ModelCatalogue(_model())
    )
    handle = credential_store.register("openai", "receipt-privacy-test")
    store = InMemoryAutonomousEvidenceBackedCheckpointStore()

    def projector(_value: object, context: object) -> list[dict[str, object]]:
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    result = AutonomousEvidenceBackedController(
        agent,
        "receipt-privacy-job",
        store,
    ).run(
        task="keep exact provider keys in privileged receipt lookup only",
        requests=_requests(agent, ("science",)),
        acquirer=_ResumableFixtureAcquirer("receipt-privacy-evidence"),
        projector=projector,
        evaluator=_ResumableFixtureEvaluator(),
        journal=InMemoryAutonomousEvidenceRuntimeJournal(),
        credentials={"openai": handle},
        model_candidates=_model(),
        domains=("science",),
        run_mode="domain",
        approve_source_dispatch=True,
        approve_provider_call=True,
        resumable_policy_identity=_provider_policy_identity(),
    )
    head = result["run"].checkpoint.provider_dispatch_head_digest
    assert head is not None
    projections = store.provider_dispatch_receipt_projections()
    assert projections == store.provider_dispatch_receipts()
    assert len(projections) == 1
    assert "provider_idempotency_key" not in projections[0]
    assert "provider_idempotency_key_digest" in projections[0]
    privileged = store.provider_dispatch_receipt(head)
    assert privileged is not None
    exact_key = privileged.provider_idempotency_key
    object.__setattr__(privileged, "provider_idempotency_key", "0" * 64)
    reread = store.provider_dispatch_receipt(head)
    assert reread is not None
    assert reread.provider_idempotency_key == exact_key
