from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import threading

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousCapabilityRuntime,
    DOMAIN_TOOL_BINDING_PLAN_SCHEMA,
    AutonomousDomainTool,
    AutonomousDomainToolBinding,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    InMemoryAutonomousCapabilityJournalStore,
    ProviderToolCall,
    ToolCatalogue,
    builtin_autonomous_workflow_strategies,
    builtin_autonomous_domain_tool_profiles,
    content_digest,
    plan_mcp_catalogue_bindings,
)
from prism_sdk.autonomy import _AUTONOMOUS_CAPABILITY_TOOL_ALIASES
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


def test_capability_runtime_executes_a_reviewed_stage_for_every_builtin_domain() -> None:
    tool_profiles = {profile.domain: profile for profile in builtin_autonomous_domain_tool_profiles()}
    workflows = {workflow.domain: workflow for workflow in builtin_autonomous_workflow_strategies()}
    tools: list[AutonomousDomainTool] = []
    requests: list[dict[str, object]] = []
    for domain in AUTONOMOUS_DOMAINS:
        workflow = workflows[domain]
        stage = workflow.stages[0]
        aliases = _AUTONOMOUS_CAPABILITY_TOOL_ALIASES[domain]
        binding = next(
            binding
            for binding in tool_profiles[domain].bindings
            if any(
                binding.capability == required
                or binding.capability in aliases.get(required, ())
                for required in stage.required_capabilities
            )
        )
        tools.append(
            AutonomousDomainTool(
                binding.name,
                (domain,),
                binding.capability,
                f"Read bounded {domain} state.",
                {"type": "object", "additionalProperties": False},
            )
        )
        requests.append(
            {
                "call_id": f"call-{domain}",
                "tool": binding.name,
                "arguments": {},
                "workflow_context": {
                    "domain": domain,
                    "workflow_id": workflow.workflow_id,
                    "workflow_digest": workflow.workflow_digest,
                    "stage_id": stage.id,
                },
                "input_digest": content_digest({"domain": domain}),
                "subject_digest": None,
                "parent_evidence_digests": [],
                "replay_key": f"replay-{domain}",
                "execution_id": f"execution-{domain}",
            }
        )

    registry = AutonomousDomainToolRegistry(tools)
    executed: list[str] = []
    runtime = AutonomousDomainToolRuntime(
        registry,
        executor=lambda tool, _arguments: executed.append(tool.name) or {"status": "ok"},
    )
    capability_runtime = AutonomousCapabilityRuntime(runtime)

    results = capability_runtime.execute_batch(requests, max_parallelism=4)

    assert len(results) == len(AUTONOMOUS_DOMAINS)
    assert all(result.record.status == "completed" for result in results)
    assert all(result.record.stage_contract_digest for result in results)
    assert all(result.record.output_digest for result in results)
    assert len(executed) == len(AUTONOMOUS_DOMAINS)


def test_capability_journal_rehydrates_without_replaying_or_retaining_adapter_values() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "operations")
    stage = workflow.stages[0]
    tool = AutonomousDomainTool(
        "operations_status",
        ("operations",),
        "observability",
        "Read bounded operations status.",
        {"type": "object", "additionalProperties": False},
    )
    registry = AutonomousDomainToolRegistry([tool])
    executions: list[str] = []
    journal = InMemoryAutonomousCapabilityJournalStore()
    capability_runtime = AutonomousCapabilityRuntime(
        AutonomousDomainToolRuntime(
            registry,
            executor=lambda resolved, _arguments: executions.append(resolved.name) or {"private": "transient"},
        ),
        journal=journal,
    )
    request = {
        "call_id": "operations-call-1",
        "tool": tool.name,
        "arguments": {},
        "workflow_context": {
            "domain": "operations",
            "workflow_id": workflow.workflow_id,
            "workflow_digest": workflow.workflow_digest,
            "stage_id": stage.id,
        },
        "input_digest": content_digest({"input": "operations"}),
        "subject_digest": None,
        "parent_evidence_digests": [],
        "replay_key": "operations-replay-1",
        "execution_id": "operations-execution-1",
    }

    first = capability_runtime.execute(request)
    snapshot = journal.snapshot().to_dict()
    restored_journal = InMemoryAutonomousCapabilityJournalStore()
    restored_journal.restore(snapshot)
    restarted = AutonomousCapabilityRuntime(
        AutonomousDomainToolRuntime(
            registry,
            executor=lambda _resolved, _arguments: executions.append("unexpected-replay"),
        ),
        journal=restored_journal,
    )

    assert restarted.rehydrate()["replayable"] == 1
    replayed = restarted.execute(request)

    assert first.record.status == "completed"
    assert replayed.record.replay == "replayed"
    assert replayed.value is None
    assert executions == ["operations_status"]
    record_snapshot = snapshot["entries"][0]["record"]
    assert "private" not in record_snapshot
    assert "output" not in record_snapshot
    assert "arguments" not in record_snapshot


def test_capability_runtime_deduplicates_concurrent_identical_requests() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "operations")
    stage = workflow.stages[0]
    tool = AutonomousDomainTool(
        "operations_status_concurrent",
        ("operations",),
        "observability",
        "Read bounded operations status.",
        {"type": "object", "additionalProperties": False},
    )
    registry = AutonomousDomainToolRegistry([tool])
    started = threading.Event()
    release = threading.Event()
    executions: list[str] = []

    def executor(_resolved: AutonomousDomainTool, _arguments: object) -> dict[str, str]:
        executions.append("started")
        started.set()
        assert release.wait(timeout=2)
        return {"status": "ok"}

    capability_runtime = AutonomousCapabilityRuntime(
        AutonomousDomainToolRuntime(registry, executor=executor)
    )
    request = {
        "call_id": "concurrent-call",
        "tool": tool.name,
        "arguments": {},
        "workflow_context": {
            "domain": "operations",
            "workflow_id": workflow.workflow_id,
            "workflow_digest": workflow.workflow_digest,
            "stage_id": stage.id,
        },
        "input_digest": content_digest({"input": "concurrent"}),
        "subject_digest": None,
        "parent_evidence_digests": [],
        "replay_key": "concurrent-replay",
        "execution_id": "concurrent-execution",
    }

    with ThreadPoolExecutor(max_workers=4) as pool:
        futures = [pool.submit(capability_runtime.execute, request) for _ in range(4)]
        assert started.wait(timeout=2)
        release.set()
        results = [future.result(timeout=2) for future in futures]

    assert len(executions) == 1
    assert sorted(result.record.replay for result in results) == ["fresh", "replayed", "replayed", "replayed"]
