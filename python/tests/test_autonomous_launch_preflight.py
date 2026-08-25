from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES,
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    CredentialStore,
    LLMRuntime,
    ModelCatalogue,
    openai_provider,
    validate_autonomous_launch_preflight_report,
)
from prism_sdk.domain_tools import builtin_autonomous_domain_tool_profiles
from prism_sdk.errors import ArgumentError


def _candidate() -> dict[str, object]:
    return {
        "provider": "openai",
        "model": "preflight-model",
        "capabilities": [
            "reasoning",
            "code",
            "web",
            "data",
            "science",
            "biomedical",
            "neuroscience",
            "operations",
            "enterprise",
            "coordination",
            "multimodal",
            "evaluation",
        ],
        "context_window_tokens": 32_000,
        "max_output_tokens": 2_000,
        "quality": 0.9,
        "latency_ms": 100,
        "cost_per_million_tokens": 10,
        "reliability": 0.95,
    }


def _capabilities() -> dict[str, dict[str, object]]:
    return {
        name: {
            "configured": True,
            "operational": True,
            "restart_safe": True,
            "integrity_fenced": True,
            "caller_owned": True,
        }
        for name in AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES
    }


def _agent() -> AutonomousAgent:
    runtime = LLMRuntime(CredentialStore())
    runtime.register_provider(openai_provider(base_url="https://launch-preflight.invalid"))
    return AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue([_candidate()]))


def _all_tool_names() -> list[str]:
    return sorted(
        {
            binding.name
            for profile in builtin_autonomous_domain_tool_profiles()
            for binding in profile.bindings
        }
    )


def test_launch_preflight_composes_all_twelve_domains_without_dispatch() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    report = agent.launch_preflight()

    assert report["schema"] == "bioprism-python-autonomous-launch-preflight/0.1"
    assert report["summary"]["domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["blocked_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert [row["domain"] for row in report["domains"]] == list(AUTONOMOUS_DOMAIN_NAMES)
    assert report["dispatch"] == {
        "status": "not_started",
        "authorization": "preflight_review_only;does_not_grant_provider_source_tool_or_effect_authority",
        "provider_calls": 0,
        "source_calls": 0,
        "tool_calls": 0,
        "learner_mutations": 0,
        "credential_resolution": 0,
    }
    assert validate_autonomous_launch_preflight_report(report) == report


def test_launch_preflight_projects_ready_provider_and_explicit_deployment_gates() -> None:
    agent = _agent()
    with agent.start_credential_session(session_id="launch-preflight-session") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        report = agent.launch_preflight(deployment_capabilities=_capabilities())

    assert report["agent_readiness"]["ready_provider_count"] == 1
    assert report["deployment_readiness"]["state"] == "ready_for_review"
    assert report["deployment_readiness"]["ready_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["partial_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["blocked_domain_count"] == 0
    assert "unit-test-only-not-a-provider-key" not in json.dumps(report)
    assert validate_autonomous_launch_preflight_report(report) == report


def test_launch_preflight_caller_inventories_remove_unassessed_contract_state() -> None:
    agent = _agent()
    with agent.start_credential_session(session_id="launch-preflight-inventory") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        report = agent.launch_preflight(
            available_tool_names=_all_tool_names(),
            available_evidence=("scope",),
            deployment_capabilities=_capabilities(),
        )

    assert report["contract_audit"]["runtime_status"] == "partial"
    assert report["summary"]["blocked_domain_count"] == 0
    assert report["summary"]["partial_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert all(row["contract_status"] == "valid" for row in report["domains"])


def test_launch_preflight_rejects_tampering_and_secret_shaped_capability_metadata() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    report = agent.launch_preflight()
    tampered = json.loads(json.dumps(report))
    tampered["domains"][0]["next_actions"].append("tampered")
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_launch_preflight_report(tampered)

    with pytest.raises(ArgumentError, match="secret-shaped"):
        agent.launch_preflight(
            deployment_capabilities={"persistence": {"api_key": "must-not-cross"}}
        )
