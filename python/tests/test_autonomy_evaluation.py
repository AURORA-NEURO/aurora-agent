from __future__ import annotations

import json
from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomousToolOutcomeEvidence,
    AutonomousToolOutcomeEvaluator,
    AutonomousToolReplayCase,
    AutonomousToolReplayEngine,
    BrainLearningLedger,
    AutonomyPersistenceError,
)
from prism_sdk.errors import ArgumentError


def _outcome(domain: str, *, execution_id: str = "evaluation-run") -> AutonomousToolOutcomeEvidence:
    return AutonomousToolOutcomeEvidence(
        execution_id=execution_id,
        domain=domain,
        capability="observation",
        risk_class="read_only",
        call_id=f"call-{domain}",
        tool=f"{domain}_observe",
        status="executed",
        schema_digest="a" * 64,
        arguments_digest="b" * 64,
        output_digest="c" * 64,
        evidence={"quality_gate": "passed", "sample_count": 3},
    )


def test_tool_evaluator_records_value_only_learning_and_journal_state(tmp_path) -> None:
    journal = AutonomousExecutionJournal(tmp_path / "evaluation.jsonl")
    controller = AutonomousExecutionController(
        execution_id="evaluation-run",
        domain="operations",
        capability="observation",
        risk_class="read_only",
        policy=AutonomousExecutionPolicy(max_steps=4),
        journal=journal,
    )
    outcome = _outcome("operations")
    controller.admit_tool_call(tool=outcome.tool, call_id=outcome.call_id, read_only=True, approval_required=False)
    controller.record_tool_outcome(tool=outcome.tool, call_id=outcome.call_id, status="executed", outcome_digest=outcome.output_digest)
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda _input: {"reward": 1.0, "passed": True},
        evaluator_id="tool-quality",
        evaluator_version="v1",
    )
    ledger = BrainLearningLedger(tmp_path / "learning.jsonl")
    report = evaluator.evaluate_and_record(
        outcome,
        controller=controller,
        bandit_state={"generation": 0, "arms": []},
        bandit_updater=lambda state, _decision, _outcome: {**state, "generation": state["generation"] + 1},
        ledger=ledger,
    )
    assert report["decision"]["passed"] is True
    assert report["next_state"]["generation"] == 1
    assert report["recording"]["record_index"] == 0
    text = json.dumps(ledger.records())
    assert "quality_gate" not in text
    assert '"arguments":' not in text.lower()
    assert journal.state("evaluation-run").last_evaluation_digest is not None


def test_replay_covers_every_autonomous_domain_and_reports_disagreement() -> None:
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda input_value: {"reward": 1.0 if input_value["status"] == "executed" else -1.0, "passed": input_value["status"] == "executed"},
        evaluator_id="replay-evaluator",
        evaluator_version="v2",
    )
    cases = [
        AutonomousToolReplayCase(
            execution_id=f"run-{domain}",
            domain=domain,
            capability="observation",
            risk_class="read_only",
            call_id=f"call-{domain}",
            tool=f"{domain}_observe",
            status="executed",
            schema_digest="a" * 64,
            arguments_digest="b" * 64,
            output_digest="c" * 64,
            evidence={"domain_index": index},
        )
        for index, domain in enumerate(AUTONOMOUS_DOMAINS)
    ]
    expected = evaluator.assess(cases[0].outcome()).decision_digest
    cases[0] = replace(cases[0], expected_decision_digest="d" * 64)
    report = AutonomousToolReplayEngine().replay(
        cases,
        evaluator,
        bandit_state={"generation": 0},
        bandit_updater=lambda state, _decision, _outcome: {**state, "generation": state["generation"] + 1},
    )
    assert report.cases == len(AUTONOMOUS_DOMAINS)
    assert report.disagreements == 1
    assert set(report.by_domain) == set(AUTONOMOUS_DOMAINS)
    assert report.next_bandit_state["generation"] == len(AUTONOMOUS_DOMAINS)
    assert expected is not None


def test_tool_evaluator_rejects_raw_or_secret_shaped_evidence() -> None:
    with pytest.raises(ArgumentError):
        AutonomousToolOutcomeEvidence(
            execution_id="safe",
            domain="operations",
            capability="observation",
            risk_class="read_only",
            call_id="call-1",
            tool="observe",
            status="executed",
            evidence={"api_key": "not allowed"},
        )
