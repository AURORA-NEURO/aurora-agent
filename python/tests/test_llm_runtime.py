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
    ProviderRequest,
    anthropic_provider,
    openai_provider,
)
from prism_sdk.brain import AutonomousBrain, BrainLearningLedger, BrainRunResult


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
            selection={"selected_model": {"provider": "openai", "model": "test-model"}, "decision_digest": "a" * 64},
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
            encoded = json.dumps(ledger.records())
            self.assertNotIn("super-secret", encoded)
            self.assertNotIn("api_key", encoded)


if __name__ == "__main__":
    unittest.main()
