from __future__ import annotations

import json
import hashlib
from typing import Any

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainEvaluatorDecision,
    BrainEpisodicMemory,
    BrainOutcomeEvaluator,
    BrainRunError,
    InMemoryAutonomousDecisionCycleStateStore,
    LLMRuntime,
    ModelCatalogue,
)


class _Workspace:
    def tool(self, name: str, arguments: dict[str, Any] | None = None) -> dict[str, Any]:
        args = {} if arguments is None else dict(arguments)
        if name == "brain_model_select_contextual":
            context = args.get("context")
            if not isinstance(context, dict):
                raise AssertionError("model selection context is missing")
            identity = {
                field: context.get(field)
                for field in ("domain", "capability", "risk_class", "task_family")
            }
            context_digest = hashlib.sha256(
                json.dumps(identity, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            ).hexdigest()
            return {
                "context_digest": context_digest,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "replan-model"},
                    "decision_digest": "d" * 64,
                    "ranking": [
                        {
                            "model_id": "offline/replan-model",
                            "eligible": True,
                            "reasons": [],
                            "base_score": 1.0,
                            "exploration_bonus": 0.0,
                            "score": 1.0,
                            "observed_pulls": 0,
                        }
                    ],
                },
            }
        if name == "brain_model_select":
            return {
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "replan-model"},
                    "decision_digest": "d" * 64,
                    "ranking": [],
                },
            }
        if name == "brain_prompt_assemble":
            return {
                "messages": [
                    {"role": "system", "content": str(args.get("system"))},
                    {"role": "user", "content": str(args.get("task"))},
                ],
                "prompt_digest": "a" * 64,
            }
        if name == "brain_plan":
            return {
                "ok": True,
                "plan": {
                    "requires_approval": True,
                    "steps": [{"effect": "provider_call"}],
                    "plan_digest": "b" * 64,
                },
            }
        if name == "brain_outcome_record":
            state = json.loads(json.dumps(args["bandit_state"]))
            arm_id = str(args["arm_id"])
            arms = list(state.get("arms", []))
            existing = next((arm for arm in arms if arm.get("arm_id") == arm_id), None)
            if existing is None:
                arms.append(
                    {
                        "arm_id": arm_id,
                        "pulls": 1,
                        "reward_sum": args["assessment"]["reward"],
                        "failures": int(args["assessment"]["failed"]),
                    }
                )
            else:
                existing["pulls"] += 1
                existing["reward_sum"] += args["assessment"]["reward"]
                existing["failures"] += int(args["assessment"]["failed"])
            state["arms"] = sorted(arms, key=lambda arm: arm["arm_id"])
            state["generation"] = int(state.get("generation", 0)) + 1
            return {"ok": True, "status": "recorded", "next_state": state}
        raise AssertionError(f"unexpected workspace tool: {name}")


def _candidate() -> dict[str, Any]:
    return {
        "provider": "offline",
        "model": "replan-model",
        "requires_credential": False,
        "capabilities": [
            "reasoning",
            "code",
            "science",
            "data",
            "web",
            "biomedical",
            "operations",
            "enterprise",
            "coordination",
            "multimodal",
            "evaluation",
        ],
        "context_window_tokens": 32_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 1,
        "cost_per_million_tokens": 0,
        "reliability": 0.99,
    }


def _agent(calls: list[str], memory: BrainEpisodicMemory | None = None) -> AutonomousAgent:
    runtime = LLMRuntime()

    def handle(request: Any) -> dict[str, str]:
        calls.append(request.model)
        return {"output_text": "private provider response", "id": f"offline-{len(calls)}"}

    runtime.register_in_memory_provider("offline", handle)
    return AutonomousAgent(
        _Workspace(),
        runtime,
        memory=memory,
        model_catalogue=ModelCatalogue([_candidate()]),
    )


def _evaluator(seen: list[dict[str, Any]], *, request_replan: bool = True) -> BrainOutcomeEvaluator:
    def assess(value: dict[str, Any]) -> BrainEvaluatorDecision:
        seen.append(dict(value))
        should_replan = request_replan and len(seen) == 1
        return BrainEvaluatorDecision(
            evaluator_id="cycle-quality",
            evaluator_version="1.0.0",
            reward=0.2 if should_replan else 0.95,
            passed=not should_replan,
            failed=should_replan,
            failure_class="insufficient_evidence" if should_replan else None,
            replan_requested=should_replan,
            replan_instruction=(
                "Narrow the reviewed scope and retry the bounded task."
                if should_replan
                else None
            ),
        )

    return BrainOutcomeEvaluator(
        assess,
        evaluator_id="cycle-quality",
        evaluator_version="1.0.0",
    )


def test_auto_replan_cycle_freezes_route_and_returns_secret_safe_learning_projection(tmp_path: Any) -> None:
    calls: list[str] = []
    memory = BrainEpisodicMemory(tmp_path / "cross-domain.sqlite3")
    agent = _agent(calls, memory)
    task = "write python code for the dataset pipeline"
    route = agent.route(
        task=task,
        min_confidence=0.20,
        min_margin=0.10,
        max_domains=3,
    )
    assert route.cross_domain is True
    seen: list[dict[str, Any]] = []

    try:
        result = agent.run_auto_replan_cycle(
            task=task,
            credentials={},
            model_candidates=[_candidate()],
            evaluator=_evaluator(seen),
            route_override=route,
            max_replans=1,
            min_confidence=0.20,
            min_margin=0.10,
            approve_provider_call=True,
        )

        assert result.status == "completed"
        assert result.mode == "cross_domain"
        assert result.route.route_digest == route.route_digest
        assert result.replan_count == 1
        assert len(result.attempt_results) == 2
        assert len(result.evaluations) == 6
        assert len(calls) == 6  # two specialist calls plus synthesis for each frozen attempt
        assert len(seen) == 6
        public = json.dumps(result.to_dict())
        assert "private provider response" not in public
        assert "Narrow the reviewed scope" not in public
        assert '"replan_instruction":' not in public
        assert "api_key" not in public.lower()
        assert result.to_dict()["route"]["route_digest"] == route.route_digest
    finally:
        memory.close()


def test_auto_replan_cycle_persists_and_rehydrates_without_reinvoking_provider(tmp_path: Any) -> None:
    calls: list[str] = []
    memory = BrainEpisodicMemory(tmp_path / "restart.sqlite3")
    agent = _agent(calls, memory)
    task = "debug this repository implementation"
    route = agent.route(task=task)
    store = InMemoryAutonomousDecisionCycleStateStore()
    try:
        first = agent.run_auto_replan_cycle(
            task=task,
            credentials={},
            model_candidates=[_candidate()],
            evaluator=_evaluator([], request_replan=False),
            route_override=route,
            max_replans=0,
            decision_cycle_id="auto-replan-restart",
            decision_cycle_store=store,
            approve_provider_call=True,
        )
        assert first.status == "completed"
        before_resume = len(calls)
        persisted = store.load("auto-replan-restart")
        assert persisted is not None
        assert persisted.phase == "terminal"
        assert persisted.route_digest == route.route_digest
        assert persisted.outcome_digest is not None

        resumed = agent.run_auto_replan_cycle(
            task=task,
            credentials={},
            model_candidates=[_candidate()],
            evaluator=_evaluator([], request_replan=True),
            route_override=route,
            max_replans=0,
            decision_cycle_id="auto-replan-restart",
            decision_cycle_store=store,
            resume_decision_cycle=True,
            decision_cycle_rehydrate_result=lambda _context: first.final,
            approve_provider_call=True,
        )
        assert resumed.status == "completed"
        assert resumed.to_dict() == first.to_dict()
        assert len(calls) == before_resume
    finally:
        memory.close()


def test_auto_replan_cycle_covers_every_builtin_domain_and_abstains_before_provider() -> None:
    calls: list[str] = []
    agent = _agent(calls)
    tasks = {
        "coding": "debug this Rust repository implementation",
        "browser": "compare browser research sources",
        "data": "validate this dataset schema",
        "science": "design a scientific experiment hypothesis",
        "biomedical": "review biomedical clinical evidence",
        "neuroscience": "analyze neuroscience neural signals",
        "operations": "plan an operations incident rollback",
        "enterprise": "review enterprise governance compliance",
        "multi_agent": "delegate a multi agent specialist subtask",
        "multimodal": "align multimodal image audio evidence",
        "cross_domain": "synthesize cross domain evidence",
        "evaluation": "run an evaluation benchmark holdout",
    }
    assert set(tasks) == set(AUTONOMOUS_DOMAINS)
    for domain, task in tasks.items():
        route = agent.route(task=task)
        assert route.abstained is False, domain
        assert domain in route.selected_domains, (domain, route.selected_domains)

    review = agent.run_auto_replan_cycle(
        task="please explain an entirely unclassified household question",
        credentials={},
        model_candidates=[_candidate()],
        evaluator=_evaluator([]),
        approve_provider_call=True,
    )
    assert review.status == "route_review_required"
    assert review.final is not None
    assert review.final.result is None
    assert calls == []

    semantic_review = agent.run_auto_replan_cycle(
        task="debug this repository implementation",
        credentials={},
        model_candidates=[_candidate()],
        evaluator=_evaluator([]),
        semantic_routing=True,
        approve_provider_call=False,
    )
    assert semantic_review.status == "approval_required"
    assert semantic_review.semantic_route is not None
    assert semantic_review.semantic_route.status == "approval_required"
    assert calls == []


def test_auto_replan_cycle_rejects_tampered_route_and_evaluator_payload(tmp_path: Any) -> None:
    calls: list[str] = []
    memory = BrainEpisodicMemory(tmp_path / "invalid.sqlite3")
    agent = _agent(calls, memory)
    route = agent.route(task="debug this repository implementation")
    with pytest.raises(BrainRunError, match="route_override task"):
        agent.run_auto_replan_cycle(
            task="validate this dataset schema",
            credentials={},
            model_candidates=[_candidate()],
            evaluator=_evaluator([]),
            route_override=route,
            approve_provider_call=True,
        )

    invalid = BrainOutcomeEvaluator(
        lambda _value: {
            "reward": 0.0,
            "passed": False,
            "failed": True,
            "replan_requested": True,
            "replan_instruction": "retry with api_key=must-never-cross",
        },
        evaluator_id="cycle-quality",
        evaluator_version="1.0.0",
    )
    try:
        with pytest.raises(BrainRunError, match="secret material"):
            agent.run_auto_replan_cycle(
                task="debug this repository implementation",
                credentials={},
                model_candidates=[_candidate()],
                evaluator=invalid,
                route_override=route,
                max_replans=1,
                approve_provider_call=True,
            )
        assert calls == ["replan-model"]
    finally:
        memory.close()


def test_auto_decision_cycle_selects_single_and_cross_domain_kernels_without_leaking_provider_values(tmp_path: Any) -> None:
    calls: list[str] = []
    memory = BrainEpisodicMemory(tmp_path / "auto-cycle.sqlite3")
    agent = _agent(calls, memory)

    single = agent.run_auto_cycle(
        task="debug this repository implementation",
        credentials={},
        model_candidates=[_candidate()],
        approve_provider_call=True,
    )
    assert single.status == "completed"
    assert single.mode == "single_domain"
    assert single.cycle is not None
    assert single.cycle.run is not None
    assert single.private_result is not None

    seen: list[dict[str, Any]] = []
    cross = agent.run_auto_cycle(
        task="write python code for the dataset pipeline",
        credentials={},
        model_candidates=[_candidate()],
        evaluator=_evaluator(seen, request_replan=False),
        approve_provider_call=True,
    )
    assert cross.status == "completed"
    assert cross.mode == "cross_domain"
    assert cross.cycle is not None
    assert cross.cycle.evaluation is not None
    assert len(seen) == 3
    public = json.dumps(cross.to_dict())
    assert "private provider response" not in public
    assert "api_key" not in public.lower()
    assert cross.to_dict()["next_action"] == "complete"
    memory.close()


def test_auto_decision_cycle_covers_every_builtin_domain_and_review_boundaries() -> None:
    calls: list[str] = []
    agent = _agent(calls)
    tasks = {
        "coding": "debug this repository implementation",
        "browser": "compare browser research sources",
        "data": "validate this dataset schema",
        "science": "design a scientific experiment hypothesis",
        "biomedical": "review biomedical clinical evidence",
        "neuroscience": "analyze neuroscience neural signals",
        "operations": "plan an operations incident rollback",
        "enterprise": "review enterprise governance compliance",
        "multi_agent": "delegate a multi agent specialist subtask",
        "multimodal": "align multimodal image audio evidence",
        "cross_domain": "synthesize cross domain evidence",
        "evaluation": "run an evaluation benchmark holdout",
    }
    assert set(tasks) == set(AUTONOMOUS_DOMAINS)
    for domain, task in tasks.items():
        result = agent.run_auto_cycle(
            task=task,
            domain=domain,
            credentials={},
            model_candidates=[_candidate()],
            approve_provider_call=True,
        )
        assert result.status == "completed", domain
        assert result.mode == "single_domain", (domain, result.mode)
        assert result.route.selected_domains == (domain,)

    before_review = len(calls)
    review = agent.run_auto_cycle(
        task="please explain an entirely unclassified household question",
        credentials={},
        model_candidates=[_candidate()],
        approve_provider_call=True,
    )
    assert review.status == "route_review_required"
    assert review.mode is None
    assert review.cycle is not None
    assert review.cycle.run is None
    assert review.to_dict()["next_action"] == "review_route"
    assert len(calls) == before_review

    semantic_review = agent.run_auto_cycle(
        task="debug this repository implementation",
        credentials={},
        model_candidates=[_candidate()],
        semantic_routing=True,
        approve_provider_call=False,
    )
    assert semantic_review.status == "approval_required"
    assert semantic_review.cycle is None
    assert semantic_review.semantic_route is not None
    assert len(calls) == before_review


def test_auto_decision_cycle_rehydrates_private_result_without_reinvoking_provider() -> None:
    calls: list[str] = []
    agent = _agent(calls)
    task = "debug this repository implementation"
    route = agent.route(task=task)
    store = InMemoryAutonomousDecisionCycleStateStore()

    first = agent.run_auto_cycle(
        task=task,
        route_override=route,
        credentials={},
        model_candidates=[_candidate()],
        decision_cycle_id="auto-cycle-restart",
        decision_cycle_store=store,
        approve_provider_call=True,
    )
    assert first.status == "completed"
    assert first.private_result is not None
    before_resume = len(calls)
    persisted = store.load("auto-cycle-restart")
    assert persisted is not None
    assert persisted.phase == "terminal"
    assert persisted.route_digest == route.route_digest

    resumed = agent.run_auto_cycle(
        task=task,
        route_override=route,
        credentials={},
        model_candidates=[_candidate()],
        decision_cycle_id="auto-cycle-restart",
        decision_cycle_store=store,
        resume_decision_cycle=True,
        decision_cycle_rehydrate_result=lambda _context: first.private_result,
        approve_provider_call=True,
    )
    assert resumed.to_dict() == first.to_dict()
    assert len(calls) == before_resume


def test_auto_decision_cycle_rejects_route_reuse_for_a_different_task() -> None:
    agent = _agent([])
    route = agent.route(task="debug this repository implementation")
    with pytest.raises(BrainRunError, match="route_override task"):
        agent.run_auto_cycle(
            task="validate this dataset schema",
            route_override=route,
            credentials={},
            model_candidates=[_candidate()],
            approve_provider_call=True,
        )
