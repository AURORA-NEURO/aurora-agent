import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_POLICY_DOMAINS,
    AutonomousDomainPolicyError,
    AutonomousPlanBuilder,
    AutonomousPromptBuilder,
    AutonomousTaskSpec,
    autonomous_domain_policy,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_domain_policies,
    evaluate_autonomous_domain_policy,
)


def test_policies_cover_every_domain_with_unique_bounded_digests():
    policies = builtin_autonomous_domain_policies()
    assert tuple(policy.domain for policy in policies) == AUTONOMOUS_DOMAIN_POLICY_DOMAINS
    assert len({policy.policy_digest for policy in policies}) == len(policies)
    for policy in policies:
        assert policy.to_dict()["schema"] == "bioprism-autonomous-domain-policy/0.1"
        assert policy.max_input_tokens > 0
        assert policy.max_output_tokens > 0
        assert autonomous_domain_policy(policy.domain).policy_digest == policy.policy_digest


def test_admission_explains_review_and_hard_blocks():
    policy = autonomous_domain_policy("coding")
    admitted = evaluate_autonomous_domain_policy(
        policy,
        route_confidence=1,
        selection_confidence=1,
        selection_margin=1,
        estimated_input_tokens=100,
        requested_output_tokens=100,
        estimated_cost_units=1,
        structured_response=True,
        evidence_ready=True,
        evaluator_configured=True,
        plan_accepted=True,
        effects_requested=True,
        effects_approved=True,
    )
    assert admitted.decision == "admitted"
    review = evaluate_autonomous_domain_policy(policy, route_confidence=0.1)
    assert review.decision == "review_required"
    biomedical = evaluate_autonomous_domain_policy(
        autonomous_domain_policy("biomedical"),
        route_confidence=1,
        selection_confidence=1,
        selection_margin=1,
        structured_response=True,
        evidence_ready=True,
        evaluator_configured=True,
        plan_accepted=True,
        effects_requested=True,
    )
    assert biomedical.decision == "blocked"
    assert "effects_forbidden_by_policy" in biomedical.reasons
    over_budget = evaluate_autonomous_domain_policy(
        policy,
        estimated_input_tokens=policy.max_input_tokens + 1,
    )
    assert over_budget.decision == "blocked"
    assert "input_budget_exceeded" in over_budget.reasons


def test_prompt_and_plan_bind_policy_metadata_for_every_domain():
    profiles = builtin_autonomous_domain_profiles()
    assert tuple(profile.domain for profile in profiles) == AUTONOMOUS_DOMAIN_POLICY_DOMAINS
    for profile in profiles:
        spec = AutonomousTaskSpec(
            task=f"prepare a bounded {profile.domain} review",
            domain=profile.domain,
            capability=profile.default_capability,
            risk_class=profile.risk_class,
        )
        prompt = AutonomousPromptBuilder.build(spec, profile)
        policy_context = next(item for item in prompt["context"] if item["id"] == "autonomy-domain-policy")
        policy_payload = json.loads(policy_context["content"])
        assert policy_payload["domain_execution_policy"]["domain"] == profile.domain
        plan = AutonomousPlanBuilder.build(spec)
        assert plan["domain_policy_digest"] == autonomous_domain_policy(profile.domain).policy_digest


def test_policy_rejects_unknown_override():
    with pytest.raises(AutonomousDomainPolicyError):
        autonomous_domain_policy("coding", {"unknown": 1})
