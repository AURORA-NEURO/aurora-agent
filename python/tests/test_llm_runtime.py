from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from tempfile import TemporaryDirectory
import threading
import unittest

from prism_sdk.llm_runtime import (
    CredentialError,
    CredentialStore,
    LLMRuntime,
    ProviderError,
    ProviderOnboarding,
    ProviderRequest,
    ProviderTool,
    anthropic_provider,
    openai_compatible_provider,
    openai_provider,
)
from prism_sdk.brain import AutonomousBrain, BrainLearningLedger, BrainRunError, BrainRunResult


class _ProviderHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib handler protocol
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        self.server.seen_headers = {key.lower(): value for key, value in self.headers.items()}  # type: ignore[attr-defined]
        self.server.seen_body = body  # type: ignore[attr-defined]
        if self.path == "/failure":
            payload = b'{"error":"authorization secret-super-secret was rejected"}'
            self.send_response(401)
        elif self.path == "/unavailable":
            payload = b'{"error":"temporarily unavailable"}'
            self.send_response(503)
        elif self.path == "/flaky":
            self.server.flaky_calls = getattr(self.server, "flaky_calls", 0) + 1  # type: ignore[attr-defined]
            if self.server.flaky_calls < 3:  # type: ignore[attr-defined]
                payload = b'{"error":"temporarily unavailable"}'
                self.send_response(503)
            else:
                payload = b'{"id":"resp_flaky","model":"test-model","output_text":"hello","usage":{"total_tokens":3}}'
                self.send_response(200)
        elif self.path == "/json":
            payload = b'{"id":"resp_json","model":"test-model","output_text":"{\\"answer\\":\\"yes\\",\\"score\\":1}","usage":{"total_tokens":3}}'
            self.send_response(200)
        elif self.path == "/mission":
            mission_text = json.dumps(
                {
                    "mission": {
                        "steps": [
                            {
                                "id": "inspect",
                                "domain": "engineering",
                                "capability": "platform_status",
                                "objective": "inspect the bounded platform state",
                                "tool": "developer_platform_status",
                                "arguments": {},
                            }
                        ]
                    }
                },
                separators=(",", ":"),
            )
            payload = json.dumps(
                {
                    "id": "resp_mission",
                    "model": "test-model",
                    "output_text": mission_text,
                    "usage": {"total_tokens": 8},
                },
                separators=(",", ":"),
            ).encode()
            self.send_response(200)
        elif self.path == "/tool":
            payload = json.dumps(
                {
                    "id": "resp_tool",
                    "model": "test-model",
                    "output": [
                        {
                            "type": "function_call",
                            "call_id": "call-1",
                            "name": "developer_platform_status",
                            "arguments": '{"scope":"workspace"}',
                        }
                    ],
                    "usage": {"total_tokens": 6},
                },
                separators=(",", ":"),
            ).encode()
            self.send_response(200)
        else:
            payload = b'{"id":"resp_test","model":"test-model","output_text":"hello","usage":{"total_tokens":3}}'
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("X-Request-Id", "request-test")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_args: object) -> None:
        return


class LlmRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = HTTPServer(("127.0.0.1", 0), _ProviderHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base_url = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.thread.join(timeout=2)
        cls.server.server_close()

    def test_user_key_becomes_only_an_opaque_in_memory_handle(self) -> None:
        store = CredentialStore()
        handle = store.register("openai", "super-secret", ttl_seconds=60)
        metadata = store.metadata(handle)
        self.assertNotIn("super-secret", repr(handle))
        self.assertNotIn("super-secret", json.dumps(metadata))
        self.assertEqual(metadata["secret_persistence"], "in_memory_only")

    def test_prompt_path_is_injectable_for_no_echo_ui_and_tests(self) -> None:
        store = CredentialStore()
        handle = store.prompt("anthropic", reader=lambda prompt: "typed-secret")
        self.assertEqual(handle.provider, "anthropic")
        self.assertNotIn("typed-secret", repr(handle))

    def test_provider_onboarding_exposes_redacted_byok_lifecycle(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        onboarding = ProviderOnboarding(runtime)
        onboarding.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True)
        )
        before = onboarding.status("openai")
        self.assertFalse(before["ready"])
        self.assertEqual(before["next_action"], "collect_user_credential")

        prompt_handle = onboarding.configure_from_prompt(
            "openai",
            reader=lambda _prompt: "prompt-secret",
            ttl_seconds=60,
        )
        ready = onboarding.status("openai")
        self.assertTrue(ready["ready"])
        self.assertEqual(ready["credential"]["credential_count"], 1)
        self.assertNotIn("prompt-secret", json.dumps(ready))
        self.assertNotIn("prompt-secret", json.dumps(store.metadata(prompt_handle)))

        onboarding.revoke(prompt_handle)
        self.assertFalse(onboarding.status("openai")["ready"])

        environment_handle = onboarding.configure_from_environment(
            "openai",
            environ={"OPENAI_API_KEY": "environment-secret"},
        )
        self.assertEqual(store.metadata(environment_handle)["source"], "environment")

        references: list[str] = []
        resolver_handle = onboarding.configure_from_resolver(
            "openai",
            "secret-manager://workspace/openai",
            lambda reference: references.append(reference) or "resolver-secret",
        )
        self.assertEqual(references, ["secret-manager://workspace/openai"])
        self.assertEqual(store.metadata(resolver_handle)["source"], "external_resolver")
        self.assertNotIn("resolver-secret", json.dumps(onboarding.status("openai")))

    def test_openai_responses_call_resolves_secret_only_into_auth_header(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True)
        )
        handle = store.register("openai", "super-secret")
        response = runtime.invoke(
            "openai",
            ProviderRequest(
                model="test-model",
                messages=({"role": "user", "content": "hello"},),
            ),
            credential=handle,
        )
        self.assertEqual(response.text, "hello")
        self.assertEqual(response.request_id, "request-test")
        self.assertEqual(self.server.seen_headers["authorization"], "Bearer super-secret")  # type: ignore[attr-defined]
        self.assertNotIn(b"super-secret", self.server.seen_body)  # type: ignore[attr-defined]

    def test_provider_native_tool_calls_are_typed_and_never_executed(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/tool")
        )
        handle = store.register("openai", "super-secret")
        tool = ProviderTool.from_mcp_schema(
            {
                "name": "developer_platform_status",
                "description": "Read bounded platform status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"scope": {"type": "string"}},
                    "additionalProperties": False,
                },
            }
        )
        response = runtime.invoke(
            "openai",
            ProviderRequest(
                model="test-model",
                messages=({"role": "user", "content": "inspect"},),
                tools=(tool,),
                tool_choice="auto",
            ),
            credential=handle,
        )
        self.assertEqual(response.text, "")
        self.assertEqual(len(response.tool_calls), 1)
        self.assertEqual(response.tool_calls[0].name, "developer_platform_status")
        self.assertEqual(response.tool_calls[0].arguments, {"scope": "workspace"})
        self.assertEqual(response.to_dict()["tool_calls"][0]["execution"], "not_started")  # type: ignore[index]
        body = json.loads(self.server.seen_body)  # type: ignore[attr-defined]
        self.assertEqual(body["tools"][0]["name"], "developer_platform_status")
        self.assertEqual(body["tool_choice"], "auto")

        chat_config = openai_compatible_provider(
            "local",
            self.base_url,
            allow_insecure_http=True,
        )
        anthropic_config = anthropic_provider(
            base_url=self.base_url,
            allow_insecure_http=True,
        )
        chat_body = LLMRuntime._body(
            chat_config,
            ProviderRequest(model="m", messages=(), tools=(tool,), tool_choice="required"),
        )
        anthropic_body = LLMRuntime._body(
            anthropic_config,
            ProviderRequest(model="m", messages=(), tools=(tool,), tool_choice="auto"),
        )
        self.assertEqual(chat_body["tools"][0]["function"]["name"], "developer_platform_status")
        self.assertEqual(anthropic_body["tools"][0]["input_schema"]["type"], "object")
        with self.assertRaisesRegex(ProviderError, "unrequested tool call"):
            runtime.invoke(
                "openai",
                ProviderRequest(
                    model="test-model",
                    messages=({"role": "user", "content": "inspect"},),
                    tools=(ProviderTool("other_tool"),),
                ),
                credential=handle,
            )

    def test_revocation_and_provider_mismatch_fail_closed(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            anthropic_provider(base_url=self.base_url, allow_insecure_http=True)
        )
        handle = store.register("openai", "super-secret")
        with self.assertRaises(CredentialError):
            runtime.invoke(
                "anthropic",
                ProviderRequest(
                    model="test-model",
                    messages=({"role": "user", "content": "hello"},),
                ),
                credential=handle,
            )
        store.revoke(handle)
        with self.assertRaises(CredentialError):
            store.metadata(handle)

    def test_provider_error_does_not_echo_secret_bearing_error_body(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(
                base_url=self.base_url,
                allow_insecure_http=True,
                # The fake server chooses its error path; this is still a normal provider config.
                path="/failure",
            )
        )
        handle = store.register("openai", "super-secret")
        with self.assertRaisesRegex(ProviderError, r"HTTP status 401") as context:
            runtime.invoke(
                "openai",
                ProviderRequest(
                    model="test-model",
                    messages=({"role": "user", "content": "hello"},),
                ),
                credential=handle,
            )
        self.assertNotIn("super-secret", str(context.exception))

    def test_retries_transient_failures_and_resets_the_circuit_on_success(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store, sleeper=lambda _seconds: None)
        runtime.register_provider(
            openai_provider(
                base_url=self.base_url,
                allow_insecure_http=True,
                path="/flaky",
                max_attempts=3,
                circuit_breaker_failure_threshold=2,
            )
        )
        handle = store.register("openai", "super-secret")
        response = runtime.invoke(
            "openai",
            ProviderRequest(model="test-model", messages=({"role": "user", "content": "hello"},)),
            credential=handle,
        )
        self.assertEqual(response.text, "hello")
        self.assertEqual(self.server.flaky_calls, 3)  # type: ignore[attr-defined]
        self.assertEqual(runtime.provider_status("openai")["circuit"], "closed")

    def test_circuit_breaker_refuses_after_bounded_transient_failures(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store, sleeper=lambda _seconds: None)
        runtime.register_provider(
            openai_provider(
                base_url=self.base_url,
                allow_insecure_http=True,
                path="/unavailable",
                max_attempts=1,
                circuit_breaker_failure_threshold=2,
            )
        )
        handle = store.register("openai", "super-secret")
        request = ProviderRequest(model="test-model", messages=({"role": "user", "content": "hello"},))
        for _ in range(2):
            with self.assertRaisesRegex(ProviderError, r"HTTP status 503"):
                runtime.invoke("openai", request, credential=handle)
        with self.assertRaisesRegex(ProviderError, r"circuit is open") as context:
            runtime.invoke("openai", request, credential=handle)
        self.assertTrue(context.exception.circuit_open)

    def test_structured_output_is_parsed_and_validated_locally(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store, sleeper=lambda _seconds: None)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/json")
        )
        handle = store.register("openai", "super-secret")
        response = runtime.invoke(
            "openai",
            ProviderRequest(
                model="test-model",
                messages=({"role": "user", "content": "json"},),
                require_json=True,
                response_schema={
                    "type": "object",
                    "required": ["answer", "score"],
                    "properties": {"answer": {"type": "string"}, "score": {"type": "integer"}},
                    "additionalProperties": False,
                },
            ),
            credential=handle,
        )
        self.assertEqual(response.structured, {"answer": "yes", "score": 1})

    def test_autonomous_brain_routes_structured_decision_through_mission_approval(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "plan"}], "prompt_digest": "p"}
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "plan",
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    self.missions.append(arguments)
                    execute = arguments.get("policy", {}).get("execute", False)  # type: ignore[union-attr]
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "executed" if execute else "planned",
                        "mission_status": "succeeded" if execute else "planned",
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/mission")
        )
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        kwargs = {
            "task": "inspect the platform",
            "model_selection": {
                "input_tokens": 10,
                "requested_output_tokens": 100,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            "prompt": {"max_input_tokens": 100},
            "plan": {"allowed_tools": ["provider.invoke"], "max_cost": 10},
            "credentials": {"openai": handle},
            "mission_policy": {
                "allowed_tools": ["developer_platform_status"],
                "max_steps": 4,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            "approve_provider_call": True,
        }
        preview = AutonomousBrain(workspace, runtime).run_mission(**kwargs)
        self.assertEqual(preview.status, "mission_approval_required")
        self.assertIsNone(preview.execution)
        self.assertEqual(len(workspace.missions), 1)
        self.assertFalse(workspace.missions[0]["policy"]["execute"])  # type: ignore[index]
        self.assertEqual(workspace.missions[0]["policy"]["allowed_tools"], ["developer_platform_status"])  # type: ignore[index]

        dispatched = AutonomousBrain(workspace, runtime).run_mission(
            **kwargs,
            approve_mission_dispatch=True,
        )
        self.assertEqual(dispatched.status, "mission_dispatched")
        self.assertEqual(len(workspace.missions), 3)
        self.assertTrue(workspace.missions[-1]["policy"]["execute"])  # type: ignore[index]
        self.assertNotIn("super-secret", json.dumps(dispatched.to_dict()))

    def test_routed_mission_uses_live_catalogue_and_narrows_policy(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.route_arguments: dict[str, object] | None = None
                self.prompt_arguments: dict[str, object] | None = None
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "capability_route":
                    self.route_arguments = arguments
                    return {
                        "ok": True,
                        "workflow": "capability_route",
                        "route_id": "r" * 64,
                        "catalog_digest": "c" * 64,
                        "goal": "inspect the platform",
                        "unresolved_needs": [],
                        "recommended_tools": ["developer_platform_status"],
                        "needs": [
                            {
                                "id": "task",
                                "resolution": "ranked_candidates",
                                "candidate_groups": ["developer_platform"],
                                "candidate_domains": ["engineering"],
                                "candidate_tools": ["developer_platform_status"],
                            }
                        ],
                        "tool_schemas": [
                            {
                                "name": "developer_platform_status",
                                "inputSchema": {"type": "object", "properties": {}},
                            }
                        ],
                        "schema_attachment": {"requested": True, "returned": 1, "missing": []},
                    }
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    self.prompt_arguments = arguments
                    context = arguments["context"]  # type: ignore[index]
                    assert any(
                        chunk["id"] == "capability-route"  # type: ignore[index]
                        for chunk in context  # type: ignore[union-attr]
                    )
                    return {"messages": [{"role": "user", "content": "plan"}], "prompt_digest": "p"}
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "plan",
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    self.missions.append(arguments)
                    execute = arguments.get("policy", {}).get("execute", False)  # type: ignore[union-attr]
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "mission_status": "succeeded" if execute else "planned",
                        "execution": "executed" if execute else "planned",
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/mission")
        )
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_mission(
            task="inspect the platform",
            model_selection={
                "input_tokens": 10,
                "requested_output_tokens": 100,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            mission_policy={
                "allowed_tools": ["developer_platform_status", "onco_response_assess"],
                "max_steps": 4,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            route_request={"needs": [{"id": "task", "query": "inspect platform"}]},
            enforce_route_tools=True,
            approve_provider_call=True,
        )
        self.assertEqual(result.status, "mission_approval_required")
        self.assertIsNotNone(result.route)
        self.assertEqual(workspace.route_arguments["include_tools"], True)  # type: ignore[index]
        self.assertEqual(
            workspace.missions[0]["policy"]["allowed_tools"],  # type: ignore[index]
            ["developer_platform_status"],
        )
        self.assertNotIn("super-secret", json.dumps(result.to_dict()))

    def test_routed_mission_refuses_unresolved_needs_before_provider_call(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.provider_selected = False

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "capability_route":
                    return {
                        "ok": True,
                        "workflow": "capability_route",
                        "route_id": "r" * 64,
                        "catalog_digest": "c" * 64,
                        "goal": "unknown task",
                        "unresolved_needs": ["task"],
                        "recommended_tools": [],
                        "needs": [
                            {
                                "id": "task",
                                "resolution": "unresolved",
                                "candidate_groups": [],
                                "candidate_domains": [],
                                "candidate_tools": [],
                            }
                        ],
                        "tool_schemas": [],
                    }
                if name == "brain_model_select":
                    self.provider_selected = True
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        handle = store.register("openai", "super-secret")
        with self.assertRaisesRegex(BrainRunError, "unresolved needs"):
            AutonomousBrain(Workspace(), runtime).run_mission(
                task="unknown task",
                model_selection={"models": [{"provider": "openai", "model": "test-model"}]},
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
                credentials={"openai": handle},
                mission_policy={"allowed_tools": ["developer_platform_status"]},
                route_request={},
                approve_provider_call=True,
            )

    def test_provider_tool_intent_is_converted_to_mission_preflight(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "capability_route":
                    return {
                        "ok": True,
                        "workflow": "capability_route",
                        "route_id": "r" * 64,
                        "catalog_digest": "c" * 64,
                        "goal": "inspect the platform",
                        "unresolved_needs": [],
                        "recommended_tools": ["developer_platform_status"],
                        "needs": [
                            {
                                "id": "task",
                                "resolution": "explicit",
                                "candidate_groups": ["developer_platform"],
                                "candidate_domains": ["engineering"],
                                "candidate_tools": ["developer_platform_status"],
                            }
                        ],
                        "tool_schemas": [
                            {
                                "name": "developer_platform_status",
                                "description": "Read bounded platform status.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {"scope": {"type": "string"}},
                                },
                            }
                        ],
                    }
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "inspect"}], "prompt_digest": "p"}
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "plan",
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    self.missions.append(arguments)
                    return {"ok": True, "workflow": "agent_mission", "mission_status": "planned"}
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/tool")
        )
        handle = store.register("openai", "super-secret")
        result = AutonomousBrain(Workspace(), runtime).run_mission(
            task="inspect the platform",
            model_selection={"models": [{"provider": "openai", "model": "test-model"}]},
            prompt={"max_input_tokens": 1_000},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            mission_policy={
                "allowed_tools": ["developer_platform_status"],
                "max_steps": 2,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            route_request={"needs": [{"id": "task", "tool": "developer_platform_status"}]},
            enforce_route_tools=True,
            approve_provider_call=True,
        )
        self.assertEqual(result.status, "mission_approval_required")
        self.assertEqual(result.mission["steps"][0]["tool"], "developer_platform_status")  # type: ignore[index]
        self.assertEqual(result.mission["steps"][0]["arguments"], {"scope": "workspace"})  # type: ignore[index]
        self.assertIsNone(result.execution)

    def test_autonomous_loop_stops_at_approval_before_provider_effect(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "hello"}], "prompt_digest": "p"}
                return {
                    "ok": True,
                    "plan": {
                        "requires_approval": True,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "plan",
                    },
                }

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        handle = store.register("openai", "super-secret")
        result = AutonomousBrain(Workspace(), runtime).run(
            task="hello",
            model_selection={
                "input_tokens": 10,
                "requested_output_tokens": 10,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
        )
        self.assertEqual(result.status, "approval_required")
        self.assertIsNone(result.response)

    def test_autonomous_loop_invokes_only_after_explicit_provider_approval(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "hello"}], "prompt_digest": "p"}
                return {
                    "ok": True,
                    "plan": {
                        "requires_approval": True,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "plan",
                    },
                }

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        handle = store.register("openai", "super-secret")
        result = AutonomousBrain(Workspace(), runtime).run(
            task="hello",
            model_selection={
                "input_tokens": 10,
                "requested_output_tokens": 10,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            approve_provider_call=True,
        )
        self.assertEqual(result.status, "completed_provider_call")
        self.assertIsNotNone(result.response)
        self.assertEqual(result.response.text, "hello")

    def test_autonomous_loop_exposes_bounded_structured_provider_contract(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "json"}], "prompt_digest": "p"}
                return {
                    "ok": True,
                    "plan": {
                        "requires_approval": True,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "plan",
                    },
                }

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/json")
        )
        handle = store.register("openai", "super-secret")
        result = AutonomousBrain(Workspace(), runtime).run(
            task="json",
            model_selection={
                "input_tokens": 10,
                "requested_output_tokens": 10,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            approve_provider_call=True,
            max_output_tokens=128,
            require_json=True,
            response_schema={
                "type": "object",
                "required": ["answer"],
                "properties": {"answer": {"type": "string"}},
            },
            idempotency_key="brain-run-1",
        )
        self.assertEqual(result.status, "completed_provider_call")
        self.assertEqual(result.response.structured["answer"], "yes")  # type: ignore[union-attr]
        self.assertEqual(json.loads(self.server.seen_body)["max_output_tokens"], 128)  # type: ignore[attr-defined]
        self.assertEqual(self.server.seen_headers["idempotency-key"], "brain-run-1")  # type: ignore[attr-defined]

    def test_autonomous_loop_uses_contextual_model_selection_and_keeps_context_identity(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.contextual_arguments: dict[str, object] | None = None

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select_contextual":
                    self.contextual_arguments = arguments
                    return {
                        "schema": "bioprism-brain-contextual-model-selection/0.1",
                        "context_digest": "c" * 64,
                        "selection_status": "contextual_selection_mixed_history",
                        "selection": {
                            "selected_model": {"provider": "openai", "model": "test-model"},
                            "decision_digest": "a" * 64,
                        },
                    }
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "hello"}], "prompt_digest": "b" * 64}
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "c" * 64,
                        },
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run(
            task="hello",
            model_selection={
                "input_tokens": 10,
                "requested_output_tokens": 10,
                "models": [{"provider": "openai", "model": "test-model"}],
            },
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            approve_provider_call=True,
            context={"domain": "engineering", "capability": "platform_status", "risk_class": "low"},
            contextual_observations=[
                {"context_digest": "c" * 64, "arm_id": "openai/test-model", "pulls": 2, "reward_sum": 1.5}
            ],
        )
        self.assertEqual(result.status, "completed_provider_call")
        self.assertEqual(result.selection["context_digest"], "c" * 64)
        self.assertEqual(workspace.contextual_arguments["context"]["domain"], "engineering")  # type: ignore[index]

    def test_evaluator_outcome_is_persisted_without_provider_text_or_credentials(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                self.last = (name, arguments)
                return {
                    "ok": True,
                    "status": "recorded_evaluator_reward",
                    "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                    "learning_evidence": {"schema": "bioprism-brain-learning-evidence/0.1", "evidence_digest": "e" * 64},
                }

        result = BrainRunResult(
            run_id="run-1",
            status="completed_provider_call",
            selection={
                "selected_model": {"provider": "openai", "model": "test-model"},
                "decision_digest": "a" * 64,
                "context_digest": "e" * 64,
            },
            prompt={"prompt_digest": "b" * 64},
            plan={"plan": {"plan_digest": "c" * 64}},
            response=None,
            outcome_digest="d" * 64,
        )
        workspace = Workspace()
        with TemporaryDirectory() as directory:
            ledger = BrainLearningLedger(f"{directory}/learning.jsonl")
            report = AutonomousBrain(workspace, LLMRuntime()).record_evaluator_outcome(
                result,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                evaluator_id="json_contract",
                evaluator_version="1",
                reward=0.8,
                passed=True,
                ledger=ledger,
            )
            self.assertEqual(report["status"], "recorded_evaluator_reward")
            self.assertEqual(ledger.latest_state()["generation"], 1)  # type: ignore[index]
            self.assertEqual(ledger.latest_state("e" * 64)["generation"], 1)  # type: ignore[index]
            self.assertIsNone(ledger.latest_state("f" * 64))
            encoded = json.dumps(ledger.records())
            self.assertNotIn("super-secret", encoded)
            self.assertNotIn("api_key", encoded)


if __name__ == "__main__":
    unittest.main()
