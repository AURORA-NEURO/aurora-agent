from __future__ import annotations

import copy
import pytest

from prism_sdk.autonomous_execution_policy import (
    AUTONOMOUS_EXECUTION_POLICY_DOMAINS,
    AutonomousExecutionPolicy,
    validate_autonomous_execution_policy_decision,
    validate_autonomous_execution_policy_state,
)
from prism_sdk.brain import AutonomousBrain
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.errors import ArgumentError


def _digest(character: str) -> str:
    return character * 64


def _candidate(arm_id: str, domain: str, **overrides):
    value = {
        "arm_id": arm_id,
        "domain": domain,
        "path": "provider",
        "capabilities": ["reasoning", "structured_output"],
        "quality_prior": 0.7,
        "reliability": 0.8,
        "cost_units": 4,
        "latency_ms": 120,
        "risk": 0.1,
        "structured_output": True,
        "effects_supported": True,
        "provider": "test-provider",
        "model": f"test-model-{arm_id}",
    }
    value.update(overrides)
    return value


def test_joint_execution_policy_selects_every_domain_and_settles_credit() -> None:
    policy = AutonomousExecutionPolicy(exploration=0.4)
    initial_state = policy.snapshot()
    candidates = [_candidate(f"arm-{domain}", domain) for domain in AUTONOMOUS_EXECUTION_POLICY_DOMAINS]
    decision = policy.select(
        {
            "context_digest": _digest("a"),
            "requested_domains": list(AUTONOMOUS_EXECUTION_POLICY_DOMAINS),
            "required_capabilities": ["reasoning"],
            "preferred_capabilities": ["structured_output"],
            "structured_output_required": True,
            "max_cost_units": 10,
            "max_latency_ms": 500,
            "max_risk": 0.5,
        },
        candidates,
    )
    assert decision.posture == "selected"
    assert decision.selected_arm_id == "arm-biomedical"
    assert len(decision.rankings) == len(AUTONOMOUS_EXECUTION_POLICY_DOMAINS)
    assert {row.domain for row in decision.rankings} == set(AUTONOMOUS_EXECUTION_POLICY_DOMAINS)
    assert decision.context.context_digest == _digest("a")
    assert "prompt text must not be retained" not in str(decision.to_dict())

    settlement_input = {
        "settlement_id": "settle-1",
        "arm_id": decision.selected_arm_id,
        "decision_digest": decision.decision_digest,
        "outcome_digest": _digest("b"),
        "reward": 0.92,
        "passed": True,
        "evaluator_id": "domain-evaluator",
        "evaluator_version": "2026.08",
    }
    settlement = policy.settle(decision, **settlement_input)
    assert settlement.idempotent_replay is False
    assert settlement.generation == 1
    assert next(arm for arm in policy.snapshot()["arms"] if arm["arm_id"] == decision.selected_arm_id)["pulls"] == 1
    replay = policy.settle(decision, **settlement_input)
    assert replay.idempotent_replay is True
    assert replay.next_state_digest == settlement.next_state_digest
    assert policy.snapshot()["generation"] == 1
    with pytest.raises(ArgumentError, match="roll back"):
        policy.restore(initial_state)
    restored = validate_autonomous_execution_policy_state(policy.snapshot())
    assert restored.state_digest == policy.snapshot()["state_digest"]
    restored_decision = validate_autonomous_execution_policy_decision(copy.deepcopy(decision.to_dict()))
    assert restored_decision.decision_digest == decision.decision_digest


def test_joint_execution_policy_gates_before_scoring_and_preserves_review_posture() -> None:
    policy = AutonomousExecutionPolicy()
    decision = policy.select(
        {
            "requested_domains": ["coding"],
            "required_capabilities": ["search"],
            "required_path": "evidence_first",
            "evidence_required": True,
            "structured_output_required": True,
            "max_cost_units": 20,
            "max_latency_ms": 1_000,
            "max_risk": 0.5,
        },
        [
            _candidate("blocked", "coding", path="provider", capabilities=["reasoning"], evidence_ready=False),
            _candidate("eligible-review", "coding", path="evidence_first", capabilities=["search"], evidence_ready=True, approval_required=True),
            _candidate("wrong-domain", "science", path="evidence_first", capabilities=["search"], evidence_ready=True),
        ],
    )
    assert decision.posture == "review_required"
    assert decision.selected_arm_id == "eligible-review"
    assert decision.review_reasons == ("candidate_approval_required",)
    blocked = next(row for row in decision.rankings if row.arm_id == "blocked")
    assert blocked.eligible is False
    assert "path_not_requested" in blocked.reasons
    assert "required_capability_missing" in blocked.reasons
    assert "evidence_not_ready" in blocked.reasons
    assert next(row for row in decision.rankings if row.arm_id == "wrong-domain").eligible is False


def test_joint_execution_policy_refuses_impossible_work_and_rejects_forgery() -> None:
    policy = AutonomousExecutionPolicy()
    refused = policy.select({"requested_domains": ["neuroscience"], "max_risk": 0.01}, [_candidate("unsafe", "neuroscience", risk=0.9)])
    assert refused.posture == "refused"
    assert refused.selected_arm_id is None
    assert "risk_budget_exceeded" in refused.refusal_reasons
    with pytest.raises(ArgumentError, match="refused"):
        policy.settle(refused, settlement_id="bad", arm_id="unsafe", decision_digest=refused.decision_digest, outcome_digest=_digest("c"), reward=1, passed=True, evaluator_id="evaluator", evaluator_version="1")
    state = policy.snapshot()
    forged = dict(state)
    forged["generation"] = 1
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_execution_policy_state(forged)


def test_brain_composes_route_admission_with_joint_policy_without_provider_dispatch() -> None:
    brain = AutonomousBrain(object(), LLMRuntime())
    result = brain.select_execution_policy(
        task="analyze the science evidence",
        domain="science",
        hints=("science",),
        candidates=[
            _candidate("science-arm", "science", evidence_ready=True, structured_output=True),
        ],
    )
    assert result["schema"] == "bioprism-python-autonomous-brain-execution-policy/0.1"
    assert result["decision"]["selected_arm_id"] == "science-arm"
    assert "analyze the science evidence" not in str(result)
