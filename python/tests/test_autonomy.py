from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import threading

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousBrain,
    AutonomousDomainRegistry,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousWorkflowRegistry,
    BrainRunError,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    BrainApprovalRouter,
    BrainJobStore,
    BrainWorker,
    CredentialStore,
    LLMRuntime,
    ModelCandidate,
    ModelCatalogue,
    ProviderHealthLedger,
    ProviderError,
    ProviderToolResult,
    openai_provider,
)


class _ProviderHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib handler protocol
        length = int(self.headers.get("Content-Length", "0"))
        self.server.request_body = self.rfile.read(length)  # type: ignore[attr-defined]
        request = json.loads(self.server.request_body.decode("utf-8"))  # type: ignore[attr-defined]
        has_tools = bool(request.get("tools"))
        has_tool_result = any(
            isinstance(item, dict) and item.get("type") == "function_call_output"
            for item in request.get("input", [])
        )
        if has_tools and not has_tool_result:
            response = {
                "id": "autonomy-tool-call",
                "model": "test-model",
                "output": [
                    {
                        "type": "function_call",
                        "call_id": "autonomy-call-1",
                        "name": "developer_platform_status",
                        "arguments": '{"scope":"workspace"}',
                    }
                ],
                "usage": {"total_tokens": 6},
            }
        elif has_tools:
            response = {
                "id": "autonomy-tool-complete",
                "model": "test-model",
                "output_text": "continued bounded answer",
                "usage": {"total_tokens": 9},
            }
        else:
            response = {
                "id": "autonomy-response",
                "model": "test-model",
                "output_text": "bounded answer",
                "usage": {"total_tokens": 4},
            }
        payload = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args: object) -> None:
        return


class _StructuredWorkflowProviderHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib handler protocol
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length).decode("utf-8"))
        stage_id = "unknown"
        for item in request.get("input", []):
            content = item.get("content") if isinstance(item, dict) else None
            if isinstance(content, str) and "Execute workflow stage " in content:
                stage_id = content.split("Execute workflow stage ", 1)[1].split(":", 1)[0]
                break
        blocked = getattr(self.server, "block_stage", None) == stage_id
        large_checkpoint = getattr(self.server, "large_checkpoint", False)
        evidence = (
            [f"evidence for {stage_id} {index} " + ("x" * 480) for index in range(32)]
            if large_checkpoint and not blocked
            else [] if blocked else [f"evidence for {stage_id}"]
        )
        response = {
            "id": f"workflow-{stage_id}",
            "model": "test-model",
            "output_text": json.dumps(
                {
                    "stage_id": stage_id,
                    "status": "blocked" if blocked else "completed",
                    "evidence": evidence,
                    "uncertainty": [],
                    "notes": "x" * 16_000 if large_checkpoint and not blocked else "bounded stage result",
                    "next_actions": [],
                }
            ),
            "usage": {"total_tokens": 8},
        }
        payload = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args: object) -> None:
        return


class _Workspace:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        self.calls.append((name, args))
        if name == "brain_model_select_contextual":
            return {
                "context_digest": "c" * 64,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "openai", "model": "test-model"},
                    "decision_digest": "d" * 64,
                },
            }
        if name == "brain_model_select":
            return {
                "selected_model": {"provider": "openai", "model": "test-model"},
                "decision_digest": "d" * 64,
            }
        if name == "brain_prompt_assemble":
            assert isinstance(args.get("context"), list)
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
        if name == "capability_route":
            return {
                "ok": True,
                "workflow": "capability_route",
                "route_id": "route-autonomy-test",
                "catalog_digest": "k" * 64,
                "goal": args.get("goal"),
                "needs": [
                    {
                        "id": "workspace-status",
                        "resolution": "resolved",
                        "candidate_groups": ["operations"],
                        "candidate_domains": ["operations"],
                        "candidate_tools": ["developer_platform_status"],
                    }
                ],
                "recommended_tools": ["developer_platform_status"],
                "tool_schemas": [
                    {
                        "name": "developer_platform_status",
                        "description": "Read bounded workspace status.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"scope": {"type": "string"}},
                            "required": ["scope"],
                        },
                    }
                ],
                "unresolved_needs": [],
                "schema_attachment": {"status": "attached"},
                "route_coverage": {"resolved": 1, "unresolved": 0},
                "execution": "not_started",
                "evidence_digest": "r" * 64,
            }
        if name == "brain_outcome_record":
            return {
                "ok": True,
                "status": "recorded",
                "next_state": {
                    "schema": "bioprism-brain-bandit/0.1",
                    "generation": 1,
                    "arms": [
                        {
                            "arm_id": "openai/test-model",
                            "pulls": 1,
                            "reward_sum": 0.8,
                            "failures": 0,
                            "disabled": False,
                        }
                    ],
                },
                "learning_evidence": {"evidence_digest": "e" * 64},
            }
        raise AssertionError(f"unexpected tool {name}")


def _model() -> list[dict[str, object]]:
    return [
        {
            "provider": "openai",
            "model": "test-model",
            "capabilities": [
                "reasoning", "code", "science", "data", "web", "biomedical", "operations",
                "enterprise", "coordination", "multimodal", "evaluation",
            ],
            "context_window_tokens": 16_000,
            "max_output_tokens": 2_048,
            "quality": 0.9,
            "latency_ms": 20,
            "cost_per_million_tokens": 10,
            "reliability": 0.95,
        }
    ]


def _runtime() -> tuple[LLMRuntime, CredentialStore, HTTPServer, threading.Thread]:
    server = HTTPServer(("127.0.0.1", 0), _ProviderHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    store = CredentialStore()
    runtime = LLMRuntime(store)
    runtime.register_provider(
        openai_provider(
            base_url=f"http://127.0.0.1:{server.server_port}",
            allow_insecure_http=True,
            timeout_seconds=2.0,
            max_attempts=1,
        )
    )
    return runtime, store, server, thread


def _structured_runtime() -> tuple[LLMRuntime, CredentialStore, HTTPServer, threading.Thread]:
    server = HTTPServer(("127.0.0.1", 0), _StructuredWorkflowProviderHandler)
    server.block_stage = None  # type: ignore[attr-defined]
    server.large_checkpoint = False  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    store = CredentialStore()
    runtime = LLMRuntime(store)
    runtime.register_provider(
        openai_provider(
            base_url=f"http://127.0.0.1:{server.server_port}",
            allow_insecure_http=True,
            timeout_seconds=2.0,
            max_attempts=1,
        )
    )
    return runtime, store, server, thread


def test_model_catalogue_and_agent_facade_connect_readiness_session_and_execution():
    unconfigured = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([_model()[0]]),
    ).readiness()
    assert unconfigured["providers"][0]["next_action"] == "register_provider"
    assert AutonomousAgent(_Workspace(), LLMRuntime()).learning_state() == {
        "schema": "bioprism-brain-bandit/0.1",
        "generation": 0,
        "arms": [],
    }

    runtime, _store, server, thread = _runtime()
    try:
        catalogue = ModelCatalogue([_model()[0]])
        agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=catalogue)
        assert agent.models() == catalogue.candidates()
        before = agent.readiness()
        assert before["models"][0]["eligible_for_selection"] is False
        assert before["providers"][0]["next_action"] == "collect_user_credential"

        with agent.onboarding.start_session(session_id="test-session") as session:
            session.register_value("openai", "test-secret")
            assert session.handles()["openai"].provider == "openai"
            ready = agent.readiness()
            assert ready["models"][0]["eligible_for_selection"] is True
            assert "test-secret" not in json.dumps(ready)
            result = agent.run(
                task="produce a bounded implementation review",
                domain="coding",
                credentials=session,
                approve_provider_call=True,
            )
            assert result.status == "completed_provider_call"
            assert result.response.text == "bounded answer"
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_model_candidate_catalogue_is_typed_and_deterministic():
    second = dict(_model()[0])
    second["model"] = "second-model"
    catalogue = ModelCatalogue([second, _model()[0]])
    assert [item["model"] for item in catalogue.candidates()] == ["second-model", "test-model"]
    replacement = ModelCandidate.from_mapping({**_model()[0], "quality": 0.95})
    catalogue.register(replacement, replace_existing=True)
    assert catalogue.get("openai", "test-model").quality == 0.95  # type: ignore[union-attr]
    with pytest.raises(ProviderError):
        ModelCandidate.from_mapping({**_model()[0], "api_key": "must-not-enter-catalogue"})


def test_autonomous_agent_replays_provider_health_into_selection_and_readiness(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    try:
        handle = store.register("openai", "health-agent-secret")
        health_ledger = ProviderHealthLedger(tmp_path / "provider-health.jsonl")
        health_ledger.record(
            {
                "schema": "bioprism-llm-provider-observation/0.1",
                "provider": "openai",
                "model": "test-model",
                "status": "provider_refused",
                "outcome": "failure",
                "latency_ms": 99,
                "observed_at": 100,
                "failure_class": "circuit_open",
                "circuit": "open",
                "consecutive_failures": 3,
                "opened_until": 9_000_000_000,
            }
        )
        agent = AutonomousAgent(
            _Workspace(),
            runtime,
            model_catalogue=ModelCatalogue(_model()),
            health_ledger=health_ledger,
        )
        readiness = agent.readiness()
        assert readiness["provider_health"]["openai"]["circuit"] == "open"
        assert readiness["providers"][0]["health"]["consecutive_failures"] == 3
        captured: dict[str, object] = {}

        def capture(**kwargs: object) -> str:
            captured.update(kwargs)
            return "captured"

        agent.orchestrator.run = capture  # type: ignore[method-assign]
        assert agent.run(
            task="inspect a bounded implementation",
            domain="coding",
            credentials={"openai": handle},
        ) == "captured"
        overrides = captured["selection_overrides"]
        assert isinstance(overrides, dict)
        assert overrides["provider_health"]["openai"]["circuit"] == "open"
        assert "health-agent-secret" not in json.dumps(readiness)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_autonomous_agent_workflow_and_cross_domain_wrappers_share_catalogue_and_state():
    runtime, store, server, thread = _runtime()
    try:
        handle = store.register("openai", "wrapper-secret")
        agent = AutonomousAgent(
            _Workspace(),
            runtime,
            model_catalogue=ModelCatalogue(_model()),
        )
        blueprint = agent.prepare(
            task="execute a bounded staged coding review",
            domain="coding",
        )
        captured: dict[str, object] = {}

        def capture_workflow(**kwargs: object) -> str:
            captured.update(kwargs)
            return "workflow-captured"

        agent.orchestrator.run_workflow = capture_workflow  # type: ignore[method-assign]
        assert agent.run_workflow(
            blueprint=blueprint,
            credentials={"openai": handle},
            approve_provider_call=True,
        ) == "workflow-captured"
        assert captured["model_candidates"] == agent.models()
        assert captured["credentials"] == {"openai": handle}
        assert captured["bandit_state"] == {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": 0,
            "arms": [],
        }

        captured.clear()

        def capture_cross_domain(**kwargs: object) -> str:
            captured.update(kwargs)
            return "cross-domain-captured"

        agent.orchestrator.run_cross_domain = capture_cross_domain  # type: ignore[method-assign]
        assert agent.run_cross_domain(
            task="reconcile a bounded coding and science review",
            subtasks=(
                {"id": "code", "domain": "coding", "task": "review implementation"},
                {"id": "science", "domain": "science", "task": "review evidence"},
            ),
            credentials={"openai": handle},
        ) == "cross-domain-captured"
        assert captured["model_candidates"] == agent.models()
        assert captured["bandit_state"] == {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": 0,
            "arms": [],
        }

        captured.clear()

        def capture_workflow_learning(**kwargs: object) -> str:
            captured.update(kwargs)
            return "workflow-learning-captured"

        agent.orchestrator.run_workflow_learning = capture_workflow_learning  # type: ignore[method-assign]
        assert agent.run_workflow_learning(
            blueprint=blueprint,
            credentials={"openai": handle},
            max_stage_calls=1,
        ) == "workflow-learning-captured"
        assert captured["bandit_state"] == {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": 0,
            "arms": [],
        }
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_autonomous_agent_composes_domain_tools_into_native_tool_loop():
    runtime, store, server, thread = _runtime()

    class ToolWorkspace(_Workspace):
        def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
            if name == "developer_platform_status":
                self.calls.append((name, {} if arguments is None else dict(arguments)))
                return {"status": "ready", "scope": (arguments or {}).get("scope")}
            return super().tool(name, arguments)

    workspace = ToolWorkspace()
    registry = AutonomousDomainToolRegistry(
        [
            AutonomousDomainTool(
                name="developer_platform_status",
                domains=("operations", "cross_domain"),
                capability="observability",
                description="Read bounded workspace status.",
                parameters={
                    "type": "object",
                    "properties": {"scope": {"type": "string"}},
                    "required": ["scope"],
                    "additionalProperties": False,
                },
            )
        ]
    )
    handle = store.register("openai", "agent-domain-tool-secret")
    agent = AutonomousAgent(
        workspace,
        runtime,
        model_catalogue=ModelCatalogue(_model()),
        tool_registry=registry,
    )
    try:
        result = agent.run(
            task="inspect the workspace status",
            domain="operations",
            credentials={"openai": handle},
            execution_mode="tool_loop",
            approve_provider_call=True,
            tool_loop_options={"max_turns": 3},
        )
        assert result.status == "completed_provider_tool_loop"
        assert any(name == "developer_platform_status" for name, _ in workspace.calls)
        assert agent.tools("operations")[0]["risk_class"] == "read_only"
        assert agent.tool_receipts()[0]["status"] == "executed"
        assert "workspace" not in json.dumps(agent.tool_receipts())
        assert "agent-domain-tool-secret" not in json.dumps(result.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_builtin_domain_registry_covers_every_autonomous_domain_and_blueprint_redacts_task():
    registry = AutonomousDomainRegistry.with_builtin_profiles()
    assert {entry["domain"] for entry in registry.catalogue()} == {
        "coding",
        "browser",
        "data",
        "science",
        "biomedical",
        "neuroscience",
        "operations",
        "enterprise",
        "multi_agent",
        "multimodal",
        "cross_domain",
        "evaluation",
    }
    runtime, _store, server, thread = _runtime()
    try:
        brain = AutonomousBrain(_Workspace(), runtime)
        blueprint = brain.prepare_autonomous(
            task="use the private api key only in the provider transport",
            domain="coding",
            context={"repository": "aurora", "mode": "review"},
            constraints=("do not modify files",),
            desired_outputs=("risk summary", "verification plan"),
        )
        public = blueprint.to_dict()
        assert public["task"]["task_digest"]
        assert "private api key" not in json.dumps(public).lower()
        assert public["prompt"]["context_ids"] == [
            "autonomy-domain-policy",
            "autonomy-workflow-contract",
            "autonomy-constraints",
            "autonomy-desired-outputs",
            "autonomy-user-context",
        ]
        assert blueprint.plan["steps"][0]["effect"] == "provider_call"
        structured = brain.prepare_autonomous(
            task="return a structured review",
            domain="coding",
            require_json=True,
        )
        assert structured.spec.response_schema is not None
        assert structured.spec.response_schema["properties"]["workflow_id"]["enum"] == ["coding_delivery"]  # type: ignore[index]
        for domain in AUTONOMOUS_DOMAINS:
            domain_blueprint = brain.prepare_autonomous(
                task=f"prepare a bounded {domain} review",
                domain=domain,
            )
            assert domain_blueprint.workflow.domain == domain
            assert domain_blueprint.workflow.stages
            assert domain_blueprint.plan["workflow_id"] == domain_blueprint.workflow.workflow_id
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_builtin_workflow_registry_drives_all_domains_with_valid_stage_dags():
    registry = AutonomousWorkflowRegistry.with_builtin_strategies()
    strategies = registry.catalogue()
    assert len(strategies) == 12
    assert {item["domain"] for item in strategies} == set(AUTONOMOUS_DOMAINS)
    for strategy in strategies:
        assert strategy["schema"] == "bioprism-python-autonomous-workflow/0.1"
        assert len(strategy["workflow_digest"]) == 64
        stages = strategy["stages"]
        assert 4 <= len(stages) <= 5
        stage_ids = {stage["id"] for stage in stages}
        assert all(set(stage["depends_on"]).issubset(stage_ids) for stage in stages)
        assert strategy["evaluator_signals"]
        assert "learn the safest workspace inspection" not in json.dumps(strategy)


def test_run_autonomous_selects_assembles_plans_and_preserves_provider_approval():
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    handle = store.register("openai", "transport-secret")
    brain = AutonomousBrain(workspace, runtime)
    try:
        waiting = brain.run_autonomous(
            task="review the implementation",
            domain="coding",
            model_candidates=_model(),
            credentials={"openai": handle},
        )
        assert waiting.status == "approval_required"
        assert not hasattr(server, "request_body")

        completed = brain.run_autonomous(
            task="review the implementation",
            domain="coding",
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
        )
        assert completed.status == "completed_provider_call"
        assert completed.response is not None
        assert completed.response.text == "bounded answer"
        assert any(name == "brain_model_select_contextual" for name, _ in workspace.calls)
        prompt_call = next(args for name, args in workspace.calls if name == "brain_prompt_assemble")
        assert any(chunk["id"] == "autonomy-domain-policy" for chunk in prompt_call["context"])  # type: ignore[index]
        assert "transport-secret" not in json.dumps(completed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_autonomous_learning_records_explicit_reward_and_only_metadata_in_memory(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    handle = store.register("openai", "learning-secret")
    memory = BrainEpisodicMemory(tmp_path / "autonomy.sqlite3")
    ledger = BrainLearningLedger(tmp_path / "learning.jsonl")
    evaluator = BrainOutcomeEvaluator(
        lambda _input: {"reward": 0.8, "passed": True, "failed": False},
        evaluator_id="test-autonomy-evaluator",
        evaluator_version="1",
    )
    brain = AutonomousBrain(workspace, runtime, memory=memory)
    try:
        result = brain.run_autonomous(
            task="learn a safer code review response",
            domain="coding",
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            learn=True,
            evaluator=evaluator,
            bandit_state={
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [
                    {
                        "arm_id": "openai/test-model",
                        "pulls": 0,
                        "reward_sum": 0.0,
                        "failures": 0,
                        "disabled": False,
                    }
                ],
            },
            ledger=ledger,
        )
        assert result.status == "completed"
        assert result.replan_count == 0
        assert len(result.evaluations) == 1
        assert result.bandit_state["generation"] == 1  # type: ignore[index]
        assert len(result.memory_receipts) == 2
        memory_text = (tmp_path / "autonomy.sqlite3").read_bytes()
        assert b"learn a safer code review response" not in memory_text
        assert b"learning-secret" not in memory_text
        assert "learning-secret" not in json.dumps(result.to_dict())
    finally:
        memory.close()
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_autonomous_tool_loop_executes_only_through_caller_callback():
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    handle = store.register("openai", "tool-loop-secret")
    brain = AutonomousBrain(workspace, runtime)
    callback_calls: list[str] = []

    def authorize(calls: tuple[object, ...]) -> tuple[ProviderToolResult, ...]:
        callback_calls.extend(getattr(call, "name", "") for call in calls)
        return (
            ProviderToolResult(
                call_id=getattr(calls[0], "call_id"),
                content={"status": "ready"},
                approved=True,
            ),
        )

    try:
        waiting = brain.run_autonomous(
            task="inspect the workspace status",
            domain="operations",
            execution_mode="tool_loop",
            model_candidates=_model(),
            credentials={"openai": handle},
            route_request={"needs": [{"id": "workspace-status", "query": "workspace status"}]},
            tool_loop_options={"authorize_and_execute": authorize, "max_turns": 3},
        )
        assert waiting.status == "approval_required"
        assert callback_calls == []
        completed = brain.run_autonomous(
            task="inspect the workspace status",
            domain="operations",
            execution_mode="tool_loop",
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            auto_route=True,
            tool_loop_options={"authorize_and_execute": authorize, "max_turns": 3},
        )
        assert completed.status == "completed_provider_tool_loop"
        assert callback_calls == ["developer_platform_status"]
        assert completed.provider_loop is not None
        assert completed.provider_loop.final_response is not None
        assert completed.provider_loop.final_response.text == "continued bounded answer"
        assert any(name == "capability_route" for name, _ in workspace.calls)
        assert "tool-loop-secret" not in json.dumps(completed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_supplied_bandit_state_changes_the_next_adaptive_model_choice():
    runtime, credentials, server, thread = _runtime()

    class BanditWorkspace(_Workspace):
        def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
            if name == "brain_model_select":
                args = {} if arguments is None else arguments
                observations = args.get("observations", [])
                scores: dict[str, float] = {}
                if isinstance(observations, list):
                    for observation in observations:
                        if not isinstance(observation, dict):
                            continue
                        arm_id = observation.get("arm_id")
                        pulls = observation.get("pulls", 0)
                        reward_sum = observation.get("reward_sum", 0.0)
                        if isinstance(arm_id, str) and isinstance(pulls, int) and pulls > 0 and isinstance(reward_sum, (int, float)):
                            scores[arm_id] = float(reward_sum) / pulls
                selected = max(scores, key=scores.get) if scores else "openai/model-a"
                return {
                    "selected_model": {
                        "provider": selected.split("/", 1)[0],
                        "model": selected.split("/", 1)[1],
                    },
                    "decision_digest": "d" * 64,
                }
            return super().tool(name, arguments)

    brain = AutonomousBrain(BanditWorkspace(), runtime)
    handle = credentials.register("openai", "bandit-choice-secret")
    candidates = [{**_model()[0], "model": "model-a"}, {**_model()[0], "model": "model-b"}]

    def state(a_reward: float, b_reward: float) -> dict[str, object]:
        return {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": 3,
            "arms": [
                {"arm_id": "openai/model-a", "pulls": 10, "reward_sum": a_reward, "failures": 0, "disabled": False},
                {"arm_id": "openai/model-b", "pulls": 10, "reward_sum": b_reward, "failures": 0, "disabled": False},
            ],
        }

    try:
        preferred_a = brain.run_adaptive(
            task="Choose the best bounded provider for this review.",
            model_candidates=candidates,
            prompt={"context": []},
            plan={},
            credentials={"openai": handle},
            bandit_state=state(9.0, 1.0),
        )
        preferred_b = brain.run_adaptive(
            task="Choose the best bounded provider for this review.",
            model_candidates=candidates,
            prompt={"context": []},
            plan={},
            credentials={"openai": handle},
            bandit_state=state(1.0, 9.0),
        )
        assert preferred_a.status == "approval_required"
        assert preferred_b.status == "approval_required"
        assert preferred_a.selection["selected_model"]["model"] == "model-a"  # type: ignore[index]
        assert preferred_b.selection["selected_model"]["model"] == "model-b"  # type: ignore[index]
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_autonomous_tool_loop_learning_records_loop_metadata_only(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    handle = store.register("openai", "tool-learning-secret")
    memory = BrainEpisodicMemory(tmp_path / "tool-loop.sqlite3")
    brain = AutonomousBrain(workspace, runtime, memory=memory)

    def authorize(calls: tuple[object, ...]) -> tuple[ProviderToolResult, ...]:
        return (
            ProviderToolResult(
                call_id=getattr(calls[0], "call_id"),
                content={"status": "ready"},
                approved=True,
            ),
        )

    try:
        result = brain.run_autonomous(
            task="learn the safest workspace inspection response",
            domain="operations",
            execution_mode="tool_loop",
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            route_request={"needs": [{"id": "workspace-status", "query": "workspace status"}]},
            tool_loop_options={"authorize_and_execute": authorize, "max_turns": 3},
            learn=True,
            evaluator=BrainOutcomeEvaluator(
                lambda _input: {"reward": 0.7, "passed": True, "failed": False},
                evaluator_id="tool-loop-evaluator",
                evaluator_version="1",
            ),
            bandit_state={
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [
                    {
                        "arm_id": "openai/test-model",
                        "pulls": 0,
                        "reward_sum": 0.0,
                        "failures": 0,
                        "disabled": False,
                    }
                ],
            },
        )
        assert result.status == "completed"
        assert result.final_result.status == "completed_provider_tool_loop"
        assert result.bandit_state["generation"] == 1  # type: ignore[index]
        assert len(result.memory_receipts) == 2
        assert b"safest workspace inspection" not in (tmp_path / "tool-loop.sqlite3").read_bytes()
        assert "tool-learning-secret" not in json.dumps(result.to_dict())
    finally:
        memory.close()
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_cross_domain_fans_out_then_synthesizes_with_approval_boundary():
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    handle = store.register("openai", "cross-domain-secret")
    brain = AutonomousBrain(workspace, runtime)
    subtasks = [
        {
            "id": "engineering-review",
            "task": "Review the implementation risk and verification plan.",
            "domain": "coding",
        },
        {
            "id": "data-review",
            "task": "Review the migration schema and lineage risks.",
            "domain": "data",
        },
    ]
    try:
        prepared = brain.prepare_cross_domain(
            task="Combine engineering and data review into one decision package.",
            subtasks=subtasks,
            context={"repository": "aurora", "environment": "staging"},
        )
        public = prepared.to_dict()
        assert len(public["children"]) == 2
        assert public["dependency_graph"]["fan_in"]
        assert "Review the implementation risk" not in json.dumps(public)
        waiting = brain.run_cross_domain(
            task="Combine engineering and data review into one decision package.",
            subtasks=subtasks,
            model_candidates=_model(),
            credentials={"openai": handle},
        )
        assert waiting.status == "approval_required"
        assert len(waiting.child_results) == 2
        assert waiting.synthesis_result is None
        assert not hasattr(server, "request_body")
        completed = brain.run_cross_domain(
            task="Combine engineering and data review into one decision package.",
            subtasks=subtasks,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
        )
        assert completed.status == "completed"
        assert len(completed.child_results) == 2
        assert all(result.status == "completed_provider_call" for result in completed.child_results)
        assert completed.synthesis_result is not None
        assert completed.synthesis_result.status == "completed_provider_call"
        assert "cross-domain-secret" not in json.dumps(completed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_workflow_executes_stage_dag_and_resumes_only_unfinished_stages():
    runtime, store, server, thread = _structured_runtime()
    workspace = _Workspace()
    handle = store.register("openai", "workflow-secret")
    brain = AutonomousBrain(workspace, runtime)
    try:
        blueprint = brain.prepare_autonomous(
            task="Produce a bounded implementation review.",
            domain="coding",
        )
        paused = brain.run_workflow(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            run_id="workflow-test",
            max_stage_calls=2,
        )
        assert paused.status == "paused"
        assert [item.stage.id for item in paused.stage_results] == ["scope", "inspect"]
        assert paused.checkpoint.completed_stage_ids == ("scope", "inspect")
        checkpoint_wire = json.dumps(paused.checkpoint.to_dict())
        assert "Produce a bounded implementation review" not in checkpoint_wire
        assert "workflow-secret" not in checkpoint_wire
        tampered = paused.checkpoint.to_dict()
        tampered["stages"][0]["structured"]["evidence"] = ["tampered"]  # type: ignore[index]
        with pytest.raises(BrainRunError):
            brain.run_workflow(
                blueprint=blueprint,
                model_candidates=_model(),
                credentials={"openai": handle},
                checkpoint=tampered,
                approve_provider_call=True,
            )

        resumed = brain.run_workflow(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            checkpoint=paused.checkpoint,
            run_id="workflow-test",
        )
        assert resumed.status == "completed"
        assert [item.stage.id for item in resumed.stage_results] == ["implement", "verify", "handoff"]
        assert resumed.checkpoint.completed_stage_ids == ("scope", "inspect", "implement", "verify", "handoff")
        assert "workflow-secret" not in json.dumps(resumed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_workflow_requires_approval_and_supports_explicit_blocked_stage_retry():
    runtime, store, server, thread = _structured_runtime()
    workspace = _Workspace()
    handle = store.register("openai", "workflow-approval-secret")
    brain = AutonomousBrain(workspace, runtime)
    try:
        blueprint = brain.prepare_autonomous(
            task="Prepare a reversible implementation change.",
            domain="coding",
        )
        waiting = brain.run_workflow(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            run_id="workflow-approval",
        )
        assert waiting.status == "approval_required"
        assert waiting.stage_results[0].execution_status == "approval_required"
        assert not hasattr(server, "request_body")

        server.block_stage = "scope"  # type: ignore[attr-defined]
        blocked = brain.run_workflow(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            run_id="workflow-approval",
        )
        assert blocked.status == "stage_blocked"
        assert blocked.checkpoint.stages[0]["status"] == "blocked"
        server.block_stage = None  # type: ignore[attr-defined]
        resumed = brain.run_workflow(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            checkpoint=blocked.checkpoint.to_dict(),
            retry_blocked=True,
        )
        assert resumed.status == "completed"
        assert resumed.checkpoint.completed_stage_ids == ("scope", "inspect", "implement", "verify", "handoff")
        assert "workflow-approval-secret" not in json.dumps(resumed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_workflow_stage_contract_is_executable_for_every_builtin_domain():
    runtime, store, server, thread = _structured_runtime()
    handle = store.register("openai", "all-domain-workflow-secret")
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, runtime)
    try:
        for domain in AUTONOMOUS_DOMAINS:
            blueprint = brain.prepare_autonomous(
                task=f"Prepare a bounded {domain} workflow result.",
                domain=domain,
            )
            result = brain.run_workflow(
                blueprint=blueprint,
                model_candidates=_model(),
                credentials={"openai": handle},
                approve_provider_call=True,
                run_id=f"all-domain-{domain}",
                max_stage_calls=1,
            )
            assert result.status == "paused"
            assert result.stage_results[0].stage.id == blueprint.workflow.stages[0].id
            assert result.stage_results[0].declared_status == "completed"
            assert result.checkpoint.completed_stage_ids == (blueprint.workflow.stages[0].id,)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_workflow_learning_updates_each_completed_stage_with_explicit_signals():
    runtime, store, server, thread = _structured_runtime()
    workspace = _Workspace()
    handle = store.register("openai", "workflow-learning-secret")
    brain = AutonomousBrain(workspace, runtime)
    try:
        blueprint = brain.prepare_autonomous(
            task="Produce a staged implementation review with evidence.",
            domain="coding",
        )
        result = brain.run_workflow_learning(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            run_id="workflow-learning",
            max_stage_calls=2,
            stage_evidence={
                "scope": {"signals": {"schema_valid": True}},
                "inspect": {"signals": {"evidence_complete": True}},
            },
            bandit_state={
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [
                    {
                        "arm_id": "openai/test-model",
                        "pulls": 0,
                        "reward_sum": 0.0,
                        "failures": 0,
                        "disabled": False,
                    }
                ],
            },
        )
        assert result.status == "paused"
        assert [item.stage_id for item in result.evaluations] == ["scope", "inspect"]
        assert all(item.decision.passed for item in result.evaluations)
        assert result.replan_requested is False
        assert result.bandit_state["generation"] == 1  # type: ignore[index]
        assert all(item.evidence_digest for item in result.evaluations)
        assert "workflow-learning-secret" not in json.dumps(result.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)


def test_run_workflow_learning_missing_evidence_never_defaults_to_reward(tmp_path: Path):
    runtime, store, server, thread = _structured_runtime()
    workspace = _Workspace()
    handle = store.register("openai", "workflow-missing-evidence-secret")
    brain = AutonomousBrain(workspace, runtime)
    ledger = BrainLearningLedger(tmp_path / "workflow-learning.jsonl")
    memory = BrainEpisodicMemory(tmp_path / "workflow-memory.sqlite3")
    try:
        blueprint = brain.prepare_autonomous(
            task="Evaluate a staged coding result without supplied evaluator evidence.",
            domain="coding",
        )
        result = brain.run_workflow_learning(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            run_id="workflow-missing-evidence",
            max_stage_calls=1,
            bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
            memory=memory,
            ledger=ledger,
        )
        assert result.status == "learning_replan_requested"
        assert len(result.evaluations) == 1
        assert result.evaluations[0].decision.failed is True
        assert result.evaluations[0].decision.reward == 0.0
        assert result.replan_requested is True
        assert b"Evaluate a staged coding result" not in (tmp_path / "workflow-memory.sqlite3").read_bytes()
        assert b"workflow-missing-evidence-secret" not in (tmp_path / "workflow-learning.jsonl").read_bytes()
    finally:
        memory.close()
        server.shutdown()
        thread.join(timeout=2)


def test_durable_workflow_worker_releases_one_stage_and_resumes_after_store_restart(tmp_path: Path):
    runtime, credentials, server, thread = _structured_runtime()
    handle = credentials.register("openai", "durable-workflow-secret")
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, runtime)
    job_path = tmp_path / "durable-workflow.sqlite3"
    try:
        blueprint = brain.prepare_autonomous(
            task="Execute a restart-safe staged implementation review.",
            domain="coding",
        )
        stage_evidence = {
            stage.id: {"signals": {signal: True for signal in stage.evaluator_signals}}
            for stage in blueprint.workflow.stages
        }
        packet = {
            "idempotency_key": "durable-workflow-review",
            "spec_digest": "a" * 64,
            "domain": "coding",
            "capability": "implementation_review",
            "risk_class": "review",
            "max_attempts": 8,
        }

        def resolve(metadata: dict[str, object]) -> dict[str, object]:
            assert "Execute a restart-safe" not in json.dumps(metadata)
            return {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "workflow_options": {
                    "approve_provider_call": True,
                    "stage_evidence": stage_evidence,
                },
            }

        with BrainJobStore(job_path) as store:
            job, _ = store.submit(packet)
            worker = BrainWorker(
                brain,
                store,
                worker_id="workflow-worker-a",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="workflow_learning",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            first = worker.run_once(job.job_id)
            assert first is not None
            assert first.status == "queued"
            assert first.workflow is not None
            assert first.workflow.workflow.status == "paused"
            first_record = store.get(job.job_id)
            assert first_record is not None
            assert first_record.state == "queued"
            assert first_record.checkpoint["checkpoint_storage"] == "inline"
            assert first_record.checkpoint["completed_stage_ids"] == [blueprint.workflow.stages[0].id]
            serialized = json.dumps(first_record.to_dict())
            assert "restart-safe staged implementation review" not in serialized
            assert "durable-workflow-secret" not in serialized
            assert worker.bandit_state["generation"] == 1

        with BrainJobStore(job_path) as reopened:
            restarted = BrainWorker(
                brain,
                reopened,
                worker_id="workflow-worker-b",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="workflow_learning",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            results = [restarted.run_once(job.job_id) for _ in range(len(blueprint.workflow.stages) - 1)]
            assert all(result is not None for result in results)
            assert results[-1] is not None
            assert results[-1].status == "succeeded"
            final = reopened.get(job.job_id)
            assert final is not None
            assert final.state == "succeeded"
            assert final.checkpoint["phase"] == "completed"
            assert reopened.verify_integrity()["ok"] is True
            assert any(
                any(
                    isinstance(observation, dict) and observation.get("pulls") == 1
                    for observation in (
                        arguments.get("observations", [])
                        if name == "brain_model_select"
                        else arguments.get("base", {}).get("observations", [])
                        if isinstance(arguments.get("base", {}), dict)
                        else []
                    )
                )
                for name, arguments in workspace.calls
                if name in {"brain_model_select", "brain_model_select_contextual"}
            )
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_durable_workflow_job_parks_provider_approval_without_losing_checkpoint(tmp_path: Path):
    runtime, credentials, server, thread = _structured_runtime()
    handle = credentials.register("openai", "approval-workflow-secret")
    brain = AutonomousBrain(_Workspace(), runtime)
    try:
        blueprint = brain.prepare_autonomous(
            task="Prepare a staged change that requires an operator gate.",
            domain="coding",
        )
        evidence = {
            stage.id: {"signals": {signal: True for signal in stage.evaluator_signals}}
            for stage in blueprint.workflow.stages
        }

        def resolve(_metadata: dict[str, object]) -> dict[str, object]:
            return {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "workflow_options": {
                    "approve_provider_call": False,
                    "stage_evidence": evidence,
                },
            }

        with BrainJobStore(tmp_path / "approval-workflow.sqlite3") as store:
            job, _ = store.submit(
                {
                    "idempotency_key": "approval-workflow",
                    "spec_digest": "b" * 64,
                    "domain": "coding",
                    "capability": "implementation_review",
                    "risk_class": "review",
                    "max_attempts": 4,
                }
            )
            worker = BrainWorker(
                brain,
                store,
                worker_id="approval-worker",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="workflow_learning",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            waiting = worker.run_once(job.job_id)
            assert waiting is not None
            assert waiting.status == "waiting_approval"
            record = store.get(job.job_id)
            assert record is not None
            assert record.state == "waiting_approval"
            assert record.checkpoint["job_kind"] == "autonomous_workflow"
            assert "workflow_checkpoint" in record.checkpoint
            request = BrainApprovalRouter(store).get(job.job_id)
            assert request is not None
            assert request.state == "pending"
            BrainApprovalRouter(store).approve(job.job_id, approver="operator-1")
            resumed = worker.run_once(job.job_id)
            assert resumed is not None
            assert resumed.status == "queued"
            assert resumed.workflow.workflow.checkpoint.completed_stage_ids == (blueprint.workflow.stages[0].id,)  # type: ignore[union-attr]
            serialized = json.dumps(store.get(job.job_id).to_dict())  # type: ignore[union-attr]
            assert "approval-workflow-secret" not in serialized
            assert "Prepare a staged change" not in serialized
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_durable_workflow_job_switches_to_caller_owned_checkpoint_storage_when_needed(tmp_path: Path):
    runtime, credentials, server, thread = _structured_runtime()
    server.large_checkpoint = True  # type: ignore[attr-defined]
    handle = credentials.register("openai", "large-checkpoint-secret")
    brain = AutonomousBrain(_Workspace(), runtime)
    stored_checkpoints: list[object] = []
    try:
        blueprint = brain.prepare_autonomous(
            task="Persist a large but bounded staged evidence continuation.",
            domain="coding",
        )
        evidence = {
            stage.id: {"signals": {signal: True for signal in stage.evaluator_signals}}
            for stage in blueprint.workflow.stages
        }

        def sink(_job_id: str, checkpoint: object) -> None:
            stored_checkpoints.append(checkpoint)

        def resolve(metadata: dict[str, object]) -> dict[str, object]:
            job_checkpoint = metadata.get("checkpoint", {})
            resolved: dict[str, object] = {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "workflow_options": {
                    "approve_provider_call": True,
                    "stage_evidence": evidence,
                },
            }
            if isinstance(job_checkpoint, dict) and job_checkpoint.get("checkpoint_storage") == "caller_owned":
                resolved["checkpoint"] = stored_checkpoints[-1]
            return resolved

        with BrainJobStore(tmp_path / "large-checkpoint.sqlite3") as store:
            job, _ = store.submit(
                {
                    "idempotency_key": "large-checkpoint-workflow",
                    "spec_digest": "c" * 64,
                    "domain": "coding",
                    "capability": "implementation_review",
                    "risk_class": "review",
                    "max_attempts": 4,
                }
            )
            worker = BrainWorker(
                brain,
                store,
                worker_id="large-checkpoint-worker",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="workflow_learning",
                workflow_checkpoint_sink=sink,
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            first = worker.run_once(job.job_id)
            assert first is not None and first.status == "queued"
            second = worker.run_once(job.job_id)
            assert second is not None and second.status == "queued"
            third = worker.run_once(job.job_id)
            assert third is not None and third.status == "queued"
            record = store.get(job.job_id)
            assert record is not None
            assert record.checkpoint["checkpoint_storage"] == "caller_owned"
            assert record.checkpoint["completed_stage_ids"] == ["scope", "inspect", "implement"]
            assert len(stored_checkpoints) == 2
            assert "large-checkpoint-secret" not in json.dumps(record.to_dict())
            assert "Persist a large but bounded" not in json.dumps(record.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_builtin_workflow_learning_signal_contract_covers_every_domain():
    runtime, store, server, thread = _structured_runtime()
    handle = store.register("openai", "all-domain-learning-secret")
    brain = AutonomousBrain(_Workspace(), runtime)
    try:
        for domain in AUTONOMOUS_DOMAINS:
            blueprint = brain.prepare_autonomous(
                task=f"Evaluate a bounded {domain} stage.",
                domain=domain,
            )
            stage = blueprint.workflow.stages[0]
            result = brain.run_workflow_learning(
                blueprint=blueprint,
                model_candidates=_model(),
                credentials={"openai": handle},
                approve_provider_call=True,
                run_id=f"learning-{domain}",
                max_stage_calls=1,
                stage_evidence={
                    stage.id: {"signals": {signal: True for signal in stage.evaluator_signals}}
                },
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
            )
            assert result.status == "paused"
            assert len(result.evaluations) == 1
            assert result.evaluations[0].decision.passed is True
            assert result.replan_requested is False
    finally:
        server.shutdown()
        thread.join(timeout=2)
