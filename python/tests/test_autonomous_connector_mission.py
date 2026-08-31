import json
from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousOrderedStepPlanRefinementResult,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorOperationRegistry,
    InMemoryAutonomousConnectorFeedbackLedger,
    LLMRuntime,
    MissionRequest,
    MissionStep,
    apply_autonomous_ordered_step_plan,
    connector_mission_planner_steps,
    connector_mission_protected_contract_digest,
)
from prism_sdk.authoring import content_digest
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


def _two_step_mission(*, mission_id: str = "planned-connector-mission") -> MissionRequest:
    operation = AutonomousConnectorOperationRegistry().for_domain("coding")[0]
    arguments = {
        field: {"fixture": f"planned-{field}"}
        for field in _RECOMMENDED_FIELDS[operation.operation_id]
    }
    return MissionRequest(
        mission_id=mission_id,
        goal="provider planned connector evidence",
        steps=(
            MissionStep("first", "coding", operation.capabilities[0], "first observation", "fixture", arguments=arguments),
            MissionStep("second", "coding", operation.capabilities[0], "second observation", "fixture", arguments=arguments),
        ),
    )


def _ordered_plan(mission: MissionRequest, *, review_required: bool = False) -> AutonomousOrderedStepPlanRefinementResult:
    graph = connector_mission_planner_steps(mission.steps)
    return AutonomousOrderedStepPlanRefinementResult(
        status="completed",
        task_digest=content_digest({"task": mission.goal}),
        base_plan_digest=content_digest({"steps": list(graph)}),
        protected_contract_digest=connector_mission_protected_contract_digest(mission),
        priority_step_ids=("second", "first"),
        focus_step_ids=("second",),
        review_required=review_required,
        confidence=0.91,
        selected_model={"provider": "fixture", "model": "planner"},
        selection_digest="a" * 64,
        planner_prompt_digest="b" * 64,
        planner_plan_digest="c" * 64,
        outcome_digest="d" * 64,
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


def test_provider_planned_connector_mission_accepts_only_the_verified_order(tmp_path, monkeypatch) -> None:
    agent, journal = _agent(tmp_path)
    mission = _two_step_mission()
    refinement = _ordered_plan(mission)
    planner_calls: list[dict[str, object]] = []

    def planner(**kwargs):
        planner_calls.append(kwargs)
        assert "planned-first" not in json.dumps(kwargs["steps"])
        return refinement

    monkeypatch.setattr(agent, "plan_ordered_steps_with_provider", planner)
    result = agent.run_connector_mission_with_provider_planning(
        mission=mission,
        credentials={},
        model_candidates=[],
        provider_planning_options={},
        accept_plan=True,
        execution_options={"approved": True},
    )
    assert result.status == "completed"
    assert result.execution is not None
    assert [item.step_id for item in result.execution.step_executions] == ["second", "first"]
    assert result.plan_refinement_digest == content_digest(refinement.to_dict())
    assert len(planner_calls) == 1
    serialized = json.dumps(result.to_dict())
    assert "planned-first" not in serialized
    assert journal.verify_integrity()["entries"] == 2


def test_provider_planned_connector_mission_stops_at_review_and_supports_caller_replay(tmp_path, monkeypatch) -> None:
    agent, journal = _agent(tmp_path)
    mission = _two_step_mission(mission_id="review-planned-mission")
    refinement = _ordered_plan(mission, review_required=True)
    calls = 0

    def planner(**_kwargs):
        nonlocal calls
        calls += 1
        return refinement

    monkeypatch.setattr(agent, "plan_ordered_steps_with_provider", planner)
    review = agent.run_connector_mission_with_provider_planning(
        mission=mission,
        credentials={},
        provider_planning_options={},
        accept_plan=True,
        execution_options={"approved": True},
    )
    assert review.status == "planning_review_required"
    assert review.execution is None
    assert calls == 1
    assert journal.verify_integrity()["entries"] == 0

    accepted = replace(refinement, review_required=False)
    replay = agent.run_connector_mission_with_provider_planning(
        mission=mission,
        credentials={},
        accepted_plan_refinement=accepted,
        accept_plan=True,
        execution_options={"approved": True},
    )
    assert replay.status == "completed"
    assert replay.execution is not None
    assert [item.step_id for item in replay.execution.step_executions] == ["second", "first"]
    assert calls == 1


def test_accepted_connector_plan_cannot_change_protected_contract_or_dependencies(tmp_path) -> None:
    _agent(tmp_path)
    mission = _two_step_mission(mission_id="contract-planned-mission")
    refinement = _ordered_plan(mission)
    with pytest.raises(ArgumentError, match="protected contract"):
        apply_autonomous_ordered_step_plan(
            mission,
            replace(refinement, protected_contract_digest="e" * 64),
            protected_contract_digest=connector_mission_protected_contract_digest(mission),
        )

    dependent = MissionRequest(
        mission_id="dependency-planned-mission",
        goal=mission.goal,
        steps=(
            mission.steps[0],
            replace(mission.steps[1], depends_on=("first",)),
        ),
    )
    invalid_order = replace(
        _ordered_plan(dependent),
        priority_step_ids=("second", "first"),
    )
    with pytest.raises(ArgumentError, match="dependencies"):
        apply_autonomous_ordered_step_plan(dependent, invalid_order)
