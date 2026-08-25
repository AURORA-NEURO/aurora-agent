from __future__ import annotations

import copy

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    LLMRuntime,
    audit_autonomous_domain_contracts,
    builtin_autonomous_domain_tool_profiles,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
    validate_autonomous_domain_audit_report,
)
from prism_sdk.errors import ArgumentError


def _all_tool_names() -> list[str]:
    return [
        binding.name
        for profile in builtin_autonomous_domain_tool_profiles()
        for binding in profile.bindings
    ]


def test_builtin_audit_covers_every_domain_and_is_digest_bound() -> None:
    report = audit_autonomous_domain_contracts()

    assert report["summary"]["domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["static_contract_status"] == "valid"
    assert report["summary"]["valid_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["runtime_status"] == "unassessed"
    assert {row["domain"] for row in report["rows"]} == set(AUTONOMOUS_DOMAIN_NAMES)
    assert all(row["issues"] == [] for row in report["rows"])
    assert validate_autonomous_domain_audit_report(report) == report


def test_audit_projects_tool_and_evidence_coverage_without_dispatch() -> None:
    report = audit_autonomous_domain_contracts(
        available_tool_names=_all_tool_names(),
        available_evidence=("scope",),
    )

    assert report["summary"]["runtime_status"] == "partial"
    assert all(row["tool_surface"]["assessed"] for row in report["rows"])
    assert all(row["tool_surface"]["missing_tool_names"] == [] for row in report["rows"])
    assert all(row["evidence_surface"]["assessed"] for row in report["rows"])
    assert any(row["evidence_surface"]["coverage_status"] != "complete" for row in report["rows"])
    assert all(row["execution"] == "audit_only;no_provider_source_tool_queue_or_credential_dispatch" for row in report["rows"])
    assert report["secret_material"] == "never_returned"


def test_audit_rejects_profile_drift_and_tampered_handoffs() -> None:
    profile = copy.deepcopy(builtin_autonomous_domain_profiles()[0].to_dict())
    profile["default_capability"] = "undeclared_capability"
    invalid = audit_autonomous_domain_contracts(
        profiles=(profile,),
        workflows=(builtin_autonomous_workflow_strategies()[0],),
    )
    row = invalid["rows"][0]
    assert invalid["summary"]["static_contract_status"] == "invalid"
    assert any(issue["code"] == "default_capability_unlisted" for issue in row["issues"])

    tampered = copy.deepcopy(invalid)
    tampered["rows"][0]["next_actions"].append("unexpected mutation")
    with pytest.raises(ArgumentError, match="row digest"):
        validate_autonomous_domain_audit_report(tampered)


def test_agent_domain_audit_reads_registries_without_provider_or_credential_activity() -> None:
    agent = AutonomousAgent(None, LLMRuntime())

    report = agent.domain_audit()

    assert report["summary"]["domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["summary"]["runtime_status"] == "unassessed"
    assert report["credential_posture"] == "caller_owned_opaque_handles_only;no_credentials_consumed"
    assert report["execution"] == "audit_only;no_provider_source_tool_queue_or_credential_dispatch"
