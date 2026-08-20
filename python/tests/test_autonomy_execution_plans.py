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
