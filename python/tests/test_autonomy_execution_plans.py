import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousCapabilityActivation,
    AutonomousDomainTool,
    AutonomousDomainToolBinding,
    AutonomousDomainToolRegistry,
    BrainRunError,
    compile_autonomous_workflow_stage_execution_plan,
    DomainEvaluationEvidence,
    LLMRuntime,
    ModelCatalogue,
    plan_mcp_catalogue_bindings,
)


class _Workspace:
    pass


def _model() -> dict[str, object]:
    return {
        "provider": "local",
        "model": "reasoning-model",
        "capabilities": [
            "reasoning",
            "code",
            "web",
            "data",
            "science",
            "biomedical",
            "operations",
            "enterprise",
            "coordination",
            "multimodal",
            "evaluation",
        ],
        "context_window_tokens": 16_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 20,
        "cost_per_million_tokens": 10,
        "requires_credential": False,
    }


def _catalogue() -> list[dict[str, object]]:
    return [
        {
            "name": "repository_catalog",
            "description": "Read bounded repository metadata.",
            "inputSchema": {"type": "object"},
        },
        {
            "name": "repository_bundle",
            "description": "Read a bounded repository bundle.",
            "inputSchema": {"type": "object"},
        },
    ]


def test_execution_plan_compiles_every_domain_and_preserves_runtime_contracts():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )

    packet = agent.execution_plans()

    assert packet["domain_count"] == len(AUTONOMOUS_DOMAINS)
    assert packet["domains"] == list(AUTONOMOUS_DOMAINS)
    assert {plan["domain"] for plan in packet["plans"]} == set(AUTONOMOUS_DOMAINS)
    assert all(plan["plan_digest"] for plan in packet["plans"])
    assert all(plan["workflow"]["stages"] for plan in packet["plans"])
    assert all(plan["evidence"]["obligations"] for plan in packet["plans"])
    assert all(plan["review_gates"]["provider_call_approval_required"] for plan in packet["plans"])
    assert all(plan["learning"]["context_digest"] for plan in packet["plans"])
    assert "local/reasoning-model" in {
        row["arm_id"]
        for plan in packet["plans"]
        for row in plan["models"]["candidates"]
    }
    public = json.dumps(packet)
    assert "api_key" not in public
    assert "test-secret" not in public
    assert "provider.invoke" not in public


def test_activation_plan_limits_runtime_tools_to_exact_approved_names():
    activation = AutonomousCapabilityActivation()
    catalogue = _catalogue()
    binding_plan = plan_mcp_catalogue_bindings(catalogue, domains=("coding",))
    activation.record_binding_plan(binding_plan)
    activation.approve_bindings(binding_plan, ["repository_catalog"])

    registry = AutonomousDomainToolRegistry()
    registry.register_mcp_catalogue(
        catalogue,
        {
            "repository_catalog": AutonomousDomainToolBinding(
                "repository_catalog", ("coding",), "repository_inspection"
            ),
            "repository_bundle": AutonomousDomainToolBinding(
                "repository_bundle", ("coding",), "repository_inspection"
            ),
        },
    )
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
        tool_registry=registry,
        activation=activation,
    )

    plan = agent.domain_execution_plan("coding")
    assert [row["name"] for row in plan["tools"]["registered"]] == ["repository_catalog"]
    assert [row["name"] for row in plan["tools"]["withheld"]] == ["repository_bundle"]
    assert plan["activation"]["authority"] == "activation_approved_tools_only"

    _, _, options, _ = agent._execution_inputs(
        credentials={},
        model_candidates=None,
        options={},
        tool_domains=("coding",),
    )
    assert [tool.name for tool in options["provider_tools"]] == ["repository_catalog"]
    runtime_plan = options["context"]["_aurora_execution_plan"]
    assert runtime_plan["plans"][0]["domain"] == "coding"
    assert runtime_plan["plans"][0]["activation"]["approved_tool_count"] == 1


def test_runtime_plan_context_cannot_be_spoofed_by_caller_context():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )

    with pytest.raises(BrainRunError, match="cannot override"):
        agent._execution_inputs(
            credentials={},
            model_candidates=None,
            options={"context": {"_aurora_execution_plan": {"status": "ready"}}},
            tool_domains=("coding",),
        )


def test_capability_adapters_and_evidence_contracts_cover_every_domain():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )

    packet = agent.capability_plans()

    assert packet["capability_count"] >= len(AUTONOMOUS_DOMAINS)
    assert {plan["domain"] for plan in packet["plans"]} == set(AUTONOMOUS_DOMAINS)
    for domain in AUTONOMOUS_DOMAINS:
        domain_plan = agent.domain_execution_plan(domain)
        contracts = domain_plan["capabilities"]["contracts"]
        names = {row["capability"] for row in contracts}
        assert set(domain_plan["profile"]["required_model_capabilities"])
        assert set(domain_plan["domain_pack"]["tool_capabilities"]).issubset(names)
        assert all(row["contract"]["contract_digest"] == row["contract_digest"] for row in contracts)
        assert all(row["evidence_outputs"] for row in contracts)
        assert all(row["evaluator_signals"] for row in contracts)
        assert domain_plan["capabilities"]["adapter_posture"] == "reviewed_exact_aliases; no_fuzzy_matching"

    coding_debug = agent.domain_capability_plan("coding", "debugging")
    assert "repository_inspection" in coding_debug["contract"]["tool_capabilities"]
    assert coding_debug["contract"]["evidence_outputs"]
    assert coding_debug["contract"]["evaluator_signals"]


def test_capability_dispatch_narrows_provider_tools_and_binds_the_reviewed_contract():
    registry = AutonomousDomainToolRegistry(
        [
            AutonomousDomainTool(
                name="repository_catalog",
                domains=("coding",),
                capability="repository_inspection",
                description="Read bounded repository metadata.",
                parameters={"type": "object"},
            ),
            AutonomousDomainTool(
                name="developer_workbench",
                domains=("coding",),
                capability="developer_workbench",
                description="Read bounded workbench metadata.",
                parameters={"type": "object"},
            ),
        ]
    )
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
        tool_registry=registry,
    )
    reviewed = agent.domain_capability_plan("coding", "debugging")

    _, _, options, _ = agent._execution_inputs(
        credentials={},
        model_candidates=None,
        options={
            "_aurora_capability_focus": "debugging",
            "_aurora_capability_contract": reviewed["contract"],
        },
        tool_domains=("coding",),
    )

    assert [tool.name for tool in options["provider_tools"]] == ["repository_catalog"]
    assert options["context"]["_aurora_capability_contract"]["capability"] == "debugging"
    assert options["selection_overrides"]["autonomy_capability_focus"] == "debugging"


def test_capability_dispatch_rejects_caller_contract_spoofing():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )
    reviewed = agent.domain_capability_plan("operations", "observability")

    with pytest.raises(BrainRunError, match="stale or does not match"):
        agent._execution_inputs(
            credentials={},
            model_candidates=None,
            options={
                "_aurora_capability_focus": "observability",
                "_aurora_capability_contract": {
                    **reviewed["contract"],
                    "evidence_outputs": ["invented_output"],
                },
            },
            tool_domains=("operations",),
        )


def test_run_capability_uses_the_same_provider_invocation_boundary():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )
    captured: dict[str, object] = {}

    def capture_run(**kwargs: object) -> str:
        captured.update(kwargs)
        return "capability-run-captured"

    agent.orchestrator.run = capture_run  # type: ignore[method-assign]
    result = agent.run_capability(
        task="inspect bounded repository evidence",
        domain="coding",
        capability="debugging",
        credentials={},
        approve_provider_call=True,
    )

    assert result == "capability-run-captured"
    assert captured["capability"] == "debugging"
    context = captured["context"]
    assert isinstance(context, dict)
    assert context["_aurora_capability_contract"]["capability"] == "debugging"
    assert context["_aurora_execution_plan"]["plans"][0]["domain"] == "coding"
    assert "code" in captured["required_model_capabilities"]


def test_approval_gated_capability_needs_capability_approval_separate_from_provider_approval():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )

    with pytest.raises(BrainRunError, match="requires explicit capability approval"):
        agent.run_capability(
            task="prepare the operational approval packet",
            domain="operations",
            capability="approval",
            credentials={},
            approve_provider_call=True,
        )


def test_stage_execution_plan_narrows_tools_and_binds_evidence_contracts():
    registry = AutonomousDomainToolRegistry(
        [
            AutonomousDomainTool(
                name="repository_catalog",
                domains=("coding",),
                capability="repository_inspection",
                description="Read bounded repository metadata.",
                parameters={"type": "object"},
            ),
            AutonomousDomainTool(
                name="ci_evidence_audit",
                domains=("coding",),
                capability="ci_evidence_audit",
                description="Read bounded CI evidence.",
                parameters={"type": "object"},
            ),
        ]
    )
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
        tool_registry=registry,
    )
    blueprint = agent.prepare(task="inspect a bounded repository failure", domain="coding")
    execution_plan = agent.domain_execution_plan("coding")
    inspect = next(stage for stage in blueprint.workflow.stages if stage.id == "inspect")
    provider_tools = tuple(registry.provider_tools(("coding",)))

    stage_plan = compile_autonomous_workflow_stage_execution_plan(
        blueprint,
        inspect,
        execution_plan_context=execution_plan,
        provider_tools=provider_tools,
    )

    assert stage_plan.execution_posture == "tool_backed"
    assert stage_plan.selected_tool_names == ("ci_evidence_audit", "repository_catalog")
    assert set(stage_plan.required_capabilities) == {"review", "debugging"}
    assert stage_plan.evidence_outputs == ("observations", "evidence_gaps")
    assert stage_plan.evaluator_signals == ("evidence_complete",)
    assert stage_plan.stage_plan_digest
    assert len(stage_plan.capability_contract_digests) == 2
    public = json.dumps(stage_plan.to_dict())
    assert "inspect a bounded repository failure" not in public
    assert "api_key" not in public

    tampered_plan = json.loads(json.dumps(execution_plan))
    tampered_contract = next(
        row for row in tampered_plan["capabilities"]["contracts"]
        if row["capability"] == "debugging"
    )
    tampered_contract["contract_digest"] = "d" * 64
    with pytest.raises(BrainRunError, match="stale capability contract"):
        compile_autonomous_workflow_stage_execution_plan(
            blueprint,
            inspect,
            execution_plan_context=tampered_plan,
            provider_tools=provider_tools,
        )


def test_workflow_stage_plan_is_retained_in_checkpoint_and_learning_evidence():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )
    blueprint = agent.prepare(task="prepare a bounded coding workflow", domain="coding")
    inspect = next(stage for stage in blueprint.workflow.stages if stage.id == "inspect")
    stage_plan = compile_autonomous_workflow_stage_execution_plan(blueprint, inspect)
    checkpoint = blueprint.workflow.stage_response_schema("inspect")
    assert checkpoint["required"]
    evidence = agent.orchestrator._workflow_stage_evidence(
        blueprint,
        inspect,
        {"signals": {"evidence_complete": True}},
        stage_plan.to_dict(),
    )

    assert evidence is not None
    assert evidence["stage_plan_digest"] == stage_plan.stage_plan_digest
    assert evidence["capability_contract_digests"] == list(stage_plan.capability_contract_digests)
    assert evidence["selected_tool_names"] == []


def test_stage_execution_plan_compiles_every_stage_in_every_builtin_domain():
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()]),
    )

    compiled = []
    expected_count = 0
    for domain in AUTONOMOUS_DOMAINS:
        blueprint = agent.prepare(task=f"prepare a bounded {domain} workflow", domain=domain)
        execution_plan = agent.domain_execution_plan(domain)
        expected_count += len(blueprint.workflow.stages)
        for stage in blueprint.workflow.stages:
            stage_plan = compile_autonomous_workflow_stage_execution_plan(
                blueprint,
                stage,
                execution_plan_context=execution_plan,
            )
            compiled.append(stage_plan)
            assert stage_plan.domain == domain
            assert stage_plan.stage_id == stage.id
            assert stage_plan.required_capabilities
            assert stage_plan.evidence_outputs == stage.evidence_outputs
            assert stage_plan.evaluator_signals == stage.evaluator_signals
            assert stage_plan.stage_plan_digest

    assert len(compiled) == expected_count
    assert len(compiled) >= len(AUTONOMOUS_DOMAINS) * 4


def test_evaluator_evidence_round_trips_stage_contract_digests_as_value_only_data():
    stage_plan = "a" * 64
    contract_digests = ("b" * 64, "c" * 64)
    evidence = DomainEvaluationEvidence.from_mapping(
        {
            "domain": "coding",
            "capability": "debugging",
            "risk_class": "read_only_analysis",
            "signals": {"evidence_complete": True},
            "stage_plan_digest": stage_plan,
            "capability_contract_digests": list(contract_digests),
            "selected_tool_names": ["repository_catalog"],
        }
    )

    assert evidence.stage_plan_digest == stage_plan
    assert evidence.capability_contract_digests == contract_digests
    assert evidence.to_dict()["selected_tool_names"] == ["repository_catalog"]
