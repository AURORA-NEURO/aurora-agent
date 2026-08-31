from __future__ import annotations

from dataclasses import dataclass
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainRunError,
    LLMRuntime,
    admit_autonomous_workflow_portfolio,
    validate_autonomous_workflow_portfolio_admission,
)
from prism_sdk.autonomous_workflow_portfolio import execute_autonomous_workflow_portfolio


def _real_agent() -> AutonomousAgent:
    return AutonomousAgent(object(), LLMRuntime())


def _requests():
    return [
        {"id": "root", "task": "review the coding root", "domain": "coding"},
        {"id": "science", "task": "review the science branch", "domain": "science"},
        {"id": "data", "task": "review data after coding", "domain": "data", "depends_on": ["root"]},
    ]


class _AdmissionAgent:
    def __init__(self, *, readiness_state: str = "ready_for_caller_approval"):
        self.real = _real_agent()
        self.readiness_state = readiness_state
        self.calls: list[str] = []

    def route(self, **kwargs):
        return self.real.route(**kwargs)

    def prepare(self, **kwargs):
        return self.real.prepare(**kwargs)

    def models(self, *, enabled_only: bool = False):
        plan = self.real.plan_workflow_portfolio(_requests())
        capabilities = sorted({capability for item in plan["items"] for capability in item["required_capabilities"]})
        return [
            {
                "provider": "fixture",
                "model": "universal",
                "capabilities": capabilities,
                "enabled": True,
                "cost_per_million_tokens": 1,
                "latency_ms": 10,
                "quality": 0.95,
            }
        ]

    def readiness(self, **_kwargs):
        rows = [
            {
                "domain": domain,
                "state": self.readiness_state,
                "next_actions": [],
                "compatible_model_count": 1,
                "eligible_model_count": 1,
            }
            for domain in AUTONOMOUS_DOMAINS
        ]
        return {
            "schema": "fixture-readiness/0.1",
            "domains": rows,
            "domain_learning_coverage": {
                "rows": [
                    {
                        "domain": domain,
                        "observed": True,
                        "evaluation_count": 2,
                        "explored_arm_count": 1,
                    }
                    for domain in AUTONOMOUS_DOMAINS
                ]
            },
        }

    def domain_pack_tool_plan(self, domain):
        return self.real.domain_pack_tool_plan(domain)


@dataclass(frozen=True)
class _FakeRun:
    status: str
    task_digest: str

    def to_dict(self):
        return {
            "schema": "fixture-run/0.1",
            "status": self.status,
            "task_digest": self.task_digest,
            "secret_material": "never_returned",
        }


class _AdmittedRunner(_AdmissionAgent):
    @staticmethod
    def _credential_mapping(credentials):
        return dict(credentials)

    def run_workflow(self, *, blueprint, **_kwargs):
        self.calls.append(blueprint.spec.domain)
        return _FakeRun("completed", blueprint.spec.task_digest)


def test_admission_projects_all_domains_and_roundtrips_without_task_or_credential_material():
    runner = _AdmissionAgent()
    requests = _requests()
    plan = runner.real.plan_workflow_portfolio(requests)
    admission = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=runner.models(),
        require_calibrated_learning=True,
    )

    assert admission.status == "ready_for_approval"
    assert admission.counts.eligible_count == len(requests)
    assert {item.domain for item in admission.items} == {"coding", "science", "data"}
    assert next(item for item in admission.items if item.item_id == "data").dependency_statuses == {"root": "eligible"}
    assert admission.to_dict()["execution"].startswith("admission_only")
    serialized = json.dumps(admission.to_dict(), sort_keys=True)
    assert "review the coding root" not in serialized
    assert "universal" in serialized
    assert "never_returned" in serialized
    restored = validate_autonomous_workflow_portfolio_admission(admission.to_dict())
    assert restored.admission_digest == admission.admission_digest


def test_admission_closes_dependencies_and_surfaces_model_and_calibration_gates():
    runner = _AdmissionAgent()
    requests = _requests()
    plan = runner.real.plan_workflow_portfolio(requests)
    only_reasoning = {
        "provider": "fixture",
        "model": "reasoning-only",
        "capabilities": ["reasoning"],
        "enabled": True,
    }
    admission = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=[only_reasoning],
    )
    rows = {item.item_id: item for item in admission.items}
    assert admission.status == "blocked"
    assert "selection:no_model_matches_run_constraints" in rows["root"].blockers
    assert rows["data"].status == "dependency_blocked"
    assert "dependency:not_eligible" in rows["data"].blockers

    calibrated = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=runner.models(),
        require_calibrated_learning=True,
    )
    assert calibrated.status == "ready_for_approval"

    runner.readiness_state = "credential_required"
    credential_blocked = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=runner.models(),
    )
    assert credential_blocked.status == "blocked"
    assert credential_blocked.counts.credential_required_count == len(requests)


def test_admission_rejects_tampering_and_plan_drift_before_execution():
    runner = _AdmissionAgent()
    requests = _requests()
    plan = runner.real.plan_workflow_portfolio(requests)
    admission = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=runner.models(),
    )
    tampered = dict(admission.to_dict())
    tampered["admission_digest"] = "0" * 64
    with pytest.raises(BrainRunError, match="admission digest"):
        validate_autonomous_workflow_portfolio_admission(tampered)

    changed = [dict(request) for request in requests]
    changed[0]["task"] = "a different root"
    with pytest.raises(BrainRunError, match="verification"):
        admit_autonomous_workflow_portfolio(
            runner,
            changed,
            plan=plan,
            model_candidates=runner.models(),
        )


def test_execution_requires_the_same_admission_digest_on_restart_and_dispatches_only_eligible_items():
    runner = _AdmittedRunner()
    requests = _requests()
    plan = runner.real.plan_workflow_portfolio(requests)
    admission = admit_autonomous_workflow_portfolio(
        runner,
        requests,
        plan=plan,
        model_candidates=runner.models(),
    )
    execution = execute_autonomous_workflow_portfolio(
        runner,
        plan,
        requests,
        credentials={},
        model_candidates=runner.models(),
        admission=admission.to_dict(),
        job_id="admitted-portfolio",
    )
    assert execution.status == "completed"
    assert execution.admission_digest == admission.admission_digest
    checkpoint = execution.checkpoint.to_dict()
    assert checkpoint["portfolio_input_digest"]

    with pytest.raises(BrainRunError, match="checkpoint requests"):
        execute_autonomous_workflow_portfolio(
            runner,
            plan,
            requests,
            credentials={},
            model_candidates=runner.models(),
            job_id="admitted-portfolio",
            checkpoint=checkpoint,
            rehydrate_result=lambda _context: _FakeRun("completed", "wrong"),
        )

    calls_before = len(runner.calls)
    resumed = execute_autonomous_workflow_portfolio(
        runner,
        plan,
        requests,
        credentials={},
        model_candidates=runner.models(),
        admission=admission.to_dict(),
        job_id="admitted-portfolio",
        checkpoint=checkpoint,
        rehydrate_result=lambda context: _FakeRun("completed", context.task_digest),
    )
    assert resumed.status == "completed"
    assert len(runner.calls) == calls_before
