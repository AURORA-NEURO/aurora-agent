from __future__ import annotations

import json
from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_EVALUATOR_MESH_SCHEMA,
    AutonomousBrain,
    AutonomousEvaluatorMesh,
    AutonomousEvaluatorMeshResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    LLMRuntime,
    ProviderResponse,
    build_brain_evaluation_input,
)
from prism_sdk.brain import _context_identity_digest


def _result(domain: str) -> BrainRunResult:
    context = {
        "domain": domain,
        "capability": "review",
        "risk_class": "read_only",
        "task_family": "mesh-test",
    }
    return BrainRunResult(
        run_id=f"mesh-run-{domain}",
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "mesh-provider", "model": "mesh-model"},
            "decision_digest": "a" * 64,
            "context_digest": _context_identity_digest(context),
            "context": context,
        },
        prompt={"prompt_digest": "c" * 64},
        plan={"plan": {"plan_digest": "d" * 64}},
        response=ProviderResponse(
            provider="mesh-provider",
            model="mesh-model",
            text="private provider response must not cross the evaluator boundary",
            status_code=200,
            request_id="mesh-request",
            usage={"total_tokens": 4},
            raw={"private": "wire payload"},
        ),
        outcome_digest="e" * 64,
    )


def _member(identifier: str, reward: float, passed: bool, **extra: object) -> BrainOutcomeEvaluator:
    return BrainOutcomeEvaluator(
        lambda _input: {"reward": reward, "passed": passed, **extra},
        evaluator_id=identifier,
        evaluator_version="v1",
    )


def test_mesh_accepts_agreement_and_integrates_with_value_only_learning_boundary() -> None:
    mesh = AutonomousEvaluatorMesh(
        (
            _member("reviewer-a", 0.9, True),
            _member("reviewer-b", 0.86, True),
        ),
        max_reward_spread=0.1,
    )
    detailed = mesh.evaluate_detailed(_result("coding"), evidence={"quality": 1.0})

    assert isinstance(detailed, AutonomousEvaluatorMeshResult)
    assert detailed.schema == AUTONOMOUS_EVALUATOR_MESH_SCHEMA
    assert detailed.status == "accepted"
    assert detailed.reward == 0.88
    assert detailed.passed is True
    assert detailed.reward_spread == 0.04
    assert len(detailed.member_results) == 2
    assert "private provider response" not in json.dumps(detailed.to_dict())
    assert "wire payload" not in json.dumps(detailed.to_dict())
    with pytest.raises(BrainRunError, match="mesh_digest"):
        replace(detailed, reward=0.5)

    decision = mesh.assess(_result("coding"), evidence={"quality": 1.0})
    assert decision.reward == 0.88
    assert decision.evaluator_id == "python-evaluator-mesh"
    assert decision.evidence_digest is not None


def test_mesh_covers_every_autonomous_domain_and_replays_projected_input() -> None:
    mesh = AutonomousEvaluatorMesh(
        (
            _member("reviewer-a", 0.8, True),
            _member("reviewer-b", 0.8, True),
        )
    )
    for domain in AUTONOMOUS_DOMAINS:
        result = _result(domain)
        evidence = {"domain": domain, "signals": {"evidence_complete": True}}
        projected = build_brain_evaluation_input(result, evidence=evidence)
        detailed = mesh.evaluate_detailed_value_only_input(projected)
        assert detailed.status == "accepted", domain
        assert detailed.evidence_digest == projected["evidence_digest"], domain
        assert detailed.mesh_digest and len(detailed.mesh_digest) == 64


def test_mesh_refuses_disagreement_and_member_errors_without_leaking_details() -> None:
    disagreement = AutonomousEvaluatorMesh(
        (
            _member("reviewer-a", 0.9, True),
            _member("reviewer-c", 0.2, False, failed=True, failure_class="quality_gate"),
        )
    )
    refused = disagreement.evaluate_detailed(_result("operations"), evidence={"quality": 0.2})
    assert refused.status == "disagreement"
    assert refused.reward is None
    with pytest.raises(BrainRunError, match="refused learning credit"):
        disagreement.assess(_result("operations"), evidence={"quality": 0.2})

    def raises(_input: object) -> object:
        raise RuntimeError("private evaluator transport detail")

    member_error = AutonomousEvaluatorMesh(
        (
            _member("reviewer-a", 0.9, True),
            BrainOutcomeEvaluator(raises, evaluator_id="reviewer-error", evaluator_version="v1"),
        )
    )
    errored = member_error.evaluate_detailed(_result("biomedical"), evidence={"quality": 0.2})
    encoded = json.dumps(errored.to_dict())
    assert errored.status == "member_error"
    assert errored.reward is None
    assert "private evaluator transport detail" not in encoded


def test_mesh_is_usable_by_brain_recording_adapter_without_provider_access() -> None:
    class Workspace:
        def tool(self, name: str, arguments: object = None) -> dict[str, object]:
            assert name == "brain_outcome_record"
            assert isinstance(arguments, dict)
            state = arguments["bandit_state"]
            assert isinstance(state, dict)
            return {
                "ok": True,
                "status": "recorded_evaluator_reward",
                "next_state": {**state, "generation": int(state.get("generation", 0)) + 1},
                "learning_evidence": {
                    "schema": "bioprism-brain-learning-evidence/0.1",
                    "evidence_digest": "f" * 64,
                },
            }

    mesh = AutonomousEvaluatorMesh(
        (
            _member("reviewer-a", 0.9, True),
            _member("reviewer-b", 0.86, True),
        )
    )
    brain = AutonomousBrain(Workspace(), LLMRuntime())
    _decision, report = mesh.evaluate_and_record_with_decision(
        brain,
        _result("evaluation"),
        bandit_state={"generation": 0, "arms": []},
        evidence={"quality": 1.0},
    )
    assert report["next_state"]["generation"] == 1
