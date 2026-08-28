import json

import pytest

from prism_sdk.autonomous_context_budget import (
    AUTONOMOUS_CONTEXT_BUDGET_SCHEMA,
    AutonomousContextBudgetError,
    compact_autonomous_provider_request,
)
from prism_sdk.llm_runtime import CredentialStore, LLMRuntime, ProviderRequest, ProviderTool, ProviderToolResult
from prism_sdk.brain import AutonomousBrain


def request(messages):
    return ProviderRequest(model="offline-model", messages=tuple(messages), max_output_tokens=128)


def test_context_budget_drops_oldest_removable_messages_and_protects_instructions_and_task():
    result = compact_autonomous_provider_request(
        request(
            (
                {"role": "system", "content": "Never disclose credentials."},
                {"role": "user", "content": "old task context that can be removed"},
                {"role": "assistant", "content": "old answer that can be removed"},
                {"role": "user", "content": "latest user task that must remain"},
            )
        ),
        {"max_input_tokens": 75, "preserve_recent_messages": 1},
    )

    assert result.plan.schema == AUTONOMOUS_CONTEXT_BUDGET_SCHEMA
    assert result.plan.status == "compacted"
    assert result.plan.dropped_message_count > 0
    assert [message["content"] for message in result.request.messages] == [
        "Never disclose credentials.",
        "latest user task that must remain",
    ]
    assert result.plan.protected_instruction_count == 1
    assert result.plan.messages_after == 2
    assert len(result.plan.plan_digest) >= 32


def test_context_budget_removes_assistant_tool_calls_and_results_atomically():
    result = compact_autonomous_provider_request(
        request(
            (
                {"role": "system", "content": "Use only approved tools."},
                {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": ({"id": "call-old", "name": "lookup", "arguments": "{\"query\":\"private\"}"},),
                },
                {"role": "tool", "tool_call_id": "call-old", "content": "private result"},
                {"role": "assistant", "content": "old synthesis"},
            )
        ),
        {"max_input_tokens": 45, "preserve_recent_messages": 0},
    )

    assert [message["role"] for message in result.request.messages] == ["system"]
    assert result.plan.dropped_message_indexes == (1, 2, 3)
    assert result.plan.tool_turns_dropped == 1


def test_context_budget_fails_closed_when_protected_context_cannot_fit():
    with pytest.raises(AutonomousContextBudgetError) as raised:
        compact_autonomous_provider_request(
            request(({"role": "system", "content": "This protected instruction is intentionally too large."},)),
            {"max_input_tokens": 1, "preserve_recent_messages": 0},
        )
    assert raised.value.code == "invalid_request"


def test_unchanged_context_returns_original_request_and_digest_only_plan():
    secret = "https://private.example/opaque-image.png"
    original = request(
        (
            {"role": "system", "content": "Protect private data."},
            {"role": "user", "content": [{"type": "image_url", "url": secret, "detail": "high"}]},
        )
    )
    result = compact_autonomous_provider_request(
        original,
        {"max_input_tokens": 10_000, "preserve_recent_messages": 1},
    )

    assert result.plan.status == "unchanged"
    assert result.request is original
    encoded = json.dumps(result.plan.to_dict(), sort_keys=True)
    assert secret not in encoded
    assert "Protect private data." not in encoded
    assert result.plan.to_dict()["content_retention"] == "provider_content_not_retained_in_plan"


def test_adaptive_invocation_selects_and_dispatches_the_same_compacted_request():
    seen = []

    class Workspace:
        def tool(self, name, arguments=None):
            if name == "brain_model_select":
                return {"selected_model": {"provider": "offline", "model": "offline-model"}}
            if name == "brain_prompt_assemble":
                return {
                    "messages": [
                        {"role": "system", "content": "Keep the contract."},
                        {"role": "user", "content": "old context to remove"},
                        {"role": "assistant", "content": "old response to remove"},
                        {"role": "user", "content": "current task"},
                    ],
                    "prompt_digest": "prompt-digest",
                }
            if name == "brain_plan":
                return {
                    "ok": True,
                    "plan": {
                        "requires_approval": False,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "plan-digest",
                    },
                }
            raise AssertionError(f"unexpected tool {name}")

    runtime = LLMRuntime(CredentialStore())
    runtime.register_in_memory_provider("offline", lambda request: (seen.append(request) or {"output_text": "bounded answer"}))
    result = AutonomousBrain(Workspace(), runtime).run_adaptive(
        task="select and invoke within one context contract",
        model_candidates=[
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": ["reasoning"],
                "context_window_tokens": 100_000,
                "max_output_tokens": 1_000,
                "quality": 0.9,
                "latency_ms": 10,
                "cost_per_million_tokens": 0,
                "reliability": 0.9,
                "requires_credential": False,
            }
        ],
        prompt={"max_input_tokens": 100},
        plan={"requires_approval": False, "steps": [{"effect": "provider_call"}]},
        credentials={},
        approve_provider_call=True,
        context_budget={"max_input_tokens": 75, "preserve_recent_messages": 1},
    )

    assert result.response is not None
    assert result.response.text == "bounded answer"
    assert result.context_budget["status"] == "compacted"
    assert [message["content"] for message in seen[0].messages] == ["Keep the contract.", "current task"]


def test_tool_loop_budget_protects_newest_approved_continuation_when_recent_tail_is_zero():
    seen = []
    calls = 0

    def handler(provider_request):
        nonlocal calls
        seen.append(provider_request)
        calls += 1
        if calls == 1:
            return {"tool_calls": [{"call_id": "call-1", "name": "lookup", "arguments": {"query": "current"}}]}
        assert provider_request.messages[-1]["role"] == "tool"
        return {"output_text": "tool result incorporated"}

    runtime = LLMRuntime(CredentialStore())
    runtime.register_in_memory_provider("offline", handler)
    result = runtime.invoke_tool_loop(
        "offline",
        ProviderRequest(
            model="offline-model",
            messages=(
                {"role": "system", "content": "Keep the contract."},
                {"role": "user", "content": "old context to remove"},
                {"role": "user", "content": "current task"},
            ),
            max_output_tokens=128,
            tools=(
                ProviderTool(
                    "lookup", "Read bounded data", {"type": "object"}
                ),
            ),
        ),
        authorize_and_execute=lambda tool_calls: tuple(
            ProviderToolResult(
                call.call_id, {"value": 42}, approved=True
            )
            for call in tool_calls
        ),
        context_budget={"max_input_tokens": 155, "preserve_recent_messages": 0},
    )

    assert result.status == "completed"
    assert result.turns == 2
    assert len(seen) == 2
    assert seen[1].messages[-1]["role"] == "tool"
