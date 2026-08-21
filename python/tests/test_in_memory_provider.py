from __future__ import annotations

import hashlib
import json
from typing import Any, Mapping

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousDomainRegistry,
    InMemoryProvider,
    LLMRuntime,
    ModelCatalogue,
    ProviderError,
    ProviderModelDescriptor,
    ProviderOnboarding,
    ProviderRequest,
    ProviderStreamEvent,
    ProviderTool,
    ProviderToolResult,
)


def _request(
    *,
    model: str = "offline-model",
    require_json: bool = False,
    response_schema: Mapping[str, Any] | None = None,
    tools: tuple[ProviderTool, ...] = (),
) -> ProviderRequest:
    return ProviderRequest(
        model=model,
        messages=({"role": "user", "content": "bounded local test"},),
        max_output_tokens=64,
        require_json=require_json,
        response_schema=response_schema,
        tools=tools,
    )


def test_in_memory_provider_is_explicit_credentialless_and_preserves_runtime_observations() -> None:
    seen: list[ProviderRequest] = []

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        seen.append(request)
        return {
            "model": request.model,
            "output_text": "offline answer",
            "request_id": "offline-request-1",
            "usage": {"input_tokens": 4, "output_tokens": 3},
            "raw_secret": "must never be retained",
        }

    runtime = LLMRuntime()
    config = runtime.register_in_memory_provider(
        "offline",
        handler,
        model_discovery_handler=lambda: {
            "data": [
                {
                    "id": "offline-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "capabilities": ["reasoning", "structured_output"],
                }
            ]
        },
    )

    assert isinstance(config.transport, InMemoryProvider)
    assert config.requires_credential is False
    assert config.to_metadata()["transport"] == "in_memory"
    onboarding = ProviderOnboarding(runtime)
    assert onboarding.status("offline")["ready"] is True
    assert onboarding.status("offline")["next_action"] == "ready"

    response = runtime.invoke("offline", _request())
    assert response.provider == "offline"
    assert response.model == "offline-model"
    assert response.text == "offline answer"
    assert response.raw == {
        "schema": "bioprism-llm-in-memory-provider/0.1",
        "transport": "caller_owned",
    }
    assert "raw_secret" not in json.dumps(response.to_dict())
    assert seen == [_request()]

    status = runtime.provider_status("offline")
    assert status["attempts"] == 1
    assert status["successes"] == 1
    assert status["credential_posture"] == "caller_supplied_in_memory_handle"
    assert runtime.model_status("offline", "offline-model")["success_rate"] == 1.0

    descriptors = runtime.discover_models("offline")
    assert descriptors == (
        ProviderModelDescriptor(
            "offline",
            "offline-model",
            capabilities=("reasoning", "structured_output"),
            context_window_tokens=16_000,
            max_output_tokens=2_048,
        ),
    )


def test_in_memory_provider_validates_structured_output_and_tool_intents() -> None:
    tools = (ProviderTool("read_status", parameters={"type": "object"}),)
    schema = {
        "type": "object",
        "required": ["answer"],
        "properties": {"answer": {"type": "string"}},
        "additionalProperties": False,
    }
    calls = 0

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        nonlocal calls
        calls += 1
        if request.tools and not any(
            message.get("role") == "tool" for message in request.messages
        ):
            return {
                "tool_calls": [
                    {
                        "call_id": "status-1",
                        "name": "read_status",
                        "arguments": {"scope": "workspace"},
                    }
                ]
            }
        return {"output_text": json.dumps({"answer": "ok"})}

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", handler)

    structured = runtime.invoke(
        "offline",
        _request(require_json=True, response_schema=schema),
    )
    assert structured.structured == {"answer": "ok"}

    result = runtime.invoke_tool_loop(
        "offline",
        _request(tools=tools),
        authorize_and_execute=lambda requested: [
            ProviderToolResult(
                call.call_id,
                {"status": "healthy"},
                approved=True,
            )
            for call in requested
        ],
    )
    assert result.status == "completed"
    assert result.tool_calls == 1
    assert result.final_response is not None
    assert calls == 3

    class BadNameProvider:
        def invoke(self, request: ProviderRequest) -> Mapping[str, Any]:
            return {"tool_calls": [{"call_id": "bad-1", "name": "write_file", "arguments": {}}]}

    bad_runtime = LLMRuntime()
    bad_runtime.register_in_memory_provider("bad", BadNameProvider().invoke)
    with pytest.raises(ProviderError, match="unrequested tool call"):
        bad_runtime.invoke("bad", _request(tools=tools))


def test_in_memory_provider_stream_handler_and_fallback_are_provider_neutral() -> None:
    stream_requests: list[ProviderRequest] = []

    def stream_handler(request: ProviderRequest) -> list[ProviderStreamEvent]:
        stream_requests.append(request)
        return [
            ProviderStreamEvent(
                provider="streaming",
                model=request.model,
                sequence=0,
                event_type="local.text",
                text_delta="hello",
            ),
            ProviderStreamEvent(
                provider="streaming",
                model=request.model,
                sequence=1,
                event_type="local.done",
                done=True,
            ),
        ]

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("streaming", lambda _request: "fallback", stream_handler=stream_handler)
    events = list(runtime.invoke_stream("streaming", _request()))
    assert [event.text_delta for event in events] == ["hello", ""]
    assert events[-1].done is True
    assert stream_requests == [_request()]

    fallback_runtime = LLMRuntime()
    fallback_runtime.register_in_memory_provider("fallback", lambda _request: "fallback text")
    fallback_events = list(fallback_runtime.invoke_stream("fallback", _request()))
    assert [event.text_delta for event in fallback_events] == ["fallback text", ""]
    assert fallback_events[-1].done is True


def test_in_memory_provider_redacts_handler_errors_while_preserving_retry_metadata() -> None:
    def handler(_request: ProviderRequest) -> str:
        raise ProviderError(
            "upstream api-key=local-secret must not escape",
            retryable=True,
            status_code=503,
        )

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", handler)
    with pytest.raises(ProviderError) as raised:
        runtime.invoke("offline", _request())
    assert str(raised.value) == "in-memory provider handler failed"
    assert "local-secret" not in str(raised.value)
    assert raised.value.retryable is True
    assert raised.value.status_code == 503


class _OfflineWorkspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        if name == "brain_model_select_contextual":
            context = args.get("context")
            assert isinstance(context, dict)
            identity = {field: context.get(field) for field in ("domain", "capability", "risk_class", "task_family")}
            digest = hashlib.sha256(
                json.dumps(identity, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            ).hexdigest()
            return {
                "context_digest": digest,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "offline-model"},
                    "decision_digest": "d" * 64,
                },
            }
        if name == "brain_model_select":
            return {
                "selected_model": {"provider": "offline", "model": "offline-model"},
                "decision_digest": "d" * 64,
            }
        if name == "brain_prompt_assemble":
            return {
                "messages": [
                    {"role": "system", "content": str(args.get("system"))},
                    {"role": "user", "content": str(args.get("task"))},
                ],
                "prompt_digest": "p" * 64,
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
            return {
                "ok": True,
                "status": "recorded",
                "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                "learning_evidence": {"evidence_digest": "e" * 64},
            }
        raise AssertionError(f"unexpected tool {name}")


def test_credentialless_runtime_executes_every_builtin_domain_through_agent_facade() -> None:
    runtime = LLMRuntime()
    requests: list[ProviderRequest] = []

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        requests.append(request)
        return {"output_text": f"offline result for {request.model}"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue(
        [
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]
    )
    agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=catalogue,
    )

    results = [
        agent.run(
            task=f"perform a bounded {domain} review",
            domain=domain,
            credentials={},
            approve_provider_call=True,
        )
        for domain in AUTONOMOUS_DOMAINS
    ]
    assert len(results) == len(AUTONOMOUS_DOMAINS)
    assert all(result.status == "completed_provider_call" for result in results)
    assert len(requests) == len(AUTONOMOUS_DOMAINS)
    assert {request.model for request in requests} == {"offline-model"}
