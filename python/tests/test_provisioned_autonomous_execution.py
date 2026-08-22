from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainRunError,
    LLMRuntime,
    ModelCatalogue,
)


class _OfflineWorkspace:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        self.calls.append((name, args))
        if name == "brain_model_select_contextual":
            context = args["context"]
            assert isinstance(context, dict)
            context_identity = {
                field: context.get(field)
                for field in ("domain", "capability", "risk_class")
            }
            context_identity["task_family"] = context.get("task_family")
            context_digest = hashlib.sha256(
                json.dumps(
                    context_identity,
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode("utf-8")
            ).hexdigest()
            return {
                "context_digest": context_digest,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "offline-model"},
                    "decision_digest": "d" * 64,
                },
            }
        if name == "brain_model_select":
            return {
                "selected_model": {"provider": "offline", "model": "offline-model"},
                "decision_digest": "d" * 64,
            }
        if name == "brain_prompt_assemble":
            return {
                "messages": [
                    {"role": "system", "content": str(args.get("system"))},
                    {"role": "user", "content": str(args.get("task"))},
                ],
                "prompt_digest": "a" * 64,
            }
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


def _capabilities(agent: AutonomousAgent) -> list[str]:
    return sorted(
        {
            capability
            for pack in agent.domain_packs()
            for capability in pack["model_capabilities"]
        }
        | {"structured_output", "tool_calling"}
    )


def _candidate(capabilities: list[str]) -> dict[str, object]:
    return {
        "provider": "offline",
        "model": "offline-model",
        "capabilities": capabilities,
        "context_window_tokens": 16_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 20,
        "cost_per_million_tokens": 0,
        "reliability": 0.95,
        "requires_credential": False,
    }


def _agent() -> tuple[AutonomousAgent, _OfflineWorkspace, list[str], list[str]]:
    workspace = _OfflineWorkspace()
    runtime = LLMRuntime()
    calls: list[str] = []

    def invoke(request):
        calls.append(request.model)
        return {"output_text": "offline answer"}

    runtime.register_in_memory_provider("offline", invoke)
    agent = AutonomousAgent(workspace, runtime, model_catalogue=ModelCatalogue())
    capabilities = _capabilities(agent)
    agent.register_model(_candidate(capabilities))
    return agent, workspace, capabilities, calls


def test_explicit_provisioned_execution_is_request_scoped_and_metadata_only():
    agent, _workspace, _capabilities, calls = _agent()

    wrapped = agent.run_with_provisioned_credentials(
        task="produce a bounded implementation review",
        domain="coding",
        credential_providers=("offline",),
        approve_provider_call=True,
    )

    assert wrapped.status == "completed_provider_call"
    assert wrapped.result.response.text == "offline answer"
    assert wrapped.provisioning.ready is True
    assert calls == ["offline-model"]
    projected = wrapped.to_dict()
    encoded = json.dumps(projected)
    assert projected["schema"] == "bioprism-python-autonomous-provisioned-run/0.1"
    assert projected["result_metadata"]["serialized"] is False
    assert "offline answer" not in encoded
    assert "CredentialHandle" not in encoded
    assert projected["secret_material"] == "never_returned"


def test_auto_provisioned_execution_routes_through_the_same_execution_boundary():
    agent, workspace, _capabilities, calls = _agent()

    wrapped = agent.run_auto_with_provisioned_credentials(
        task="produce a bounded coding implementation review",
        credential_providers=("offline",),
        hints=("coding",),
        allow_cross_domain=False,
        approve_provider_call=True,
    )

    assert wrapped.status == "completed"
    assert wrapped.result.route.primary_domain == "coding"
    assert wrapped.result.result.response.text == "offline answer"
    assert calls == ["offline-model"]
    assert any(name == "brain_model_select_contextual" for name, _args in workspace.calls)


def test_inventory_refresh_is_authenticated_by_the_fresh_session_and_is_strict():
    agent, _workspace, capabilities, calls = _agent()
    discoveries = 0

    def discover():
        nonlocal discoveries
        discoveries += 1
        return {
            "data": [
                {
                    "id": "offline-discovered",
                    "context_length": 16_000,
                    "max_output_tokens": 2_048,
                    "capabilities": capabilities,
                }
            ]
        }

    agent.runtime.register_in_memory_provider(
        "discovery",
        lambda request: {"output_text": "unused"},
        model_discovery_handler=discover,
    )
    # The discovered provider is deliberately not selected by the fixture workspace. This
    # proves refresh and execution remain separate policy decisions.
    wrapped = agent.run_with_provisioned_credentials(
        task="produce a bounded implementation review",
        domain="coding",
        credential_providers=("discovery",),
        refresh_inventory=True,
        inventory_prior_factory=lambda descriptor: {
            "quality": 0.84,
            "latency_ms": 40,
            "cost_per_million_tokens": 1,
            "reliability": 0.9,
            "capabilities": list(descriptor.capabilities),
        },
        approve_provider_call=True,
    )

    assert discoveries == 1
    assert wrapped.inventory is not None
    assert wrapped.inventory["status"] == "completed"
    assert wrapped.inventory["coverage"]
    assert calls == ["offline-model"]
    assert wrapped.to_dict()["inventory"]["coverage_count"] == len(wrapped.inventory["coverage"])

    with pytest.raises(BrainRunError, match="model inventory refresh failed|inventory"):
        agent.run_with_provisioned_credentials(
            task="this must not execute after discovery failure",
            domain="coding",
            credential_providers=("discovery",),
            refresh_inventory=True,
            inventory_priors={},
            approve_provider_call=True,
        )
    assert calls == ["offline-model"]


@pytest.mark.parametrize("domain", AUTONOMOUS_DOMAINS)
def test_request_scoped_facade_executes_every_reviewed_domain(domain: str):
    agent, _workspace, _capabilities, calls = _agent()

    wrapped = agent.run_with_provisioned_credentials(
        task=f"produce a bounded {domain} review",
        domain=domain,
        credential_providers=("offline",),
        approve_provider_call=True,
    )

    assert wrapped.status == "completed_provider_call"
    assert wrapped.result.response.provider == "offline"
    assert calls == ["offline-model"]
