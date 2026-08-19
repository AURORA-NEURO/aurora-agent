from __future__ import annotations

from pathlib import Path

import pytest

from prism_sdk import (
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomousProviderInvocationSession,
    AutonomyPolicyError,
    LLMRuntime,
    ProviderConfig,
    ProviderError,
    ProviderRequest,
    ProviderResponse,
    ProviderStreamEvent,
    ProviderTool,
    ProviderToolCall,
    ProviderToolResult,
)


class _FakeRuntime(LLMRuntime):
    def __init__(self, responses: list[ProviderResponse | BaseException]) -> None:
        super().__init__()
        self.responses = list(responses)
        self.transport_calls = 0
        self.register_provider(
            ProviderConfig(
                provider="local",
                base_url="https://example.invalid",
                requires_credential=False,
            )
        )

    def _post(self, config, body, headers, request):  # type: ignore[no-untyped-def]
        self.transport_calls += 1
        response = self.responses.pop(0)
        if isinstance(response, BaseException):
            raise response
        return response


class _FakeStreamRuntime(_FakeRuntime):
    def _stream(self, config, body, headers, request):  # type: ignore[no-untyped-def]
        yield ProviderStreamEvent(
            provider=config.provider,
            model=request.model,
            sequence=1,
            event_type="message.done",
            text_delta="streamed",
            done=True,
        )


def _controller(tmp_path: Path, *, max_provider_calls: int = 4, max_cost_units: float = 10.0) -> AutonomousExecutionController:
    return AutonomousExecutionController(
        execution_id="provider-accounting",
        domain="coding",
        capability="implementation_review",
        risk_class="review",
        policy=AutonomousExecutionPolicy(
            max_steps=8,
            max_provider_calls=max_provider_calls,
            max_cost_units=max_cost_units,
        ),
        journal=AutonomousExecutionJournal(tmp_path / "execution.jsonl"),
    )


def _response(*, text: str = "ok", tool_calls: tuple[ProviderToolCall, ...] = ()) -> ProviderResponse:
    return ProviderResponse(
        provider="local",
        model="model-v1",
        text=text,
        status_code=200,
        request_id="request-1",
        usage={"input_tokens": 12, "output_tokens": 7},
        raw={"metadata_only": True},
        tool_calls=tool_calls,
    )


def test_provider_invocation_admission_and_outcome_are_durable_without_payloads(tmp_path: Path) -> None:
    runtime = _FakeRuntime([_response()])
    controller = _controller(tmp_path)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        selection_digest="a" * 64,
        cost_per_million_tokens=100.0,
    )
    request = ProviderRequest(
        model="model-v1",
        messages=({"role": "user", "content": "private prompt that must not be journaled"},),
        max_output_tokens=32,
    )

    runtime.invoke("local", request, invocation_observer=observer)

    assert runtime.transport_calls == 1
    assert controller.state.provider_calls == 1
    assert controller.state.cost_units > 0
    receipt = observer.receipts[0].to_dict()
    assert receipt["outcome"] == "success"
    assert receipt["input_tokens"] == 12
    assert receipt["output_tokens"] == 7
    events = AutonomousExecutionJournal(tmp_path / "execution.jsonl").events(
        execution_id="provider-accounting"
    )
    provider_events = [row["event"] for row in events if row["event"]["kind"] == "provider_call"]
    assert any(event.get("provider_outcome") == "success" for event in provider_events)
    serialized = (tmp_path / "execution.jsonl").read_text(encoding="utf-8")
    assert "private prompt" not in serialized
    assert "private-key-value" not in serialized


def test_provider_budget_refuses_transport_before_it_is_sent(tmp_path: Path) -> None:
    runtime = _FakeRuntime([_response()])
    controller = _controller(tmp_path, max_provider_calls=1, max_cost_units=10.0)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        cost_per_million_tokens=1.0,
    )
    request = ProviderRequest(
        model="model-v1",
        messages=({"role": "user", "content": "bounded"},),
        max_output_tokens=8,
    )
    runtime.invoke("local", request, invocation_observer=observer)

    with pytest.raises(AutonomyPolicyError, match="max_provider_calls"):
        runtime.invoke("local", request, invocation_observer=observer)
    assert runtime.transport_calls == 1
    assert controller.state.provider_calls == 1


def test_tool_loop_accounts_each_continuation_turn_and_provider_failure(tmp_path: Path) -> None:
    call = ProviderToolCall(call_id="call-1", name="status", arguments={})
    runtime = _FakeRuntime([
        _response(text="", tool_calls=(call,)),
        _response(text="continued"),
    ])
    controller = _controller(tmp_path)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        cost_per_million_tokens=10.0,
        kind="tool_loop_turn",
    )
    request = ProviderRequest(
        model="model-v1",
        messages=({"role": "user", "content": "continue"},),
        tools=(ProviderTool("status"),),
    )
    result = runtime.invoke_tool_loop(
        "local",
        request,
        authorize_and_execute=lambda calls: [
            ProviderToolResult(call.call_id, {"approved": True}, approved=True)
            for call in calls
        ],
        invocation_observer=observer,
    )

    assert result.status == "completed"
    assert controller.state.provider_calls == 2
    assert len(observer.receipts) == 2
    assert all(receipt.kind == "tool_loop_turn" for receipt in observer.receipts)


def test_streaming_provider_receipt_closes_as_success(tmp_path: Path) -> None:
    runtime = _FakeStreamRuntime([])
    controller = _controller(tmp_path)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        kind="provider_stream",
    )
    request = ProviderRequest(model="model-v1", messages=({"role": "user", "content": "stream"},))

    events = list(runtime.invoke_stream("local", request, invocation_observer=observer))

    assert events[0].text_delta == "streamed"
    assert observer.receipts[0].outcome == "success"


def test_provider_error_is_recorded_and_remains_failover_evidence(tmp_path: Path) -> None:
    runtime = _FakeRuntime([ProviderError("upstream unavailable", retryable=False, status_code=503)])
    controller = _controller(tmp_path)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        selection_digest="b" * 64,
    )
    request = ProviderRequest(
        model="model-v1",
        messages=({"role": "user", "content": "bounded"},),
    )

    with pytest.raises(ProviderError):
        runtime.invoke("local", request, invocation_observer=observer)
    assert observer.receipts[0].outcome == "failure"
    assert observer.receipts[0].failure_class == "provider_error"
    assert observer.receipts[0].status_code == 503
    assert controller.state.provider_calls == 1


def test_execution_policy_caps_failover_attempts_before_transport(tmp_path: Path) -> None:
    runtime = _FakeRuntime([_response()])
    controller = _controller(tmp_path)
    observer = AutonomousProviderInvocationSession(
        controller=controller,
        provider="local",
        model="model-v1",
        attempt=3,
    )
    request = ProviderRequest(model="model-v1", messages=({"role": "user", "content": "bounded"},))

    with pytest.raises(AutonomyPolicyError, match="max_provider_failovers"):
        runtime.invoke("local", request, invocation_observer=observer)
    assert runtime.transport_calls == 0
