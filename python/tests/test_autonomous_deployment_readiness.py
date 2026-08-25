from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES,
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousDeploymentReadinessAuditor,
    CredentialStore,
    LLMRuntime,
    ModelCatalogue,
    openai_provider,
    validate_autonomous_deployment_readiness_report,
)
from prism_sdk.errors import ArgumentError


def _candidate() -> dict[str, object]:
    return {
        "provider": "openai",
        "model": "deployment-audit-model",
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


def _capabilities(*, complete: bool = True) -> dict[str, dict[str, object]]:
    values = {
        name: {
            "configured": complete,
            "operational": complete,
            "restart_safe": complete,
            "integrity_fenced": complete,
            "caller_owned": True,
        }
        for name in AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES
    }
    if not complete:
        values["persistence"]["next_actions"] = ["configure durable persistence"]
    return values


def _agent() -> AutonomousAgent:
    runtime = LLMRuntime(CredentialStore())
    runtime.register_provider(openai_provider(base_url="https://deployment-readiness.invalid"))
    return AutonomousAgent(
        object(),
        runtime,
        model_catalogue=ModelCatalogue([_candidate()]),
    )


def test_deployment_audit_covers_all_domains_without_dispatch():
    agent = _agent()
    with agent.start_credential_session(session_id="deployment-audit-session") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        report = agent.deployment_readiness(capabilities=_capabilities())

    assert report["domains"] and [row["domain"] for row in report["domains"]] == list(AUTONOMOUS_DOMAIN_NAMES)
    assert len(report["domains"]) == 12
    assert report["state"] == "ready_for_review"
    assert report["ready_domain_count"] == 12
    assert report["partial_domain_count"] == 0
    assert report["blocked_domain_count"] == 0
    assert report["provider_gate"]["ready_provider_count"] == 1
    assert report["global_blockers"] == []
    assert len(report["capabilities"]) == len(AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES)
    assert all(row["satisfies_requirement"] for row in report["capabilities"] if row["required"])
    assert report["execution"] == "audit_only;no_provider_source_tool_queue_or_credential_dispatch"
    assert report["authority"] == "audit_does_not_grant_dispatch_authority"
    assert validate_autonomous_deployment_readiness_report(report) == report
    encoded = json.dumps(report)
    assert "unit-test-only-not-a-provider-key" not in encoded
    assert "authorization" not in encoded.lower()


def test_deployment_audit_holds_uncredentialed_provider_and_each_domain():
    agent = _agent()
    report = agent.deployment_readiness(capabilities=_capabilities())

    assert report["state"] == "blocked"
    assert any(row["code"] == "credential" for row in report["global_blockers"])
    assert all(row["state"] == "blocked" for row in report["domains"])
    assert all(any(blocker["code"] == "credential" for blocker in row["blockers"]) for row in report["domains"])


def test_deployment_policy_turns_optional_gates_into_explicit_blockers():
    agent = _agent()
    with agent.start_credential_session(session_id="deployment-policy-session") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        report = agent.deployment_readiness(
            policy={
                "require_tool_catalogue": True,
                "require_evidence": True,
                "require_learning": True,
                "require_queue": True,
                "require_telemetry": True,
            },
            capabilities={
                name: _capabilities()[name]
                for name in ("persistence", "approval_authority")
            },
        )

    assert report["state"] == "blocked"
    assert {row["code"] for row in report["global_blockers"]} >= {"queue", "telemetry"}
    assert all(any(blocker["code"] == "tool_catalogue" for blocker in row["blockers"]) for row in report["domains"])
    assert all(any(blocker["code"] == "evidence_adapter" for blocker in row["blockers"]) for row in report["domains"])
    assert all(any(blocker["code"] == "learning" for blocker in row["blockers"]) for row in report["domains"])


def test_deployment_report_is_digest_bound_and_secret_shaped_input_is_rejected():
    agent = _agent()
    with agent.start_credential_session(session_id="deployment-tamper-session") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        report = agent.deployment_readiness(capabilities=_capabilities())

    tampered = json.loads(json.dumps(report))
    tampered["domains"][0]["next_actions"].append("tampered")
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_deployment_readiness_report(tampered)

    raw = agent.readiness()
    raw["forbidden"] = {"api_key": "must-not-cross"}
    with pytest.raises(ArgumentError, match="secret-shaped"):
        AutonomousDeploymentReadinessAuditor().audit({"agent": raw, "provider_plan": agent.credential_provisioning_plan()})
