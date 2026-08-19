from __future__ import annotations

import json
import hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory
import threading
import unittest

from prism_sdk.llm_runtime import (
    CredentialError,
    CredentialStore,
    LLMRuntime,
    ProviderHealthLedger,
    ProviderError,
    ProviderOnboarding,
    ProviderConfig,
    ProviderRequest,
    ProviderResponse,
    ProviderStreamEvent,
    ProviderTool,
    ProviderToolCall,
    ProviderToolResult,
    anthropic_provider,
    openai_compatible_provider,
    openai_provider,
)
from prism_sdk.brain import (
    AutonomousBrain,
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    BrainMissionResult,
    BrainToolLoopResult,
    build_brain_evaluation_input,
    MissionToolAuthorizer,
)


class _ProviderHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib handler protocol
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        self.server.request_paths = getattr(self.server, "request_paths", []) + [self.path]  # type: ignore[attr-defined]
        self.server.seen_headers = {key.lower(): value for key, value in self.headers.items()}  # type: ignore[attr-defined]
        self.server.seen_body = body  # type: ignore[attr-defined]
        stream_frames: list[bytes] | None = None
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
        elif self.path == "/stream":
            stream_frames = [
                b'event: response.created\ndata: {"type":"response.created","id":"resp_stream","model":"test-model"}\n\n',
                b'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","delta":"hel"}\n\n',
                b'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","delta":"lo"}\n\n',
                b'event: response.completed\ndata: {"type":"response.completed","response":{"id":"resp_stream","model":"test-model","usage":{"total_tokens":4}}}\n\n',
            ]
            payload = b""
            self.send_response(200)
        elif self.path == "/stream_tool":
            stream_frames = [
                b'event: response.created\ndata: {"type":"response.created","id":"resp_stream_tool","model":"test-model"}\n\n',
                b'event: response.output_item.added\ndata: {"type":"response.output_item.added","item":{"type":"function_call","id":"item-1","call_id":"call-stream-1","name":"developer_platform_status"}}\n\n',
                b'event: response.function_call_arguments.delta\ndata: {"type":"response.function_call_arguments.delta","item_id":"item-1","delta":"{\\"scope\\": "}\n\n',
                b'event: response.function_call_arguments.delta\ndata: {"type":"response.function_call_arguments.delta","item_id":"item-1","delta":"\\"workspace\\"}"}\n\n',
                b'event: response.function_call_arguments.done\ndata: {"type":"response.function_call_arguments.done","item_id":"item-1","arguments":"{\\"scope\\":\\"workspace\\"}"}\n\n',
                b'event: response.completed\ndata: {"type":"response.completed","response":{"id":"resp_stream_tool","model":"test-model","usage":{"total_tokens":6}}}\n\n',
                b'data: [DONE]\n\n',
            ]
            payload = b""
            self.send_response(200)
        elif self.path == "/continue":
            request_body = json.loads(body.decode("utf-8"))
            input_items = request_body.get("input", [])
            has_result = any(
                isinstance(item, dict) and item.get("type") == "function_call_output"
                for item in input_items
            )
            if has_result:
                payload = b'{"id":"resp_final","model":"test-model","output_text":"continued","usage":{"total_tokens":9}}'
            else:
                payload = json.dumps(
                    {
                        "id": "resp_first",
                        "model": "test-model",
                        "output": [
                            {
                                "type": "function_call",
                                "call_id": "call-loop-1",
                                "name": "developer_platform_status",
                                "arguments": "{}",
                            }
                        ],
                        "usage": {"total_tokens": 5},
                    },
                    separators=(",", ":"),
                ).encode()
            self.send_response(200)
        elif self.path == "/continue_fail_after_result":
            request_body = json.loads(body.decode("utf-8"))
            input_items = request_body.get("input", [])
            has_result = any(
                isinstance(item, dict) and item.get("type") == "function_call_output"
                for item in input_items
            )
            if has_result:
                payload = b'{"error":"continuation failed after tool authorization"}'
                self.send_response(503)
            else:
                payload = json.dumps(
                    {
                        "id": "resp_first_fail_after_result",
                        "model": "test-model",
                        "output": [
                            {
                                "type": "function_call",
                                "call_id": "call-loop-fail-after-result",
                                "name": "developer_platform_status",
                                "arguments": "{}",
                            }
                        ],
                        "usage": {"total_tokens": 5},
                    },
                    separators=(",", ":"),
                ).encode()
                self.send_response(200)
        else:
            payload = b'{"id":"resp_test","model":"test-model","output_text":"hello","usage":{"total_tokens":3}}'
            self.send_response(200)
        if stream_frames is not None:
            stream_payload = b"".join(stream_frames)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(stream_payload)))
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            self.wfile.write(stream_payload)
            return
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

    def test_credential_value_is_bounded_before_storage(self) -> None:
        store = CredentialStore()
        with self.assertRaises(CredentialError):
            store.register("openai", "x" * 16_385)

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
        instructions = onboarding.instructions("openai").to_dict()
        self.assertEqual(instructions["next_action"], "collect_user_credential")
        self.assertEqual(instructions["environment_variable"], "OPENAI_API_KEY")
        self.assertIn("protected_ui", instructions["input_methods"])
        self.assertEqual(instructions["secret_material"], "never_returned")

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

        protected_handle = onboarding.collect_user_credential(
            "openai",
            "protected-ui-secret",
            ttl_seconds=60,
        )
        self.assertEqual(store.metadata(protected_handle)["source"], "protected_ui")
        self.assertEqual(onboarding.instructions("openai").to_dict()["next_action"], "ready")
        self.assertNotIn("protected-ui-secret", json.dumps(onboarding.instructions("openai").to_dict()))
        onboarding.revoke(protected_handle)

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

    def test_adaptive_selection_rejects_revoked_handles_before_provider_invocation(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        handle = store.register("openai", "revoked-secret")
        store.revoke(handle)

        selection = AutonomousBrain(object(), runtime).build_adaptive_model_selection(
            task="verify readiness",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 10,
                    "cost_per_million_tokens": 1,
                    "reliability": 0.9,
                }
            ],
            credentials={"openai": handle},
        )

        self.assertFalse(selection["models"][0]["enabled"])  # type: ignore[index]
        self.assertFalse(selection["provider_health"]["openai"]["credential_ready"])  # type: ignore[index]
        self.assertFalse(selection["provider_health"]["openai"]["eligible"])  # type: ignore[index]
        self.assertNotIn("revoked-secret", json.dumps(selection))

    def test_credential_session_groups_handles_and_revokes_on_expiry(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        onboarding = ProviderOnboarding(runtime)
        onboarding.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True))
        now = [100.0]
        session = onboarding.start_session(ttl_seconds=10, session_id="ui-session", clock=lambda: now[0])
        self.assertEqual(session.instructions("openai").next_action, "collect_user_credential")
        handle = session.collect_user_credential("openai", "session-secret")
        self.assertIs(session.handle("openai"), handle)
        self.assertTrue(session.status().active)
        self.assertEqual(session.status().providers, ("openai",))
        self.assertNotIn("session-secret", json.dumps(session.status().to_dict()))
        self.assertNotIn("session-secret", repr(session))
        self.assertEqual(session.provider_statuses()[0]["next_action"], "ready")

        now[0] = 111.0
        self.assertFalse(session.status().active)
        with self.assertRaises(CredentialError):
            session.handle("openai")
        self.assertEqual(store.status("openai").credential_count, 0)

    def test_credentialless_provider_is_ready_without_a_fake_key(self) -> None:
        runtime = LLMRuntime(CredentialStore())
        runtime.register_provider(
            ProviderConfig(
                provider="local",
                base_url=self.base_url,
                allow_insecure_http=True,
                requires_credential=False,
            )
        )
        onboarding = ProviderOnboarding(runtime)
        status = onboarding.status("local")
        self.assertTrue(status["ready"])
        self.assertEqual(status["next_action"], "ready")
        instructions = onboarding.instructions("local")
        self.assertTrue(instructions.ready)
        self.assertEqual(instructions.requires_credential, False)

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

    def test_provider_observer_receives_value_only_success_and_failure_outcomes(self) -> None:
        store = CredentialStore()
        observations: list[dict[str, object]] = []
        runtime = LLMRuntime(store, observation_callback=observations.append)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True)
        )
        handle = store.register("openai", "super-secret")
        runtime.invoke(
            "openai",
            ProviderRequest(model="test-model", messages=({"role": "user", "content": "hello"},)),
            credential=handle,
        )
        runtime.register_provider(
            openai_provider(
                base_url=self.base_url,
                allow_insecure_http=True,
                path="/failure",
                max_attempts=1,
            )
        )
        with self.assertRaisesRegex(ProviderError, r"HTTP status 401"):
            runtime.invoke(
                "openai",
                ProviderRequest(model="test-model", messages=({"role": "user", "content": "hello"},)),
                credential=handle,
            )
        self.assertEqual([event["outcome"] for event in observations], ["success", "failure"])
        self.assertEqual(observations[0]["provider"], "openai")
        self.assertEqual(observations[1]["failure_class"], "provider_error")
        serialized = json.dumps(observations)
        self.assertNotIn("super-secret", serialized)
        self.assertNotIn("hello", serialized)

    def test_provider_health_ledger_persists_restart_safe_circuit_and_latency_metadata(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "provider-health.jsonl"
            ledger = ProviderHealthLedger(path)
            store = CredentialStore()
            runtime = LLMRuntime(store, observation_callback=ledger.record)
            runtime.register_provider(
                openai_provider(base_url=self.base_url, allow_insecure_http=True)
            )
            handle = store.register("openai", "health-secret")
            runtime.invoke(
                "openai",
                ProviderRequest(model="test-model", messages=({"role": "user", "content": "hello"},)),
                credential=handle,
            )
            # The callback path above is live telemetry; this explicit failure models the
            # circuit-opening observation that a thresholded runtime would emit.
            ledger.record(
                {
                    "schema": "bioprism-llm-provider-observation/0.1",
                    "provider": "openai",
                    "model": "test-model",
                    "status": "provider_refused",
                    "outcome": "failure",
                    "latency_ms": 42.5,
                    "observed_at": 100.0,
                    "failure_class": "circuit_open",
                    "circuit": "open",
                    "consecutive_failures": 3,
                    "opened_until": 200.0,
                }
            )
            restored = ProviderHealthLedger(path)
            snapshot = restored.health_snapshot(now=150.0)
            self.assertEqual(snapshot["openai"]["circuit"], "open")
            self.assertEqual(snapshot["openai"]["consecutive_failures"], 3)
            self.assertGreaterEqual(snapshot["openai"]["attempts"], 2)
            self.assertEqual(restored.health_snapshot(now=250.0)["openai"]["circuit"], "closed")
            serialized = json.dumps(restored.to_dict())
            self.assertNotIn("health-secret", serialized)
            self.assertNotIn("hello", serialized)
            with self.assertRaises(ProviderError):
                restored.record(
                    {
                        "schema": "bioprism-llm-provider-observation/0.1",
                        "provider": "openai",
                        "model": "test-model",
                        "status": "completed",
                        "outcome": "success",
                        "latency_ms": 1,
                        "api_key": "must-never-be-accepted",
                    }
                )

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

    def test_sse_stream_projects_text_and_terminal_events_without_raw_payloads(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/stream")
        )
        handle = store.register("openai", "super-secret")
        request = ProviderRequest(
            model="test-model",
            messages=({"role": "user", "content": "hello"},),
        )
        events = list(runtime.invoke_stream("openai", request, credential=handle))
        self.assertTrue(all(isinstance(event, ProviderStreamEvent) for event in events))
        self.assertEqual("".join(event.text_delta for event in events), "hello")
        self.assertTrue(events[-1].done)
        self.assertNotIn("super-secret", json.dumps([event.to_dict() for event in events]))
        response = runtime.collect_stream("openai", request, credential=handle)
        self.assertEqual(response.text, "hello")
        self.assertEqual(response.usage["total_tokens"], 4)
        self.assertEqual(response.raw["stream"], True)

    def test_sse_stream_finalizes_provider_tool_intent_and_validates_allowlist(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/stream_tool")
        )
        handle = store.register("openai", "super-secret")
        tool = ProviderTool("developer_platform_status", parameters={"type": "object"})
        request = ProviderRequest(
            model="test-model",
            messages=({"role": "user", "content": "inspect"},),
            tools=(tool,),
        )
        events = list(runtime.invoke_stream("openai", request, credential=handle))
        finalized = [event.tool_call for event in events if event.tool_call is not None]
        self.assertEqual(len(finalized), 1)
        self.assertEqual(finalized[0].arguments, {"scope": "workspace"})  # type: ignore[union-attr]
        response = runtime.collect_stream("openai", request, credential=handle)
        self.assertEqual(response.tool_calls[0].name, "developer_platform_status")
        with self.assertRaisesRegex(ProviderError, "unrequested streamed tool call"):
            runtime.collect_stream(
                "openai",
                ProviderRequest(
                    model="test-model",
                    messages=({"role": "user", "content": "inspect"},),
                    tools=(ProviderTool("other_tool"),),
                ),
                credential=handle,
            )

    def test_tool_continuation_translates_to_each_provider_wire_shape(self) -> None:
        call = ProviderToolCall(
            call_id="call-1",
            name="developer_platform_status",
            arguments={"scope": "workspace"},
        )
        result = ProviderToolResult(
            call_id="call-1",
            content={"status": "ready"},
            approved=True,
        )
        request = ProviderRequest(
            model="test-model",
            messages=({"role": "user", "content": "inspect"},),
            tools=(ProviderTool("developer_platform_status"),),
        ).with_tool_results((call,), (result,))
        openai_body = LLMRuntime._body(
            openai_provider(base_url=self.base_url, allow_insecure_http=True),
            request,
        )
        chat_body = LLMRuntime._body(
            openai_compatible_provider("local", self.base_url, allow_insecure_http=True),
            request,
        )
        anthropic_body = LLMRuntime._body(
            anthropic_provider(base_url=self.base_url, allow_insecure_http=True),
            request,
        )
        self.assertEqual(openai_body["input"][-2]["type"], "function_call")
        self.assertEqual(openai_body["input"][-1]["type"], "function_call_output")
        self.assertEqual(chat_body["messages"][-2]["tool_calls"][0]["function"]["name"], "developer_platform_status")
        self.assertEqual(chat_body["messages"][-1]["role"], "tool")
        self.assertEqual(anthropic_body["messages"][-2]["content"][0]["type"], "tool_use")
        self.assertEqual(anthropic_body["messages"][-1]["content"][0]["type"], "tool_result")
        with self.assertRaisesRegex(ProviderError, "require caller approval"):
            ProviderRequest(
                model="test-model",
                messages=({"role": "user", "content": "inspect"},),
            ).with_tool_results((call,), (ProviderToolResult("call-1", "no", approved=False),))

    def test_bounded_tool_loop_requires_caller_authorization_and_continues(self) -> None:
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/continue")
        )
        handle = store.register("openai", "super-secret")
        request = ProviderRequest(
            model="test-model",
            messages=({"role": "user", "content": "inspect"},),
            tools=(ProviderTool("developer_platform_status"),),
        )
        result = runtime.invoke_tool_loop(
            "openai",
            request,
            credential=handle,
            authorize_and_execute=lambda calls: [
                ProviderToolResult(
                    call_id=calls[0].call_id,
                    content={"status": "ready"},
                    approved=True,
                )
            ],
            max_turns=3,
        )
        self.assertEqual(result.status, "completed")
        self.assertEqual(result.turns, 2)
        self.assertEqual(result.final_response.text, "continued")  # type: ignore[union-attr]
        continued_body = json.loads(self.server.seen_body)  # type: ignore[attr-defined]
        self.assertEqual(continued_body["input"][-1]["type"], "function_call_output")
        refused = runtime.invoke_tool_loop(
            "openai",
            request,
            credential=handle,
            authorize_and_execute=lambda calls: [
                ProviderToolResult(call_id=calls[0].call_id, content="refused", approved=False)
            ],
        )
        self.assertEqual(refused.status, "authorization_required")
        self.assertEqual(refused.turns, 1)

    def test_mission_tool_authorizer_fails_closed_before_dispatch(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.calls: list[str] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                self.calls.append(name)
                raise AssertionError("invalid route tool must not reach agent_mission")

        authorizer = MissionToolAuthorizer(
            Workspace(),
            task="inspect",
            mission_policy={"allowed_tools": ["developer_platform_status"]},
            route={
                "workflow": "capability_route",
                "goal": "inspect",
                "unresolved_needs": [],
                "recommended_tools": ["other_tool"],
                "needs": [
                    {
                        "id": "task",
                        "candidate_domains": ["engineering"],
                        "candidate_groups": ["developer_platform"],
                        "candidate_tools": ["other_tool"],
                    }
                ],
            },
        )
        result = authorizer(
            (
                ProviderToolCall(
                    "call-1",
                    "developer_platform_status",
                    {},
                ),
            )
        )
        self.assertFalse(result[0].approved)
        self.assertEqual(authorizer.receipts[0].status, "preflight_refused")

    def test_mission_tool_authorizer_requires_dispatch_approval_after_preflight(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                assert name == "agent_mission"
                assert arguments is not None
                self.missions.append(arguments)
                return {
                    "ok": True,
                    "workflow": "agent_mission",
                    "execution": "planned",
                    "mission_status": "planned",
                    "plan": {"digest": "m" * 64, "mission_id": arguments["mission_id"]},
                }

        workspace = Workspace()
        authorizer = MissionToolAuthorizer(
            workspace,
            task="inspect",
            mission_policy={"allowed_tools": ["developer_platform_status"]},
        )
        result = authorizer(
            (ProviderToolCall("call-approval", "developer_platform_status", {}),)
        )
        self.assertFalse(result[0].approved)
        self.assertTrue(result[0].is_error)
        self.assertEqual(len(workspace.missions), 1)
        self.assertFalse(workspace.missions[0]["policy"]["execute"])  # type: ignore[index]
        self.assertEqual(authorizer.receipts[0].status, "approval_required")

    def test_autonomous_brain_uses_route_aware_mission_authorizer_for_all_tool_turns(self) -> None:
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
                        "goal": "inspect",
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
                                "description": "Read bounded platform state.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {"scope": {"type": "string"}},
                                    "additionalProperties": False,
                                },
                            }
                        ],
                        "schema_attachment": {"requested": True, "returned": 1, "missing": []},
                    }
                if name == "brain_model_select":
                    return {"selected_model": {"provider": "openai", "model": "test-model"}}
                if name == "brain_prompt_assemble":
                    assert arguments is not None
                    context = arguments.get("context", [])
                    assert any(chunk.get("id") == "capability-route" for chunk in context)  # type: ignore[union-attr]
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
                    execute = arguments.get("policy", {}).get("execute", False)  # type: ignore[union-attr]
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "executed" if execute else "planned",
                        "mission_status": "succeeded" if execute else "planned",
                        "plan": {"digest": "m" * 64, "mission_id": arguments["mission_id"]},
                        "results": [
                            {
                                "id": arguments["steps"][0]["id"],  # type: ignore[index]
                                "tool": "developer_platform_status",
                                "status": "succeeded",
                                "required": True,
                                "bytes": 24,
                                "wire": {"result": {"structuredContent": {"status": "ready"}}},
                            }
                        ],
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/continue")
        )
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_tool_loop(
            task="inspect",
            model_selection={"models": [{"provider": "openai", "model": "test-model"}]},
            prompt={"max_input_tokens": 2_000},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            mission_policy={
                "allowed_tools": ["developer_platform_status"],
                "max_steps": 2,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            route_request={"needs": [{"id": "task", "query": "inspect"}]},
            approve_provider_call=True,
            approve_mission_dispatch=True,
            max_turns=3,
        )
        self.assertEqual(result.status, "completed_provider_tool_loop")
        self.assertEqual(result.provider_loop.final_response.text, "continued")  # type: ignore[union-attr]

        self.assertEqual(len(workspace.missions), 2)
        self.assertFalse(workspace.missions[0]["policy"]["execute"])  # type: ignore[index]
        self.assertTrue(workspace.missions[1]["policy"]["execute"])  # type: ignore[index]
        self.assertEqual(result.authorization_receipts[0]["status"], "executed")
        self.assertEqual(result.authorization_receipts[0]["execution"]["results"][0]["output"], {"status": "ready"})  # type: ignore[index]
        self.assertNotIn("super-secret", json.dumps(result.to_dict()))

    def test_autonomous_brain_exposes_authorized_native_tool_loop(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
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
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/continue")
        )
        handle = store.register("openai", "super-secret")
        brain = AutonomousBrain(Workspace(), runtime)
        result = brain.run_tool_loop(
            task="inspect",
            model_selection={"models": [{"provider": "openai", "model": "test-model"}]},
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            provider_tools=(ProviderTool("developer_platform_status"),),
            authorize_and_execute=lambda calls: [
                ProviderToolResult(calls[0].call_id, {"status": "ready"}, approved=True)
            ],
            approve_provider_call=True,
            max_turns=3,
        )
        self.assertEqual(result.status, "completed_provider_tool_loop")
        self.assertEqual(result.provider_loop.final_response.text, "continued")  # type: ignore[union-attr]
        self.assertNotIn("super-secret", json.dumps(result.to_dict()))

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

    def test_adaptive_brain_builds_selection_from_registered_providers_and_ledger(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.selection: dict[str, object] | None = None

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    self.selection = arguments
                    models = arguments["models"]
                    self.assert_model_disabled(models, "unregistered")  # type: ignore[arg-type]
                    return {
                        "selected_model": {"provider": "openai", "model": "test-model"},
                        "decision_digest": "a" * 64,
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

            @staticmethod
            def assert_model_disabled(models: object, model_name: str) -> None:
                assert isinstance(models, list)
                candidate = next(model for model in models if model["model"] == model_name)
                assert candidate["enabled"] is False

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/json"))
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        with TemporaryDirectory() as directory:
            ledger = BrainLearningLedger(f"{directory}/adaptive.jsonl")
            ledger.append(
                {
                    "learning_evidence": {"evidence_digest": "e" * 64},
                    "next_state": {
                        "schema": "bioprism-brain-bandit/0.1",
                        "generation": 3,
                        "arms": [
                            {
                                "arm_id": "openai/test-model",
                                "pulls": 3,
                                "reward_sum": 2.7,
                                "failures": 0,
                                "disabled": False,
                            }
                        ],
                    },
                }
            )
            result = AutonomousBrain(workspace, runtime).run_adaptive(
                task="hello",
                model_candidates=[
                    {
                        "provider": "openai",
                        "model": "test-model",
                        "capabilities": ["reasoning"],
                        "context_window_tokens": 16_000,
                        "max_output_tokens": 2_048,
                        "quality": 0.9,
                        "latency_ms": 100,
                        "cost_per_million_tokens": 10,
                        "reliability": 0.95,
                    },
                    {
                        "provider": "unregistered",
                        "model": "unregistered",
                        "capabilities": ["reasoning"],
                        "context_window_tokens": 16_000,
                        "max_output_tokens": 2_048,
                        "quality": 1.0,
                        "latency_ms": 1,
                        "cost_per_million_tokens": 1,
                        "reliability": 1.0,
                    },
                ],
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
                credentials={"openai": handle},
                ledger=ledger,
                approve_provider_call=True,
            )
            self.assertEqual(result.status, "completed_provider_call")
            self.assertEqual(result.selection["selected_model"]["provider"], "openai")  # type: ignore[index]
            self.assertEqual(workspace.selection["observations"][0]["pulls"], 3)  # type: ignore[index]
            self.assertNotIn("super-secret", json.dumps(workspace.selection))

    def test_adaptive_context_digest_matches_rust_context_identity(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.arguments: dict[str, object] | None = None

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select_contextual":
                    assert arguments is not None
                    self.arguments = arguments
                    return {
                        "schema": "bioprism-brain-contextual-model-selection/0.1",
                        "context_digest": hashlib.sha256(
                            b'{"domain":"engineering","capability":"platform_status","risk_class":"low","task_family":null}'
                        ).hexdigest(),
                        "selection_status": "contextual_selection_exact_history",
                        "selection": {
                            "selected_model": {"provider": "openai", "model": "test-model"},
                            "decision_digest": "a" * 64,
                        },
                    }
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "hello"}], "prompt_digest": "b" * 64}
                if name == "brain_plan":
                    return {"ok": True, "plan": {"requires_approval": True, "steps": [{"effect": "provider_call"}], "plan_digest": "c" * 64}}
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/json"))
        handle = store.register("openai", "super-secret")
        context = {"domain": "engineering", "capability": "platform_status", "risk_class": "low"}
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_adaptive(
            task="hello",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 100,
                    "cost_per_million_tokens": 10,
                    "reliability": 0.95,
                }
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            context=context,
            contextual_observations=[{"arm_id": "openai/test-model", "pulls": 2, "reward_sum": 1.8}],
            approve_provider_call=True,
        )
        self.assertEqual(result.status, "completed_provider_call")
        expected = hashlib.sha256(
            b'{"domain":"engineering","capability":"platform_status","risk_class":"low","task_family":null}'
        ).hexdigest()
        self.assertEqual(result.selection["context_digest"], expected)
        self.assertEqual(workspace.arguments["observations"][0]["context_digest"], expected)  # type: ignore[index]

    def test_adaptive_invocation_fails_over_deterministically_after_provider_refusal(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    selected = next(
                        model for model in arguments["models"]  # type: ignore[index]
                        if model.get("enabled", True)  # type: ignore[union-attr]
                    )
                    return {
                        "selected_model": {
                            "provider": selected["provider"],  # type: ignore[index]
                            "model": selected["model"],  # type: ignore[index]
                        },
                        "decision_digest": "a" * 64,
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
        runtime.register_provider(
            openai_provider(base_url=self.base_url, path="/failure", allow_insecure_http=True)
        )
        runtime.register_provider(
            ProviderConfig(
                provider="fallback",
                base_url=self.base_url,
                path="/fallback",
                allow_insecure_http=True,
            )
        )
        openai_handle = store.register("openai", "openai-secret")
        fallback_handle = store.register("fallback", "fallback-secret")
        result = AutonomousBrain(Workspace(), runtime).run_adaptive(
            task="hello",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "primary",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.99,
                    "latency_ms": 10,
                    "cost_per_million_tokens": 1,
                    "reliability": 0.99,
                },
                {
                    "provider": "fallback",
                    "model": "backup",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.8,
                    "latency_ms": 20,
                    "cost_per_million_tokens": 2,
                    "reliability": 0.9,
                },
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": openai_handle, "fallback": fallback_handle},
            approve_provider_call=True,
            max_provider_failovers=1,
        )
        self.assertEqual(result.status, "completed_provider_call")
        self.assertEqual(result.response.provider, "fallback")  # type: ignore[union-attr]
        self.assertEqual(result.provider_failover["fallback_count"], 1)  # type: ignore[index]
        self.assertEqual(result.provider_failover["attempts"][0]["reason"], "provider_error")  # type: ignore[index]
        self.assertEqual(
            len(result.provider_failover["attempts"][0]["selection_audit_digest"]),  # type: ignore[index]
            64,
        )
        self.assertIn("routing_confidence", result.provider_failover["attempts"][0])  # type: ignore[index]
        self.assertIn("selection_audit", result.selection)
        self.assertNotIn("openai-secret", json.dumps(result.to_dict()))
        self.assertNotIn("fallback-secret", json.dumps(result.to_dict()))

    def test_adaptive_tool_loop_fails_over_only_before_tool_authorization(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    selected = next(
                        model for model in arguments["models"]  # type: ignore[index]
                        if model.get("enabled", True)  # type: ignore[union-attr]
                    )
                    return {
                        "selected_model": {
                            "provider": selected["provider"],  # type: ignore[index]
                            "model": selected["model"],  # type: ignore[index]
                        },
                        "decision_digest": "a" * 64,
                    }
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "inspect"}], "prompt_digest": "b" * 64}
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
        runtime.register_provider(
            openai_provider(base_url=self.base_url, path="/failure", allow_insecure_http=True)
        )
        runtime.register_provider(
            ProviderConfig(
                provider="fallback",
                base_url=self.base_url,
                path="/continue",
                allow_insecure_http=True,
            )
        )
        primary_handle = store.register("openai", "primary-loop-secret")
        fallback_handle = store.register("fallback", "fallback-loop-secret")
        calls: list[str] = []
        tool = ProviderTool("developer_platform_status", parameters={"type": "object"})
        result = AutonomousBrain(Workspace(), runtime).run_adaptive_tool_loop(
            task="inspect",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.99,
                    "latency_ms": 10,
                    "cost_per_million_tokens": 1,
                    "reliability": 0.99,
                },
                {
                    "provider": "fallback",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.8,
                    "latency_ms": 20,
                    "cost_per_million_tokens": 2,
                    "reliability": 0.9,
                },
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": primary_handle, "fallback": fallback_handle},
            tool_loop_options={
                "provider_tools": (tool,),
                "authorize_and_execute": lambda tool_calls: tuple(
                    calls.append(call.call_id) or ProviderToolResult(
                        call.call_id, {"status": "authorized"}, approved=True
                    )
                    for call in tool_calls
                ),
                "approve_provider_call": True,
                "max_turns": 3,
            },
            max_provider_failovers=1,
        )
        self.assertEqual(result.status, "completed_provider_tool_loop")
        self.assertEqual(result.brain_run.response.provider, "fallback")  # type: ignore[union-attr]
        self.assertEqual(result.brain_run.provider_failover["fallback_count"], 1)  # type: ignore[index]
        self.assertEqual(result.brain_run.provider_failover["attempts"][0]["status"], "provider_refused")  # type: ignore[index]
        self.assertEqual(calls, ["call-loop-1"])
        self.assertNotIn("primary-loop-secret", json.dumps(result.to_dict()))
        self.assertNotIn("fallback-loop-secret", json.dumps(result.to_dict()))

    def test_adaptive_tool_loop_never_retries_after_tool_authorization_started(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    selected = next(
                        model for model in arguments["models"]  # type: ignore[index]
                        if model.get("enabled", True)  # type: ignore[union-attr]
                    )
                    return {
                        "selected_model": {
                            "provider": selected["provider"],  # type: ignore[index]
                            "model": selected["model"],  # type: ignore[index]
                        },
                        "decision_digest": "a" * 64,
                    }
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "inspect"}], "prompt_digest": "b" * 64}
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

        self.server.request_paths = []  # type: ignore[attr-defined]
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(
                base_url=self.base_url,
                path="/continue_fail_after_result",
                allow_insecure_http=True,
            )
        )
        runtime.register_provider(
            ProviderConfig(
                provider="fallback",
                base_url=self.base_url,
                path="/continue",
                allow_insecure_http=True,
            )
        )
        primary_handle = store.register("openai", "primary-side-effect-secret")
        fallback_handle = store.register("fallback", "fallback-side-effect-secret")
        callback_calls: list[str] = []
        tool = ProviderTool("developer_platform_status", parameters={"type": "object"})
        with self.assertRaises(ProviderError):
            AutonomousBrain(Workspace(), runtime).run_adaptive_tool_loop(
                task="inspect",
                model_candidates=[
                    {
                        "provider": "openai",
                        "model": "test-model",
                        "context_window_tokens": 16_000,
                        "max_output_tokens": 2_048,
                        "quality": 0.99,
                        "latency_ms": 10,
                        "cost_per_million_tokens": 1,
                        "reliability": 0.99,
                    },
                    {
                        "provider": "fallback",
                        "model": "test-model",
                        "context_window_tokens": 16_000,
                        "max_output_tokens": 2_048,
                        "quality": 0.8,
                        "latency_ms": 20,
                        "cost_per_million_tokens": 2,
                        "reliability": 0.9,
                    },
                ],
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
                credentials={"openai": primary_handle, "fallback": fallback_handle},
                tool_loop_options={
                    "provider_tools": (tool,),
                    "authorize_and_execute": lambda tool_calls: tuple(
                        callback_calls.append(call.call_id) or ProviderToolResult(
                            call.call_id, {"status": "authorized"}, approved=True
                        )
                        for call in tool_calls
                    ),
                    "approve_provider_call": True,
                    "max_turns": 3,
                },
                max_provider_failovers=1,
            )
        self.assertEqual(callback_calls, ["call-loop-fail-after-result"])
        self.assertNotIn("/continue", self.server.request_paths[1:])  # type: ignore[attr-defined]

    def test_adaptive_tool_loop_selects_before_continuing_with_caller_callback(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    return {
                        "selected_model": {"provider": "openai", "model": "test-model"},
                        "decision_digest": "a" * 64,
                    }
                if name == "brain_prompt_assemble":
                    return {"messages": [{"role": "user", "content": "inspect"}], "prompt_digest": "b" * 64}
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
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/continue"))
        handle = store.register("openai", "super-secret")
        tool = ProviderTool("developer_platform_status", parameters={"type": "object"})
        result = AutonomousBrain(Workspace(), runtime).run_adaptive_tool_loop(
            task="inspect",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "capabilities": ["reasoning"],
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 100,
                    "cost_per_million_tokens": 10,
                    "reliability": 0.95,
                }
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            tool_loop_options={
                "provider_tools": (tool,),
                "authorize_and_execute": lambda calls: tuple(
                    ProviderToolResult(call.call_id, {"status": "ready"}, approved=True)
                    for call in calls
                ),
                "approve_provider_call": True,
                "max_turns": 3,
            },
        )
        self.assertEqual(result.status, "completed_provider_tool_loop")
        self.assertEqual(result.provider_loop.final_response.text, "continued")  # type: ignore[union-attr]

        class OutcomeWorkspace:
            def __init__(self) -> None:
                self.arguments: dict[str, object] | None = None

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                assert name == "brain_outcome_record"
                self.arguments = arguments
                return {
                    "ok": True,
                    "status": "recorded_evaluator_reward",
                    "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                    "learning_evidence": {"schema": "bioprism-brain-learning-evidence/0.1", "evidence_digest": "e" * 64},
                }

        outcome_workspace = OutcomeWorkspace()
        report = AutonomousBrain(outcome_workspace, runtime).record_evaluator_outcome(
            result,
            bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
            evaluator_id="tool-loop-quality-v1",
            evaluator_version="1",
            reward=0.7,
            passed=True,
        )
        self.assertEqual(report["status"], "recorded_evaluator_reward")
        self.assertNotIn("continued", json.dumps(outcome_workspace.arguments))

    def test_adaptive_tool_loop_binds_selection_and_execution_to_one_live_route(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.route_calls = 0
                self.selection_context: dict[str, object] | None = None
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "capability_route":
                    self.route_calls += 1
                    return {
                        "ok": True,
                        "workflow": "capability_route",
                        "route_id": "r" * 64,
                        "catalog_digest": "c" * 64,
                        "goal": "inspect platform",
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
                        "route_coverage": {
                            "candidate_domains": ["engineering"],
                            "candidate_groups": ["developer_platform"],
                        },
                        "tool_schemas": [
                            {
                                "name": "developer_platform_status",
                                "description": "Read bounded platform status.",
                                "inputSchema": {"type": "object", "properties": {}},
                            }
                        ],
                        "tool_schemas_omitted": 0,
                        "schema_attachment": {"requested": True, "returned": 1, "missing": []},
                    }
                if name == "brain_model_select_contextual":
                    assert arguments is not None
                    self.selection_context = arguments["context"]  # type: ignore[assignment]
                    return {
                        "context_digest": "d" * 64,
                        "selection_status": "contextual_selection_global_history_only",
                        "selection": {
                            "selected_model": {"provider": "openai", "model": "test-model"},
                            "decision_digest": "a" * 64,
                        },
                    }
                if name == "brain_prompt_assemble":
                    assert arguments is not None
                    assert any(chunk["id"] == "capability-route" for chunk in arguments["context"])  # type: ignore[index]
                    return {"messages": [{"role": "user", "content": "inspect"}], "prompt_digest": "b" * 64}
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "c" * 64,
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    self.missions.append(arguments)
                    execute = arguments["policy"]["execute"]  # type: ignore[index]
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "executed" if execute else "planned",
                        "mission_status": "succeeded" if execute else "planned",
                        "results": [],
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/continue"))
        handle = store.register("openai", "super-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_adaptive_tool_loop(
            task="inspect platform",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "capabilities": ["reasoning"],
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 100,
                    "cost_per_million_tokens": 10,
                    "reliability": 0.95,
                }
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            tool_loop_options={
                "mission_policy": {
                    "allowed_tools": ["developer_platform_status"],
                    "max_steps": 2,
                    "max_step_output_bytes": 100_000,
                    "max_total_output_bytes": 100_000,
                },
                "route_request": {"needs": [{"id": "task", "query": "platform"}]},
                "approve_provider_call": True,
                "approve_mission_dispatch": True,
                "max_turns": 3,
            },
        )
        self.assertEqual(result.status, "completed_provider_tool_loop")
        self.assertEqual(workspace.route_calls, 1)
        self.assertEqual(workspace.selection_context["domain"], "cross_domain:engineering")  # type: ignore[index]
        self.assertEqual(len(workspace.missions), 2)
        self.assertFalse(workspace.missions[0]["policy"]["execute"])  # type: ignore[index]
        self.assertTrue(workspace.missions[1]["policy"]["execute"])  # type: ignore[index]
        self.assertNotIn("super-secret", json.dumps(result.to_dict()))

    def test_adaptive_mission_reuses_one_route_across_selection_prompt_and_dispatch(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.route_calls = 0
                self.selection_contexts: list[dict[str, object]] = []
                self.missions: list[dict[str, object]] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "capability_route":
                    self.route_calls += 1
                    return {
                        "ok": True,
                        "workflow": "capability_route",
                        "route_id": "r" * 64,
                        "catalog_digest": "c" * 64,
                        "goal": "inspect platform",
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
                        "route_coverage": {
                            "candidate_domains": ["engineering"],
                            "candidate_groups": ["developer_platform"],
                        },
                        "tool_schemas": [
                            {
                                "name": "developer_platform_status",
                                "description": "Read bounded platform status.",
                                "inputSchema": {"type": "object", "properties": {}},
                            }
                        ],
                        "tool_schemas_omitted": 0,
                        "schema_attachment": {"requested": True, "returned": 1, "missing": []},
                    }
                if name == "brain_model_select_contextual":
                    assert arguments is not None
                    self.selection_contexts.append(dict(arguments["context"]))  # type: ignore[arg-type]
                    return {
                        "context_digest": "d" * 64,
                        "selection_status": "contextual_selection_global_history_only",
                        "selection": {
                            "selected_model": {"provider": "openai", "model": "test-model"},
                            "decision_digest": "a" * 64,
                        },
                    }
                if name == "brain_prompt_assemble":
                    assert arguments is not None
                    assert any(
                        chunk["id"] == "capability-route"  # type: ignore[index]
                        for chunk in arguments["context"]  # type: ignore[index]
                    )
                    return {
                        "messages": [{"role": "user", "content": "inspect"}],
                        "prompt_digest": "b" * 64,
                    }
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "c" * 64,
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    self.missions.append(arguments)
                    execute = arguments["policy"]["execute"]  # type: ignore[index]
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "executed" if execute else "planned",
                        "mission_status": "succeeded" if execute else "planned",
                        "results": [],
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/mission")
        )
        handle = store.register("openai", "adaptive-mission-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_adaptive_mission(
            task="inspect platform",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "capabilities": ["reasoning"],
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 100,
                    "cost_per_million_tokens": 10,
                    "reliability": 0.95,
                }
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": handle},
            mission_policy={
                "allowed_tools": ["developer_platform_status"],
                "max_steps": 2,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            route_request={"needs": [{"id": "task", "query": "platform"}]},
            approve_provider_call=True,
            approve_mission_dispatch=True,
        )

        self.assertEqual(result.status, "mission_dispatched")
        self.assertEqual(workspace.route_calls, 1)
        self.assertEqual(len(workspace.selection_contexts), 2)
        self.assertEqual(workspace.selection_contexts[0]["domain"], "cross_domain:engineering")
        self.assertEqual(len(workspace.missions), 2)
        self.assertFalse(workspace.missions[0]["policy"]["execute"])  # type: ignore[index]
        self.assertTrue(workspace.missions[1]["policy"]["execute"])  # type: ignore[index]
        self.assertNotIn("adaptive-mission-secret", json.dumps(result.to_dict()))

    def test_adaptive_mission_fails_over_before_mission_preflight(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.mission_calls = 0

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    selected = next(
                        model for model in arguments["models"]  # type: ignore[index]
                        if model.get("enabled", True)  # type: ignore[union-attr]
                    )
                    return {
                        "selected_model": {
                            "provider": selected["provider"],  # type: ignore[index]
                            "model": selected["model"],  # type: ignore[index]
                        },
                        "decision_digest": "a" * 64,
                    }
                if name == "brain_prompt_assemble":
                    return {
                        "messages": [{"role": "user", "content": "inspect"}],
                        "prompt_digest": "b" * 64,
                    }
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "c" * 64,
                        },
                    }
                if name == "agent_mission":
                    self.mission_calls += 1
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "planned",
                        "mission_status": "planned",
                        "results": [],
                    }
                raise AssertionError(f"unexpected tool {name}")

        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/failure")
        )
        runtime.register_provider(
            ProviderConfig(
                provider="fallback",
                base_url=self.base_url,
                path="/mission",
                allow_insecure_http=True,
            )
        )
        primary_handle = store.register("openai", "primary-mission-secret")
        fallback_handle = store.register("fallback", "fallback-mission-secret")
        workspace = Workspace()
        result = AutonomousBrain(workspace, runtime).run_adaptive_mission(
            task="inspect",
            model_candidates=[
                {
                    "provider": "openai",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.99,
                    "latency_ms": 10,
                    "cost_per_million_tokens": 1,
                    "reliability": 0.99,
                },
                {
                    "provider": "fallback",
                    "model": "test-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.8,
                    "latency_ms": 20,
                    "cost_per_million_tokens": 2,
                    "reliability": 0.9,
                },
            ],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
            credentials={"openai": primary_handle, "fallback": fallback_handle},
            mission_policy={
                "allowed_tools": ["developer_platform_status"],
                "max_steps": 2,
                "max_step_output_bytes": 100_000,
                "max_total_output_bytes": 100_000,
            },
            approve_provider_call=True,
            max_provider_failovers=1,
        )

        self.assertEqual(result.status, "mission_approval_required")
        self.assertEqual(result.brain_run.response.provider, "fallback")  # type: ignore[union-attr]
        self.assertEqual(result.brain_run.provider_failover["fallback_count"], 1)  # type: ignore[index]
        self.assertEqual(result.brain_run.provider_failover["attempts"][0]["status"], "provider_refused")  # type: ignore[index]
        self.assertEqual(workspace.mission_calls, 1)
        self.assertNotIn("primary-mission-secret", json.dumps(result.to_dict()))
        self.assertNotIn("fallback-mission-secret", json.dumps(result.to_dict()))

    def test_adaptive_mission_never_retries_after_dispatch_starts(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.dispatch_calls = 0

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select":
                    assert arguments is not None
                    selected = next(
                        model for model in arguments["models"]  # type: ignore[index]
                        if model.get("enabled", True)  # type: ignore[union-attr]
                    )
                    return {
                        "selected_model": {
                            "provider": selected["provider"],  # type: ignore[index]
                            "model": selected["model"],  # type: ignore[index]
                        },
                        "decision_digest": "a" * 64,
                    }
                if name == "brain_prompt_assemble":
                    return {
                        "messages": [{"role": "user", "content": "inspect"}],
                        "prompt_digest": "b" * 64,
                    }
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "c" * 64,
                        },
                    }
                if name == "agent_mission":
                    assert arguments is not None
                    if arguments["policy"]["execute"]:  # type: ignore[index]
                        self.dispatch_calls += 1
                        raise ProviderError("mission dispatch failed after authorization")
                    return {
                        "ok": True,
                        "workflow": "agent_mission",
                        "execution": "planned",
                        "mission_status": "planned",
                        "results": [],
                    }
                raise AssertionError(f"unexpected tool {name}")

        self.server.request_paths = []  # type: ignore[attr-defined]
        store = CredentialStore()
        runtime = LLMRuntime(store)
        runtime.register_provider(
            openai_provider(base_url=self.base_url, allow_insecure_http=True, path="/mission")
        )
        runtime.register_provider(
            ProviderConfig(
                provider="fallback",
                base_url=self.base_url,
                path="/mission",
                allow_insecure_http=True,
            )
        )
        primary_handle = store.register("openai", "primary-dispatch-secret")
        fallback_handle = store.register("fallback", "fallback-dispatch-secret")
        workspace = Workspace()
        models = [
            {
                "provider": "openai",
                "model": "test-model",
                "context_window_tokens": 16_000,
                "max_output_tokens": 2_048,
                "quality": 0.99,
                "latency_ms": 10,
                "cost_per_million_tokens": 1,
                "reliability": 0.99,
            },
            {
                "provider": "fallback",
                "model": "test-model",
                "context_window_tokens": 16_000,
                "max_output_tokens": 2_048,
                "quality": 0.8,
                "latency_ms": 20,
                "cost_per_million_tokens": 2,
                "reliability": 0.9,
            },
        ]
        with self.assertRaises(ProviderError):
            AutonomousBrain(workspace, runtime).run_adaptive_mission(
                task="inspect",
                model_candidates=models,
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"], "max_cost": 10},
                credentials={"openai": primary_handle, "fallback": fallback_handle},
                mission_policy={
                    "allowed_tools": ["developer_platform_status"],
                    "max_steps": 2,
                    "max_step_output_bytes": 100_000,
                    "max_total_output_bytes": 100_000,
                },
                approve_provider_call=True,
                approve_mission_dispatch=True,
                max_provider_failovers=1,
            )
        self.assertEqual(workspace.dispatch_calls, 1)
        self.assertEqual(self.server.request_paths.count("/mission"), 1)  # type: ignore[attr-defined]

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

    def test_evaluator_adapter_projects_all_execution_shapes_without_provider_text(self) -> None:
        response = ProviderResponse(
            provider="openai",
            model="test-model",
            text="provider-secret-answer",
            status_code=200,
            request_id="req-1",
            usage={"total_tokens": 12},
            raw={"output_text": "provider-secret-answer"},
        )
        run = BrainRunResult(
            run_id="run-evaluator",
            status="completed_provider_call",
            selection={
                "selected_model": {"provider": "openai", "model": "test-model"},
                "decision_digest": "a" * 64,
                "context_digest": "b" * 64,
            },
            prompt={"prompt_digest": "c" * 64},
            plan={"plan": {"plan_digest": "d" * 64}},
            response=response,
            outcome_digest="e" * 64,
        )
        observed: list[dict[str, object]] = []
        adapter = BrainOutcomeEvaluator(
            lambda payload: observed.append(dict(payload)) or {"reward": 0.75, "passed": True},
            evaluator_id="held-out-quality",
            evaluator_version="2026-08",
        )
        decision = adapter.assess(
            run,
            evidence={"schema_valid": True, "quality_score": 0.75},
        )
        encoded_input = json.dumps(observed[0])
        self.assertEqual(decision.evidence_digest, observed[0]["evidence_digest"])
        self.assertNotIn("provider-secret-answer", encoded_input)
        self.assertNotIn("output_text", encoded_input)
        self.assertEqual(observed[0]["response"]["tool_call_count"], 0)  # type: ignore[index]
        self.assertEqual(observed[0]["result_kind"], "run")

        tool_loop = BrainToolLoopResult(
            brain_run=run,
            status="completed_provider_tool_loop",
            provider_loop=None,
            route={"workflow": "capability_route", "evidence_digest": "f" * 64, "wire": "secret"},
        )
        loop_input = build_brain_evaluation_input(tool_loop)
        self.assertEqual(loop_input["result_kind"], "tool_loop")
        self.assertNotIn("secret", json.dumps(loop_input))

        mission = BrainMissionResult(
            brain_run=run,
            status="mission_approval_required",
            mission={"steps": [{"arguments": {"secret": "do-not-project"}}]},
            preflight={
                "workflow": "agent_mission",
                "results": [{"wire": {"result": {"structuredContent": {"secret": "hidden"}}}}],
            },
            execution=None,
        )
        mission_input = build_brain_evaluation_input(mission)
        self.assertEqual(mission_input["result_kind"], "mission")
        self.assertNotIn("do-not-project", json.dumps(mission_input))
        self.assertNotIn("hidden", json.dumps(mission_input))

    def test_evaluator_adapter_records_only_digest_bound_decision(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.arguments: dict[str, object] | None = None

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                assert name == "brain_outcome_record"
                self.arguments = arguments
                return {
                    "ok": True,
                    "status": "recorded_evaluator_reward",
                    "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                    "learning_evidence": {
                        "schema": "bioprism-brain-learning-evidence/0.1",
                        "evidence_digest": "f" * 64,
                    },
                }

        workspace = Workspace()
        brain = AutonomousBrain(workspace, LLMRuntime())
        result = BrainRunResult(
            run_id="run-adapter",
            status="completed_provider_call",
            selection={
                "selected_model": {"provider": "openai", "model": "test-model"},
                "decision_digest": "a" * 64,
            },
            prompt={"prompt_digest": "b" * 64},
            plan={"plan": {"plan_digest": "c" * 64}},
            response=None,
            outcome_digest="d" * 64,
        )
        adapter = BrainOutcomeEvaluator(
            lambda payload: {
                "reward": 0.8 if payload["evidence"]["quality"] == 0.8 else 0.0,  # type: ignore[index]
                "passed": True,
                "feedback_digest": "e" * 64,
            },
            evaluator_id="quality-gate",
            evaluator_version="1",
        )
        with TemporaryDirectory() as directory:
            ledger = BrainLearningLedger(f"{directory}/learning.jsonl")
            report = adapter.evaluate_and_record(
                brain,
                result,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                evidence={"quality": 0.8, "provider_text": "caller-owned evidence is not persisted"},
                ledger=ledger,
            )
            replays = ledger.replays(run_id="run-adapter", evaluator_id="quality-gate")
            self.assertEqual(len(replays), 1)
            self.assertEqual(replays[0]["schema"], "bioprism-brain-evaluator-replay/0.1")
            self.assertEqual(replays[0]["result_kind"], "run")
            self.assertNotIn("caller-owned evidence", json.dumps(ledger.records()))
        self.assertEqual(report["status"], "recorded_evaluator_reward")
        assert workspace.arguments is not None
        encoded = json.dumps(workspace.arguments)
        self.assertNotIn("caller-owned evidence", encoded)
        self.assertEqual(workspace.arguments["assessment"]["evidence_digest"], hashlib.sha256(  # type: ignore[index]
            json.dumps(
                {"provider_text": "caller-owned evidence is not persisted", "quality": 0.8},
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ).hexdigest())  # type: ignore[index]

    def test_evaluator_adapter_rejects_secret_fields_and_contradictory_decisions(self) -> None:
        result = BrainRunResult(
            run_id="run-reject",
            status="planned",
            selection={"selected_model": {"provider": "openai", "model": "test-model"}},
            prompt={},
            plan={},
            response=None,
            outcome_digest="a" * 64,
        )
        with self.assertRaises(BrainRunError):
            BrainOutcomeEvaluator(
                lambda _: {"reward": 1.0, "passed": True, "notes": "raw answer"},
                evaluator_id="quality",
                evaluator_version="1",
            ).assess(result)
        with self.assertRaises(BrainRunError):
            BrainOutcomeEvaluator(
                lambda _: {"reward": 1.0, "passed": True, "failed": True},
                evaluator_id="quality",
                evaluator_version="1",
            ).assess(result)
        with self.assertRaises(BrainRunError):
            BrainOutcomeEvaluator(
                lambda _: BrainEvaluatorDecision(
                    evaluator_id="other",
                    evaluator_version="1",
                    reward=1.0,
                    passed=True,
                ),
                evaluator_id="quality",
                evaluator_version="1",
            ).assess(result)
        with self.assertRaises(BrainRunError):
            build_brain_evaluation_input(result, evidence={"api_key": "must-refuse"})
        with self.assertRaises(BrainRunError):
            build_brain_evaluation_input(result, evidence={"nested": ({"apiKey": "must-refuse"},)})


if __name__ == "__main__":
    unittest.main()
