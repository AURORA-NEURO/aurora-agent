from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousWorkflowPortfolioPlan,
    BrainRunError,
    LLMRuntime,
)
from prism_sdk.autonomous_workflow_portfolio import plan_autonomous_workflow_portfolio


def _agent() -> AutonomousAgent:
    # Planning must work with an entirely unconfigured runtime: no provider or credential is
    # needed until a caller explicitly hands a ready item to the workflow execution boundary.
    return AutonomousAgent(object(), LLMRuntime())


def _requests(domains=AUTONOMOUS_DOMAINS):
    return [
        {
            "id": domain,
            "task": f"prepare a bounded {domain} review with explicit evidence limits",
            "domain": domain,
            "hints": [domain, "bounded review"],
            "context": {"caller_scope": "portfolio-test", "domain_index": index},
        }
        for index, domain in enumerate(domains)
    ]


def test_portfolio_compiles_every_builtin_domain_without_provider_dispatch():
    agent = _agent()
    requests = _requests()

    plan = agent.plan_workflow_portfolio(requests, require_all_domains=True)

    assert plan["status"] == "ready"
    assert plan["coverage"]["requested_item_count"] == len(AUTONOMOUS_DOMAINS)
    assert plan["coverage"]["ready_item_count"] == len(AUTONOMOUS_DOMAINS)
    assert plan["coverage"]["missing_domains"] == []
    assert plan["dependency_graph"]["topological_order"] == sorted(AUTONOMOUS_DOMAINS)
    assert plan["dependency_graph"]["waves"] == [sorted(AUTONOMOUS_DOMAINS)]
    assert all(item["status"] == "ready" for item in plan["items"])
    assert all(item["stage_ids"] for item in plan["items"])
    assert "bounded coding review" not in json.dumps(plan)
    assert "portfolio-test" not in json.dumps(plan)
    assert plan["execution"] == "not_started;planning_and_verification_only"
    assert plan["authorization"].startswith("portfolio_selection_does_not_authorize")

    restored = AutonomousWorkflowPortfolioPlan.from_dict(plan)
    assert restored.portfolio_digest == plan["portfolio_digest"]
    assert agent.verify_workflow_portfolio(plan, requests)["status"] == "verified"


def test_portfolio_dependency_waves_are_deterministic_and_digest_bound():
    agent = _agent()
    requests = [
        {"id": "root", "task": "review a coding root", "domain": "coding"},
        {"id": "science", "task": "review a science dependency", "domain": "science", "depends_on": ["root"]},
        {"id": "data", "task": "review data after science", "domain": "data", "depends_on": ["science"]},
        {"id": "parallel", "task": "review browser independently", "domain": "browser"},
    ]

    plan = agent.plan_workflow_portfolio(requests)

    assert plan["status"] == "ready"
    assert plan["dependency_graph"] == {
        "topological_order": ["parallel", "root", "science", "data"],
        "waves": [["parallel", "root"], ["science"], ["data"]],
        "cycle_item_ids": [],
        "edge_count": 2,
    }
    assert agent.verify_workflow_portfolio(plan, list(reversed(requests)))["status"] == "mismatch"


def test_portfolio_can_be_partial_but_never_hides_a_failed_item():
    real_agent = _agent()

    class FailingAgent:
        def route(self, **kwargs):
            return real_agent.route(**kwargs)

        def prepare(self, **kwargs):
            if kwargs.get("domain") == "science":
                raise BrainRunError("fixture failure")
            return real_agent.prepare(**kwargs)

    agent = FailingAgent()
    requests = [
        {"id": "good", "task": "review a bounded coding change", "domain": "coding"},
        {"id": "bad", "task": "review an invalid capability", "domain": "science", "capability": "not_a_reviewed_capability"},
        {"id": "dependent", "task": "depend on the failed science item", "domain": "data", "depends_on": ["bad"]},
    ]

    plan = plan_autonomous_workflow_portfolio(agent, requests, allow_partial=True).to_dict()

    assert plan["status"] == "partial"
    rows = {item["item_id"]: item for item in plan["items"]}
    assert rows["good"]["status"] == "ready"
    assert rows["bad"]["status"] == "failed"
    assert rows["dependent"]["status"] == "blocked"
    assert rows["dependent"]["error_class"] == "dependency_not_ready"
    assert plan["coverage"]["failed_item_count"] == 1
    assert plan["coverage"]["blocked_item_count"] == 1


def test_portfolio_cycles_are_explicit_and_unknown_dependencies_fail_closed():
    agent = _agent()
    cycle = agent.plan_workflow_portfolio(
        [
            {"id": "a", "task": "cycle a", "domain": "coding", "depends_on": ["b"]},
            {"id": "b", "task": "cycle b", "domain": "science", "depends_on": ["a"]},
        ]
    )

    assert cycle["status"] == "blocked"
    assert cycle["dependency_graph"]["cycle_item_ids"] == ["a", "b"]
    assert {item["error_class"] for item in cycle["items"]} == {"dependency_cycle"}

    with pytest.raises(BrainRunError):
        agent.plan_workflow_portfolio(
            [{"id": "orphan", "task": "unknown dependency", "domain": "coding", "depends_on": ["missing"]}]
        )


def test_portfolio_verification_detects_task_or_policy_drift():
    agent = _agent()
    requests = _requests(("coding", "evaluation"))
    plan = agent.plan_workflow_portfolio(requests)

    changed = [dict(item) for item in requests]
    changed[0]["task"] = "a materially different coding request"
    verification = agent.verify_workflow_portfolio(plan, changed)

    assert verification["status"] == "mismatch"
    assert any(row["item_id"] == "coding" and "task_digest" in row["codes"] for row in verification["mismatches"])
    assert any(row["item_id"] == "portfolio" for row in verification["mismatches"])

    with pytest.raises(BrainRunError):
        tampered = dict(plan)
        tampered["portfolio_digest"] = "0" * 64
        AutonomousWorkflowPortfolioPlan.from_dict(tampered)
