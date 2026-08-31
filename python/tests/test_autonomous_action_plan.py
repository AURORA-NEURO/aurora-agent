from __future__ import annotations

import json
from types import SimpleNamespace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousActionAdmission,
    AutonomousActionExecution,
    AutonomousActionPlan,
    AutonomousAgent,
    LLMRuntime,
    admit_autonomous_action_plan,
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


def test_action_plan_admission_is_explicit_and_round_trip_safe_without_credentials() -> None:
    agent = _agent()
    plan = agent.action_plan(
        task="debug a bounded repository change",
        domain="coding",
        allow_cross_domain=False,
    )

    admission = admit_autonomous_action_plan(plan)
    assert admission.status == "review_required"
    assert admission.missing_approvals == tuple(plan["required_approvals"])
    assert admission.next_action in {"review_task_decision", "approve_provider_call"}
    restored = AutonomousActionAdmission.from_dict(admission.to_dict())
    assert restored.to_dict() == admission.to_dict()

    execution = agent.execute_action_plan(
        task="debug a bounded repository change",
        plan=plan,
        domain="coding",
        allow_cross_domain=False,
    )
    assert isinstance(execution, AutonomousActionExecution)
    assert execution.status == "review_required"
    assert execution.result is None
    assert "debug a bounded repository change" not in json.dumps(execution.to_dict())


def test_action_plan_admission_covers_all_domains_and_cross_domain_path() -> None:
    agent = _agent()
    for domain in AUTONOMOUS_DOMAINS:
        plan = agent.action_plan(task=_TASKS[domain], domain=domain, allow_cross_domain=False)
        approvals = {gate: True for gate in plan["required_approvals"]}
        admission = admit_autonomous_action_plan(plan, approvals=approvals, reviewed=True)
        if plan["status"] == "blocked":
            assert admission.status == "blocked", domain
        else:
            assert admission.status == "admitted", domain
            assert admission.execution_path == plan["candidates"][0]["recommended_path"], domain

    cross_plan = agent.action_plan(
        task="coordinate coding and biomedical evidence across disciplines",
        hints=("coding", "biomedical"),
        allow_cross_domain=True,
        max_domains=3,
    )
    cross_admission = admit_autonomous_action_plan(
        cross_plan,
        approvals={gate: True for gate in cross_plan["required_approvals"]},
        reviewed=True,
    )
    assert cross_admission.status == "admitted"
    assert cross_admission.execution_path == "cross_domain"


def test_action_plan_execution_rejects_stale_task_and_tampered_admission() -> None:
    agent = _agent()
    plan = agent.action_plan(task="analyze a bounded data workflow", domain="data")
    approvals = {gate: True for gate in plan["required_approvals"]}
    admission = admit_autonomous_action_plan(plan, approvals=approvals, reviewed=True)
    tampered = dict(admission.to_dict())
    tampered["approved_approvals"] = []
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousActionAdmission.from_dict(tampered)
    with pytest.raises(Exception, match="stale"):
        agent.execute_action_plan(
            task="analyze a different bounded data workflow",
            plan=plan,
            domain="data",
            approvals=approvals,
            reviewed=True,
        )


def test_action_plan_admission_preserves_policy_block_before_dispatch() -> None:
    plan = _agent().action_plan(
        task="deploy the biomedical report and verify safety",
        domain="biomedical",
    )
    admission = admit_autonomous_action_plan(
        plan,
        approvals={gate: True for gate in plan["required_approvals"]},
        reviewed=True,
    )
    assert admission.status == "blocked"
    assert admission.next_action == "resolve_policy_block"


def test_action_plan_execution_translates_path_and_approvals_into_existing_runner_controls() -> None:
    agent = _agent()
    task = "debug a bounded repository change"
    plan = agent.action_plan(task=task, domain="coding", allow_cross_domain=False)
    approvals = {gate: True for gate in plan["required_approvals"]}
    calls: dict[str, object] = {}

    def fake_run_auto(**kwargs: object) -> SimpleNamespace:
        calls.update(kwargs)
        return SimpleNamespace(execution_status="completed", to_dict=lambda: {"status": "completed"})

    agent.run_auto = fake_run_auto  # type: ignore[method-assign]
    execution = agent.execute_action_plan(
        task=task,
        plan=plan,
        domain="coding",
        approvals=approvals,
        reviewed=True,
        credentials=object(),
    )
    assert execution.status == "completed"
    assert calls["approve_provider_call"] is True
    assert calls["domain_policy_plan_accepted"] is True
    assert calls["workflow_execution"] is True
