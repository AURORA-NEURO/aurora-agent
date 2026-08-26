from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousProviderEvaluatorAssessment,
    AutonomousProviderInvocationReceipt,
    AutonomousProviderOutcomeEvaluator,
    autonomous_provider_outcome_evaluation_input,
    autonomous_provider_receipt_identity,
    content_digest,
    settle_autonomous_provider_model_outcome,
    LLMRuntime,
)
from prism_sdk.errors import ArgumentError


def _receipt(domain: str, index: int, *, outcome: str = "success") -> AutonomousProviderInvocationReceipt:
    return AutonomousProviderInvocationReceipt(
        execution_id=f"provider-evaluation-{index}",
        provider="provider-fixture",
        model=f"model-{index}",
        kind="answer",
        attempt=0,
        turn=0,
        status="completed" if outcome == "success" else "provider_refused",
        outcome=outcome,
        input_tokens=128,
        output_tokens=64 if outcome == "success" else 0,
        estimated_cost_units=0.25,
        actual_cost_units=0.2 if outcome == "success" else 0.0,
        latency_ms=40 + index,
        selection_digest=content_digest({"selection": "reviewed", "domain": domain}),
        outcome_digest=content_digest({"provider": "provider-fixture", "model": f"model-{index}", "domain": domain, "outcome": outcome}),
        request_id_digest=None,
        failure_class=None if outcome == "success" else "rate_limited",
        status_code=None if outcome == "success" else 429,
    )


def test_provider_receipts_drive_explicit_contextual_model_learning_across_all_domains() -> None:
    receipts = tuple(_receipt(domain, index) for index, domain in enumerate(AUTONOMOUS_DOMAINS))
    contexts = {}
    evidence = {}
    for index, receipt in enumerate(receipts):
        identity = autonomous_provider_receipt_identity(receipt)
        context = {
            "domain": AUTONOMOUS_DOMAINS[index],
            "capability": "answer",
            "risk_class": "read_only",
            "task_family": "provider-evaluation",
        }
        projected = autonomous_provider_outcome_evaluation_input(receipt, context=context)
        contexts[identity] = {**context, "context_digest": projected.context_digest, "contract_digest": receipt.selection_digest}
        evidence[identity] = {"evaluator_signal": "reviewed"}

    callback_inputs = []

    def evaluate(value):
        callback_inputs.append(value)
        assert value["evidence"]["evaluator_signal"] == "reviewed"
        assert value.get("prompt") is None
        assert value.get("response") is None
        assert value.get("messages") is None
        assert value.get("credentials") is None
        return {"reward": 0.8, "passed": True}

    evaluator = AutonomousProviderOutcomeEvaluator(
        evaluate,
        evaluator_id="provider-quality",
        evaluator_version="2026-08-26",
    )
    settled = evaluator.evaluate_receipts(
        receipts,
        contexts=contexts,
        evidence=evidence,
        learning_state={"generation": 0},
        learning_updater=settle_autonomous_provider_model_outcome,
    )
    assert settled.status == "completed"
    assert settled.receipts == len(AUTONOMOUS_DOMAINS)
    assert len(callback_inputs) == len(AUTONOMOUS_DOMAINS)
    assert len(settled.by_domain) == len(AUTONOMOUS_DOMAINS)
    assert len(settled.by_model) == len(AUTONOMOUS_DOMAINS)
    assert settled.next_learning_state["generation"] == len(AUTONOMOUS_DOMAINS)
    assert len(settled.next_learning_state["contextual_states"]) == len(AUTONOMOUS_DOMAINS)
    assert all(len(row["arms"]) == 1 for row in settled.next_learning_state["contextual_states"])
    assert len(settled.next_learning_state["credited_outcomes"]) == len(AUTONOMOUS_DOMAINS)
    assert all(item["learning_update"] == "applied" for item in settled.evaluations)
    assert all(item["context_digest"] for item in settled.evaluations)
    assert "prompt" not in json.dumps(settled.to_dict()).lower()
    assert "response" not in json.dumps(settled.to_dict()).lower()
    assert "credentials" not in json.dumps(settled.to_dict()).lower()

    replayed = evaluator.evaluate_receipts(
        receipts,
        contexts=contexts,
        evidence=evidence,
        learning_state=settled.next_learning_state,
        learning_updater=settle_autonomous_provider_model_outcome,
    )
    assert all(item["idempotent_replay"] for item in replayed.evaluations)
    assert replayed.next_learning_state == settled.next_learning_state
    assert replayed.learning_digest == settled.learning_digest


def test_provider_evaluation_rejects_duplicates_unsafe_evidence_tampered_context_and_bad_rewards() -> None:
    receipt = _receipt("coding", 1, outcome="failure")
    identity = autonomous_provider_receipt_identity(receipt)
    evaluator = AutonomousProviderOutcomeEvaluator(
        lambda _value: {"reward": 0.0, "passed": False, "failed": True},
        evaluator_id="provider-quality-safe",
        evaluator_version="1",
    )
    with pytest.raises(ArgumentError, match="duplicate identities"):
        evaluator.evaluate_receipts((receipt, receipt))
    with pytest.raises(ArgumentError, match="transient or secret-shaped"):
        evaluator.evaluate_receipts((receipt,), evidence={identity: {"response": "forbidden"}})
    with pytest.raises(ArgumentError, match="does not match"):
        evaluator.evaluate_receipts(
            (receipt,),
            contexts={identity: {"domain": "coding", "capability": "answer", "risk_class": "read_only", "context_digest": "a" * 64}},
        )
    bad = AutonomousProviderOutcomeEvaluator(
        lambda _value: {"reward": 2.0, "passed": True},
        evaluator_id="provider-quality-bad",
        evaluator_version="1",
    )
    with pytest.raises(ArgumentError, match=r"within \[-1, 1\]"):
        bad.evaluate_receipts((receipt,))
    projected = autonomous_provider_outcome_evaluation_input(receipt, context={"domain": "coding", "capability": "answer", "risk_class": "read_only"})
    assert projected.status == "provider_refused"
    assert projected.outcome == "failure"
    assert not hasattr(projected, "prompt")


def test_provider_assessment_dataclass_is_accepted() -> None:
    receipt = _receipt("evaluation", 4)
    evaluator = AutonomousProviderOutcomeEvaluator(
        lambda _value: AutonomousProviderEvaluatorAssessment(reward=0.5, passed=True),
        evaluator_id="provider-quality-dataclass",
        evaluator_version="1",
    )
    report = evaluator.evaluate_receipts((receipt,), learning_state={"generation": 0}, learning_updater=settle_autonomous_provider_model_outcome)
    assert report.evaluations[0]["reward"] == 0.5


def test_agent_facade_uses_the_default_model_learning_updater() -> None:
    receipt = _receipt("coding", 5)
    evaluator = AutonomousProviderOutcomeEvaluator(
        lambda _value: {"reward": 0.75, "passed": True},
        evaluator_id="facade-provider-quality",
        evaluator_version="1",
    )
    agent = AutonomousAgent(object(), LLMRuntime())
    report = agent.evaluate_provider_receipts(evaluator=evaluator, receipts=(receipt,))
    assert report.next_learning_state["generation"] == 1
    assert report.next_learning_state["arms"][0]["arm_id"] == "provider-fixture/model-5"
