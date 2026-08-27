import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorOperationRegistry,
    InMemoryAutonomousConnectorFeedbackLedger,
    LLMRuntime,
    MissionRequest,
    MissionStep,
)
from prism_sdk.autonomous_builtin_connectors import _RECOMMENDED_FIELDS
from prism_sdk.errors import ArgumentError


def _agent(tmp_path):
    journal = AutonomousConnectorReceiptJournal(tmp_path / "mission-receipts.jsonl")
    agent = AutonomousAgent(object(), LLMRuntime())
    agent.register_builtin_domain_connectors(receipt_store=journal)
    return agent, journal


def _mission(domain: str, *, mission_id: str = "connector-mission") -> MissionRequest:
    operation = AutonomousConnectorOperationRegistry().for_domain(domain)[0]
    arguments = {
        field: {"fixture": f"{domain}-metadata"}
        for field in _RECOMMENDED_FIELDS[operation.operation_id]
    }
    return MissionRequest(
        mission_id=mission_id,
        goal=f"offline {domain} evidence observation",
        steps=(
            MissionStep(
                id="observe",
                domain=domain,
                capability=operation.capabilities[0],
                objective=f"observe {domain}",
                tool="connector_fixture",
                arguments=arguments,
            ),
        ),
    )


def test_connector_mission_executes_all_domains_without_credentials(tmp_path) -> None:
    agent, journal = _agent(tmp_path)

    for domain in AUTONOMOUS_DOMAINS:
        result = agent.run_connector_mission(mission=_mission(domain, mission_id=f"mission-{domain}"), approved=True)
        assert result.status == "completed", domain
        assert result.completed_step_ids == ("observe",), domain
        assert result.next_step_ids == (), domain
        assert result.step_executions[0].status == "completed", domain
        serialized = json.dumps(result.to_dict())
        assert "fixture" not in serialized
        assert json.loads(serialized)["step_executions"][0]["value_retained"] is False

    assert journal.verify_integrity()["entries"] == len(AUTONOMOUS_DOMAINS)


def test_connector_mission_quality_gate_holds_dependents_and_requires_retry(tmp_path) -> None:
    agent, journal = _agent(tmp_path)
    operation = AutonomousConnectorOperationRegistry().for_domain("coding")[0]
    arguments = {field: {"fixture": "coding-quality"} for field in _RECOMMENDED_FIELDS[operation.operation_id]}
    mission = MissionRequest(
        mission_id="quality-gated-mission",
        goal="hold dependent work until the observation is reviewed",
        steps=(
            MissionStep("first", "coding", operation.capabilities[0], "first", "fixture", arguments=arguments),
            MissionStep("second", "coding", operation.capabilities[0], "second", "fixture", arguments=arguments, depends_on=("first",)),
        ),
    )
    evaluations = {"first": 0, "second": 0}

    def quality(context):
        evaluations[context.step.id] += 1
        passed = not (context.step.id == "first" and evaluations[context.step.id] == 1)
        return {
            "evaluator_id": "connector-quality-reviewer",
            "evaluator_version": "1",
            "reward": 1.0 if passed else 0.0,
            "passed": passed,
            "evidence_digest": None,
        }

    first = agent.run_connector_mission(mission=mission, approved=True, quality_evaluator=quality)
    assert first.status == "blocked"
    assert first.completed_step_ids == ()
    assert first.next_step_ids == ("first",)
    assert first.checkpoint["steps"][0]["status"] == "quality_blocked"
    assert first.checkpoint["steps"][0]["quality"]["passed"] is False
    assert "coding-quality" not in json.dumps(first.to_dict())
    assert evaluations == {"first": 1, "second": 0}

    held = agent.run_connector_mission(
        mission=mission,
        checkpoint=first.checkpoint,
        approved=True,
        quality_evaluator=quality,
    )
    assert held.status == "checkpoint_blocked"
    assert evaluations == {"first": 1, "second": 0}

    retried = agent.run_connector_mission(
        mission=mission,
        checkpoint=first.checkpoint,
        approved=True,
        retry_blocked=True,
        quality_evaluator=quality,
    )
    assert retried.status == "completed"
    assert retried.completed_step_ids == ("first", "second")
    assert evaluations == {"first": 2, "second": 1}
    assert retried.step_executions[0].quality["passed"] is True
    assert journal.verify_integrity()["entries"] == 3


def test_connector_mission_quality_evaluator_covers_every_domain(tmp_path) -> None:
    agent, _journal = _agent(tmp_path)
    for domain in AUTONOMOUS_DOMAINS:
        result = agent.run_connector_mission(
            mission=_mission(domain, mission_id=f"quality-{domain}"),
            approved=True,
            quality_evaluator=lambda context: {
                "evaluator_id": "all-domain-quality",
                "evaluator_version": "1",
                "reward": 1.0,
                "passed": True,
                "evidence_digest": None,
            },
        )
        assert result.status == "completed", domain
        assert result.step_executions[0].quality["domain"] == domain
        assert result.step_executions[0].quality["passed"] is True


def test_connector_mission_dependency_outputs_require_explicit_resume_rehydration(tmp_path) -> None:
    agent, journal = _agent(tmp_path)
    operation = AutonomousConnectorOperationRegistry().for_domain("coding")[0]
    arguments = {field: {"fixture": "coding"} for field in _RECOMMENDED_FIELDS[operation.operation_id]}
    mission = MissionRequest(
        mission_id="dependent-mission",
        goal="restart safe dependency chain",
        steps=(
            MissionStep("first", "coding", operation.capabilities[0], "first", "fixture", arguments=arguments),
            MissionStep("second", "coding", operation.capabilities[0], "second", "fixture", arguments=arguments, depends_on=("first",)),
        ),
    )

    first = agent.run_connector_mission(mission=mission, approved=True, max_step_calls=1)
    assert first.status == "paused"
    assert first.completed_step_ids == ("first",)

    missing = agent.run_connector_mission(
        mission=mission,
        checkpoint=first.checkpoint,
        approved=True,
    )
    assert missing.status == "reconciliation_required"
    assert missing.next_step_ids == ("second",)
    assert journal.verify_integrity()["entries"] == 1

    resumed = agent.run_connector_mission(
        mission=mission,
        checkpoint=first.checkpoint,
        approved=True,
        resume_outputs={"first": {"rehydrated": True}},
    )
    assert resumed.status == "completed"
    assert resumed.completed_step_ids == ("first", "second")
    assert journal.verify_integrity()["entries"] == 2


def test_connector_mission_approval_and_explicit_feedback_settlement(tmp_path) -> None:
    agent, _journal = _agent(tmp_path)
    mission = _mission("coding", mission_id="approval-feedback-mission")
    refused = agent.run_connector_mission(mission=mission, approved=False)
    assert refused.status == "approval_required"
    assert refused.step_executions[0].status == "approval_required"

    ledger = InMemoryAutonomousConnectorFeedbackLedger()
    result = agent.run_connector_mission(
        mission=_mission("coding", mission_id="approved-feedback-mission"),
        approved=True,
        feedback_ledger=ledger,
        feedback_by_step={
            "observe": {
                "feedback_id": "feedback-1",
                "evaluator_id": "fixture-evaluator",
                "evaluator_version": "1",
                "reward": 0.75,
                "passed": True,
                "source": "caller_evaluator",
                "evidence_digest": None,
            }
        },
    )
    assert result.status == "completed"
    assert result.feedback_receipts[0]["reward"] == 0.75
    signals = ledger.signals(domain="coding", capability=mission.steps[0].capability)
    assert signals["builtin.offline-evidence.coding"]["evaluator_reward"] == 0.75
    assert signals["builtin.offline-evidence.coding"]["success_rate"] == 1.0
    with pytest.raises(ArgumentError, match="caller_evaluator"):
        agent.run_connector_mission(
            mission=_mission("coding", mission_id="bad-feedback-mission"),
            approved=True,
            feedback_ledger=ledger,
            feedback_by_step={
                "observe": {
                    "feedback_id": "feedback-bad",
                    "evaluator_id": "fixture-evaluator",
                    "evaluator_version": "1",
                    "reward": 0.75,
                    "passed": True,
                    "source": "transport",
                }
            },
        )


def test_connector_mission_callback_flows_through_agent_facade(tmp_path) -> None:
    agent, _journal = _agent(tmp_path)
    events: list[dict[str, object]] = []
    result = agent.run_connector_mission(
        mission=_mission("coding", mission_id="callback-mission"),
        approved=True,
        trace_event_callback=lambda **event: events.append(event),
    )
    assert result.status == "completed"
    assert [event["phase"] for event in events] == ["connector_started", "connector_finished"]


def test_connector_mission_replay_requires_payload_rehydration(tmp_path) -> None:
    agent, journal = _agent(tmp_path)
    mission = _mission("coding", mission_id="replay-mission")
    first = agent.run_connector_mission(mission=mission, approved=True)
    assert first.status == "completed"

    empty_checkpoint = {
        **first.checkpoint,
        "steps": [],
        "completed_step_ids": [],
    }
    missing = agent.run_connector_mission(
        mission=mission,
        checkpoint=empty_checkpoint,
        approved=True,
    )
    assert missing.status == "reconciliation_required"
    assert missing.step_executions[0].status == "reconciliation_required"
    assert journal.verify_integrity()["entries"] == 1

    restored = agent.run_connector_mission(
        mission=mission,
        checkpoint=empty_checkpoint,
        approved=True,
        rehydrate_payload=lambda _receipt: {
            **first.step_executions[0].value,
        },
    )
    assert restored.status == "completed"
    assert restored.step_executions[0].status == "completed"
    assert journal.verify_integrity()["entries"] == 1
