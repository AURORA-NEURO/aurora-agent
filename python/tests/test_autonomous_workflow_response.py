from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousBrain,
    AutonomousWorkflowCheckpoint,
    AutonomousWorkflowStageResult,
    BrainLearningLedger,
    BrainRunError,
    BrainRunResult,
    LLMRuntime,
    ProviderResponse,
    builtin_autonomous_workflow_strategies,
    evaluate_autonomous_workflow_stage_response,
    replay_autonomous_workflow_stage_response_evaluation,
    validate_autonomous_workflow_stage_response_evaluation,
)
from prism_sdk.autonomy import _record_workflow_stage_response_feedback
from prism_sdk.errors import ArgumentError


class _LearningWorkspace:
    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []

    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        assert name == "brain_outcome_record"
        payload = {} if arguments is None else dict(arguments)
        self.calls.append(payload)
        state = payload["bandit_state"]
        assert isinstance(state, Mapping)
        return {
            "ok": True,
            "status": "recorded_evaluator_reward",
            "next_state": {**dict(state), "generation": int(state.get("generation", 0)) + 1},
            "learning_evidence": {
                "schema": "bioprism-brain-learning-evidence/0.1",
                "evidence_digest": "f" * 64,
                "evaluator_id": payload["assessment"]["evaluator_id"],
                "evaluator_version": payload["assessment"]["evaluator_version"],
            },
        }


def _workflow_by_domain() -> dict[str, Any]:
    return {workflow.domain: workflow for workflow in builtin_autonomous_workflow_strategies()}


def _stage_response(stage_id: str, *, complete: bool = True) -> dict[str, Any]:
    return {
        "stage_id": stage_id,
        "status": "completed" if complete else "blocked",
        "evidence": ["bounded stage evidence"] if complete else [],
        "uncertainty": ["bounded stage uncertainty"] if complete else [],
        "notes": "bounded stage notes" if complete else "",
        "next_actions": ["bounded caller-approved next action"] if complete else [],
    }


def _stage_evaluation(domain: str, *, complete: bool = True) -> tuple[Any, dict[str, Any], Any]:
    workflow = _workflow_by_domain()[domain]
    stage = workflow.stages[0]
    response = _stage_response(stage.id, complete=complete)
    evaluation = evaluate_autonomous_workflow_stage_response(
        response,
        domain=domain,
        workflow_id=workflow.workflow_id,
        workflow_digest=workflow.workflow_digest,
        stage_id=stage.id,
    )
    return workflow, response, evaluation


def test_stage_integrity_evaluation_covers_every_builtin_domain_and_replays() -> None:
    workflows = _workflow_by_domain()
    assert set(workflows) == set(AUTONOMOUS_DOMAINS)

    for domain in AUTONOMOUS_DOMAINS:
        workflow, response, evaluation = _stage_evaluation(domain)
        assert evaluation.passed is True
        assert evaluation.failed is False
        assert evaluation.reward == 1.0
        assert evaluation.evaluator_id == f"autonomous-{domain}-workflow-stage-integrity"
        assert validate_autonomous_workflow_stage_response_evaluation(evaluation.to_dict()).to_dict() == evaluation.to_dict()
        replayed = replay_autonomous_workflow_stage_response_evaluation(response, evaluation)
        assert replayed.evaluation_digest == evaluation.evaluation_digest
        assert replayed.workflow_digest == workflow.workflow_digest


def test_stage_integrity_evaluation_is_bounded_and_rejects_tampering() -> None:
    workflow, response, evaluation = _stage_evaluation("coding")
    incomplete = evaluate_autonomous_workflow_stage_response(
        _stage_response(workflow.stages[0].id, complete=False),
        domain="coding",
        workflow_id=workflow.workflow_id,
        workflow_digest=workflow.workflow_digest,
        stage_id=workflow.stages[0].id,
    )
    assert incomplete.failed is True
    assert incomplete.replan_requested is True
    assert set(incomplete.missing_signals) == {
        "evidence_present",
        "uncertainty_reported",
        "notes_present",
        "next_actions_present",
    }

    tampered_digest = dict(evaluation.to_dict())
    tampered_digest["evaluation_digest"] = "0" * 64
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_workflow_stage_response_evaluation(tampered_digest)

    unsafe = dict(evaluation.to_dict())
    unsafe["replan_instruction"] = "send gsk_fixture_redacted"
    with pytest.raises(ArgumentError, match="credential"):
        validate_autonomous_workflow_stage_response_evaluation(unsafe)

    changed = dict(response)
    changed["notes"] = "changed after settlement"
    with pytest.raises(ArgumentError, match="drift"):
        replay_autonomous_workflow_stage_response_evaluation(changed, evaluation)


def test_stage_evaluation_round_trips_in_checkpoint_and_binds_workflow_identity() -> None:
    workflow, response, evaluation = _stage_evaluation("coding")
    stage = workflow.stages[0]
    stage_result = AutonomousWorkflowStageResult(
        stage=stage,
        execution_status="completed",
        declared_status="completed",
        result=None,
        structured=response,
        evidence=tuple(response["evidence"]),
        uncertainty=tuple(response["uncertainty"]),
        response_digest="b" * 64,
        response_evaluation=evaluation.to_dict(),
    )
    snapshot = stage_result.checkpoint_snapshot()
    assert snapshot is not None
    assert snapshot["response_digest"] != evaluation.response_digest
    checkpoint = AutonomousWorkflowCheckpoint(
        run_id="workflow-response-checkpoint",
        task_digest="c" * 64,
        workflow_id=workflow.workflow_id,
        workflow_digest=workflow.workflow_digest,
        stages=(snapshot,),
    )
    restored = AutonomousWorkflowCheckpoint.from_dict(checkpoint.to_dict())
    assert restored.stages[0]["response_evaluation"]["evaluation_digest"] == evaluation.evaluation_digest  # type: ignore[index]

    changed_output = checkpoint.to_dict()
    changed_output["stages"] = [dict(changed_output["stages"][0])]  # type: ignore[index]
    changed_output["stages"][0]["structured"] = dict(changed_output["stages"][0]["structured"])  # type: ignore[index]
    changed_output["stages"][0]["structured"]["notes"] = "tampered after evaluation"  # type: ignore[index]
    with pytest.raises((ArgumentError, BrainRunError), match="drift"):
        AutonomousWorkflowCheckpoint.from_dict(changed_output)

    tampered = checkpoint.to_dict()
    tampered["stages"] = [dict(tampered["stages"][0])]  # type: ignore[index]
    tampered["stages"][0]["response_evaluation"] = dict(tampered["stages"][0]["response_evaluation"])  # type: ignore[index]
    tampered["stages"][0]["response_evaluation"]["workflow_id"] = "other-workflow"  # type: ignore[index]
    with pytest.raises((ArgumentError, BrainRunError)):
        AutonomousWorkflowCheckpoint.from_dict(tampered)


def test_stage_response_feedback_is_separate_idempotent_bandit_signal(tmp_path: Path) -> None:
    workflow, response, evaluation = _stage_evaluation("coding")
    run = BrainRunResult(
        run_id="workflow-stage-learning",
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "offline", "model": "fixture"},
            "decision_digest": "a" * 64,
            "context_digest": "b" * 64,
        },
        prompt={"prompt_digest": "c" * 64},
        plan={"plan": {"plan_digest": "d" * 64}},
        response=ProviderResponse(
            provider="offline",
            model="fixture",
            text="provider response is caller-owned",
            status_code=200,
            request_id="workflow-stage-learning",
            usage={},
            raw={},
            structured=response,
        ),
        outcome_digest="e" * 64,
    )
    stage_result = AutonomousWorkflowStageResult(
        stage=workflow.stages[0],
        execution_status="completed",
        declared_status="completed",
        result=run,
        structured=response,
        response_digest="f" * 64,
        response_evaluation=evaluation.to_dict(),
    )
    workspace = _LearningWorkspace()
    ledger = BrainLearningLedger(tmp_path / "workflow-stage-learning.jsonl")
    brain = AutonomousBrain(workspace, LLMRuntime())
    state, record = _record_workflow_stage_response_feedback(
        brain,
        stage_result,
        bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
        ledger=ledger,
    ) or ({}, {})

    assert state["generation"] == 1
    assert record["kind"] == "structured_response"
    assert record["response_evaluation"]["evaluation_digest"] == evaluation.evaluation_digest
    assert len(workspace.calls) == 1
    assert workspace.calls[0]["assessment"]["evaluator_id"] == evaluation.evaluator_id
    assert workspace.calls[0]["idempotency_key"].startswith("workflow-stage-response:")
    assert ledger.replays(run_id=run.run_id)[0]["evaluation_digest"] == evaluation.evaluation_digest
    assert "provider response is caller-owned" not in json.dumps(ledger.records())
