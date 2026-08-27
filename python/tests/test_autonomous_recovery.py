from __future__ import annotations

import copy

import pytest

from prism_sdk import (
    AUTONOMOUS_RECOVERY_ACTIONS,
    AutonomousRecoveryPlan,
    plan_autonomous_recovery,
    validate_autonomous_recovery_plan,
)
from prism_sdk.domain_tools import AUTONOMOUS_DOMAIN_NAMES
from prism_sdk.errors import ArgumentError


def test_recovery_planning_gives_every_builtin_domain_an_explicit_completion_handoff() -> None:
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        plan = plan_autonomous_recovery({"domain": domain, "capability": "bounded_review", "status": "completed"})
        assert plan.status == "completed"
        assert plan.next_action == "complete"
        assert plan.actions == ("complete",)
        assert len(plan.domain_guardrails) == 2
        assert validate_autonomous_recovery_plan(plan.to_dict())["plan_digest"] == plan.plan_digest
        assert "private task" not in str(plan.to_dict())


def test_recovery_planning_preserves_bounded_retry_intent_and_exhaustion() -> None:
    retry = plan_autonomous_recovery(
        {
            "domain": "coding",
            "capability": "provider_call",
            "status": "failed",
            "failure_code": "http_5xx",
            "retryable": True,
            "retry_count": 1,
            "max_retries": 3,
        }
    )
    assert retry.status == "retryable"
    assert retry.next_action == "retry_provider"
    assert retry.reason_codes[0] == "bounded_retry_budget_remains"

    exhausted = plan_autonomous_recovery(
        {
            "domain": "coding",
            "capability": "provider_call",
            "status": "failed",
            "failure_code": "http_5xx",
            "retryable": True,
            "retry_count": 3,
            "max_retries": 3,
        }
    )
    assert exhausted.status == "blocked"
    assert exhausted.next_action == "stop_and_escalate"
    assert exhausted.reason_codes == ("retry_budget_exhausted",)


def test_recovery_planning_gives_reconciliation_and_approval_precedence_over_retry() -> None:
    uncertain = plan_autonomous_recovery(
        {
            "domain": "operations",
            "capability": "incident_response",
            "status": "failed",
            "failure_code": "transport",
            "retryable": True,
            "reconciliation_required": True,
        }
    )
    assert uncertain.status == "reconciliation_required"
    assert uncertain.next_action == "reconcile_external_effect"
    assert "retry_provider" not in uncertain.actions

    approval = plan_autonomous_recovery(
        {
            "domain": "enterprise",
            "capability": "change_request",
            "status": "approval_required",
            "retryable": True,
            "approval_required": True,
        }
    )
    assert approval.status == "held"
    assert approval.next_action == "approve_provider_call"
    assert "stop_and_escalate" in approval.actions


def test_recovery_planning_separates_credential_route_quality_and_policy_remediation() -> None:
    assert plan_autonomous_recovery({"domain": "science", "capability": "analysis", "status": "failed", "failure_code": "credential"}).next_action == "collect_credential"
    assert plan_autonomous_recovery({"domain": "browser", "capability": "search", "status": "abstained", "route_reviewed": False}).next_action == "review_route"
    assert plan_autonomous_recovery({"domain": "biomedical", "capability": "review", "status": "response_review_required", "response_quality_passed": False}).next_action == "review_response_quality"
    assert plan_autonomous_recovery({"domain": "evaluation", "capability": "audit", "status": "policy_blocked", "policy_admitted": False}).next_action == "review_domain_policy"


def test_recovery_plans_reject_secret_shaped_observations_and_tampering() -> None:
    with pytest.raises(ArgumentError, match="unsupported fields"):
        plan_autonomous_recovery({"domain": "coding", "capability": "review", "status": "failed", "prompt": "private task"})
    plan = plan_autonomous_recovery({"domain": "data", "capability": "audit", "status": "completed"})
    forged = copy.deepcopy(plan.to_dict())
    forged["next_action"] = "retry_provider"
    with pytest.raises(ArgumentError, match="next_action|digest"):
        validate_autonomous_recovery_plan(forged)
    forged = copy.deepcopy(plan.to_dict())
    forged["domain_guardrails"] = ["prompt"]
    with pytest.raises(ArgumentError, match="domain|secret|identifier|digest"):
        validate_autonomous_recovery_plan(forged)
    with pytest.raises(ArgumentError, match="boolean"):
        plan_autonomous_recovery({"domain": "coding", "capability": "review", "status": "failed", "response_quality_passed": "yes"})
    assert "complete" in AUTONOMOUS_RECOVERY_ACTIONS
    assert isinstance(plan, AutonomousRecoveryPlan)
