from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
import threading

from prism_sdk import (
    AutonomousBrain,
    AutonomousDomainRegistry,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    CredentialStore,
    LLMRuntime,
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
            "capabilities": ["reasoning", "code", "science", "data", "web"],
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
        blueprint = AutonomousBrain(_Workspace(), runtime).prepare_autonomous(
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
            "autonomy-constraints",
            "autonomy-desired-outputs",
            "autonomy-user-context",
        ]
        assert blueprint.plan["steps"][0]["effect"] == "provider_call"
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


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
