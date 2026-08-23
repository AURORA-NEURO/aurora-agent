from __future__ import annotations

import json
from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousCapabilityRuntime,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    AutonomousDomainToolReceipt,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomousToolOutcomeEvidence,
    AutonomousToolOutcomeEvaluator,
    AutonomousToolReplayCase,
    AutonomousToolReplayEngine,
    BrainLearningLedger,
    builtin_autonomous_domain_tool_profiles,
    builtin_autonomous_workflow_strategies,
    content_digest,
)
from prism_sdk.autonomy import _AUTONOMOUS_CAPABILITY_TOOL_ALIASES
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


def _capability_results() -> tuple:
    profiles = {profile.domain: profile for profile in builtin_autonomous_domain_tool_profiles()}
    workflows = {workflow.domain: workflow for workflow in builtin_autonomous_workflow_strategies()}
    tools = []
    requests = []
    for domain in AUTONOMOUS_DOMAINS:
        workflow = workflows[domain]
        stage = workflow.stages[0]
        aliases = _AUTONOMOUS_CAPABILITY_TOOL_ALIASES[domain]
        binding = next(
            binding
            for binding in profiles[domain].bindings
            if any(
                binding.capability == required
                or binding.capability in aliases.get(required, ())
                for required in stage.required_capabilities
            )
        )
        tools.append(
            AutonomousDomainTool(
                binding.name,
                (domain,),
                binding.capability,
                f"Read bounded {domain} state.",
                {"type": "object", "additionalProperties": False},
            )
        )
        requests.append(
            {
                "call_id": f"evaluation-call-{domain}",
                "tool": binding.name,
                "arguments": {},
                "workflow_context": {
                    "domain": domain,
                    "workflow_id": workflow.workflow_id,
                    "workflow_digest": workflow.workflow_digest,
                    "stage_id": stage.id,
                },
                "input_digest": content_digest({"domain": domain}),
                "subject_digest": None,
                "parent_evidence_digests": [],
                "replay_key": f"evaluation-replay-{domain}",
                "execution_id": f"evaluation-execution-{domain}",
            }
        )
    runtime = AutonomousCapabilityRuntime(
        AutonomousDomainToolRuntime(
            AutonomousDomainToolRegistry(tools),
            executor=lambda _tool, _arguments: {"status": "ok"},
        )
    )
    return runtime.execute_batch(requests, max_parallelism=4)


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


def test_live_receipt_batch_evaluation_advances_bandit_without_transport_reward_inference(tmp_path) -> None:
    receipts = [
        AutonomousDomainToolReceipt(
            call_id="call-operations-1",
            tool="operations_observe",
            status="executed",
            schema_digest="a" * 64,
            arguments_digest="b" * 64,
            output_digest="c" * 64,
            execution_id="live-run",
            domain="operations",
            capability="observation",
            risk_class="read_only",
        ),
        AutonomousDomainToolReceipt(
            call_id="call-operations-2",
            tool="operations_observe",
            status="execution_failed",
            schema_digest="a" * 64,
            arguments_digest="d" * 64,
            execution_id="live-run",
            domain="operations",
            capability="observation",
            risk_class="read_only",
        ),
    ]
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda value: {
            "reward": 1.0 if value["evidence"].get("quality_gate") == "passed" else -1.0,
            "passed": value["evidence"].get("quality_gate") == "passed",
        },
        evaluator_id="live-tool-quality",
        evaluator_version="v1",
    )
    ledger = BrainLearningLedger(tmp_path / "live-tool-learning.jsonl")
    report = evaluator.evaluate_receipts(
        receipts,
        evidence={
            "call-operations-1": {"quality_gate": "passed"},
            "call-operations-2": {"quality_gate": "failed"},
        },
        bandit_state={"generation": 0},
        bandit_updater=lambda state, _decision, _outcome: {**state, "generation": state["generation"] + 1},
        ledger=ledger,
    )
    assert report.status == "completed"
    assert report.receipts == 2
    assert report.by_domain == {"operations": 2}
    assert report.by_status == {"executed": 1, "execution_failed": 1}
    assert report.next_bandit_state["generation"] == 2
    assert len(report.evaluations) == 2
    assert report.learning_digest
    assert "quality_gate" not in json.dumps(ledger.records())
    assert "arguments" not in json.dumps(report.to_dict()).lower()


def test_live_receipt_batch_namespaces_reused_provider_call_ids_by_execution() -> None:
    receipts = (
        AutonomousDomainToolReceipt(
            call_id="reused-call-id",
            tool="workspace_lookup",
            status="executed",
            execution_id="execution-a",
            domain="coding",
            capability="discovery",
            risk_class="read_only",
        ),
        AutonomousDomainToolReceipt(
            call_id="reused-call-id",
            tool="workspace_lookup",
            status="executed",
            execution_id="execution-b",
            domain="operations",
            capability="discovery",
            risk_class="read_only",
        ),
    )
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda value: {
            "reward": 1.0 if value["evidence"].get("accepted") else -1.0,
            "passed": bool(value["evidence"].get("accepted")),
        },
        evaluator_id="namespaced-tool-quality",
        evaluator_version="v1",
    )
    report = evaluator.evaluate_receipts(
        receipts,
        evidence={
            "execution-a:reused-call-id": {"accepted": True},
            "execution-b:reused-call-id": {"accepted": False},
        },
    )
    assert report.receipts == 2
    assert [item["reward"] for item in report.evaluations] == [1.0, -1.0]

    with pytest.raises(ArgumentError, match="duplicate execution_id/call_id"):
        evaluator.evaluate_receipts((receipts[0], replace(receipts[0])))


def test_capability_results_settle_stage_observations_across_every_domain_and_replay_idempotently(tmp_path) -> None:
    results = _capability_results()
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda value: {
            "reward": 1.0 if value["evidence"]["caller_evidence"].get("quality_gate") == "passed" else -1.0,
            "passed": value["evidence"]["caller_evidence"].get("quality_gate") == "passed",
        },
        evaluator_id="capability-quality",
        evaluator_version="v1",
    )
    ledger = BrainLearningLedger(tmp_path / "capability-learning.jsonl")
    evidence = {
        result.record.request_digest: {"quality_gate": "passed"}
        for result in results
    }

    report = evaluator.evaluate_capability_results(
        results,
        evidence=evidence,
        bandit_state={"generation": 0},
        bandit_updater=lambda state, _decision, _outcome: {
            **state,
            "generation": state["generation"] + 1,
        },
        ledger=ledger,
    )

    assert report.receipts == len(AUTONOMOUS_DOMAINS)
    assert set(report.by_domain) == set(AUTONOMOUS_DOMAINS)
    assert report.next_bandit_state["generation"] == len(AUTONOMOUS_DOMAINS)
    assert all(item["status"] == "completed" for item in report.evaluations)
    assert len(ledger.records()) == len(AUTONOMOUS_DOMAINS)
    assert "private" not in json.dumps(ledger.records()).lower()

    restarted_evaluator = AutonomousToolOutcomeEvaluator(
        lambda _value: {"reward": -1.0, "passed": False},
        evaluator_id="capability-quality",
        evaluator_version="v1",
    )
    replay = restarted_evaluator.evaluate_capability_result(
        results[0],
        evidence=evidence[results[0].record.request_digest],
        bandit_state={"generation": 999},
        bandit_updater=lambda state, _decision, _outcome: {
            **state,
            "generation": state["generation"] + 1,
        },
        ledger=ledger,
    )
    assert replay.evaluations[0]["idempotent_replay"] is True
    assert replay.next_bandit_state["generation"] == 1
    assert len(ledger.records()) == len(AUTONOMOUS_DOMAINS)


def test_reconciliation_required_capability_cannot_receive_learning_credit_without_explicit_reconciliation() -> None:
    result = _capability_results()[0]
    reconciliating_record = replace(
        result.record,
        status="reconciliation_required",
        output_digest=None,
        output_bytes=0,
        observations=(),
        evidence_digest=None,
        evidence_status="not_evaluated",
        missing_evidence_outputs=result.record.required_evidence_outputs,
    )
    evaluator = AutonomousToolOutcomeEvaluator(
        lambda _value: {"reward": 0.5, "passed": True},
        evaluator_id="reconciliation-quality",
        evaluator_version="v1",
    )
    with pytest.raises(ArgumentError, match="explicit reconciliation"):
        evaluator.evaluate_capability_result(reconciliating_record)


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
