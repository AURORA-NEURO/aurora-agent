from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    ProviderToolCall,
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
