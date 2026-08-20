from __future__ import annotations

import hashlib
import json
from dataclasses import replace

import pytest

from prism_sdk.brain import (
    AutonomousBrain,
    BrainLearningEpisode,
    BrainLearningLedger,
    BrainLearningTrajectory,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    _bandit_observations,
)
from prism_sdk.llm_runtime import LLMRuntime, ProviderResponse


class _Workspace:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        payload = {} if arguments is None else dict(arguments)
        self.calls.append((name, payload))
        if name != "brain_outcome_record":
            raise AssertionError(f"unexpected tool {name}")
        state = payload["bandit_state"]
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


def _result() -> BrainRunResult:
    return BrainRunResult(
        run_id="episode-run",
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "openai", "model": "model-a"},
            "decision_digest": "a" * 64,
            "context_digest": "b" * 64,
            "selection_audit": {"routing_confidence": 0.7},
        },
        prompt={"prompt_digest": "c" * 64},
        plan={"plan": {"plan_digest": "d" * 64}},
        response=ProviderResponse(
            provider="openai",
            model="model-a",
            text="provider response must not be retained",
            status_code=200,
            request_id="request-1",
            usage={"total_tokens": 8},
            raw={},
        ),
        outcome_digest="e" * 64,
    )


def _empty_state() -> dict[str, object]:
    return {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}


def test_delayed_learning_episode_is_restart_safe_and_settles_once(tmp_path) -> None:
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, LLMRuntime())
    ledger_path = tmp_path / "learning.jsonl"
    ledger = BrainLearningLedger(ledger_path)
    episode = brain.prepare_learning_episode(
        _result(),
        evidence={"quality": 0.9},
        ledger=ledger,
    )

    assert isinstance(episode, BrainLearningEpisode)
    assert episode.status == "pending"
    assert len(ledger.pending_episodes()) == 1
    assert ledger.begin_episode(episode)["idempotent"] is True
    encoded = json.dumps(episode.to_dict())
    assert "provider response must not be retained" not in encoded
    assert "api_key" not in encoded

    restored = BrainLearningLedger(ledger_path)
    evaluator = BrainOutcomeEvaluator(
        lambda value: {
            "reward": value["evidence"]["quality"],  # type: ignore[index]
            "passed": True,
        },
        evaluator_id="quality-evaluator",
        evaluator_version="1",
    )
    restored_episode = restored.pending_episodes()[0]
    decision, report = evaluator.evaluate_episode(
        brain,
        restored_episode,
        bandit_state=_empty_state(),
        evidence={"quality": 0.9},
        ledger=restored,
    )

    assert decision.reward == 0.9
    assert report["status"] == "recorded_evaluator_reward"
    assert restored.pending_episodes() == []
    outcome_call = workspace.calls[-1][1]
    sent_state = outcome_call["bandit_state"]
    assert isinstance(sent_state, dict)
    assert sent_state["arms"][0]["arm_id"] == "openai/model-a"  # type: ignore[index]
    assert outcome_call["run"]["outcome_digest"] == "e" * 64  # type: ignore[index]
    assert outcome_call["idempotency_key"] == f"episode:{restored_episode.episode_id}"

    with pytest.raises(BrainRunError, match="already settled"):
        evaluator.evaluate_episode(
            brain,
            episode,
            bandit_state=_empty_state(),
            evidence={"quality": 0.9},
            ledger=restored,
        )


def test_delayed_episode_rejects_changed_evidence() -> None:
    brain = AutonomousBrain(_Workspace(), LLMRuntime())
    episode = brain.prepare_learning_episode(_result(), evidence={"quality": 0.9})
    evaluator = BrainOutcomeEvaluator(
        lambda _: {"reward": 1.0, "passed": True},
        evaluator_id="quality-evaluator",
        evaluator_version="1",
    )
    with pytest.raises(BrainRunError, match="does not match"):
        evaluator.evaluate_episode(
            brain,
            episode,
            bandit_state=_empty_state(),
            evidence={"quality": 0.1},
        )


def test_immediate_learning_bootstraps_selected_arm_from_empty_state() -> None:
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, LLMRuntime())
    report = brain.record_evaluator_outcome(
        _result(),
        bandit_state=_empty_state(),
        evaluator_id="quality-evaluator",
        evaluator_version="1",
        reward=0.5,
        passed=True,
    )
    assert report["status"] == "recorded_evaluator_reward"
    sent_state = workspace.calls[-1][1]["bandit_state"]
    assert isinstance(sent_state, dict)
    assert sent_state["arms"][0]["arm_id"] == "openai/model-a"  # type: ignore[index]


def test_contextual_learning_persists_scoped_arm_and_sends_canonical_identity(tmp_path) -> None:
    context = {
        "domain": "coding",
        "capability": "implementation",
        "risk_class": "engineering_change",
        "task_family": "coding_delivery",
    }
    context_digest = hashlib.sha256(
        json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    result = replace(
        _result(),
        selection={
            "selected_model": {"provider": "openai", "model": "model-a"},
            "decision_digest": "a" * 64,
            "context_digest": context_digest,
            "context": context,
        },
    )
    workspace = _Workspace()
    ledger = BrainLearningLedger(tmp_path / "contextual.jsonl")
    report = AutonomousBrain(workspace, LLMRuntime()).record_evaluator_outcome(
        result,
        bandit_state=_empty_state(),
        evaluator_id="quality-evaluator",
        evaluator_version="context-1",
        reward=0.8,
        passed=True,
        ledger=ledger,
    )
    assert report["status"] == "recorded_evaluator_reward"
    payload = workspace.calls[-1][1]
    assert payload["context_digest"] == context_digest
    assert payload["context"] == context
    sent_state = payload["bandit_state"]
    assert isinstance(sent_state, dict)
    assert sent_state["arms"] == []
    contextual = sent_state["contextual_states"][0]  # type: ignore[index]
    assert contextual["context_digest"] == context_digest
    assert contextual["arms"][0]["arm_id"] == "openai/model-a"
    scoped = ledger.contextual_state(context)
    assert scoped["context_digest"] == context_digest
    assert scoped["bandit_state"]["arms"][0]["arm_id"] == "openai/model-a"  # type: ignore[index]


def test_contextual_observations_overlay_global_cold_start_without_cross_context_leakage() -> None:
    context = {
        "domain": "coding",
        "capability": "implementation",
        "risk_class": "engineering_change",
        "task_family": "coding_delivery",
    }
    context_digest = hashlib.sha256(
        json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    state = {
        "arms": [
            {"arm_id": "global-a", "pulls": 10, "reward_sum": 4.0, "failures": 0, "disabled": False},
            {"arm_id": "global-b", "pulls": 8, "reward_sum": 3.0, "failures": 0, "disabled": False},
        ],
        "contextual_states": [
            {
                "context_digest": context_digest,
                "context": context,
                "generation": 1,
                "observed": True,
                "arms": [
                    {"arm_id": "global-a", "pulls": 3, "reward_sum": 2.7, "failures": 0, "disabled": False},
                    {"arm_id": "context-only", "pulls": 2, "reward_sum": 1.8, "failures": 0, "disabled": False},
                ],
            }
        ],
    }
    observations = _bandit_observations(state, context_digest=context_digest)
    by_arm = {row["arm_id"]: row for row in observations}
    assert by_arm["global-a"]["reward_sum"] == 2.7
    assert by_arm["global-b"]["reward_sum"] == 3.0
    assert by_arm["context-only"]["reward_sum"] == 1.8


def test_trajectory_credit_assignment_is_discounted_and_restart_safe(tmp_path) -> None:
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, LLMRuntime())
    first = _result()
    second = replace(first, run_id="episode-run-2", outcome_digest="f" * 64)
    ledger = BrainLearningLedger(tmp_path / "trajectory.jsonl")
    trajectory = brain.prepare_learning_trajectory(
        [first, second],
        evidence_by_step=[{"quality": 0.4}, {"quality": 0.2}],
        trajectory_id="workflow-trajectory",
        discount=0.5,
        terminal_reward=0.25,
        ledger=ledger,
    )
    assert isinstance(trajectory, BrainLearningTrajectory)
    assert len(ledger.pending_episodes()) == 2

    evaluator = BrainOutcomeEvaluator(
        lambda value: {
            "reward": value["evidence"]["quality"],  # type: ignore[index]
            "passed": True,
        },
        evaluator_id="quality-evaluator",
        evaluator_version="trajectory-1",
    )
    restored = BrainLearningLedger(ledger.path)
    result = evaluator.evaluate_trajectory(
        brain,
        trajectory,
        bandit_state=_empty_state(),
        evidence_by_step=[{"quality": 0.4}, {"quality": 0.2}],
        ledger=restored,
    )

    assert result.status == "settled"
    assert result.credited_rewards == pytest.approx((0.5625, 0.325))
    assert len(result.recordings) == 2
    assert BrainLearningLedger(ledger.path).pending_episodes() == []
    outcome_calls = [payload for name, payload in workspace.calls if name == "brain_outcome_record"]
    assert [payload["assessment"]["reward"] for payload in outcome_calls] == pytest.approx([0.5625, 0.325])  # type: ignore[index]
    assert all(payload["assessment"]["evaluator_version"] == "trajectory-1" for payload in outcome_calls)  # type: ignore[index]
