from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    DOMAIN_TOOL_BINDING_PLAN_SCHEMA,
    AutonomousDomainTool,
    AutonomousDomainToolBinding,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    ProviderToolCall,
    ToolCatalogue,
    builtin_autonomous_domain_tool_profiles,
    plan_mcp_catalogue_bindings,
)
from prism_sdk.errors import ArgumentError


def _read_tool() -> AutonomousDomainTool:
    return AutonomousDomainTool(
        name="workspace_status",
        domains=("operations", "cross_domain"),
        capability="observability",
        description="Read bounded workspace status.",
        parameters={
            "type": "object",
            "properties": {"scope": {"type": "string"}},
            "required": ["scope"],
            "additionalProperties": False,
        },
    )


def test_registry_projects_one_provider_schema_across_multiple_domains() -> None:
    registry = AutonomousDomainToolRegistry([_read_tool()])

    operations = registry.provider_tools(("operations",))
    coding = registry.provider_tools(("coding",))
    cross_domain = registry.provider_tools(("cross_domain",))

    assert [tool.name for tool in operations] == ["workspace_status"]
    assert [tool.name for tool in coding] == ["workspace_status"]
    assert [tool.name for tool in cross_domain] == ["workspace_status"]
    assert registry.catalogue(("operations",))[0]["schema_digest"]
    assert registry.to_dict()["digest"] == registry.digest


def test_runtime_auto_executes_read_only_and_refuses_unapproved_effects() -> None:
    effect = AutonomousDomainTool(
        name="release_apply",
        domains=("coding",),
        capability="delivery",
        description="Apply an already reviewed release.",
        parameters={"type": "object", "additionalProperties": False},
        risk_class="external_effect",
        read_only=False,
        approval_required=True,
    )
    registry = AutonomousDomainToolRegistry([_read_tool(), effect])
    executed: list[str] = []

    runtime = AutonomousDomainToolRuntime(
        registry,
        executor=lambda tool, _arguments: executed.append(tool.name) or {"status": "ok"},
    )
    read_result = runtime((ProviderToolCall("read-1", "workspace_status", {"scope": "repo"}),))
    effect_result = runtime((ProviderToolCall("effect-1", "release_apply", {}),))

    assert read_result[0].approved is True
    assert read_result[0].is_error is False
    assert effect_result[0].approved is False
    assert effect_result[0].is_error is True
    assert executed == ["workspace_status"]
    assert runtime.receipts[-1].status == "approval_required"
    assert "repo" not in str(runtime.receipts[-1].to_dict())

    approved_runtime = AutonomousDomainToolRuntime(
        registry,
        executor=lambda tool, _arguments: executed.append(tool.name) or {"status": "applied"},
        approve=lambda tool, _call: tool.name == "release_apply",
    )
    approved = approved_runtime((ProviderToolCall("effect-2", "release_apply", {}),))
    assert approved[0].approved is True
    assert executed[-1] == "release_apply"


def test_runtime_refuses_schema_mismatch_and_secret_shaped_arguments() -> None:
    registry = AutonomousDomainToolRegistry([_read_tool()])
    runtime = AutonomousDomainToolRuntime(registry, executor=lambda _tool, _arguments: {"status": "ok"})

    missing_required = runtime((ProviderToolCall("bad-1", "workspace_status", {}),))
    secret_argument = runtime(
        (ProviderToolCall("bad-2", "workspace_status", {"scope": "repo", "api_key": "do-not-pass"}),)
    )

    assert missing_required[0].approved is False
    assert secret_argument[0].approved is False
    assert [receipt.status for receipt in runtime.receipts] == ["schema_refused", "schema_refused"]


def test_every_builtin_autonomous_domain_can_have_a_registered_tool_contract() -> None:
    registry = AutonomousDomainToolRegistry(
        [
            AutonomousDomainTool(
                name=f"{domain}_observe",
                domains=(domain,),
                capability="observation",
                description=f"Read bounded {domain} observations.",
                parameters={"type": "object", "additionalProperties": False},
            )
            for domain in AUTONOMOUS_DOMAINS
        ]
    )

    assert len(registry.catalogue()) == len(AUTONOMOUS_DOMAINS)
    for domain in AUTONOMOUS_DOMAINS:
        assert any(tool["domains"] == [domain] for tool in registry.catalogue((domain,)))


def test_effectful_tools_cannot_be_declared_without_approval() -> None:
    with pytest.raises(ArgumentError):
        AutonomousDomainTool(
            name="unsafe_effect",
            domains=("operations",),
            capability="operations",
            description="Must not be implicitly executable.",
            parameters={"type": "object"},
            risk_class="external_effect",
            read_only=False,
            approval_required=False,
        )


def test_registry_binds_a_live_catalogue_only_with_explicit_policy_and_is_atomic() -> None:
    catalogue = ToolCatalogue.from_definitions(
        [
            {
                "name": "operations_status",
                "description": "Read bounded operational status.",
                "inputSchema": {"type": "object", "additionalProperties": False},
            },
            {
                "name": "release_apply",
                "description": "Apply a reviewed release.",
                "inputSchema": {"type": "object", "additionalProperties": False},
            },
        ]
    )
    registry = AutonomousDomainToolRegistry()

    with pytest.raises(ArgumentError, match="missing explicit bindings"):
        registry.register_mcp_catalogue(
            catalogue,
            {
                "operations_status": AutonomousDomainToolBinding(
                    "operations_status", ("operations",), "observability"
                )
            },
        )
    assert registry.catalogue() == []

    registered = registry.register_mcp_catalogue(
        catalogue,
        {
            "operations_status": {
                "domains": ["operations", "cross_domain"],
                "capability": "observability",
            },
            "release_apply": AutonomousDomainToolBinding(
                "release_apply",
                ("operations",),
                "delivery",
                risk_class="external_effect",
                read_only=False,
                approval_required=True,
            ),
        },
    )

    assert [tool.name for tool in registered] == ["operations_status", "release_apply"]
    assert registry.resolve("operations_status").parameters == catalogue.get("operations_status").input_schema
    assert registry.resolve("release_apply").approval_required is True
    assert registry.to_dict()["execution"] == "metadata_only"


def test_registry_rejects_binding_for_a_tool_not_in_the_live_catalogue() -> None:
    catalogue = ToolCatalogue.from_definitions(
        [
            {
                "name": "operations_status",
                "inputSchema": {"type": "object"},
            }
        ]
    )
    with pytest.raises(ArgumentError, match="absent from the live catalogue"):
        AutonomousDomainToolRegistry().register_mcp_catalogue(
            catalogue,
            {
                "typo_status": {
                    "name": "typo_status",
                    "domains": ["operations"],
                    "capability": "observability",
                }
            },
            require_all=False,
        )


def test_binding_planner_covers_every_domain_without_mutating_or_authorizing() -> None:
    catalogue = ToolCatalogue.from_definitions(
        [
            {
                "name": "repository_catalog",
                "description": "Read a bounded repository catalogue.",
                "inputSchema": {"type": "object"},
            },
            {
                "name": "tabular_ingest",
                "description": "Ingest a bounded tabular source.",
                "inputSchema": {"type": "object"},
            },
            {
                "name": "mystery_workspace_tool",
                "description": "Not present in any reviewed profile.",
                "inputSchema": {"type": "object"},
            },
        ]
    )
    registry = AutonomousDomainToolRegistry()

    plan = registry.plan_mcp_catalogue_bindings(catalogue)

    assert plan["schema"] == DOMAIN_TOOL_BINDING_PLAN_SCHEMA
    assert plan["catalogue_digest"] == catalogue.digest
    assert plan["domains"] == list(AUTONOMOUS_DOMAINS)
    assert set(plan["coverage"]) == set(AUTONOMOUS_DOMAINS)
    assert len(builtin_autonomous_domain_tool_profiles()) == len(AUTONOMOUS_DOMAINS)
    assert "repository_catalog" in plan["proposed_bindings"]
    assert plan["proposed_bindings"]["repository_catalog"]["read_only"] is True
    assert plan["proposed_bindings"]["repository_catalog"]["domains"] == ["browser", "coding"]
    assert "tabular_ingest" in plan["review_required_bindings"]
    assert plan["review_required_bindings"]["tabular_ingest"]["approval_required"] is True
    assert "tabular_ingest" not in plan["proposed_bindings"]
    assert plan["unclassified_tools"] == ["mystery_workspace_tool"]
    assert plan["execution"] == "planning_only; no_registry_mutation; no_tool_execution"
    assert registry.catalogue() == []


def test_binding_planner_can_scope_domains_and_reports_missing_exact_capabilities() -> None:
    plan = plan_mcp_catalogue_bindings(
        ToolCatalogue.from_definitions(
            [
                {
                    "name": "world_validate",
                    "inputSchema": {"type": "object"},
                }
            ]
        ),
        domains=("data", "evaluation"),
    )

    assert plan["domains"] == ["data", "evaluation"]
    assert set(plan["coverage"]) == {"data", "evaluation"}
    assert plan["coverage"]["data"]["available_tools"] == ["world_validate"]
    assert plan["coverage"]["evaluation"]["available_tools"] == []
    assert "token_context_plan" in plan["coverage"]["data"]["missing_tools"]
    assert plan["coverage"]["data"]["coverage_ratio"] > 0
