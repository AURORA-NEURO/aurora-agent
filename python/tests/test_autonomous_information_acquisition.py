from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousInformationAcquisitionCandidate,
    AutonomousInformationAcquisitionObservation,
    AutonomousInformationAcquisitionPolicy,
    LLMRuntime,
    content_digest,
    plan_autonomous_information_acquisition,
    replan_autonomous_information_acquisition,
    validate_autonomous_information_acquisition_plan,
)
from prism_sdk.errors import ArgumentError


def candidate(candidate_id: str, domain: str, *, score: float = 0.8, cost: float = 0.1, depends_on: tuple[str, ...] = (), status: str = "available") -> AutonomousInformationAcquisitionCandidate:
    return AutonomousInformationAcquisitionCandidate(
        candidate_id=candidate_id,
        domain=domain,
        capability="evidence_acquisition",
        source_id=f"source-{candidate_id}",
        information_gain=score,
        uncertainty_reduction=score,
        reliability=0.9,
        freshness=0.95,
        coverage=0.8,
        cost=cost,
        latency_ms=100,
        risk=0.05,
        conflict_risk=0.05,
        priority=0.5,
        status=status,
        depends_on=depends_on,
    )


def task_digest() -> str:
    return content_digest({"task": "choose the next bounded evidence acquisition"})


def test_planner_covers_all_domains_deterministically_without_dispatch() -> None:
    candidates = tuple(candidate(f"candidate-{domain}", domain) for domain in AUTONOMOUS_DOMAINS)
    plan = plan_autonomous_information_acquisition(
        task_digest=task_digest(),
        candidates=candidates,
        requested_domains=AUTONOMOUS_DOMAINS,
        policy=AutonomousInformationAcquisitionPolicy(
            max_cost=2.0,
            max_items=len(AUTONOMOUS_DOMAINS),
            require_domain_coverage=True,
            exploration=0.0,
        ),
    )

    assert plan.status == "ready"
    assert plan.selected_domains == AUTONOMOUS_DOMAINS
    assert len(plan.selected) == len(AUTONOMOUS_DOMAINS)
    assert plan.missing_domains == ()
    assert plan.coverage_ratio == 1.0
    projection = plan.to_dict()
    assert projection["execution"].startswith("planning_only")
    assert projection["retention"].startswith("metadata_only")
    assert "choose the next" not in str(projection)


def test_dependency_order_and_budget_omissions_are_explicit() -> None:
    base = candidate("base", "coding", score=0.2, cost=0.2)
    dependent = candidate("dependent", "coding", score=1.0, cost=0.2, depends_on=("base",))
    plan = plan_autonomous_information_acquisition(
        task_digest=task_digest(),
        candidates=(dependent, base),
        requested_domains=("coding",),
        policy={"max_cost": 0.4, "max_items": 2, "exploration": 0.0},
    )

    assert [item.candidate_id for item in plan.selected] == ["base", "dependent"]
    assert plan.consumed_cost == pytest.approx(0.4)

    held = plan_autonomous_information_acquisition(
        task_digest=task_digest(),
        candidates=(candidate("too-expensive", "coding", cost=0.8),),
        requested_domains=("coding",),
        policy={"max_cost": 0.2, "max_items": 1, "exploration": 0.0},
    )
    assert held.status in {"blocked", "empty", "partial"}
    assert held.omissions[0].reason == "budget_exceeded"


def test_replan_uses_value_only_observation_and_fences_candidate_drift() -> None:
    first = candidate("first", "science", score=0.95)
    second = candidate("second", "science", score=0.6)
    plan = plan_autonomous_information_acquisition(
        task_digest=task_digest(),
        candidates=(first, second),
        requested_domains=("science",),
        policy={"max_cost": 0.2, "max_items": 1, "exploration": 0.0},
    )
    observation = AutonomousInformationAcquisitionObservation(
        candidate_id="first",
        status="failed",
        value_digest="a" * 64,
        evaluator_digest="b" * 64,
    )
    replanned = replan_autonomous_information_acquisition(
        plan,
        candidates=(first, second),
        observations=(observation,),
    )

    assert replanned.generation == 2
    assert replanned.prior_plan_digest == plan.plan_digest
    assert replanned.observations_digest is not None
    assert replanned.selected[0].candidate_id == "second"
    assert validate_autonomous_information_acquisition_plan(replanned).plan_digest == replanned.plan_digest

    repeated = replan_autonomous_information_acquisition(
        replanned,
        candidates=(first, second),
        observations=(AutonomousInformationAcquisitionObservation(candidate_id="second", status="accepted"),),
    )
    assert repeated.generation == 3
    assert repeated.prior_plan_digest == replanned.plan_digest

    with pytest.raises(ArgumentError):
        replan_autonomous_information_acquisition(
            plan,
            candidates=(candidate("first", "science", score=0.1), second),
            observations=(observation,),
        )


def test_secret_metadata_and_invalid_source_state_fail_closed() -> None:
    with pytest.raises(ArgumentError):
        candidate("secret", "coding").__class__(
            candidate_id="secret",
            domain="coding",
            capability="evidence_acquisition",
            source_id="source-secret",
            information_gain=0.5,
            uncertainty_reduction=0.5,
            reliability=0.9,
            freshness=0.9,
            coverage=0.8,
            cost=0.1,
            latency_ms=100,
            risk=0.1,
            conflict_risk=0.1,
            metadata={"api_key": "must never enter a plan"},
        )
    stale = plan_autonomous_information_acquisition(
        task_digest=task_digest(),
        candidates=(candidate("stale", "coding", status="stale"),),
        requested_domains=("coding",),
    )
    assert stale.omissions[0].reason == "stale_not_allowed"


def test_agent_facade_binds_explicit_domains_without_provider_or_source_dispatch() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    plan = agent.plan_information_acquisition(
        task="choose a bounded coding evidence acquisition",
        domains=("coding",),
        candidates=(candidate("coding-next", "coding"),),
        policy={"max_items": 1, "exploration": 0.0},
    )

    assert plan.requested_domains == ("coding",)
    assert plan.selected[0].candidate_id == "coding-next"
    assert plan.route_digest is not None
    assert plan.to_dict()["execution"].startswith("planning_only")
    assert plan.to_dict()["secret_material"] == "never_returned"
