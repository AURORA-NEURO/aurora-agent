from __future__ import annotations

import json
import threading
from dataclasses import dataclass

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousWorkflowPortfolioPlan,
    BrainRunError,
    LLMRuntime,
)
from prism_sdk.autonomous_workflow_portfolio import (
    execute_autonomous_workflow_portfolio,
    plan_autonomous_workflow_portfolio,
)


@dataclass(frozen=True)
class _FakeRun:
    status: str
    task_digest: str

    def to_dict(self):
        return {
            "schema": "fixture-workflow-run/0.1",
            "status": self.status,
            "task_digest": self.task_digest,
            "secret_material": "never_returned",
        }


class _ExecutionAgent:
    def __init__(self, outcomes=None):
        self.real = _agent()
        self.outcomes = dict(outcomes or {})
        self.task_to_item = {}
        self.calls = []
        self._lock = threading.Lock()

    def bind(self, requests):
        self.task_to_item = {request["task"]: request["id"] for request in requests}

    def route(self, **kwargs):
        return self.real.route(**kwargs)

    def prepare(self, **kwargs):
        return self.real.prepare(**kwargs)

    @staticmethod
    def _credential_mapping(credentials):
        return dict(credentials)

    @staticmethod
    def _resolve_candidates(_candidates):
        return []

    def run_workflow(self, *, blueprint, **_kwargs):
        item_id = self.task_to_item.get(blueprint.spec.task, blueprint.spec.domain)
        with self._lock:
            self.calls.append(item_id)
        return _FakeRun(self.outcomes.get(item_id, "completed"), blueprint.spec.task_digest)


def _execution_requests():
    return [
        {"id": "root", "task": "execute a coding root", "domain": "coding"},
        {"id": "parallel", "task": "execute an independent science review", "domain": "science"},
        {"id": "dependent", "task": "execute data after root", "domain": "data", "depends_on": ["root"]},
    ]


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


def test_portfolio_execution_dispatches_dependency_waves_and_persists_metadata_only():
    agent = _ExecutionAgent()
    requests = _execution_requests()
    agent.bind(requests)
    plan = agent.real.plan_workflow_portfolio(requests)
    checkpoints = []

    result = plan_autonomous_workflow_portfolio(agent, requests).to_dict()
    assert result["portfolio_digest"] == plan["portfolio_digest"]
    execution = execute_autonomous_workflow_portfolio(
        agent,
        plan,
        requests,
        credentials={},
        model_candidates=(),
        job_id="portfolio-test-job",
        max_parallelism=2,
        checkpoint_sink=checkpoints.append,
    )

    assert execution.status == "completed"
    assert execution.completed_count == 3
    assert execution.failed_count == 0
    assert execution.blocked_count == 0
    assert execution.executed_waves == (("parallel", "root"), ("dependent",))
    assert agent.calls.index("root") < agent.calls.index("dependent")
    assert execution.checkpoint.settled_item_ids == ("dependent", "parallel", "root")
    assert checkpoints[-1].status == "completed"
    serialized = json.dumps(execution.to_dict(), sort_keys=True)
    assert "execute a coding root" not in serialized
    assert "raw_run_caller_owned;not_serialized" in serialized
    assert all(checkpoint.to_dict()["secret_material"] == "never_returned" for checkpoint in checkpoints)


def test_portfolio_execution_failure_blocks_dependents_and_approval_is_explicit():
    requests = [
        {"id": "failed-root", "task": "fail a coding root", "domain": "coding"},
        {"id": "dependent", "task": "wait for failed root", "domain": "data", "depends_on": ["failed-root"]},
    ]
    failed_agent = _ExecutionAgent({"failed-root": "stage_failed"})
    failed_agent.bind(requests)
    plan = failed_agent.real.plan_workflow_portfolio(requests)
    failed = execute_autonomous_workflow_portfolio(
        failed_agent,
        plan,
        requests,
        credentials={},
        model_candidates=(),
        job_id="failure-job",
        stop_on_error=True,
    )

    assert failed.status == "blocked"
    assert failed.failed_count == 1
    assert failed.blocked_count == 1
    rows = {item.item_id: item for item in failed.items}
    assert rows["failed-root"].error_class == "failed"
    assert rows["dependent"].error_class == "stop_on_error"

    approval_agent = _ExecutionAgent({"failed-root": "approval_required"})
    approval_agent.bind(requests)
    approval_plan = approval_agent.real.plan_workflow_portfolio(requests)
    approval = execute_autonomous_workflow_portfolio(
        approval_agent,
        approval_plan,
        requests,
        credentials={},
        model_candidates=(),
        job_id="approval-job",
    )
    assert approval.status == "approval_required"
    assert approval.approval_required_count == 1
    assert {item.item_id: item for item in approval.items}["dependent"].status == "blocked"


def test_portfolio_execution_rehydrates_successes_before_dispatch_and_rejects_bad_checkpoint():
    requests = _execution_requests()
    first_agent = _ExecutionAgent()
    first_agent.bind(requests)
    plan = first_agent.real.plan_workflow_portfolio(requests)
    first = execute_autonomous_workflow_portfolio(
        first_agent,
        plan,
        requests,
        credentials={},
        model_candidates=(),
        job_id="restart-job",
    )
    checkpoint = first.checkpoint.to_dict()

    resumed_agent = _ExecutionAgent()
    resumed_agent.bind(requests)
    resumed = execute_autonomous_workflow_portfolio(
        resumed_agent,
        plan,
        requests,
        credentials={},
        model_candidates=(),
        job_id="restart-job",
        checkpoint=checkpoint,
        rehydrate_result=lambda context: _FakeRun("completed", context.task_digest),
    )
    assert resumed.status == "completed"
    assert resumed.completed_count == 3
    assert resumed_agent.calls == []

    with pytest.raises(BrainRunError, match="rehydrate_result"):
        execute_autonomous_workflow_portfolio(
            resumed_agent,
            plan,
            requests,
            credentials={},
            model_candidates=(),
            job_id="restart-job",
            checkpoint=checkpoint,
        )

    tampered = dict(checkpoint)
    tampered["settled_result_digests"] = ["0" * 64] * len(checkpoint["settled_result_digests"])
    with pytest.raises(BrainRunError, match="checkpoint digest"):
        execute_autonomous_workflow_portfolio(
            resumed_agent,
            plan,
            requests,
            credentials={},
            model_candidates=(),
            job_id="restart-job",
            checkpoint=tampered,
            rehydrate_result=lambda context: _FakeRun("completed", context.task_digest),
        )
