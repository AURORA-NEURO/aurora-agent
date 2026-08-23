from __future__ import annotations

import inspect

import pytest

from prism_sdk import (
    AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA,
    AUTONOMOUS_DOMAINS,
    ApiClient,
    AutonomousApiToolError,
    AutonomousDomainTool,
    ToolCatalogue,
    builtin_autonomous_domain_tool_profiles,
    create_autonomous_api_tool_executor,
)
from prism_sdk.errors import ApiError, ArgumentError


def _catalogue_and_tools() -> tuple[ToolCatalogue, dict[str, AutonomousDomainTool]]:
    profiles = builtin_autonomous_domain_tool_profiles()
    definitions: dict[str, dict[str, object]] = {}
    tools: dict[str, AutonomousDomainTool] = {}
    for profile in profiles:
        binding = profile.bindings[0]
        definitions.setdefault(
            binding.name,
            {
                "name": binding.name,
                "description": f"Reviewed {binding.name} bridge.",
                "inputSchema": {"type": "object", "additionalProperties": True},
            },
        )
        tools[profile.domain] = AutonomousDomainTool.from_mcp_definition(
            definitions[binding.name],
            domains=binding.domains,
            capability=binding.capability,
            risk_class=binding.risk_class,
            read_only=binding.read_only,
            approval_required=binding.approval_required,
        )
    return ToolCatalogue.from_definitions(list(definitions.values())), tools


def test_reviewed_api_executor_invokes_every_builtin_domain_without_discovery(monkeypatch) -> None:
    catalogue, tools = _catalogue_and_tools()
    client = ApiClient("https://prism.test")
    calls: list[tuple[str, dict[str, object]]] = []

    def call_tool(name: str, arguments: dict[str, object]) -> dict[str, object]:
        calls.append((name, arguments))
        return {
            "ok": True,
            "tool": name,
            "request_id": f"request-{len(calls)}",
            "mcp": {"result": {"structuredContent": {"checked": True, "tool": name}}},
        }

    monkeypatch.setattr(client, "call_tool", call_tool)
    monkeypatch.setattr(client, "tools", lambda: (_ for _ in ()).throw(AssertionError("discovery is forbidden")))
    executor = create_autonomous_api_tool_executor(client, catalogue=catalogue)

    assert AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA.endswith("/0.1")
    assert set(tools) == set(AUTONOMOUS_DOMAINS)
    for domain, tool in tools.items():
        assert executor(tool, {}) == {"checked": True, "tool": tool.name}
        assert calls[-1] == (tool.name, {})
    assert len(calls) == len(AUTONOMOUS_DOMAINS)


def test_api_executor_requires_the_exact_reviewed_catalogue_and_bounds_failures(monkeypatch) -> None:
    catalogue, tools = _catalogue_and_tools()
    client = ApiClient("https://prism.test")
    executor = create_autonomous_api_tool_executor(client, catalogue=catalogue)
    tool = tools["coding"]

    monkeypatch.setattr(
        client,
        "call_tool",
        lambda _name, _arguments: {
            "ok": True,
            "mcp": {"result": {"isError": True, "content": [{"text": "private-server-detail"}]}},
        },
    )
    with pytest.raises(AutonomousApiToolError) as refusal:
        executor(tool, {})
    assert refusal.value.reason == "remote_refusal"
    assert "private-server-detail" not in str(refusal.value)

    monkeypatch.setattr(
        client,
        "call_tool",
        lambda _name, _arguments: (_ for _ in ()).throw(
            ApiError(503, {"error": "Bearer private-token-material"})
        ),
    )
    with pytest.raises(AutonomousApiToolError) as transport:
        executor(tool, {})
    assert transport.value.reason == "transport_failed"
    assert "private-token-material" not in str(transport.value)

    monkeypatch.setattr(client, "call_tool", lambda _name, _arguments: {"ok": True, "mcp": {}})
    with pytest.raises(AutonomousApiToolError) as malformed:
        executor(tool, {})
    assert malformed.value.reason == "invalid_response"

    unknown = AutonomousDomainTool(
        "unreviewed_tool",
        ("coding",),
        "unreviewed",
        "An unreviewed tool.",
        {"type": "object"},
    )
    with pytest.raises(AutonomousApiToolError) as schema:
        executor(unknown, {})
    assert schema.value.reason == "schema_refused"


def test_api_executor_and_catalogue_reject_credential_shaped_configuration() -> None:
    catalogue, _tools = _catalogue_and_tools()
    with pytest.raises(ArgumentError):
        create_autonomous_api_tool_executor(object(), catalogue=catalogue)  # type: ignore[arg-type]
    with pytest.raises(ArgumentError):
        create_autonomous_api_tool_executor(ApiClient("https://prism.test"), catalogue=object())  # type: ignore[arg-type]

    # Credential provisioning remains on ApiClient/ProviderOnboarding; the adapter has no key
    # parameter and cannot accidentally become a second credential ingestion surface.
    assert "bearer_token" not in inspect.signature(create_autonomous_api_tool_executor).parameters
