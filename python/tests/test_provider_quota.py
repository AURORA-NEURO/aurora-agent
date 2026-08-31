from __future__ import annotations

import pytest

from prism_sdk import (
    JsonProviderQuotaPersistence,
    LLMRuntime,
    ProviderQuotaController,
    ProviderQuotaError,
    ProviderError,
    ProviderRequest,
    ProviderStreamEvent,
    validate_provider_quota_snapshot,
)


def _request() -> ProviderRequest:
    return ProviderRequest(
        model="offline-model",
        messages=({"role": "user", "content": "offline fixture prompt"},),
        max_output_tokens=256,
    )


def test_provider_model_quota_reserves_settles_and_rolls_windows() -> None:
    now = [1_000_000.0]
    quota = ProviderQuotaController(clock=lambda: now[0])
    quota.set_policy(
        {
            "provider": "offline",
            "model": "offline-model",
            "windowMs": 1_000,
            "maxRequests": 2,
            "maxOutputTokens": 512,
            "maxConcurrent": 1,
        }
    )

    first = quota.reserve(
        {
            "provider": "offline",
            "model": "offline-model",
            "inputTokens": 12,
            "outputTokens": 256,
        }
    )
    assert quota.status("offline", "offline-model")[0]["requests_reserved"] == 1
    with pytest.raises(ProviderQuotaError) as raised:
        quota.reserve(
            {
                "provider": "offline",
                "model": "offline-model",
                "inputTokens": 1,
                "outputTokens": 1,
            }
        )
    assert raised.value.code == "quota_exceeded"
    assert "concurrent" in raised.value.dimensions
    with pytest.raises(ProviderError):
        first.settle()
    first.mark_dispatched()
    settlement = first.settle({"inputTokens": 8, "outputTokens": 32, "costUnits": 1.5})
    assert settlement["charged_output_tokens"] == 32
    assert quota.status("offline", "offline-model")[0]["requests_used"] == 1

    second = quota.reserve(
        {
            "provider": "offline",
            "model": "offline-model",
            "inputTokens": 1,
            "outputTokens": 1,
        }
    )
    second.mark_dispatched()
    second.settle()
    with pytest.raises(ProviderQuotaError) as raised:
        quota.reserve(
            {
                "provider": "offline",
                "model": "offline-model",
                "inputTokens": 1,
                "outputTokens": 1,
            }
        )
    assert "requests" in raised.value.dimensions
    now[0] += 1_000
    next_window = quota.reserve(
        {
            "provider": "offline",
            "model": "offline-model",
            "inputTokens": 1,
            "outputTokens": 1,
        }
    )
    next_window.release()
    assert quota.status("offline", "offline-model")[0]["window_start"] == 1_001_000


def test_provider_quota_snapshot_is_canonical_digest_checked_and_metadata_only() -> (
    None
):
    quota = ProviderQuotaController(clock=lambda: 2_000_000.0)
    quota.set_policy(
        {
            "provider": "offline",
            "model": "offline-model",
            "windowMs": 10_000,
            "maxRequests": 5,
            "maxOutputTokens": 512,
        }
    )
    reservation = quota.reserve(
        {
            "provider": "offline",
            "model": "offline-model",
            "inputTokens": 2,
            "outputTokens": 8,
        }
    )
    reservation.mark_dispatched()
    reservation.settle({"inputTokens": 2, "outputTokens": 3})
    snapshot = quota.snapshot()
    assert (
        snapshot["retention"]
        == "metadata_only;provider_model_counters_no_prompts_credentials_or_payloads"
    )
    assert snapshot["secret_material"] == "never_returned"

    encoded: list[str | None] = [None]
    persistence = JsonProviderQuotaPersistence(
        type(
            "Store",
            (),
            {
                "read": lambda self: encoded[0],
                "write": lambda self, value: encoded.__setitem__(0, value),
            },
        )()
    )
    persistence.write(snapshot)
    assert persistence.read() == snapshot
    assert encoded[0] is not None
    encoded[0] = encoded[0].replace('"requests":1', '"requests":2')
    with pytest.raises(ProviderError):
        persistence.read()
    with pytest.raises(ProviderError):
        validate_provider_quota_snapshot({**snapshot, "snapshot_digest": "0" * 64})


def test_llm_runtime_enforces_shared_quota_at_in_memory_transport_and_stream() -> None:
    calls = [0]
    quota = ProviderQuotaController(clock=lambda: 3_000_000.0)
    quota.set_policy(
        {
            "provider": "offline",
            "model": "offline-model",
            "windowMs": 10_000,
            "maxRequests": 1,
            "maxOutputTokens": 512,
        }
    )
    runtime = LLMRuntime(provider_quota=quota)

    def handler(_request: ProviderRequest) -> dict[str, object]:
        calls[0] += 1
        return {
            "output_text": "deterministic fixture",
            "usage": {"input_tokens": 4, "output_tokens": 6},
        }

    runtime.register_in_memory_provider("offline", handler)
    assert runtime.invoke("offline", _request()).text == "deterministic fixture"
    assert calls[0] == 1
    with pytest.raises(ProviderQuotaError):
        runtime.invoke("offline", _request())
    assert calls[0] == 1

    stream_quota = ProviderQuotaController(clock=lambda: 4_000_000.0)
    stream_quota.set_policy(
        {
            "provider": "offline-stream",
            "model": "offline-model",
            "windowMs": 10_000,
            "maxRequests": 1,
            "maxOutputTokens": 512,
        }
    )
    stream_runtime = LLMRuntime(provider_quota=stream_quota)

    def stream_handler(request: ProviderRequest):
        yield ProviderStreamEvent(
            provider="offline-stream",
            model=request.model,
            sequence=0,
            event_type="text",
            text_delta="bounded",
        )
        yield ProviderStreamEvent(
            provider="offline-stream",
            model=request.model,
            sequence=1,
            event_type="done",
            done=True,
        )

    stream_runtime.register_in_memory_provider(
        "offline-stream", lambda _request: "unused", stream_handler=stream_handler
    )
    assert len(list(stream_runtime.invoke_stream("offline-stream", _request()))) == 2
    with pytest.raises(ProviderQuotaError):
        list(stream_runtime.invoke_stream("offline-stream", _request()))
