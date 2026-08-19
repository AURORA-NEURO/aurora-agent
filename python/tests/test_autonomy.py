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
    AutonomousDomainPackRegistry,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomousPlanHoldoutCase,
    AutonomousPlanHoldoutEvaluator,
    AutonomousPlanRefinementResult,
    AutonomousToolOutcomeEvaluator,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousCrossDomainCheckpoint,
    AutonomousCrossDomainResult,
    AutonomousRoutingHoldoutCase,
    AutonomousRoutingHoldoutEvaluator,
    AutonomousTaskRouter,
    AutonomousWorkflowRegistry,
    BrainRunError,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    BrainApprovalRouter,
    BrainJobStore,
    BrainWorker,
    build_brain_evaluation_input,
    CredentialStore,
    LLMRuntime,
    ModelCandidate,
    ModelCatalogue,
    ProviderHealthLedger,
    ProviderError,
    ProviderToolResult,
    builtin_autonomous_domain_evaluator_profiles,
    openai_provider,
    content_digest,
)


class _ProviderHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib handler protocol
        length = int(self.headers.get("Content-Length", "0"))
        self.server.request_body = self.rfile.read(length)  # type: ignore[attr-defined]
        request = json.loads(self.server.request_body.decode("utf-8"))  # type: ignore[attr-defined]
        request_text = json.dumps(request)
        if "Propose a bounded cross-domain planning refinement" in request_text:
            response = {
                "id": "autonomy-cross-domain-plan-refinement",
                "model": "test-model",
                "output_text": json.dumps(
                    {
                        "priority_order": ["data-review", "engineering-review"],
                        "focus_child_ids": ["data-review"],
                        "review_required": False,
                        "confidence": 0.82,
                        "abstain": False,
                    }
                ),
                "usage": {"total_tokens": 10},
            }
            payload = json.dumps(response).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        if "Propose a bounded planning refinement for the reviewed workflow" in request_text:
            response = {
                "id": "autonomy-plan-refinement",
                "model": "test-model",
                "output_text": json.dumps(
                    {
                        "priority_order": ["scope", "inspect", "implement", "verify", "handoff"],
                        "focus_stage_ids": ["inspect", "verify"],
                        "review_required": False,
                        "confidence": 0.85,
                        "abstain": False,
                    }
                ),
                "usage": {"total_tokens": 10},
            }
            payload = json.dumps(response).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        if "Classify the following user request against the reviewed AURORA autonomous domain" in request_text:
            scores = {
                domain: (0.91 if domain == "neuroscience" else 0.08)
                for domain in AUTONOMOUS_DOMAINS
            }
            response = {
                "id": "autonomy-semantic-route",
                "model": "test-model",
                "output_text": json.dumps(
                    {
                        "candidates": [
                            {"domain": domain, "score": score}
                            for domain, score in scores.items()
                        ],
                        "selected_domains": ["neuroscience"],
                        "confidence": 0.91,
                        "abstain": False,
                    }
                ),
                "usage": {"total_tokens": 12},
            }
            payload = json.dumps(response).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
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
        assert agent.credential_statuses()[0]["provider"] == "openai"
        before = agent.readiness()
        assert {row["domain"] for row in before["route_catalogue"]} == set(AUTONOMOUS_DOMAINS)
        assert len(before["workflows"]) == len(AUTONOMOUS_DOMAINS)
        assert before["models"][0]["eligible_for_selection"] is False
        assert before["providers"][0]["next_action"] == "collect_user_credential"

        assert agent.credential_status("openai")["next_action"] == "collect_user_credential"
        with agent.start_credential_session(session_id="test-session") as session:
            session.register_value("openai", "test-secret")
            assert session.handles()["openai"].provider == "openai"
            ready = agent.readiness()
            assert ready["models"][0]["eligible_for_selection"] is True
            assert ready["semantic_routing"]["domain_count"] == len(AUTONOMOUS_DOMAINS)
            assert ready["semantic_routing"]["requires_caller_provider_approval"] is True
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
                domains=("operations", "coding", "cross_domain"),
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
        tool_plan = agent.domain_pack_tool_plan("operations")
        assert "observability" in tool_plan["covered_tool_capabilities"]
        assert tool_plan["pack_digest"] == agent.domain_pack("operations")["pack_digest"]
        assert agent.tool_receipts()[0]["status"] == "executed"
        assert "workspace" not in json.dumps(agent.tool_receipts())
        assert "agent-domain-tool-secret" not in json.dumps(result.to_dict())
        call_id = agent.tool_receipts()[0]["call_id"]
        learning = agent.evaluate_tool_receipts(
            evaluator=AutonomousToolOutcomeEvaluator(
                lambda value: {"reward": 0.75 if value["status"] == "executed" else -0.5, "passed": value["status"] == "executed"},
                evaluator_id="operations-tool-quality",
                evaluator_version="v1",
            ),
            evidence={call_id: {"quality_gate": "passed"}},
            bandit_state={"generation": 0},
            bandit_updater=lambda state, _decision, _outcome: {**state, "generation": state["generation"] + 1},
        )
        assert learning.status == "completed"
        assert learning.next_bandit_state["generation"] == 1
        assert learning.by_domain == {"operations": 1}
        cross = agent.run_cross_domain(
            task="inspect operations and coding workspace status",
            subtasks=[
                {"id": "operations-check", "task": "inspect operations status", "domain": "operations", "execution_mode": "tool_loop"},
                {"id": "coding-check", "task": "inspect coding status", "domain": "coding", "execution_mode": "tool_loop"},
            ],
            credentials={"openai": handle},
            model_candidates=_model(),
            child_execution_mode="tool_loop",
            synthesize=False,
            tool_loop_options={"max_turns": 3},
            approve_provider_call=True,
        )
        assert cross.status == "children_completed"
        cross_receipts = agent.tool_receipts()[-2:]
        assert {receipt["domain"] for receipt in cross_receipts} == {"operations", "coding"}
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_autonomous_agent_persists_native_tool_execution_and_terminal_state(tmp_path):
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
                domains=("operations",),
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
    handle = store.register("openai", "agent-domain-tool-persistence-secret")
    journal = AutonomousExecutionJournal(tmp_path / "agent-execution.jsonl")
    agent = AutonomousAgent(
        workspace,
        runtime,
        model_catalogue=ModelCatalogue(_model()),
        tool_registry=registry,
        execution_journal=journal,
        execution_policy=AutonomousExecutionPolicy(max_steps=16, max_tool_calls=4),
    )
    try:
        result = agent.run(
            task="inspect the workspace status",
            domain="operations",
            credentials={"openai": handle},
            execution_id="persisted-agent-run",
            execution_mode="tool_loop",
            approve_provider_call=True,
            tool_loop_options={"max_turns": 3},
        )
        assert result.status == "completed_provider_tool_loop"
        state = agent.execution_state("persisted-agent-run")
        assert state is not None
        assert state["status"] == "completed"
        kinds = {event["event"]["kind"] for event in agent.execution_events("persisted-agent-run")}
        assert {"started", "provider_call", "tool_intent", "tool_outcome", "completed"}.issubset(kinds)
        provider_events = [
            row["event"]
            for row in agent.execution_events("persisted-agent-run")
            if row["event"]["kind"] == "provider_call"
        ]
        assert any(event.get("provider") == "openai" for event in provider_events)
        assert len(result.brain_run.provider_invocations) == 2
        assert all("content" not in receipt for receipt in result.brain_run.provider_invocations)
        evaluation_input = build_brain_evaluation_input(result)
        assert len(evaluation_input["provider_invocations"]) == 2
        assert journal.verify_integrity()["verified"] is True
        assert "agent-domain-tool-persistence-secret" not in json.dumps(agent.execution_events("persisted-agent-run"))
        with pytest.raises(BrainRunError):
            agent.run(
                task="inspect the workspace status",
                domain="operations",
                credentials={"openai": handle},
                execution_id="persisted-agent-run",
                resume_execution=True,
                execution_mode="tool_loop",
                approve_provider_call=True,
                tool_loop_options={"max_turns": 1},
            )
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
            "autonomy-domain-pack",
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
            assert domain_blueprint.domain_pack.domain == domain
            assert domain_blueprint.selection_context["domain_pack_digest"] == domain_blueprint.domain_pack.pack_digest
            assert domain_blueprint.plan["steps"][0]["arguments"]["domain_pack_digest"] == domain_blueprint.domain_pack.pack_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_domain_packs_cover_every_domain_and_bind_workflow_evaluator_and_tools():
    domain_registry = AutonomousDomainRegistry.with_builtin_profiles()
    workflow_registry = AutonomousWorkflowRegistry.with_builtin_strategies()
    packs = AutonomousDomainPackRegistry.with_builtin_packs(domain_registry, workflow_registry)
    evaluator_profiles = {
        profile.domain: profile
        for profile in builtin_autonomous_domain_evaluator_profiles()
    }
    assert {entry["domain"] for entry in packs.catalogue()} == set(AUTONOMOUS_DOMAINS)
    assert len(packs.digest) == 64
    for domain in AUTONOMOUS_DOMAINS:
        profile = domain_registry.resolve(domain)
        workflow = workflow_registry.resolve(domain)
        pack = packs.resolve(domain)
        assert pack.workflow_id == workflow.workflow_id
        assert pack.evaluator_domain == profile.evaluator_domain
        assert pack.evaluator_id == evaluator_profiles[domain].evaluator_id
        assert set(profile.required_model_capabilities).issubset(pack.model_capabilities)
        assert set(profile.capabilities).issubset(pack.tool_capabilities)
        assert set(workflow.evaluator_signals).issubset(pack.evidence_requirements)
        assert set(evaluator_profiles[domain].required_signals).issubset(pack.evidence_requirements)
        assert len(pack.pack_digest) == 64
        public = pack.to_dict()
        assert "private api key" not in json.dumps(public).lower()

    runtime = LLMRuntime()
    agent = AutonomousAgent(_Workspace(), runtime)
    readiness = agent.readiness()
    assert len(readiness["domain_packs"]) == len(AUTONOMOUS_DOMAINS)
    assert readiness["domain_pack_registry_digest"] == packs.digest


def test_provider_free_router_covers_every_domain_and_abstains_without_evidence():
    registry = AutonomousDomainRegistry.with_builtin_profiles()
    router = AutonomousTaskRouter(registry)
    for domain in AUTONOMOUS_DOMAINS:
        proposal = router.route(f"please prepare a bounded {domain} task")
        assert proposal.abstained is False
        assert proposal.primary_domain == domain
        assert proposal.route_digest == router.route(f"please prepare a bounded {domain} task").route_digest
        assert "please prepare" not in json.dumps(proposal.to_dict())

    unknown = router.route("please explain an entirely unclassified household question")
    assert unknown.abstained is True
    assert unknown.reason == "no_matching_evidence"
    assert unknown.selected_domains == ()
    assert unknown.to_dict()["retention"].startswith("task_text_transient_only")


def test_held_out_routing_and_planning_evaluators_are_value_only_and_cover_every_domain():
    router = AutonomousTaskRouter(AutonomousDomainRegistry.with_builtin_profiles())
    cases = tuple(
        AutonomousRoutingHoldoutCase(
            case_id=f"holdout-{domain}",
            task=f"please prepare a bounded {domain} task",
            expected_domains=(domain,),
        )
        for domain in AUTONOMOUS_DOMAINS
    )
    routing_report = AutonomousRoutingHoldoutEvaluator(
        router,
        evaluator_id="routing-holdout",
        evaluator_version="2026-08-19",
    ).evaluate(cases)
    assert routing_report.case_count == len(AUTONOMOUS_DOMAINS)
    assert routing_report.exact_match_count == len(AUTONOMOUS_DOMAINS)
    assert routing_report.to_dict()["coverage"] == 1.0
    public_routing = json.dumps(routing_report.to_dict())
    assert "please prepare" not in public_routing
    assert "expected_domains" not in public_routing

    agent = AutonomousAgent(_Workspace(), LLMRuntime())
    plan_cases = []
    for domain in AUTONOMOUS_DOMAINS:
        blueprint = agent.prepare(task=f"prepare a bounded {domain} workflow", domain=domain)
        stage_ids = tuple(stage.id for stage in blueprint.workflow.stages)
        refinement = AutonomousPlanRefinementResult(
            status="completed",
            task_digest=blueprint.spec.task_digest,
            base_plan_digest=content_digest(blueprint.plan),
            workflow_digest=blueprint.workflow.workflow_digest,
            priority_stage_ids=stage_ids,
            focus_stage_ids=(stage_ids[1],),
            review_required=False,
            confidence=1.0,
        )
        plan_cases.append(
            AutonomousPlanHoldoutCase(
                case_id=f"plan-holdout-{domain}",
                blueprint=blueprint,
                refinement=refinement,
                expected_priority_stage_ids=stage_ids,
            )
        )
    plan_report = AutonomousPlanHoldoutEvaluator(
        evaluator_id="planning-holdout",
        evaluator_version="2026-08-19",
    ).evaluate(tuple(plan_cases))
    assert plan_report.case_count == len(AUTONOMOUS_DOMAINS)
    assert plan_report.exact_order_count == len(AUTONOMOUS_DOMAINS)
    assert plan_report.to_dict()["exact_order_accuracy"] == 1.0
    assert "fix the Rust" not in json.dumps(plan_report.to_dict())


def test_provider_semantic_router_reconciles_all_domains_and_builds_a_blueprint():
    runtime, store, server, thread = _runtime()
    try:
        agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
        with agent.onboarding.start_session(session_id="semantic-route-session") as session:
            session.register_value("openai", "semantic-route-secret")
            task = "compare synaptic oscillation artifacts across two measurement protocols"
            result = agent.route_with_provider(
                task=task,
                credentials=session,
                approve_provider_call=True,
            )
            assert result.status == "completed"
            assert result.deterministic_route.abstained is True
            assert result.route.primary_domain == "neuroscience"
            assert result.route.source == "provider_semantic_hybrid"
            assert len(result.semantic_candidates) == len(AUTONOMOUS_DOMAINS)
            assert result.semantic_selected_domains == ("neuroscience",)
            assert result.selected_model == {"provider": "openai", "model": "test-model"}
            public = json.dumps(result.to_dict())
            assert "semantic-route-secret" not in public
            assert task not in public

            blueprint = agent.prepare_auto_with_provider(
                task=task,
                credentials=session,
                approve_provider_call=True,
            )
            assert blueprint.semantic_route is not None
            assert blueprint.semantic_route.status == "completed"
            assert blueprint.blueprint is not None
            assert blueprint.blueprint.profile.domain == "neuroscience"
            assert blueprint.blueprint.spec.context["autonomous_route"]["route_digest"] == blueprint.route.route_digest

            executed = agent.run_auto(
                task=task,
                credentials=session,
                semantic_routing=True,
                approve_provider_call=True,
            )
            assert executed.status == "completed"
            assert executed.route.primary_domain == "neuroscience"
            assert executed.result is not None
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_provider_semantic_router_requires_approval_and_never_builds_executable_work():
    runtime, store, server, thread = _runtime()
    try:
        agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
        with agent.onboarding.start_session(session_id="semantic-route-approval-session") as session:
            session.register_value("openai", "semantic-approval-secret")
            blueprint = agent.prepare_auto_with_provider(
                task="compare synaptic oscillation artifacts across two measurement protocols",
                credentials=session,
            )
            assert blueprint.semantic_route is not None
            assert blueprint.semantic_route.status == "approval_required"
            assert blueprint.route.abstained is True
            assert blueprint.blueprint is None
            assert blueprint.cross_domain_blueprint is None
            assert not hasattr(server, "request_body")
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_provider_plan_refinement_is_dependency_closed_and_approval_gated():
    runtime, store, server, thread = _runtime()
    try:
        agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
        blueprint = agent.prepare(task="fix the Rust tests in the repository", domain="coding")
        with agent.onboarding.start_session(session_id="plan-refinement-session") as session:
            session.register_value("openai", "plan-refinement-secret")
            waiting = agent.plan_with_provider(blueprint=blueprint, credentials=session)
            assert waiting.status == "approval_required"
            assert waiting.priority_stage_ids == ()
            refined = agent.plan_with_provider(
                blueprint=blueprint,
                credentials=session,
                approve_provider_call=True,
            )
            assert refined.status == "completed"
            assert refined.priority_stage_ids == ("scope", "inspect", "implement", "verify", "handoff")
            assert refined.focus_stage_ids == ("inspect", "verify")
            assert refined.review_required is False
            assert refined.to_dict()["authorization"].startswith("plan_proposal_only")
            public = json.dumps(refined.to_dict())
            assert "plan-refinement-secret" not in public
            assert blueprint.spec.task not in public
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_provider_cross_domain_plan_refinement_reorders_only_existing_children():
    runtime, _, server, thread = _runtime()
    workspace = _Workspace()
    agent = AutonomousAgent(workspace, runtime)
    task = "Combine engineering and data review into one decision package."
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
        blueprint = agent.prepare_cross_domain(task=task, subtasks=subtasks)
        with agent.onboarding.start_session(session_id="cross-domain-plan-session") as session:
            session.register_value("openai", "cross-domain-plan-secret")
            waiting = agent.plan_cross_domain_with_provider(
                blueprint=blueprint,
                credentials=session,
                model_candidates=_model(),
            )
            assert waiting.status == "approval_required"
            refined = agent.plan_cross_domain_with_provider(
                blueprint=blueprint,
                credentials=session,
                model_candidates=_model(),
                approve_provider_call=True,
            )
            assert isinstance(refined, AutonomousCrossDomainPlanRefinementResult)
            assert refined.status == "completed"
            assert refined.priority_child_ids == ("data-review", "engineering-review")
            assert refined.focus_child_ids == ("data-review",)
            assert refined.review_required is False
            public = json.dumps(refined.to_dict())
            assert task not in public
            assert "cross-domain-plan-secret" not in public

            executed = agent.run_cross_domain(
                task=task,
                subtasks=subtasks,
                credentials=session,
                model_candidates=_model(),
                accepted_plan_refinement=refined,
                approve_provider_call=True,
            )
            assert executed.status == "completed"
            assert executed.execution_child_ids == ("data-review", "engineering-review")
            assert executed.plan_refinement_digest == content_digest(refined.to_dict())
            assert executed.synthesis_result is not None
            with pytest.raises(BrainRunError, match="base"):
                agent.run_cross_domain(
                    task=task,
                    subtasks=[
                        {
                            "id": "engineering-review",
                            "task": "Review a changed implementation scope and verification plan.",
                            "domain": "coding",
                        },
                        subtasks[1],
                    ],
                    credentials=session,
                    model_candidates=_model(),
                    accepted_plan_refinement=refined,
                    approve_provider_call=True,
                )
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_router_explicitly_surfaces_cross_domain_ambiguity_and_review_policy():
    registry = AutonomousDomainRegistry.with_builtin_profiles()
    router = AutonomousTaskRouter(registry)
    proposal = router.route(
        "write python code for the dataset pipeline",
        min_confidence=0.20,
        min_margin=0.10,
        max_domains=3,
        allow_cross_domain=True,
    )
    assert proposal.abstained is False
    assert proposal.cross_domain is True
    assert set(proposal.selected_domains) == {"coding", "data"}
    assert proposal.reason == "cross_domain"

    review = router.route(
        "write python code for the dataset pipeline",
        min_confidence=0.20,
        min_margin=0.10,
        allow_cross_domain=False,
    )
    assert review.abstained is True
    assert review.reason == "insufficient_margin"

    agent = AutonomousAgent(_Workspace(), LLMRuntime())
    auto_blueprint = agent.prepare_auto(
        task="write python code for the dataset pipeline",
        min_confidence=0.20,
        min_margin=0.10,
    )
    assert auto_blueprint.route.cross_domain is True
    assert auto_blueprint.cross_domain_blueprint is not None
    assert auto_blueprint.blueprint is None


def test_agent_prepare_auto_and_run_auto_reuse_explicit_runtime_boundaries():
    runtime, store, server, thread = _runtime()
    try:
        agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
        with agent.onboarding.start_session(session_id="auto-route-session") as session:
            session.register_value("openai", "auto-route-secret")
            blueprint = agent.prepare_auto(task="fix the Rust tests in the repository")
            assert blueprint.route.primary_domain == "coding"
            assert blueprint.blueprint is not None
            assert blueprint.cross_domain_blueprint is None
            assert blueprint.blueprint.spec.context["autonomous_route"]["route_digest"] == blueprint.route.route_digest
            result = agent.run_auto(
                task="fix the Rust tests in the repository",
                credentials=session,
                approve_provider_call=True,
            )
            assert result.status == "completed"
            assert result.route.primary_domain == "coding"
            assert result.result is not None
            assert "auto-route-secret" not in json.dumps(result.to_dict())

            review = agent.run_auto(
                task="please explain an entirely unclassified household question",
                credentials=session,
                approve_provider_call=True,
            )
            assert review.status == "route_review_required"
            assert review.result is None
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_auto_can_execute_a_accepted_plan_through_the_checkpointable_workflow():
    runtime, store, server, thread = _structured_runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    try:
        task = "fix the Rust tests in the repository"
        blueprint = agent.prepare_auto(task=task)
        assert blueprint.blueprint is not None
        stage_ids = tuple(stage.id for stage in blueprint.blueprint.workflow.stages)
        refinement = AutonomousPlanRefinementResult(
            status="completed",
            task_digest=blueprint.blueprint.spec.task_digest,
            base_plan_digest=content_digest(blueprint.blueprint.plan),
            workflow_digest=blueprint.blueprint.workflow.workflow_digest,
            priority_stage_ids=stage_ids,
            focus_stage_ids=(stage_ids[1],),
            review_required=False,
            confidence=1.0,
        )
        handle = store.register("openai", "auto-workflow-secret")
        first = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            workflow_execution=True,
            accepted_plan_refinement=refinement,
            workflow_max_stage_calls=2,
            approve_provider_call=True,
            run_id="auto-workflow",
        )
        assert first.status == "completed"
        assert first.result is not None
        assert first.result.status == "paused"
        assert [item.stage.id for item in first.result.stage_results] == ["scope", "inspect"]
        assert first.result.checkpoint.plan_refinement_digest == content_digest(refinement.to_dict())

        resumed = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            workflow_execution=True,
            accepted_plan_refinement=refinement,
            workflow_checkpoint=first.result.checkpoint,
            approve_provider_call=True,
            run_id="auto-workflow",
        )
        assert resumed.result is not None
        assert resumed.result.status == "completed"
        assert resumed.result.checkpoint.completed_stage_ids == stage_ids
        with pytest.raises(BrainRunError, match="plan refinement"):
            agent.run_auto(
                task=task,
                credentials={"openai": handle},
                workflow_execution=True,
                workflow_checkpoint=first.result.checkpoint,
                approve_provider_call=True,
                run_id="auto-workflow",
            )
        assert "auto-workflow-secret" not in json.dumps(resumed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_auto_exposes_explicit_workflow_online_learning_modes():
    runtime, store, server, thread = _structured_runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    try:
        task = "fix the Rust tests in the repository"
        handle = store.register("openai", "auto-workflow-learning-secret")
        bandit_state = {
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
        }
        evidence = {
            "scope": {"signals": {"schema_valid": True}},
            "inspect": {"signals": {"evidence_complete": True}},
        }
        online = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            workflow_execution=True,
            workflow_learning=True,
            workflow_stage_evidence=evidence,
            workflow_max_stage_calls=2,
            bandit_state=bandit_state,
            approve_provider_call=True,
            run_id="auto-workflow-online",
        )
        assert online.result is not None
        assert online.result.status == "paused"
        assert [item.stage_id for item in online.result.evaluations] == ["scope", "inspect"]
        assert all(item.decision.passed for item in online.result.evaluations)
        assert online.result.bandit_state["generation"] == 1

        trajectory = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            workflow_execution=True,
            workflow_trajectory_learning=True,
            workflow_stage_evidence=evidence,
            workflow_max_stage_calls=2,
            workflow_trajectory_discount=0.5,
            workflow_trajectory_terminal_reward=0.25,
            bandit_state=bandit_state,
            approve_provider_call=True,
            run_id="auto-workflow-trajectory",
        )
        assert trajectory.result is not None
        assert trajectory.result.status == "paused"
        assert len(trajectory.result.trajectory_result.credited_rewards) == 2
        assert "auto-workflow-learning-secret" not in json.dumps(trajectory.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_run_auto_exposes_explicit_cross_domain_online_learning_modes(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    memory = BrainEpisodicMemory(tmp_path / "auto-cross-domain-learning.sqlite3")
    workspace = _Workspace()
    handle = store.register("openai", "auto-cross-domain-learning-secret")
    evaluator = BrainOutcomeEvaluator(
        lambda _input: {"reward": 0.8, "passed": True, "failed": False},
        evaluator_id="auto-cross-domain-quality",
        evaluator_version="1",
    )
    bandit_state = {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}
    evidence = {
        "route-coding": {"signals": {"schema_valid": True}},
        "route-data": {"signals": {"evidence_complete": True}},
        "synthesis": {"signals": {"decision_traceable": True}},
    }
    try:
        agent = AutonomousAgent(workspace, runtime, memory=memory)
        task = "write python code for the dataset pipeline"
        online = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            model_candidates=_model(),
            min_confidence=0.20,
            min_margin=0.10,
            cross_domain_learning=True,
            cross_domain_evidence=evidence,
            cross_domain_evaluator=evaluator,
            bandit_state=bandit_state,
            approve_provider_call=True,
        )
        assert online.result is not None
        assert online.result.status == "completed"
        assert len(online.result.evaluations) == 3
        assert online.result.cross_domain.synthesis_result is not None
        assert online.result.bandit_state["generation"] == 1

        trajectory = agent.run_auto(
            task=task,
            credentials={"openai": handle},
            model_candidates=_model(),
            min_confidence=0.20,
            min_margin=0.10,
            cross_domain_trajectory_learning=True,
            cross_domain_evidence=evidence,
            cross_domain_evaluator=evaluator,
            cross_domain_trajectory_discount=0.5,
            cross_domain_trajectory_terminal_reward=0.25,
            bandit_state=bandit_state,
            approve_provider_call=True,
        )
        assert trajectory.result is not None
        assert trajectory.result.status == "completed"
        assert len(trajectory.result.evaluations) == 3
        assert len(trajectory.result.trajectory_result.credited_rewards) == 3
        public = json.dumps(trajectory.to_dict())
        assert "auto-cross-domain-learning-secret" not in public
    finally:
        memory.close()
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
        assert any(chunk["id"] == "autonomy-domain-pack" for chunk in prompt_call["context"])  # type: ignore[index]
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


def test_cross_domain_learning_updates_state_between_children_and_synthesis(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    memory = BrainEpisodicMemory(tmp_path / "cross-domain-learning.sqlite3")
    handle = store.register("openai", "cross-domain-learning-secret")
    evaluator = BrainOutcomeEvaluator(
        lambda _input: {"reward": 0.8, "passed": True, "failed": False},
        evaluator_id="cross-domain-quality",
        evaluator_version="1",
    )
    try:
        agent = AutonomousAgent(workspace, runtime, memory=memory)
        result = agent.run_cross_domain_learning(
            task="coordinate a bounded engineering and data review",
            subtasks=[
                {"id": "engineering", "task": "review implementation risks", "domain": "coding"},
                {"id": "data", "task": "review lineage risks", "domain": "data"},
            ],
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            evaluator=evaluator,
            bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
        )
        assert result.status == "completed"
        assert len(result.cross_domain.child_results) == 2
        assert result.cross_domain.synthesis_result is not None
        assert len(result.evaluations) == 3
        assert [item["scope"] for item in result.evaluations] == ["child", "child", "synthesis"]
        assert result.bandit_state["generation"] == 1  # type: ignore[index]
        selection_calls = [
            arguments
            for name, arguments in workspace.calls
            if name == "brain_model_select_contextual"
        ]
        assert len(selection_calls) >= 3
        assert any(
            arguments.get("contextual_observations")
            or arguments.get("observations")
            for arguments in selection_calls[1:]
        )
        assert b"cross-domain-learning-secret" not in (tmp_path / "cross-domain-learning.sqlite3").read_bytes()
        assert "cross-domain-learning-secret" not in json.dumps(result.to_dict())
    finally:
        memory.close()
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_cross_domain_trajectory_learning_credits_children_and_synthesis(tmp_path: Path):
    runtime, store, server, thread = _runtime()
    workspace = _Workspace()
    memory = BrainEpisodicMemory(tmp_path / "cross-domain-trajectory.sqlite3")
    handle = store.register("openai", "cross-domain-trajectory-secret")
    evaluator = BrainOutcomeEvaluator(
        lambda _input: {"reward": 0.6, "passed": True, "failed": False},
        evaluator_id="cross-domain-trajectory-quality",
        evaluator_version="1",
    )
    try:
        agent = AutonomousAgent(workspace, runtime, memory=memory)
        result = agent.run_cross_domain_trajectory_learning(
            task="coordinate a delayed-credit engineering and data review",
            subtasks=[
                {"id": "engineering", "task": "review implementation risks", "domain": "coding"},
                {"id": "data", "task": "review lineage risks", "domain": "data"},
            ],
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            evaluator=evaluator,
            trajectory_discount=0.5,
            trajectory_terminal_reward=0.25,
            bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
        )
        assert result.status == "completed"
        assert len(result.evaluations) == 3
        assert len(result.trajectory_result.credited_rewards) == 3
        assert result.trajectory_result.credited_rewards[0] >= result.trajectory_result.credited_rewards[-1]
        assert "cross-domain-trajectory-secret" not in json.dumps(result.to_dict())
    finally:
        memory.close()
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


def test_run_workflow_trajectory_learning_assigns_terminal_credit_after_stages():
    runtime, store, server, thread = _structured_runtime()
    workspace = _Workspace()
    handle = store.register("openai", "workflow-trajectory-secret")
    brain = AutonomousBrain(workspace, runtime)
    try:
        blueprint = brain.prepare_autonomous(
            task="Produce a staged trajectory-learning implementation review.",
            domain="coding",
        )
        result = brain.run_workflow_trajectory_learning(
            blueprint=blueprint,
            model_candidates=_model(),
            credentials={"openai": handle},
            approve_provider_call=True,
            run_id="workflow-trajectory",
            max_stage_calls=2,
            trajectory_discount=0.5,
            trajectory_terminal_reward=0.25,
            stage_evidence={
                "scope": {"signals": {"schema_valid": True}},
                "inspect": {"signals": {"evidence_complete": True}},
            },
            bandit_state={
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [],
            },
        )
        assert result.status == "paused"
        assert result.trajectory_result is not None
        assert len(result.trajectory_result.credited_rewards) == 2
        assert [item.recording["trajectory_step"] for item in result.evaluations] == [0, 1]
        assert "workflow-trajectory-secret" not in json.dumps(result.to_dict())
    finally:
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


def test_durable_cross_domain_worker_resumes_children_and_synthesis_across_restart(tmp_path: Path):
    runtime, credentials, server, thread = _runtime()
    handle = credentials.register("openai", "durable-cross-domain-secret")
    brain = AutonomousBrain(_Workspace(), runtime)
    job_path = tmp_path / "durable-cross-domain.sqlite3"
    task = "Execute a restart-safe engineering and data review package."
    subtasks = [
        {"id": "engineering", "task": "Review implementation risk.", "domain": "coding"},
        {"id": "data", "task": "Review data lineage risk.", "domain": "data"},
    ]
    blueprint = brain.prepare_cross_domain(task=task, subtasks=subtasks)
    caller_results: dict[str, object] = {}
    try:
        packet = {
            "idempotency_key": "durable-cross-domain-review",
            "spec_digest": "d" * 64,
            "domain": "cross_domain",
            "capability": "cross_domain_synthesis",
            "risk_class": "review",
            "max_attempts": 8,
        }

        def resolve(metadata: dict[str, object]) -> dict[str, object]:
            serialized = json.dumps(metadata)
            assert task not in serialized
            assert "durable-cross-domain-secret" not in serialized
            record_checkpoint = metadata.get("checkpoint", {})
            checkpoint = record_checkpoint if isinstance(record_checkpoint, dict) else {}
            checkpoint = checkpoint.get("cross_domain_checkpoint")
            completed: dict[str, object] = {}
            if isinstance(checkpoint, dict):
                for child_id in checkpoint.get("completed_child_ids", []):
                    if child_id in caller_results:
                        completed[child_id] = caller_results[child_id]
            return {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "completed_child_results": completed,
                "cross_domain_options": {"approve_provider_call": True},
            }

        with BrainJobStore(job_path) as store:
            job, _ = store.submit(packet)
            worker = BrainWorker(
                brain,
                store,
                worker_id="cross-domain-worker-a",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="cross_domain",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            first = worker.run_once(job.job_id)
            assert first is not None and first.status == "queued"
            assert first.workflow is not None
            caller_results[first.workflow.item_id] = first.workflow.result
            first_record = store.get(job.job_id)
            assert first_record is not None
            assert first_record.checkpoint["job_kind"] == "autonomous_cross_domain"
            assert first_record.checkpoint["completed_child_ids"] == ["engineering"]
            checkpoint = AutonomousCrossDomainCheckpoint.from_dict(first_record.checkpoint["cross_domain_checkpoint"])
            assert checkpoint.next_child_id == "data"
            assert checkpoint.child_result_digests == {"engineering": first.workflow.child_result_digests["engineering"]}
            assert task not in json.dumps(first_record.to_dict())
            assert "durable-cross-domain-secret" not in json.dumps(first_record.to_dict())

        with BrainJobStore(job_path) as reopened:
            restarted = BrainWorker(
                brain,
                reopened,
                worker_id="cross-domain-worker-b",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="cross_domain",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            second = restarted.run_once(job.job_id)
            assert second is not None and second.status == "queued"
            assert second.workflow is not None
            caller_results[second.workflow.item_id] = second.workflow.result
            third = restarted.run_once(job.job_id)
            assert third is not None and third.status == "succeeded"
            assert third.workflow is not None
            assert third.workflow.phase == "synthesis"
            final = reopened.get(job.job_id)
            assert final is not None and final.state == "succeeded"
            metadata = final.checkpoint["result_metadata"]
            assert metadata["completed_child_ids"] == ["engineering", "data"]
            assert metadata["synthesis_result_digest"] == third.workflow.result.outcome_digest
            assert task not in json.dumps(final.to_dict())
            assert "durable-cross-domain-secret" not in json.dumps(final.to_dict())
            assert reopened.verify_integrity()["ok"] is True
            memory = BrainEpisodicMemory(tmp_path / "durable-cross-domain-learning.sqlite3")
            try:
                settled = AutonomousAgent(_Workspace(), runtime, memory=memory).settle_cross_domain_trajectory_learning(
                    cross_domain=AutonomousCrossDomainResult(
                        status="completed",
                        blueprint=blueprint,
                        child_results=tuple(caller_results[child_id] for child_id in ("engineering", "data")),  # type: ignore[arg-type]
                        synthesis_result=third.workflow.result,
                        execution_child_ids=third.workflow.execution_child_ids,
                    ),
                    bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                    evaluator=BrainOutcomeEvaluator(
                        lambda _input: {"reward": 0.7, "passed": True, "failed": False},
                        evaluator_id="durable-cross-domain-quality",
                        evaluator_version="1",
                    ),
                    trajectory_discount=0.5,
                )
                assert len(settled.evaluations) == 3
                assert settled.trajectory_result.credited_rewards[0] >= settled.trajectory_result.credited_rewards[-1]
            finally:
                memory.close()
    finally:
        server.shutdown()
        thread.join(timeout=2)


def test_durable_cross_domain_worker_parks_and_releases_provider_approval(tmp_path: Path):
    runtime, credentials, server, thread = _runtime()
    handle = credentials.register("openai", "durable-cross-domain-approval-secret")
    brain = AutonomousBrain(_Workspace(), runtime)
    blueprint = brain.prepare_cross_domain(
        task="Prepare an approved cross-domain review.",
        subtasks=[
            {"id": "engineering", "task": "Review implementation risk.", "domain": "coding"},
            {"id": "data", "task": "Review lineage risk.", "domain": "data"},
        ],
    )
    approved = False
    try:
        def resolve(metadata: dict[str, object]) -> dict[str, object]:
            record_checkpoint = metadata.get("checkpoint", {})
            checkpoint = record_checkpoint if isinstance(record_checkpoint, dict) else {}
            completed = {}
            nested = checkpoint.get("cross_domain_checkpoint")
            if isinstance(nested, dict):
                assert nested.get("completed_child_ids") == []
            return {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "completed_child_results": completed,
                "cross_domain_options": {"approve_provider_call": approved},
            }

        with BrainJobStore(tmp_path / "cross-domain-approval.sqlite3") as store:
            job, _ = store.submit(
                {
                    "idempotency_key": "cross-domain-approval",
                    "spec_digest": "e" * 64,
                    "domain": "cross_domain",
                    "capability": "cross_domain_synthesis",
                    "risk_class": "review",
                    "max_attempts": 4,
                }
            )
            worker = BrainWorker(
                brain,
                store,
                worker_id="cross-domain-approval-worker",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="cross_domain",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            waiting = worker.run_once(job.job_id)
            assert waiting is not None and waiting.status == "waiting_approval"
            record = store.get(job.job_id)
            assert record is not None and record.state == "waiting_approval"
            assert record.checkpoint["cross_domain_status"] == "approval_required"
            assert record.checkpoint["next_child_id"] == "engineering"
            assert BrainApprovalRouter(store).get(job.job_id) is not None
            approved = True
            BrainApprovalRouter(store).approve(job.job_id, approver="operator-1")
            resumed = worker.run_once(job.job_id)
            assert resumed is not None and resumed.status == "queued"
            assert resumed.workflow is not None and resumed.workflow.item_id == "engineering"
            assert "durable-cross-domain-approval-secret" not in json.dumps(resumed.job)
    finally:
        server.shutdown()
        thread.join(timeout=2)


def test_durable_workflow_worker_rehydrates_accepted_plan_refinement_across_restart(tmp_path: Path):
    runtime, credentials, server, thread = _structured_runtime()
    handle = credentials.register("openai", "durable-accepted-plan-secret")
    workspace = _Workspace()
    brain = AutonomousBrain(workspace, runtime)
    job_path = tmp_path / "durable-accepted-plan.sqlite3"
    try:
        blueprint = brain.prepare_autonomous(
            task="Execute a restart-safe staged implementation review with an accepted plan.",
            domain="coding",
        )
        stage_ids = tuple(stage.id for stage in blueprint.workflow.stages)
        refinement = AutonomousPlanRefinementResult(
            status="completed",
            task_digest=blueprint.spec.task_digest,
            base_plan_digest=content_digest(blueprint.plan),
            workflow_digest=blueprint.workflow.workflow_digest,
            priority_stage_ids=stage_ids,
            focus_stage_ids=(stage_ids[1],),
            review_required=False,
            confidence=1.0,
        )
        refinement_digest = content_digest(refinement.to_dict())
        stage_evidence = {
            stage.id: {"signals": {signal: True for signal in stage.evaluator_signals}}
            for stage in blueprint.workflow.stages
        }
        packet = {
            "idempotency_key": "durable-accepted-plan-review",
            "spec_digest": "c" * 64,
            "domain": "coding",
            "capability": "implementation_review",
            "risk_class": "review",
            "max_attempts": 8,
        }

        def resolve(metadata: dict[str, object]) -> dict[str, object]:
            recorded = metadata.get("accepted_plan_refinement_digest")
            assert recorded in {None, refinement_digest}
            assert "restart-safe staged implementation review" not in json.dumps(metadata)
            return {
                "blueprint": blueprint,
                "model_candidates": _model(),
                "credentials": {"openai": handle},
                "workflow_options": {
                    "approve_provider_call": True,
                    "stage_evidence": stage_evidence,
                    "accepted_plan_refinement": refinement,
                },
            }

        with BrainJobStore(job_path) as store:
            job, _ = store.submit(packet)
            worker = BrainWorker(
                brain,
                store,
                worker_id="accepted-plan-worker-a",
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
            assert first.workflow.workflow.checkpoint.plan_refinement_digest == refinement_digest
            first_record = store.get(job.job_id)
            assert first_record is not None
            assert first_record.checkpoint["accepted_plan_refinement_digest"] == refinement_digest

        with BrainJobStore(job_path) as reopened:
            restarted = BrainWorker(
                brain,
                reopened,
                worker_id="accepted-plan-worker-b",
                resolver=resolve,
                evaluator=None,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                execution_kind="workflow_learning",
                lease_seconds=10,
                heartbeat_seconds=0.1,
            )
            results = [restarted.run_once(job.job_id) for _ in range(len(stage_ids) - 1)]
            assert all(result is not None for result in results)
            assert results[-1] is not None
            assert results[-1].status == "succeeded"
            final = reopened.get(job.job_id)
            assert final is not None
            assert final.state == "succeeded"
            assert final.checkpoint["result_metadata"]["accepted_plan_refinement_digest"] == refinement_digest
            serialized = json.dumps(final.to_dict())
            assert "restart-safe staged implementation review" not in serialized
            assert "durable-accepted-plan-secret" not in serialized
            assert reopened.verify_integrity()["ok"] is True
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
