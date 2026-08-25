from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousBrain,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    AutonomousPromptLearningState,
    select_adaptive_autonomous_prompts,
    settle_autonomous_prompt_selection,
    AutonomousTaskOrchestrator,
    LLMRuntime,
    builtin_autonomous_prompt_registry,
    builtin_autonomous_prompt_templates,
    create_autonomous_llm_evidence_adapter,
    content_digest,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.brain import _context_identity_digest


class _PromptWorkspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        if name == "brain_model_select_contextual":
            raw_context = args["context"]
            assert isinstance(raw_context, dict)
            identity = {field: raw_context.get(field) for field in ("domain", "capability", "risk_class", "task_family")}
            return {
                "context_digest": _context_identity_digest(identity),
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "openai", "model": "prompt-model"},
                    "decision_digest": "d" * 64,
                },
            }
        if name == "brain_model_select":
            return {"selected_model": {"provider": "openai", "model": "prompt-model"}, "decision_digest": "d" * 64}
        if name == "brain_prompt_assemble":
            raise AssertionError("verified prompt override should bypass workspace prompt assembly")
        if name == "brain_plan":
            return {
                "ok": True,
                "plan": {
                    "requires_approval": True,
                    "steps": [{"effect": "provider_call"}],
                    "plan_digest": "b" * 64,
                },
            }
        raise AssertionError(f"unexpected workspace tool: {name}")


def _prompt_model() -> dict[str, object]:
    return {
        "provider": "openai",
        "model": "prompt-model",
        "capabilities": [
            "reasoning", "code", "science", "data", "web", "biomedical", "neuroscience",
            "operations", "enterprise", "coordination", "multimodal", "evaluation",
        ],
        "context_window_tokens": 16_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 20,
        "cost_per_million_tokens": 10,
        "reliability": 0.95,
    }


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


def test_builtin_specialist_prompt_pack_covers_every_domain_with_domain_specific_capabilities() -> None:
    registry = builtin_autonomous_prompt_registry()
    assert len(registry.manifests) == len(AUTONOMOUS_DOMAINS)
    assert {manifest.domain for manifest in registry.manifests} == set(AUTONOMOUS_DOMAINS)
    plan = registry.select_for(
        [
            {
                "domain": domain,
                "stage": "answer",
                "required_capabilities": ("analysis", f"domain:{domain}"),
            }
            for domain in AUTONOMOUS_DOMAINS
        ]
    )
    for domain in AUTONOMOUS_DOMAINS:
        context = _context(domain)
        context["requirement"] = {
            "domain": domain,
            "stage_id": "answer",
            "requirement_id": f"{domain}:answer:answer",
            "objective": f"Produce a useful reviewed result for {domain}.",
        }
        rendered = registry.render(plan, context)
        assert f"{domain} specialist" in rendered.messages[0]["content"]
        assert f"Produce a useful reviewed result for {domain}." in rendered.messages[1]["content"]
        assert f"Produce a useful reviewed result for {domain}." not in json.dumps(rendered.to_dict())


def test_builtin_prompt_pack_rejects_partial_duplicates_and_missing_objectives() -> None:
    assert len(builtin_autonomous_prompt_templates(("science", "evaluation"))) == 2
    with pytest.raises(ArgumentError, match="duplicate"):
        builtin_autonomous_prompt_templates(("science", "science"))
    with pytest.raises(ArgumentError, match="unsupported"):
        builtin_autonomous_prompt_templates(("not-a-domain",))
    registry = builtin_autonomous_prompt_registry(("science",))
    plan = registry.select_for([{"domain": "science", "stage": "answer", "required_capabilities": ("analysis",)}])
    with pytest.raises(ArgumentError, match="requires a bounded objective"):
        registry.render(plan, _context("science"))


def test_prompt_learning_explores_registry_arms_and_settles_idempotently_without_retaining_prompt_text() -> None:
    registry = AutonomousPromptRegistry(
        (
            _template("science", "prompt-science-a", content="variant A transient"),
            _template("science", "prompt-science-b", content="variant B transient"),
        )
    )
    state = AutonomousPromptLearningState.empty(registry.registry_digest)
    first = select_adaptive_autonomous_prompts(
        registry,
        [{"domain": "science", "stage": "answer", "required_capabilities": ()}],
        state=state,
    )
    assert first.arm_ids[0]
    settled = settle_autonomous_prompt_selection(
        registry,
        state,
        first,
        arm_id=first.arm_ids[0],
        evaluator_id="science-rubric",
        evaluator_version="1",
        reward=0.9,
        passed=True,
        settlement_key="a" * 64,
    )
    assert settled.status == "settled"
    assert settled.next_state.generation == 1
    second = select_adaptive_autonomous_prompts(
        registry,
        [{"domain": "science", "stage": "answer", "required_capabilities": ()}],
        state=settled.next_state,
    )
    assert second.arm_ids[0] != first.arm_ids[0]
    replay = settle_autonomous_prompt_selection(
        registry,
        settled.next_state,
        first,
        arm_id=first.arm_ids[0],
        evaluator_id="science-rubric",
        evaluator_version="1",
        reward=0.9,
        passed=True,
        settlement_key="a" * 64,
    )
    assert replay.status == "replayed"
    assert replay.next_state.state_digest == settled.next_state.state_digest
    projection = json.dumps(settled.next_state.to_dict())
    assert "variant A transient" not in projection
    assert "variant B transient" not in projection


def test_prompt_learning_rejects_stale_registry_and_credential_shaped_ledger_state() -> None:
    registry = AutonomousPromptRegistry((_template("science", "prompt-science-a"),))
    state = AutonomousPromptLearningState.empty(registry.registry_digest)
    selection = select_adaptive_autonomous_prompts(
        registry,
        [{"domain": "science", "stage": "answer", "required_capabilities": ()}],
        state=state,
    )
    registry.register(_template("science", "prompt-science-a", content="replacement"), replace=True)
    with pytest.raises(ArgumentError, match="stale"):
        settle_autonomous_prompt_selection(
            registry,
            state,
            selection,
            arm_id=selection.arm_ids[0],
            evaluator_id="science-rubric",
            evaluator_version="1",
            reward=0.5,
            passed=True,
        )
    with pytest.raises(ArgumentError, match="fields"):
        AutonomousPromptLearningState.from_dict(
            {
                **state.to_dict(),
                "settlements": [{"secret": "must-not-cross"}],
                "state_digest": None,
            }
        )


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


def test_builtin_specialist_prompt_pack_drives_an_offline_provider_invocation() -> None:
    runtime = LLMRuntime()
    requests: list[object] = []
    runtime.register_in_memory_provider(
        "builtin-prompt-fixture",
        lambda request: (requests.append(request) or {"text": json.dumps({"answer": "ok"})}),
    )
    registry = builtin_autonomous_prompt_registry(("biomedical",))
    plan = registry.select_for([{"domain": "biomedical", "stage": "answer", "required_capabilities": ("analysis", "domain:biomedical")}])
    context = _context("biomedical")
    context["requirement"] = {
        "domain": "biomedical",
        "stage_id": "answer",
        "requirement_id": "biomedical:answer:answer",
        "objective": "Compare bounded evidence without making a clinical recommendation.",
    }
    adapter = create_autonomous_llm_evidence_adapter(
        adapter_id="biomedical-builtin-prompt",
        version="1",
        domain="biomedical",
        provider="builtin-prompt-fixture",
        runtime=runtime,
        capabilities=("llm_evidence",),
        model="fixture-model",
        prompt_registry=registry,
        prompt_selection=plan,
        require_json=True,
    )
    adapter.acquire(context)
    assert requests[0].messages[0]["role"] == "system"  # type: ignore[attr-defined]
    assert "never diagnose" in requests[0].messages[0]["content"]  # type: ignore[attr-defined]


def test_high_level_orchestrator_uses_builtin_prompt_registry_for_direct_runs() -> None:
    runtime = LLMRuntime()
    requests: list[object] = []
    runtime.register_in_memory_provider(
        "openai",
        lambda request: (requests.append(request) or {"text": "bounded prompt answer"}),
    )
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(_PromptWorkspace(), runtime))
    prompt_registry = builtin_autonomous_prompt_registry()
    result = orchestrator.run(
        task="Review a bounded neuroscience experiment.",
        domain="neuroscience",
        model_candidates=[_prompt_model()],
        credentials={},
        approve_provider_call=True,
        prompt_registry=prompt_registry,
        prompt_learning_state=AutonomousPromptLearningState.empty(prompt_registry.registry_digest),
    )
    assert result.status == "completed_provider_call"
    assert len(requests) == 1
    assert requests[0].messages[0]["role"] == "system"  # type: ignore[attr-defined]
    assert "neuroscience specialist" in requests[0].messages[0]["content"]  # type: ignore[attr-defined]
    prompt_projection = result.prompt["autonomous_prompt"]
    assert prompt_projection["mode"] == "registry_selection"  # type: ignore[index]
    assert prompt_projection["selection_policy"] == "ucb1_explicit_evaluator_v1"  # type: ignore[index]
    assert len(prompt_projection["adaptive_selection_digest"]) == 64  # type: ignore[index]
    assert len(requests[0].idempotency_key) == 64  # type: ignore[attr-defined]
    public_prompt = result.to_dict()["prompt"]
    assert "messages" not in public_prompt
    assert public_prompt["message_count"] >= 2


def test_high_level_cross_domain_prompt_registry_reaches_children_and_synthesis() -> None:
    runtime = LLMRuntime()
    requests: list[object] = []
    runtime.register_in_memory_provider(
        "openai",
        lambda request: (requests.append(request) or {"text": f"answer-{len(requests)}"}),
    )
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(_PromptWorkspace(), runtime))
    result = orchestrator.run_cross_domain(
        task="Compare a biomedical intervention with a neuroscience signal study.",
        subtasks=(
            {"id": "bio", "domain": "biomedical", "task": "Review the biomedical safety evidence."},
            {"id": "neuro", "domain": "neuroscience", "task": "Review the neuroscience signal limits."},
        ),
        model_candidates=[_prompt_model()],
        credentials={},
        approve_provider_call=True,
        prompt_registry=builtin_autonomous_prompt_registry(),
    )
    assert result.status == "completed"
    assert len(requests) == 3
    assert all("autonomous_prompt" in run.prompt for run in result.child_results)
    assert result.synthesis_result is not None
    assert result.synthesis_result.prompt["autonomous_prompt"]["domain"] == "cross_domain"  # type: ignore[index]


def test_high_level_run_refuses_a_stale_prompt_selection_before_dispatch() -> None:
    runtime = LLMRuntime()
    requests: list[object] = []
    runtime.register_in_memory_provider(
        "openai",
        lambda request: (requests.append(request) or {"text": "must not dispatch"}),
    )
    registry = builtin_autonomous_prompt_registry(("coding",))
    selection = registry.select_for([{"domain": "coding", "stage": "answer", "required_capabilities": ()}])
    registry.register(
        _template("coding", prompt_id="builtin.coding.specialist", content="replacement"),
        replace=True,
    )
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(_PromptWorkspace(), runtime))
    with pytest.raises(ArgumentError, match="stale"):
        orchestrator.run(
            task="Review a bounded implementation.",
            domain="coding",
            model_candidates=[_prompt_model()],
            credentials={},
            approve_provider_call=True,
            prompt_registry=registry,
            prompt_selection=selection,
        )
    assert requests == []
