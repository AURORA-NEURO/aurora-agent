from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousActionPlan,
    AutonomousAgent,
    LLMRuntime,
)
from prism_sdk.errors import ArgumentError


_TASKS = {
    "coding": "debug a bounded repository change",
    "browser": "compare web sources and citation gaps",
    "data": "profile a dataset schema and missingness",
    "science": "design a reproducible experiment and uncertainty report",
    "biomedical": "review biomedical evidence with safety boundaries",
    "neuroscience": "analyze neural signal preprocessing and limitations",
    "operations": "prepare a reversible incident rollback runbook",
    "enterprise": "map governance ownership and approvals",
    "multi_agent": "delegate specialists and reconcile evidence",
    "multimodal": "align document image and audio observations",
    "cross_domain": "synthesize evidence across several disciplines",
    "evaluation": "replay a benchmark and analyze evaluator failures",
}


def _agent() -> AutonomousAgent:
    return AutonomousAgent(object(), LLMRuntime())


def test_action_plan_covers_every_builtin_domain_without_provider_dispatch() -> None:
    agent = _agent()

    for domain in AUTONOMOUS_DOMAINS:
        task = _TASKS[domain]
        plan = agent.action_plan(
            task=task,
            domain=domain,
            allow_cross_domain=False,
        )
        assert plan["status"] in {"ready", "review_required", "blocked"}, domain
        assert plan["selected_domains"] == [domain], domain
        assert plan["route_digest"] == plan["candidates"][0]["route_digest"], domain
        assert plan["candidates"][0]["domain"] == domain, domain
        assert len(plan["plan_digest"]) == 64, domain
        assert task not in json.dumps(plan), domain
        assert plan["secret_material"] == "never_returned"


def test_action_plan_aggregates_cross_domain_children_and_synthesis() -> None:
    plan = _agent().action_plan(
        task="coordinate coding and biomedical evidence across disciplines",
        hints=("coding", "biomedical"),
        allow_cross_domain=True,
        max_domains=3,
    )

    assert plan["cross_domain"] is True
    assert set(plan["selected_domains"]) == {"coding", "biomedical"}
    assert [candidate["role"] for candidate in plan["candidates"]] == ["child", "child", "synthesis"]
    assert plan["recommended_path"] == "cross_domain"
    assert "plan_acceptance" in plan["required_approvals"]
    assert plan["next_action"] in plan["next_actions"]
    assert all(candidate["route_digest"] == plan["route_digest"] for candidate in plan["candidates"])


def test_action_plan_is_round_trip_safe_and_rejects_tampering() -> None:
    public = _agent().action_plan(
        task="analyze a bounded data workflow",
        domain="data",
        allow_cross_domain=False,
    )

    restored = AutonomousActionPlan.from_dict(public)
    assert restored.to_dict() == public

    tampered = dict(public)
    tampered["next_action"] = "approve_provider_call"
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousActionPlan.from_dict(tampered)

    candidate_tampered = dict(public)
    candidate_tampered["candidates"] = [dict(public["candidates"][0], recommended_path="workflow")]
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousActionPlan.from_dict(candidate_tampered)


def test_action_plan_blocks_forbidden_biomedical_effects_before_any_provider_boundary() -> None:
    plan = _agent().action_plan(
        task="deploy the biomedical report and verify safety",
        domain="biomedical",
    )

    assert plan["status"] == "blocked"
    assert plan["next_action"] == "resolve_policy_block"
    assert plan["blocking_reasons"]
    assert "requested_effect_forbidden_by_domain_policy" in json.dumps(plan)


def test_action_plan_abstains_when_the_router_has_no_reviewed_evidence() -> None:
    plan = _agent().action_plan(
        task="zzzz qqqq an unclassified request",
        allow_cross_domain=False,
    )

    assert plan["status"] == "route_review_required"
    assert plan["next_action"] == "review_route"
    assert plan["candidates"] == []
    assert plan["required_approvals"] == []
