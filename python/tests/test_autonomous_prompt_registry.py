from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    LLMRuntime,
    create_autonomous_llm_evidence_adapter,
    content_digest,
)
from prism_sdk.errors import ArgumentError


def _template(domain: str, prompt_id: str | None = None, *, content: str = "transient prompt") -> AutonomousPromptTemplate:
    return AutonomousPromptTemplate(
        prompt_id=prompt_id or f"prompt-{domain}",
        version="1.0.0",
        domain=domain,
        capabilities=("analysis", "llm_evidence"),
        stages=("answer",),
        template_digest=content_digest({"prompt": prompt_id or domain, "version": "1.0.0", "content": content}),
        render=lambda _context: [{"role": "user", "content": content}],
    )


def _context(domain: str) -> dict[str, object]:
    requirement = {
        "domain": domain,
        "stage_id": "answer",
        "requirement_id": f"{domain}:answer:answer",
    }
    return {
        "plan_digest": content_digest({"domain": domain}),
        "requirement": requirement,
        "request": {
            "source_id": "prompt-fixture",
            "request_id": f"request-{domain}",
            "metadata": {"fixture": "offline"},
        },
    }


def test_prompt_registry_selects_and_renders_every_autonomous_domain_without_persisting_messages() -> None:
    registry = AutonomousPromptRegistry(tuple(_template(domain) for domain in AUTONOMOUS_DOMAINS))
    plan = registry.select_for(
        [
            {"domain": domain, "stage": "answer", "required_capabilities": ("analysis",)}
            for domain in AUTONOMOUS_DOMAINS
        ]
    )
    assert len(plan.rows) == len(AUTONOMOUS_DOMAINS)
    assert plan.registry_digest == registry.registry_digest
    assert len(plan.plan_digest) == 64

    result = registry.render(plan, _context("science"))
    assert result.messages[0]["content"] == "transient prompt"
    projection = json.dumps(result.to_dict())
    assert "transient prompt" not in projection
    assert result.to_dict()["retention"] == "rendered_messages_transient;digest_only_projection"


def test_prompt_registry_rejects_stale_selection_after_template_replacement() -> None:
    registry = AutonomousPromptRegistry((_template("coding"),))
    plan = registry.select_for([{"domain": "coding", "stage": "answer", "required_capabilities": ("analysis",)}])
    registry.register(_template("coding", content="replacement prompt"), replace=True)
    with pytest.raises(ArgumentError, match="stale"):
        registry.verify_selection(plan)


def test_prompt_registry_rejects_credential_shaped_rendered_fields_and_tampered_plan() -> None:
    unsafe = AutonomousPromptTemplate(
        prompt_id="unsafe-prompt",
        version="1",
        domain="science",
        capabilities=("analysis",),
        stages=("answer",),
        template_digest="a" * 64,
        render=lambda _context: [{"role": "user", "content": {"api_key": "must-not-cross"}}],
    )
    with pytest.raises(ArgumentError, match="credential-shaped"):
        unsafe.render_transient(_context("science"))

    registry = AutonomousPromptRegistry((_template("science"),))
    plan = registry.select_for([{"domain": "science", "stage": "answer", "required_capabilities": ("analysis",)}])
    payload = plan.to_dict()
    payload["rows"][0]["selected_manifest_digest"] = "0" * 64  # type: ignore[index]
    payload["plan_digest"] = AutonomousPromptSelectionPlan.from_dict({**payload, "plan_digest": None}).plan_digest  # type: ignore[arg-type]
    with pytest.raises(ArgumentError, match="stale or tampered"):
        registry.verify_selection(payload)


def test_llm_adapter_can_invoke_only_after_prompt_selection_and_binds_rendered_digest_to_request() -> None:
    runtime = LLMRuntime()
    requests: list[object] = []

    def handler(request: object) -> dict[str, object]:
        requests.append(request)
        return {"text": json.dumps({"answer": "ok"})}

    runtime.register_in_memory_provider("prompt-fixture", handler)
    registry = AutonomousPromptRegistry((_template("science"),))
    plan = registry.select_for([{"domain": "science", "stage": "answer", "required_capabilities": ("analysis",)}])
    adapter = create_autonomous_llm_evidence_adapter(
        adapter_id="science-prompt-adapter",
        version="1",
        domain="science",
        provider="prompt-fixture",
        runtime=runtime,
        capabilities=("llm_evidence",),
        model="fixture-model",
        prompt_registry=registry,
        prompt_selection=plan,
        require_json=True,
    )
    adapter.acquire(_context("science"))
    request = requests[0]
    assert request.messages[0]["content"] == "transient prompt"  # type: ignore[attr-defined]
    assert len(request.idempotency_key) == 64  # type: ignore[attr-defined]
    assert adapter.to_dict()["prompt"]["mode"] == "registry_selection"  # type: ignore[index]
