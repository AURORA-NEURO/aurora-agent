import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_POLICY_DOMAINS,
    AutonomousAgent,
    AutonomousBrain,
    AutonomousDomainPolicyError,
    AutonomousTaskOrchestrator,
    AutonomousPlanBuilder,
    AutonomousPromptBuilder,
    AutonomousTaskSpec,
    autonomous_domain_policy,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_domain_policies,
    evaluate_autonomous_domain_policy,
    LLMRuntime,
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


def test_strict_orchestrator_policy_blocks_every_domain_before_provider_dispatch():
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    for domain in AUTONOMOUS_DOMAIN_POLICY_DOMAINS:
        with pytest.raises(AutonomousDomainPolicyError, match="strict autonomous domain policy"):
            orchestrator.run(
                task=f"strictly review a bounded {domain} task",
                domain=domain,
                model_candidates=(),
                credentials={},
                domain_policy_mode="strict",
                approve_provider_call=True,
            )


def test_strict_provider_planning_is_gated_for_every_domain_before_model_selection():
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    for domain in AUTONOMOUS_DOMAIN_POLICY_DOMAINS:
        blueprint = orchestrator.prepare(
            task=f"prepare a bounded {domain} plan",
            domain=domain,
        )
        held = orchestrator.plan_with_provider(
            blueprint=blueprint,
            model_candidates=(),
            credentials={},
            domain_policy_mode="strict",
            approve_provider_call=True,
        )
        assert held.status == "policy_review_required"
        assert held.domain_policy_admission is not None
        assert held.domain_policy_admission.domain == domain
        assert "evaluator_required" in held.domain_policy_admission.reasons

        admitted = orchestrator.plan_with_provider(
            blueprint=blueprint,
            model_candidates=(),
            credentials={},
            domain_policy_mode="strict",
            domain_policy_evidence_ready=True,
            domain_policy_evaluator_configured=True,
            approve_provider_call=False,
        )
        assert admitted.status == "approval_required"
        assert admitted.domain_policy_admission is not None
        assert admitted.domain_policy_admission.decision == "admitted"


def test_strict_semantic_routing_is_gated_before_model_selection():
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    held = orchestrator.route_with_provider(
        task="route a bounded coding and science review",
        model_candidates=(),
        credentials={},
        domain_policy_mode="strict",
        approve_provider_call=True,
    )
    assert held.status == "policy_review_required"
    assert held.domain_policy_admission is not None
    assert held.domain_policy_admission.domain == "cross_domain"
    assert "evaluator_required" in held.domain_policy_admission.reasons

    approval = orchestrator.route_with_provider(
        task="route a bounded coding and science review",
        model_candidates=(),
        credentials={},
        domain_policy_mode="strict",
        domain_policy_evidence_ready=True,
        domain_policy_evaluator_configured=True,
        approve_provider_call=False,
    )
    assert approval.status == "approval_required"
    assert approval.domain_policy_admission is not None
    assert approval.domain_policy_admission.decision == "admitted"


def test_strict_automatic_provider_planning_stops_before_model_selection():
    agent = AutonomousAgent(object(), LLMRuntime())
    result = agent.run_auto(
        task="prepare a bounded coding plan",
        credentials={},
        model_candidates=(),
        planning_mode="provider",
        domain_policy_mode="strict",
        approve_provider_call=True,
    )
    assert result.status == "planning_review_required"
    assert result.planning is not None
    assert result.planning.status == "policy_review_required"
    assert result.planning.domain_policy_admission is not None
    assert result.planning.domain_policy_admission.domain == "coding"


def test_strict_automatic_semantic_routing_stops_before_model_selection():
    agent = AutonomousAgent(object(), LLMRuntime())
    result = agent.run_auto(
        task="prepare a bounded coding plan",
        credentials={},
        model_candidates=(),
        semantic_routing=True,
        domain_policy_mode="strict",
        approve_provider_call=True,
    )
    assert result.status == "policy_review_required"
    assert result.semantic_route is not None
    assert result.semantic_route.status == "policy_review_required"
    assert result.semantic_route.domain_policy_admission is not None
    assert result.semantic_route.domain_policy_admission.domain == "cross_domain"


def test_strict_workflow_stage_rechecks_policy_before_provider_dispatch():
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    blueprint = orchestrator.prepare(
        task="execute a bounded staged coding review",
        domain="coding",
    )
    with pytest.raises(AutonomousDomainPolicyError, match="strict autonomous domain policy"):
        orchestrator.run_workflow(
            blueprint=blueprint,
            model_candidates=(
                {
                    "provider": "openai",
                    "model": "test-model",
                    "capabilities": ["code", "reasoning"],
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 20,
                    "cost_per_million_tokens": 10,
                },
            ),
            credentials={},
            domain_policy_mode="strict",
            approve_provider_call=True,
        )
