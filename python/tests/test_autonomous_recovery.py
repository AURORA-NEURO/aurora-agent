from __future__ import annotations

import copy
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_RECOVERY_ACTIONS,
    AutonomousAgent,
    AutonomousRecoveryPlan,
    AutonomousRecoveryHandoffLedger,
    AutonomousRecoveryHandoffPersistenceCoordinator,
    LLMRuntime,
    TransactionalJsonAutonomousRecoveryHandoffPersistence,
    plan_autonomous_recovery,
    validate_autonomous_recovery_handoff,
    validate_autonomous_recovery_handoff_snapshot,
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


def test_recovery_handoffs_are_idempotent_review_gated_and_cover_every_domain() -> None:
    ledger = AutonomousRecoveryHandoffLedger()
    for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES, start=1):
        plan = plan_autonomous_recovery({"domain": domain, "capability": "provider_call", "status": "failed", "failure_code": "provider_error"})
        result = ledger.submit(plan, run_id_digest=str(index).zfill(64), attempt=0)
        assert result["status"] == "accepted"
        assert result["handoff"]["status"] == "queued"
        assert result["handoff"]["domain"] == domain
        assert validate_autonomous_recovery_handoff(result["handoff"])["handoff_digest"] == result["handoff"]["handoff_digest"]
        assert "private task" not in str(result["handoff"])
        assert "gsk-" not in str(result["handoff"])

    retry_plan = plan_autonomous_recovery({"domain": "coding", "capability": "provider_call", "status": "failed", "retryable": True, "retry_count": 0, "max_retries": 2})
    accepted = ledger.submit(retry_plan, run_id_digest="a" * 64, attempt=0)
    duplicate = ledger.submit(retry_plan, run_id_digest="a" * 64, attempt=0)
    assert duplicate["status"] == "duplicate"
    with pytest.raises(ArgumentError, match="stale"):
        ledger.review(accepted["handoff"]["handoff_id"], decision="approve_retry", expected_revision=99, reviewer_digest="b" * 64)
    reviewed = ledger.review(accepted["handoff"]["handoff_id"], decision="approve_retry", expected_revision=1, reviewer_digest="b" * 64)
    assert reviewed["handoff"]["status"] == "retry_approved"
    assert reviewed["handoff"]["selected_action"] == "retry_provider"
    with pytest.raises(ArgumentError, match="already reviewed"):
        ledger.review(accepted["handoff"]["handoff_id"], decision="close", expected_revision=2, reviewer_digest="b" * 64)
    snapshot = ledger.snapshot()
    assert validate_autonomous_recovery_handoff_snapshot(snapshot)["snapshot_digest"] == snapshot["snapshot_digest"]
    restored = AutonomousRecoveryHandoffLedger()
    restored.restore(snapshot)
    assert restored.get(accepted["handoff"]["handoff_id"]).handoff_digest == reviewed["handoff"]["handoff_digest"]
    assert len(restored.entries(status="retry_approved", domain="coding")) == 1


def test_recovery_handoff_decisions_fail_closed_for_credentials_reconcile_and_cas() -> None:
    ledger = AutonomousRecoveryHandoffLedger()
    credential = ledger.submit(
        plan_autonomous_recovery({"domain": "science", "capability": "provider_call", "status": "failed", "failure_code": "credential"}),
        run_id_digest="c" * 64,
        attempt=0,
    )
    with pytest.raises(ArgumentError, match="does not authorize"):
        ledger.review(credential["handoff"]["handoff_id"], decision="approve_retry", expected_revision=1, reviewer_digest="d" * 64)
    uncertain = ledger.submit(
        plan_autonomous_recovery({"domain": "operations", "capability": "incident_response", "status": "failed", "reconciliation_required": True}),
        run_id_digest="e" * 64,
        attempt=0,
    )
    reconciled = ledger.review(uncertain["handoff"]["handoff_id"], decision="approve_reconciliation", expected_revision=1, reviewer_digest="f" * 64)
    assert reconciled["handoff"]["status"] == "reconciliation_required"
    assert reconciled["handoff"]["selected_action"] == "reconcile_external_effect"

    class Store:
        value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            current = None if self.value is None else json.loads(self.value)["snapshot_digest"]
            if current != expected:
                return False
            self.value = value
            return True

    persistence = TransactionalJsonAutonomousRecoveryHandoffPersistence(Store())
    first = AutonomousRecoveryHandoffLedger()
    first_coordinator = AutonomousRecoveryHandoffPersistenceCoordinator(first, persistence)
    assert first_coordinator.restore() is None
    first_coordinator.flush()
    second = AutonomousRecoveryHandoffLedger()
    second_coordinator = AutonomousRecoveryHandoffPersistenceCoordinator(second, persistence)
    second_coordinator.restore()
    first.submit(plan_autonomous_recovery({"domain": "data", "capability": "audit", "status": "failed"}), run_id_digest="1" * 64, attempt=0)
    first_coordinator.flush()
    second.submit(plan_autonomous_recovery({"domain": "browser", "capability": "search", "status": "failed"}), run_id_digest="2" * 64, attempt=0)
    with pytest.raises(ArgumentError, match="compare-and-swap"):
        second_coordinator.flush()
    forged = copy.deepcopy(first.snapshot())
    forged["entries"][0]["status"] = "escalated"
    with pytest.raises(ArgumentError, match="digest|inconsistent"):
        validate_autonomous_recovery_handoff_snapshot(forged)


def test_high_level_agent_exposes_recovery_without_widening_execution_authority() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    ledger = AutonomousRecoveryHandoffLedger()
    result = agent.submit_recovery_handoff(
        ledger,
        {"domain": "multimodal", "capability": "alignment", "status": "failed", "failure_code": "provider_error"},
        run_id_digest="9" * 64,
        attempt=1,
    )
    assert result["handoff"]["domain"] == "multimodal"
    assert result["handoff"]["status"] == "queued"
    assert agent.plan_recovery({"domain": "multimodal", "capability": "alignment", "status": "completed"}).next_action == "complete"
