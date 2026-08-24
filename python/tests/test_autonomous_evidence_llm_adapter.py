from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousEvidencePlan,
    AutonomousEvidenceRequirement,
    AutonomousEvidenceRuntime,
    AutonomousLLMEvidenceAdapter,
    AutonomousLLMEvidenceAdapterRouter,
    LLMRuntime,
    ProviderError,
    create_autonomous_llm_evidence_adapter,
    create_autonomous_llm_evidence_adapter_router,
    content_digest,
)
from prism_sdk.errors import ArgumentError


class _AcceptAll:
    evaluator_id = "llm-fixture-evaluator"
    evaluator_version = "v1"

    def evaluate(self, _input: object) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


def _plan(domain: str) -> tuple[AutonomousEvidencePlan, AutonomousEvidenceRequirement]:
    workflow_digest = content_digest({"workflow": domain})
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


def _request(requirement: AutonomousEvidenceRequirement) -> dict[str, object]:
    return {
        "requirement_id": requirement.requirement_id,
        "source_id": "credentialless-llm-fixture",
        "request_id": f"request-{requirement.domain}",
        "metadata": {"fixture": "offline", "domain": requirement.domain},
    }


def _router(runtime: LLMRuntime) -> AutonomousLLMEvidenceAdapterRouter:
    adapters: dict[str, AutonomousLLMEvidenceAdapter] = {}
    for domain in AUTONOMOUS_DOMAINS:
        adapters[domain] = create_autonomous_llm_evidence_adapter(
            adapter_id=f"fixture-{domain}",
            version="v1",
            domain=domain,
            provider="credentialless-fixture",
            runtime=runtime,
            capabilities=("llm_evidence",),
            model_for_context=lambda context: f"fixture-{context['requirement'].domain}",  # type: ignore[index]
            prompt_for_context=lambda context: [
                {
                    "role": "user",
                    "content": context["requirement"].objective,  # type: ignore[index]
                }
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
    return create_autonomous_llm_evidence_adapter_router(adapters, require_all_domains=True)


def test_credentialless_llm_adapter_routes_and_completes_every_domain() -> None:
    runtime = LLMRuntime()
    calls: list[object] = []

    def handler(request: object) -> dict[str, object]:
        calls.append(request)
        model = request.model  # type: ignore[attr-defined]
        return {
            "text": json.dumps({"answer": f"grounded-{model}"}),
            "usage": {"input_tokens": 12, "output_tokens": 4},
        }

    runtime.register_in_memory_provider("credentialless-fixture", handler)
    router = _router(runtime)
    assert set(router.domains) == set(AUTONOMOUS_DOMAINS)
    assert router.to_dict()["secret_material"] == "never_returned"

    for domain in AUTONOMOUS_DOMAINS:
        plan, requirement = _plan(domain)
        result = AutonomousEvidenceRuntime(plan).execute(
            [_request(requirement)],
            acquirer=router,
            projector=router,
            evaluator=_AcceptAll(),
        )
        assert result.status == "completed", domain
        assert result.receipts[0].status == "observed", domain
        assert result.assessments[0].verdict == "accepted", domain
        assert result.values

    assert len(calls) == len(AUTONOMOUS_DOMAINS)


def test_llm_adapter_uses_digest_bound_idempotency_and_redacts_credential_metadata() -> None:
    runtime = LLMRuntime()
    observed_requests: list[object] = []

    def handler(request: object) -> dict[str, object]:
        observed_requests.append(request)
        return {"text": json.dumps({"answer": "ok"})}

    runtime.register_in_memory_provider(
        "credentialless-fixture",
        handler,
    )
    handle = runtime.credentials.register("credentialless-fixture", "fixture-secret-only-in-memory")
    _plan_value, requirement = _plan("science")
    adapter = create_autonomous_llm_evidence_adapter(
        adapter_id="science-llm",
        version="v1",
        domain="science",
        provider="credentialless-fixture",
        runtime=runtime,
        capabilities=("llm_evidence",),
        model="fixture-science",
        credential=handle,
        prompt_for_context=lambda _context: [{"role": "user", "content": "answer"}],
        require_json=True,
    )
    context = {
        "plan_digest": _plan_value.plan_digest,
        "requirement": requirement,
        "request": _request(requirement),
    }
    adapter.acquire(context)
    assert len(observed_requests) == 1
    idempotency_key = observed_requests[0].idempotency_key  # type: ignore[attr-defined]
    assert idempotency_key == content_digest(
        {
            "schema": "bioprism-python-autonomous-llm-evidence-adapter/0.1",
            "plan_digest": _plan_value.plan_digest,
            "requirement_id": requirement.requirement_id,
            "source_id": "credentialless-llm-fixture",
            "source_digest": None,
            "request_id": "request-science",
            "metadata": {"fixture": "offline", "domain": "science"},
        }
    )
    projection = json.dumps(adapter.to_dict())
    assert "fixture-secret-only-in-memory" not in projection
    assert "credential_id" not in projection


def test_llm_adapter_fails_closed_on_provider_errors_and_secret_shaped_outputs() -> None:
    failing_runtime = LLMRuntime()

    def fail(_request: object) -> object:
        raise ProviderError("fixture transport failure", retryable=True)

    failing_runtime.register_in_memory_provider("credentialless-failure", fail)
    plan, requirement = _plan("coding")
    failing_adapter = create_autonomous_llm_evidence_adapter(
        adapter_id="coding-failing-llm",
        version="v1",
        domain="coding",
        provider="credentialless-failure",
        runtime=failing_runtime,
        capabilities=("llm_evidence",),
        model="fixture-coding",
        prompt_for_context=lambda _context: [{"role": "user", "content": "answer"}],
        require_json=True,
    )
    failed = AutonomousEvidenceRuntime(plan).execute(
        [_request(requirement)],
        acquirer=failing_adapter,
    )
    assert failed.status == "failed"
    assert failed.receipts[0].status == "failed"
    assert failed.receipts[0].error_class == "ProviderError"

    secret_runtime = LLMRuntime()
    secret_runtime.register_in_memory_provider(
        "credentialless-secret",
        lambda _request: {"text": json.dumps({"secret": "must-not-cross-boundary"})},
    )
    secret_adapter = create_autonomous_llm_evidence_adapter(
        adapter_id="coding-secret-output",
        version="v1",
        domain="coding",
        provider="credentialless-secret",
        runtime=secret_runtime,
        capabilities=("llm_evidence",),
        model="fixture-coding",
        prompt_for_context=lambda _context: [{"role": "user", "content": "answer"}],
        require_json=True,
    )
    with pytest.raises(ArgumentError, match="credential-shaped"):
        secret_adapter.acquire({
            "plan_digest": plan.plan_digest,
            "requirement": requirement,
            "request": _request(requirement),
        })
