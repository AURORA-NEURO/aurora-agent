from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_STREAM_COMPLETION_SCHEMA,
    AutonomousStreamRuntime,
    LLMRuntime,
    ProviderError,
    ProviderRequest,
    ProviderStreamEvent,
)
from prism_sdk.domain_tools import AUTONOMOUS_DOMAIN_NAMES


def _request(model: str = "stream-model", *, messages: tuple[dict[str, object], ...] | None = None) -> ProviderRequest:
    return ProviderRequest(
        model=model,
        messages=messages or ({"role": "user", "content": "Return a bounded stream."},),
        max_output_tokens=128,
    )


def _event(provider: str, model: str, sequence: int, text: str, *, done: bool = False) -> ProviderStreamEvent:
    return ProviderStreamEvent(
        provider=provider,
        model=model,
        sequence=sequence,
        event_type="fixture.done" if done else "fixture.text",
        text_delta=text,
        done=done,
        usage={"output_tokens": 1} if done else {},
    )


def test_autonomous_stream_compacts_context_and_returns_metadata_only_completion() -> None:
    seen: list[ProviderRequest] = []

    def stream_handler(request: ProviderRequest) -> list[ProviderStreamEvent]:
        seen.append(request)
        return [
            _event("stream-offline", request.model, 0, "bounded "),
            _event("stream-offline", request.model, 1, "answer", done=True),
        ]

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("stream-offline", lambda _request: "unused", stream_handler=stream_handler)
    handle = AutonomousStreamRuntime(runtime).open(
        _request(
            messages=(
                {"role": "system", "content": "Protect the contract."},
                {"role": "user", "content": "old context to remove"},
                {"role": "assistant", "content": "old answer to remove"},
                {"role": "user", "content": "current task"},
            )
        ),
        provider="stream-offline",
        model="stream-model",
        context_budget={"max_input_tokens": 75, "preserve_recent_messages": 1},
    )

    assert handle.context_budget is not None
    assert handle.context_budget["status"] == "compacted"
    assert handle.completion is None
    events = list(handle.events)
    completion = handle.completion
    assert completion is not None
    assert [event.text_delta for event in events] == ["bounded ", "answer"]
    assert completion.schema == AUTONOMOUS_STREAM_COMPLETION_SCHEMA
    assert completion.status == "completed"
    assert completion.event_count == 2
    assert completion.text_delta_bytes == len("bounded answer".encode("utf-8"))
    assert completion.done_seen is True
    assert len(completion.provider_invocations) == 1
    assert completion.provider_invocations[0]["outcome"] == "success"
    assert completion.provider_failover is None
    assert "bounded" not in json.dumps(completion.to_dict())
    assert [message["content"] for message in seen[0].messages] == [
        "Protect the contract.",
        "current task",
    ]
    with pytest.raises(ProviderError, match="single-consumer"):
        _ = handle.events


def test_autonomous_stream_fails_over_only_before_first_event() -> None:
    calls: list[str] = []

    def primary_stream(_request: ProviderRequest) -> list[ProviderStreamEvent]:
        calls.append("primary")
        raise ProviderError("temporary outage", retryable=True, status_code=503)

    def backup_stream(request: ProviderRequest) -> list[ProviderStreamEvent]:
        calls.append("backup")
        return [_event("stream-backup", request.model, 0, "recovered", done=True)]

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("stream-primary", lambda _request: "unused", stream_handler=primary_stream)
    runtime.register_in_memory_provider("stream-backup", lambda _request: "unused", stream_handler=backup_stream)
    handle = AutonomousStreamRuntime(runtime).open(
        _request("primary-model"),
        provider="stream-primary",
        model="primary-model",
        fallbacks=({"provider": "stream-backup", "model": "backup-model"},),
        max_provider_failovers=1,
    )

    events = list(handle.events)
    completion = handle.completion
    assert completion is not None
    assert calls == ["primary", "backup"]
    assert events[0].provider == "stream-backup"
    assert events[0].model == "backup-model"
    assert completion.status == "completed"
    assert completion.provider_failover is not None
    assert completion.provider_failover["fallback_count"] == 1
    assert [row["outcome"] for row in completion.provider_invocations] == ["failure", "success"]


def test_autonomous_stream_refuses_partial_replay_and_marks_abandonment() -> None:
    calls: list[str] = []

    def partial_stream(request: ProviderRequest):
        calls.append("partial")
        yield _event("stream-partial", request.model, 0, "partial")
        raise ProviderError("connection lost", retryable=True, status_code=503)

    def backup_stream(request: ProviderRequest) -> list[ProviderStreamEvent]:
        calls.append("backup")
        return [_event("stream-backup", request.model, 0, "unsafe", done=True)]

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("stream-partial", lambda _request: "unused", stream_handler=partial_stream)
    runtime.register_in_memory_provider("stream-backup", lambda _request: "unused", stream_handler=backup_stream)
    handle = AutonomousStreamRuntime(runtime).open(
        _request(),
        provider="stream-partial",
        model="stream-model",
        fallbacks=(("stream-backup", "backup-model"),),
        max_provider_failovers=1,
    )
    with pytest.raises(ProviderError):
        list(handle.events)
    completion = handle.completion
    assert completion is not None
    assert calls == ["partial"]
    assert completion.status == "failed"
    assert completion.event_count == 1
    assert completion.provider_failover is None

    abandoned = AutonomousStreamRuntime(runtime).open(
        _request(),
        provider="stream-partial",
        model="stream-model",
    )
    iterator = abandoned.events
    next(iterator)
    iterator.close()
    abandoned_completion = abandoned.completion
    assert abandoned_completion is not None
    assert abandoned_completion.status == "abandoned"


def test_autonomous_stream_contract_is_available_for_every_builtin_domain() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "stream-domains",
        lambda _request: "unused",
        stream_handler=lambda request: [_event("stream-domains", request.model, 0, "domain", done=True)],
    )
    stream_runtime = AutonomousStreamRuntime(runtime)
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        handle = stream_runtime.open(
            _request(),
            provider="stream-domains",
            model="domain-model",
            selection={"strategy": f"domain:{domain}"},
        )
        list(handle.events)
        assert handle.completion is not None
        assert handle.completion.status == "completed"
