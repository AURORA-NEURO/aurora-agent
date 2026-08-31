from __future__ import annotations

import hashlib
import json
from typing import Any, Mapping, Sequence

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousBrain,
    AutonomousMissionReplanCheckpoint,
    BrainEvaluatorDecision,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    InMemoryAutonomousMissionReplanStateStore,
    AutonomousMissionReplanPersistenceCoordinator,
    JsonAutonomousMissionReplanSnapshotPersistence,
    LLMRuntime,
    ModelCatalogue,
    ProviderHealthLedger,
    AutonomousPromptLearningPersistenceCoordinator,
    builtin_autonomous_prompt_registry,
    run_autonomous_mission_replan_cycle,
)


class _Workspace:
    def __init__(self) -> None:
        self.states: list[dict[str, Any]] = []

    def tool(self, name: str, arguments: dict[str, Any] | None = None) -> dict[str, Any]:
        args = {} if arguments is None else dict(arguments)
        if name != "brain_outcome_record":
            raise AssertionError(f"unexpected workspace tool: {name}")
        state = json.loads(json.dumps(args["bandit_state"]))
        arm_id = str(args["arm_id"])
        assessment = args["assessment"]
        arms = list(state.get("arms", []))
        arm = next((row for row in arms if row.get("arm_id") == arm_id), None)
        if arm is None:
            arms.append(
                {
                    "arm_id": arm_id,
                    "pulls": 1,
                    "reward_sum": assessment["reward"],
                    "failures": int(assessment["failed"]),
                }
            )
        else:
            arm["pulls"] += 1
            arm["reward_sum"] += assessment["reward"]
            arm["failures"] += int(assessment["failed"])
        state["arms"] = sorted(arms, key=lambda row: row["arm_id"])
        state["generation"] = int(state.get("generation", 0)) + 1
        self.states.append(state)
        return {"ok": True, "status": "recorded", "next_state": state}


class _TextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value


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


def _bandit_state() -> dict[str, Any]:
    return {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}


def _mission_result(
    index: int,
    *,
    dispatched: bool = False,
    domain: str = "coding",
    prompt: Mapping[str, Any] | None = None,
) -> BrainMissionResult:
    context = {
        "domain": domain,
        "capability": "implementation",
        "risk_class": "research",
        "task_family": None,
    }
    run = BrainRunResult(
        run_id=f"synthetic-run-{index}",
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "offline", "model": "replan-model"},
            "decision_digest": "d" * 64,
            "context_digest": hashlib.sha256(
                json.dumps(
                    context,
                    ensure_ascii=False,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            ).hexdigest(),
            "context": context,
        },
        prompt={"prompt_digest": "p" * 64} if prompt is None else dict(prompt),
        plan={"plan": {"plan_digest": "l" * 64}},
        response=None,
        outcome_digest=(f"{index:064x}")[-64:],
    )
    return BrainMissionResult(
        brain_run=run,
        status="mission_dispatched" if dispatched else "mission_approval_required",
        mission={
            "mission_id": f"synthetic-mission-{index}",
            "private_provider_text": "this must remain caller-owned",
        },
        preflight={"results": []},
        execution={"mission_status": "dispatched", "results": []} if dispatched else None,
        route={"route_digest": "r" * 64},
    )


def _evaluator(
    seen: list[Mapping[str, Any]],
    *,
    instruction: str = "Reduce uncertainty and retry the bounded mission.",
    request_replan_once: bool = True,
) -> BrainOutcomeEvaluator:
    def assess(value: Mapping[str, Any]) -> BrainEvaluatorDecision:
        seen.append(dict(value))
        retry = request_replan_once and len(seen) == 1
        return BrainEvaluatorDecision(
            evaluator_id="mission-quality",
            evaluator_version="1.0.0",
            reward=0.2 if retry else 0.95,
            passed=not retry,
            failed=retry,
            failure_class="insufficient_evidence" if retry else None,
            replan_requested=retry,
            replan_instruction=instruction if retry else None,
        )

    return BrainOutcomeEvaluator(
        assess,
        evaluator_id="mission-quality",
        evaluator_version="1.0.0",
    )


def _brain(workspace: _Workspace) -> AutonomousBrain:
    return AutonomousBrain(workspace, LLMRuntime())


def test_mission_replan_is_bounded_and_serializes_only_metadata(monkeypatch: pytest.MonkeyPatch) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)
    calls: list[Mapping[str, Any]] = []

    def run_adaptive_mission(**kwargs: Any) -> BrainMissionResult:
        calls.append(dict(kwargs["prompt"]))
        return _mission_result(len(calls))

    monkeypatch.setattr(brain, "run_adaptive_mission", run_adaptive_mission)
    seen: list[Mapping[str, Any]] = []
    result = run_autonomous_mission_replan_cycle(
        brain,
        task="debug the bounded mission",
        model_candidates=[_candidate()],
        prompt={"system": "system", "context": []},
        plan={"steps": []},
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator(seen),
        bandit_state=_bandit_state(),
        max_replans=1,
    )

    assert result.status == "completed"
    assert result.replan_count == 1
    assert len(result.attempts) == 2
    assert len(result.evaluations) == 2
    assert len(calls) == 2
    public = json.dumps(result.to_dict())
    assert "private_provider_text" not in public
    assert "Reduce uncertainty" not in public
    assert '"replan_instruction":' not in public
    assert "secret_material" in public
    assert len(seen) == 2
    assert "Reduce uncertainty" in json.dumps(calls[1])


def test_mission_replan_inserts_feedback_into_versioned_prompt_override(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)
    prompts: list[Mapping[str, Any]] = []

    def run_adaptive_mission(**kwargs: Any) -> BrainMissionResult:
        prompts.append(dict(kwargs["prompt"]))
        return _mission_result(len(prompts))

    monkeypatch.setattr(brain, "run_adaptive_mission", run_adaptive_mission)
    result = run_autonomous_mission_replan_cycle(
        brain,
        task="retry a versioned mission prompt",
        model_candidates=[_candidate()],
        prompt={
            "system": "system",
            "context": [],
            "_provider_messages_override": {
                "messages": [
                    {"role": "system", "content": "versioned system"},
                    {"role": "user", "content": "versioned task"},
                ],
                "metadata": {
                    "mode": "registry_selection",
                    "retention": "prompt_messages_transient;digest_only_projection",
                    "secret_material": "never_returned",
                },
            },
        },
        plan={"steps": []},
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator([]),
        bandit_state=_bandit_state(),
        max_replans=1,
    )

    assert result.status == "completed"
    assert len(prompts) == 2
    second_override = prompts[1]["_provider_messages_override"]
    assert isinstance(second_override, Mapping)
    second_messages = second_override["messages"]
    assert isinstance(second_messages, Sequence)
    assert any(
        isinstance(message, Mapping)
        and "autonomy-mission-replan-2" in str(message.get("content", ""))
        for message in second_messages
    )


def test_mission_replan_checkpoint_resumes_handoff_without_replaying_attempt(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)
    calls: list[Mapping[str, Any]] = []

    def run_adaptive_mission(**kwargs: Any) -> BrainMissionResult:
        calls.append(dict(kwargs["prompt"]))
        return _mission_result(len(calls))

    monkeypatch.setattr(brain, "run_adaptive_mission", run_adaptive_mission)
    seen_first: list[Mapping[str, Any]] = []
    store = InMemoryAutonomousMissionReplanStateStore()
    instruction = "Reduce uncertainty and retry the bounded mission."
    versioned_prompt = {
        "system": "system",
        "context": [],
        "_provider_messages_override": {
            "messages": [
                {"role": "system", "content": "versioned system"},
                {"role": "user", "content": "versioned task"},
            ],
            "metadata": {
                "mode": "registry_selection",
                "retention": "prompt_messages_transient;digest_only_projection",
                "secret_material": "never_returned",
            },
        },
    }

    def interrupt(checkpoint: AutonomousMissionReplanCheckpoint) -> None:
        if checkpoint.phase == "replan_scheduled":
            raise RuntimeError("simulated process stop after checkpoint flush")

    with pytest.raises(RuntimeError, match="simulated process stop"):
        run_autonomous_mission_replan_cycle(
            brain,
            task="restart the bounded mission",
            model_candidates=[_candidate()],
            prompt=versioned_prompt,
            plan={"steps": []},
            credentials={},
            mission_policy={"allowed_tools": ["read_only"]},
            evaluator=_evaluator(seen_first, instruction=instruction),
            bandit_state=_bandit_state(),
            max_replans=1,
            root_mission_id="restartable-mission",
            state_store=store,
            checkpoint_sink=interrupt,
        )
    assert len(calls) == 1
    persisted = store.load("restartable-mission")
    assert persisted is not None
    assert persisted.phase == "replan_handoff"
    assert persisted.replan_instruction_digest is not None

    seen_resume: list[Mapping[str, Any]] = []
    resumed = run_autonomous_mission_replan_cycle(
        brain,
        task="restart the bounded mission",
        model_candidates=[_candidate()],
        prompt=versioned_prompt,
        plan={"steps": []},
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator(seen_resume, request_replan_once=False),
        bandit_state=workspace.states[-1],
        max_replans=1,
        root_mission_id="restartable-mission",
        state_store=store,
        resume=True,
        rehydrate_instruction=lambda _context: instruction,
    )

    assert resumed.status == "completed"
    assert resumed.replan_count == 1
    assert len(calls) == 2
    assert len(seen_first) == 1
    assert len(seen_resume) == 1
    assert "Reduce uncertainty" in json.dumps(calls[1])
    resumed_override = calls[1]["_provider_messages_override"]
    assert isinstance(resumed_override, Mapping)
    resumed_messages = resumed_override["messages"]
    assert isinstance(resumed_messages, Sequence)
    assert any(
        isinstance(message, Mapping)
        and "autonomy-mission-replan-2" in str(message.get("content", ""))
        for message in resumed_messages
    )
    terminal = store.load("restartable-mission")
    assert terminal is not None
    assert terminal.phase == "terminal"
    assert terminal.generation >= 5


def test_mission_replan_rejects_tampering_and_post_dispatch_retry(monkeypatch: pytest.MonkeyPatch) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)
    calls = 0

    def run_adaptive_mission(**_kwargs: Any) -> BrainMissionResult:
        nonlocal calls
        calls += 1
        return _mission_result(calls, dispatched=True)

    monkeypatch.setattr(brain, "run_adaptive_mission", run_adaptive_mission)
    seen: list[Mapping[str, Any]] = []
    result = run_autonomous_mission_replan_cycle(
        brain,
        task="do not replay dispatched work",
        model_candidates=[_candidate()],
        prompt={"context": []},
        plan={"steps": []},
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator(seen),
        bandit_state=_bandit_state(),
        max_replans=1,
    )
    assert result.status == "replan_blocked_after_dispatch"
    assert result.replan_count == 0
    assert calls == 1

    with pytest.raises(BrainRunError, match="evaluator callback failed"):
        run_autonomous_mission_replan_cycle(
            brain,
            task="reject unsafe retry feedback",
            model_candidates=[_candidate()],
            prompt={"context": []},
            plan={"steps": []},
            credentials={},
            mission_policy={"allowed_tools": ["read_only"]},
            evaluator=_evaluator(
                [],
                instruction="retry with api_key=must-never-cross",
            ),
            bandit_state=_bandit_state(),
            max_replans=1,
        )


def test_mission_replan_snapshot_persistence_is_canonical_and_digest_bound(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)

    monkeypatch.setattr(
        brain,
        "run_adaptive_mission",
        lambda **_kwargs: _mission_result(1),
    )
    store = InMemoryAutonomousMissionReplanStateStore()
    result = run_autonomous_mission_replan_cycle(
        brain,
        task="persist the mission cursor",
        model_candidates=[_candidate()],
        prompt={"context": []},
        plan={"steps": []},
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator([], request_replan_once=False),
        bandit_state=_bandit_state(),
        max_replans=0,
        root_mission_id="persistent-mission",
        state_store=store,
    )
    assert result.checkpoint is not None
    text_store = _TextStore()
    persistence = JsonAutonomousMissionReplanSnapshotPersistence(text_store)
    coordinator = AutonomousMissionReplanPersistenceCoordinator(store, persistence)
    snapshot = coordinator.flush()
    assert text_store.value is not None
    assert "persist the mission cursor" not in text_store.value
    assert "private_provider_text" not in text_store.value

    restored_store = InMemoryAutonomousMissionReplanStateStore()
    restored_coordinator = AutonomousMissionReplanPersistenceCoordinator(
        restored_store,
        JsonAutonomousMissionReplanSnapshotPersistence(text_store),
    )
    restored = restored_coordinator.restore()
    assert restored is not None
    assert restored.snapshot_digest == snapshot.snapshot_digest
    restored_state = restored_store.load("persistent-mission")
    assert restored_state is not None
    assert restored_state.phase == "terminal"

    text_store.value = text_store.value.replace("\"snapshot_digest\":", "\"tampered\":", 1)
    with pytest.raises(BrainRunError, match="unsupported or missing|digest|canonical"):
        persistence.read()


def test_agent_facade_prepares_and_runs_every_builtin_domain(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = _Workspace()
    agent = AutonomousAgent(
        workspace,
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_candidate()]),
    )
    calls: list[str] = []

    def run_adaptive_mission(**_kwargs: Any) -> BrainMissionResult:
        calls.append("run")
        return _mission_result(len(calls))

    monkeypatch.setattr(agent.brain, "run_adaptive_mission", run_adaptive_mission)
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
        seen: list[Mapping[str, Any]] = []
        result = agent.run_mission_replan_cycle(
            task=task,
            domain=domain,
            credentials={},
            mission_policy={"allowed_tools": ["read_only"]},
            evaluator=_evaluator(seen, request_replan_once=False),
            model_candidates=[_candidate()],
            max_replans=0,
        )
        assert result.status == "completed"
        assert len(result.attempts) == 1
        assert len(seen) == 1
    assert len(calls) == len(AUTONOMOUS_DOMAINS)


def test_agent_mission_replan_settles_model_quality_for_every_live_attempt(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Any,
) -> None:
    workspace = _Workspace()
    health = ProviderHealthLedger(tmp_path / "mission-quality.jsonl")
    agent = AutonomousAgent(
        workspace,
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_candidate()]),
        health_ledger=health,
    )
    calls = 0

    def run_adaptive_mission(**_kwargs: Any) -> BrainMissionResult:
        nonlocal calls
        calls += 1
        return _mission_result(calls)

    monkeypatch.setattr(agent.brain, "run_adaptive_mission", run_adaptive_mission)
    seen: list[Mapping[str, Any]] = []
    result = agent.run_mission_replan_cycle(
        task="adapt model choice from held-out mission quality",
        domain="coding",
        credentials={},
        mission_policy={"allowed_tools": ["read_only"]},
        evaluator=_evaluator(seen),
        model_candidates=[_candidate()],
        max_replans=1,
    )

    assert result.status == "completed"
    assert calls == 2
    assert all(row["model_quality"]["status"] == "recorded" for row in result.evaluations)
    model_health = health.model_health_snapshot()
    assert model_health["offline/replan-model"]["quality_observations"] == 2
    assert model_health["offline/replan-model"]["quality_mean"] == pytest.approx(0.575)
    public = json.dumps(result.to_dict())
    assert "adapt model choice" not in public
    assert "private_provider_text" not in public


def test_agent_mission_replan_uses_adaptive_prompt_learning_for_every_domain(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    registry = builtin_autonomous_prompt_registry()
    coordinator = AutonomousPromptLearningPersistenceCoordinator(registry)
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_candidate()]),
        prompt_learning_coordinator=coordinator,
    )

    for index, domain in enumerate(AUTONOMOUS_DOMAINS, start=1):
        def run_adaptive_mission(**kwargs: Any) -> BrainMissionResult:
            prompt = kwargs["prompt"]
            override = prompt.get("_provider_messages_override")
            assert isinstance(override, Mapping)
            metadata = override.get("metadata")
            assert isinstance(metadata, Mapping)
            return _mission_result(
                index,
                domain=domain,
                prompt={
                    "prompt_digest": f"{index:064x}",
                    "autonomous_prompt": dict(metadata),
                },
            )

        monkeypatch.setattr(agent.brain, "run_adaptive_mission", run_adaptive_mission)
        result = agent.run_mission_replan_cycle(
            task=f"adapt the bounded {domain} mission prompt",
            domain=domain,
            credentials={},
            mission_policy={"allowed_tools": ["read_only"]},
            evaluator=_evaluator([], request_replan_once=False),
            model_candidates=[_candidate()],
            max_replans=0,
        )

        selections = agent.prompt_learning_selections(result)
        assert len(selections) == 1, domain
        assert selections[0].plan.rows[0].domain == domain
        quality = result.evaluations[0]["model_quality"]
        assert quality["prompt_learning"]["selection_count"] == 1
        settled = agent.settle_prompt_learning(
            selections[0],
            arm_id=selections[0].arm_ids[0],
            evaluator_id=f"{domain}-prompt-quality",
            evaluator_version="1",
            reward=0.8,
            passed=True,
            outcome_digest=result.attempts[0].outcome_digest,
        )
        assert settled.status == "settled"
        public = json.dumps(result.to_dict())
        assert "adapt the bounded" not in public
        assert '"messages":' not in public


def test_mission_replan_rejects_payload_shaped_model_quality_projection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    workspace = _Workspace()
    brain = _brain(workspace)

    monkeypatch.setattr(brain, "run_adaptive_mission", lambda **_kwargs: _mission_result(1))
    with pytest.raises(BrainRunError, match="unsupported fields"):
        run_autonomous_mission_replan_cycle(
            brain,
            task="reject payload-shaped quality",
            model_candidates=[_candidate()],
            prompt={"system": "system", "context": []},
            plan={"steps": []},
            credentials={},
            mission_policy={"allowed_tools": ["read_only"]},
            evaluator=_evaluator([]),
            bandit_state=_bandit_state(),
            model_quality_callback=lambda _result, _decision: {
                "private_provider_text": "must never cross the checkpoint boundary",
            },
            max_replans=0,
        )
