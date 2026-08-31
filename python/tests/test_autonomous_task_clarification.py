from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainRunError,
    AutonomousTaskClarificationError,
    LLMRuntime,
    autonomous_domain_policy,
    autonomous_domain_task_lens,
    content_digest,
    infer_autonomous_task_decision,
    infer_autonomous_task_intent,
    plan_autonomous_task_clarification,
    resolve_autonomous_task_clarification,
    validate_autonomous_task_decision,
    validate_autonomous_task_clarification_plan,
    validate_autonomous_task_clarification_resolution,
)


def _artifacts(task: str = "analyze the dataset lineage", domain: str = "data"):
    lens = autonomous_domain_task_lens(domain)
    intent = infer_autonomous_task_intent(
        task=task,
        task_digest=content_digest({"task": task}),
        domain=domain,
        capability="data_analysis" if domain == "data" else "reasoning",
        risk_class="read_only",
        workflow_id="data_workflow" if domain == "data" else f"{domain}_workflow",
        lens=lens,
    )
    policy = autonomous_domain_policy(domain)
    decision = infer_autonomous_task_decision(
        intent=intent,
        lens=lens,
        policy=policy,
        required_model_capabilities=("reasoning", "structured_output"),
    )
    return intent, lens, policy, decision


def test_clarification_plan_is_cross_runtime_digest_bound() -> None:
    intent, lens, policy, decision = _artifacts()
    plan = plan_autonomous_task_clarification(
        intent=intent,
        lens=lens,
        policy=policy,
        decision=decision,
    )

    assert plan.plan_digest == "56d71e320c406c0266f5c25150c7b1107c7e766e75a75b56cef89df38d7392f6"
    assert plan.status == "required"
    assert [question.kind for question in plan.questions] == ["output", "evidence"]
    assert all(intent.task_digest not in question.prompt for question in plan.questions)
    public = plan.to_dict()
    assert "analyze the dataset lineage" not in json.dumps(public)
    assert public["authorization"] == "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions"
    assert validate_autonomous_task_clarification_plan(public).plan_digest == plan.plan_digest


def test_task_decision_replay_rejects_tampering_and_stale_artifacts() -> None:
    intent, lens, policy, decision = _artifacts()
    public = decision.to_dict()
    assert validate_autonomous_task_decision(public).decision_digest == decision.decision_digest
    replayed = validate_autonomous_task_decision(public, intent=intent, lens=lens, policy=policy)
    assert replayed.decision_digest == decision.decision_digest

    tampered = {**public, "next_actions": ["bypass_review"]}
    with pytest.raises(Exception, match="digest|metadata"):
        validate_autonomous_task_decision(tampered)
    with pytest.raises(Exception, match="does not match"):
        validate_autonomous_task_decision(public, intent=intent, lens=lens, policy=policy, required_model_capabilities=("reasoning",))
    with pytest.raises(Exception, match="together"):
        validate_autonomous_task_decision(public, intent=intent)


def test_clarification_answers_are_transient_and_require_complete_contracts() -> None:
    intent, lens, policy, decision = _artifacts()
    plan = plan_autonomous_task_clarification(intent=intent, lens=lens, policy=policy, decision=decision)
    output_id, evidence_id = (question.question_id for question in plan.questions)

    partial = resolve_autonomous_task_clarification(
        plan,
        task_digest=intent.task_digest,
        answers={output_id: "PRIVATE output answer"},
    )
    assert partial.status == "still_required"
    assert partial.answered_count == 1
    assert evidence_id in partial.unanswered_question_ids
    assert "PRIVATE output answer" not in json.dumps(partial.to_dict())

    resolved = resolve_autonomous_task_clarification(
        plan,
        task_digest=intent.task_digest,
        answers={output_id: "PRIVATE output answer", evidence_id: "caller catalogue"},
    )
    assert resolved.status == "resolved"
    assert resolved.required_answer_count == 2
    assert len(resolved.answer_digests) == 2
    restored = validate_autonomous_task_clarification_resolution(resolved.to_dict(), plan=plan.to_dict())
    assert restored.resolution_digest == resolved.resolution_digest

    tampered = plan.to_dict()
    tampered["plan_digest"] = "0" * 64
    with pytest.raises(AutonomousTaskClarificationError):
        validate_autonomous_task_clarification_plan(tampered)
    with pytest.raises(AutonomousTaskClarificationError):
        resolve_autonomous_task_clarification(plan, task_digest="0" * 64, answers={})
    with pytest.raises(AutonomousTaskClarificationError):
        resolve_autonomous_task_clarification(plan, task_digest=intent.task_digest, answers={"unknown": "x"})
    tampered_receipt = resolved.to_dict()
    tampered_receipt["resolution_digest"] = "0" * 64
    with pytest.raises(AutonomousTaskClarificationError):
        validate_autonomous_task_clarification_resolution(tampered_receipt, plan=plan)


def test_clarification_handles_all_domains_and_blocked_policy_without_bypass() -> None:
    for domain in AUTONOMOUS_DOMAINS:
        intent, lens, policy, decision = _artifacts(
            task=f"review the {domain} workflow and report verification gaps",
            domain=domain,
        )
        assert validate_autonomous_task_decision(
            decision.to_dict(), intent=intent, lens=lens, policy=policy
        ).decision_digest == decision.decision_digest
        plan = plan_autonomous_task_clarification(intent=intent, lens=lens, policy=policy, decision=decision)
        assert plan.domain == domain
        assert plan.review_dimensions
        assert len(plan.plan_digest) == 64

    intent, lens, policy, decision = _artifacts(
        task="deploy the biomedical report and verify safety",
        domain="biomedical",
    )
    blocked = plan_autonomous_task_clarification(intent=intent, lens=lens, policy=policy, decision=decision)
    assert blocked.status == "blocked"
    assert blocked.questions == ()
    assert "policy_blocker" in blocked.missing_contracts
    with pytest.raises(AutonomousTaskClarificationError):
        resolve_autonomous_task_clarification(blocked, task_digest=intent.task_digest, answers={"bypass": "yes"})


def test_agent_facade_uses_the_same_preflight_and_answer_receipt() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    task = "analyze the dataset lineage"
    plan = agent.clarification_plan(task=task, domain="data")
    blueprint = agent.prepare(task=task, domain="data")
    assert blueprint.task_intent is not None
    assert blueprint.task_lens is not None
    assert blueprint.domain_policy is not None
    assert blueprint.task_decision is not None
    assert plan.intent_digest == blueprint.task_intent.intent_digest
    assert agent.validate_task_lens(
        value=blueprint.task_lens.to_dict(),
        expected_domain="data",
    ).lens_digest == blueprint.task_lens.lens_digest
    assert agent.validate_task_intent(
        value=blueprint.task_intent.to_dict(),
        lens=blueprint.task_lens.to_dict(),
        expected_task_digest=blueprint.spec.task_digest,
    ).intent_digest == blueprint.task_intent.intent_digest
    assert agent.validate_task_decision(
        value=blueprint.task_decision.to_dict(),
        intent=blueprint.task_intent,
        lens=blueprint.task_lens,
        policy=blueprint.domain_policy,
    ).decision_digest == blueprint.task_decision.decision_digest
    assert len(plan.plan_digest) == 64
    answers = {question.question_id: "caller-owned boundary" for question in plan.questions}
    receipt = agent.resolve_clarification(plan=plan, task=task, answers=answers)
    assert receipt.status == "resolved"
    restored = agent.validate_clarification(plan=plan.to_dict(), receipt=receipt.to_dict())
    assert restored.resolution_digest == receipt.resolution_digest
    with pytest.raises(AutonomousTaskClarificationError):
        agent.validate_clarification(plan={**plan.to_dict(), "plan_digest": "0" * 64}, receipt=receipt.to_dict())


def test_recompile_clarification_rebuilds_a_fresh_blueprint_for_every_domain() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    for domain in AUTONOMOUS_DOMAINS:
        task = f"review the {domain} workflow"
        plan = agent.clarification_plan(task=task, domain=domain)
        answers = {
            question.question_id: question.options[0] if question.answer_kind == "choice" else "caller-owned clarified boundary"
            for question in plan.questions
        }
        receipt = agent.resolve_clarification(plan=plan, task=task, answers=answers)
        recompiled = agent.recompile_clarification(
            plan=plan.to_dict(),
            receipt=receipt.to_dict(),
            task=task,
            clarified_task=f"review the {domain} workflow and produce a bounded verification report",
            desired_outputs=("bounded verification report",),
        )
        assert recompiled.domain == domain
        assert recompiled.blueprint.spec.domain == domain
        assert recompiled.recompiled_task_digest == recompiled.blueprint.spec.task_digest
        public = json.dumps(recompiled.to_dict())
        assert task not in public
        assert "caller-owned clarified boundary" not in public
        assert recompiled.to_dict()["authorization"] == "recompile_only; provider_source_tool_and_effect_gates_remain_required"
        restored = agent.validate_clarification_recompile(
            value=recompiled.to_dict(),
            plan=plan.to_dict(),
            receipt=receipt.to_dict(),
        )
        assert restored["recompile_digest"] == recompiled.recompile_digest
        tampered = recompiled.to_dict()
        tampered["recompile_digest"] = "0" * 64
        with pytest.raises(AutonomousTaskClarificationError):
            agent.validate_clarification_recompile(value=tampered, plan=plan, receipt=receipt)

    task = "review the data workflow"
    plan = agent.clarification_plan(task=task, domain="data")
    partial = agent.resolve_clarification(plan=plan, task=task, answers={})
    with pytest.raises(BrainRunError):
        agent.recompile_clarification(
            plan=plan,
            receipt=partial,
            task=task,
            clarified_task="review the data workflow and produce a report",
        )
    with pytest.raises(BrainRunError):
        agent.recompile_clarification(
            plan=plan,
            receipt=agent.resolve_clarification(
                plan=plan,
                task=task,
                answers={question.question_id: "caller-owned boundary" for question in plan.questions},
            ),
            task="a different task",
            clarified_task="review the data workflow and produce a report",
        )
