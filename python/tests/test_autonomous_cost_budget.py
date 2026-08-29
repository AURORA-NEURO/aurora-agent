from concurrent.futures import ThreadPoolExecutor

import pytest

from prism_sdk.autonomous_cost_budget import (
    AutonomousCostBudget,
    AutonomousCostBudgetError,
)
from prism_sdk import ProviderQuotaController, ProviderQuotaError
from prism_sdk.llm_runtime import (
    LLMRuntime,
    ProviderError,
    ProviderRequest,
    ProviderStreamEvent,
    ProviderTool,
    ProviderToolResult,
)


def _request() -> ProviderRequest:
    return ProviderRequest(
        model="offline-model",
        messages=({"role": "user", "content": "bounded request"},),
        max_output_tokens=32,
    )


def test_cost_budget_snapshot_and_idempotent_release_preserve_accounting():
    budget = AutonomousCostBudget(10)
    release = budget.reserve(3)
    assert budget.snapshot() == {
        "max_cost_units": 10.0,
        "consumed_cost_units": 3.0,
        "remaining_cost_units": 7.0,
    }

    restored = AutonomousCostBudget.from_snapshot(budget.snapshot())
    assert restored.consumed_cost_units == 3.0
    release()
    release()
    assert budget.consumed_cost_units == 0.0
    assert restored.consumed_cost_units == 3.0


def test_cost_budget_refuses_overflow_with_structured_accounting():
    budget = AutonomousCostBudget(1)
    budget.reserve(0.75)
    with pytest.raises(AutonomousCostBudgetError) as raised:
        budget.reserve(0.26)
    assert raised.value.code == "quota_exceeded"
    assert raised.value.max_cost_units == 1.0
    assert raised.value.consumed_cost_units == 0.75
    assert raised.value.requested_cost_units == 0.26

    with pytest.raises(ValueError, match="snapshot is malformed"):
        AutonomousCostBudget.from_snapshot(
            {
                "max_cost_units": 1,
                "consumed_cost_units": 0.5,
                "remaining_cost_units": 0.4,
            }
        )


def test_cost_budget_admission_is_atomic_under_parallel_fanout():
    budget = AutonomousCostBudget(10)

    def reserve() -> bool:
        try:
            budget.reserve(1)
            return True
        except AutonomousCostBudgetError:
            return False

    with ThreadPoolExecutor(max_workers=32) as pool:
        outcomes = list(pool.map(lambda _item: reserve(), range(32)))
    assert sum(outcomes) == 10
    assert budget.consumed_cost_units == 10.0


def test_runtime_releases_aggregate_cost_when_observer_refuses_before_dispatch():
    calls = 0

    class RefusingObserver:
        def before(self, _metadata):
            raise ProviderError("local policy refused", code="quota_exceeded")

        def after(self, _metadata, _response, _error, _latency_ms):
            raise AssertionError("after must not run when before refuses")

    def handler(_request):
        nonlocal calls
        calls += 1
        return "must not dispatch"

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", handler)
    budget = AutonomousCostBudget(2)
    with pytest.raises(ProviderError, match="local policy refused"):
        runtime.invoke(
            "offline",
            _request(),
            invocation_observer=RefusingObserver(),
            estimated_cost_units=1,
            reserve_cost=budget.reserve,
        )
    assert calls == 0
    assert budget.consumed_cost_units == 0.0


def test_runtime_retains_aggregate_cost_after_provider_dispatch_failure():
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "offline",
        lambda _request: (_ for _ in ()).throw(
            ProviderError("transport failed after dispatch", retryable=True)
        ),
    )
    budget = AutonomousCostBudget(2)
    with pytest.raises(ProviderError, match="in-memory provider handler failed"):
        runtime.invoke(
            "offline",
            _request(),
            estimated_cost_units=1,
            reserve_cost=budget.reserve,
        )
    assert budget.consumed_cost_units == 1.0


def test_stream_budget_is_lazy_and_collect_stream_reserves_once():
    calls = 0

    def stream_handler(request):
        nonlocal calls
        calls += 1
        yield ProviderStreamEvent(
            provider="offline-stream",
            model=request.model,
            sequence=0,
            event_type="text",
            text_delta="answer",
        )
        yield ProviderStreamEvent(
            provider="offline-stream",
            model=request.model,
            sequence=1,
            event_type="done",
            done=True,
        )

    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "offline-stream", lambda _request: "unused", stream_handler=stream_handler
    )
    budget = AutonomousCostBudget(3)
    iterator = runtime.invoke_stream(
        "offline-stream",
        _request(),
        estimated_cost_units=1,
        reserve_cost=budget.reserve,
    )
    assert budget.consumed_cost_units == 0.0
    assert next(iterator).text_delta == "answer"
    assert budget.consumed_cost_units == 1.0
    iterator.close()
    assert calls == 1

    response = runtime.collect_stream(
        "offline-stream",
        _request(),
        estimated_cost_units=1,
        reserve_cost=budget.reserve,
    )
    assert response.text == "answer"
    assert budget.consumed_cost_units == 2.0


def test_unconsumed_stream_does_not_leak_provider_or_aggregate_admission():
    quota = ProviderQuotaController()
    quota.set_policy({
        "provider": "offline-stream-lazy",
        "model": "offline-model",
        "windowMs": 60_000,
        "maxRequests": 1,
    })
    runtime = LLMRuntime(provider_quota=quota)
    runtime.register_in_memory_provider(
        "offline-stream-lazy",
        lambda _request: "unused",
        stream_handler=lambda request: iter((
            ProviderStreamEvent(
                provider="offline-stream-lazy",
                model=request.model,
                sequence=0,
                event_type="done",
                done=True,
            ),
        )),
    )
    budget = AutonomousCostBudget(1)
    abandoned = runtime.invoke_stream(
        "offline-stream-lazy",
        _request(),
        estimated_cost_units=1,
        reserve_cost=budget.reserve,
    )
    abandoned.close()
    assert budget.consumed_cost_units == 0.0
    assert len(list(runtime.invoke_stream("offline-stream-lazy", _request()))) == 1
    with pytest.raises(ProviderQuotaError):
        list(runtime.invoke_stream("offline-stream-lazy", _request()))


def test_tool_loop_charges_each_provider_turn_against_one_budget():
    calls = 0

    def handler(request):
        nonlocal calls
        calls += 1
        if calls == 1:
            return {
                "tool_calls": [
                    {"call_id": "call-1", "name": "lookup", "arguments": {"q": "safe"}}
                ]
            }
        return {"output_text": "complete"}

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline-loop", handler)
    budget = AutonomousCostBudget(2)
    result = runtime.invoke_tool_loop(
        "offline-loop",
        ProviderRequest(
            model="offline-model",
            messages=({"role": "user", "content": "use lookup"},),
            max_output_tokens=32,
            tools=(ProviderTool("lookup", parameters={"type": "object"}),),
        ),
        authorize_and_execute=lambda calls: [
            ProviderToolResult(calls[0].call_id, {"value": 42}, approved=True)
        ],
        max_turns=3,
        estimated_cost_units=1,
        reserve_cost=budget.reserve,
    )
    assert result.status == "completed"
    assert result.turns == 2
    assert calls == 2
    assert budget.consumed_cost_units == 2.0


def test_tool_loop_fails_before_third_turn_when_shared_budget_is_exhausted():
    calls = 0

    def handler(_request):
        nonlocal calls
        calls += 1
        return {
            "tool_calls": [
                {"call_id": f"call-{calls}", "name": "lookup", "arguments": {}}
            ]
        }

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline-loop", handler)
    budget = AutonomousCostBudget(2)
    with pytest.raises(AutonomousCostBudgetError):
        runtime.invoke_tool_loop(
            "offline-loop",
            ProviderRequest(
                model="offline-model",
                messages=({"role": "user", "content": "continue"},),
                max_output_tokens=32,
                tools=(ProviderTool("lookup", parameters={"type": "object"}),),
            ),
            authorize_and_execute=lambda calls: [
                ProviderToolResult(call.call_id, {"ok": True}, approved=True)
                for call in calls
            ],
            max_turns=4,
            estimated_cost_units=1,
            reserve_cost=budget.reserve,
        )
    assert calls == 2
    assert budget.consumed_cost_units == 2.0
