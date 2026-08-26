import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    JsonLLMRuntimeHealthSnapshotPersistence,
    LLMRuntime,
    LLMRuntimeHealthPersistenceCoordinator,
    ProviderError,
    TransactionalJsonLLMRuntimeHealthSnapshotPersistence,
)
from prism_sdk.brain import BrainRunError
from prism_sdk.llm_runtime import ProviderRequest


class _CasTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


def _request(model: str) -> ProviderRequest:
    return ProviderRequest(
        model=model,
        messages=({"role": "user", "content": "transient runtime-health test prompt"},),
    )


def _register_domain_providers(runtime: LLMRuntime) -> None:
    for domain in AUTONOMOUS_DOMAINS:
        provider = f"offline-{domain}"

        def handler(_request: ProviderRequest, *, _domain: str = domain) -> dict[str, object]:
            if _domain == "operations":
                raise ProviderError("synthetic transport failure", retryable=True, status_code=503)
            return {"output_text": "transient response", "usage": {"input_tokens": 3, "output_tokens": 2}}

        runtime.register_in_memory_provider(
            provider,
            handler,
            circuit_breaker_failure_threshold=1,
        )


def _populate_runtime(runtime: LLMRuntime) -> None:
    _register_domain_providers(runtime)
    for domain in AUTONOMOUS_DOMAINS:
        provider = f"offline-{domain}"
        request = _request(f"model-{domain}")
        if domain == "operations":
            with pytest.raises(ProviderError, match="in-memory provider handler failed"):
                runtime.invoke(provider, request)
        else:
            runtime.invoke(provider, request)


def test_python_runtime_health_persistence_restarts_every_domain_and_circuit_state() -> None:
    backend = _CasTextStore()
    source_runtime = LLMRuntime()
    _populate_runtime(source_runtime)
    source = LLMRuntimeHealthPersistenceCoordinator(
        source_runtime,
        TransactionalJsonLLMRuntimeHealthSnapshotPersistence(backend),
    )
    source_agent = AutonomousAgent(object(), source_runtime, runtime_health_persistence=source)

    flushed = source_agent.flush_runtime_health()
    assert flushed["snapshot_generation"] == 1
    assert len(flushed["providers"]) == len(AUTONOMOUS_DOMAINS)
    assert len(flushed["models"]) == len(AUTONOMOUS_DOMAINS)
    assert "transient runtime-health test prompt" not in json.dumps(flushed)
    assert "transient response" not in json.dumps(flushed)
    assert source_runtime.provider_status("offline-operations")["circuit"] == "open"
    assert source_runtime.health_snapshot() == flushed

    partially_registered = LLMRuntime()
    partially_registered.register_in_memory_provider("offline-coding", lambda _request: "unused")
    with pytest.raises(ProviderError, match="unregistered provider"):
        partially_registered.restore_health(flushed)
    assert partially_registered.provider_status("offline-coding")["attempts"] == 0

    restored_runtime = LLMRuntime()
    _register_domain_providers(restored_runtime)
    restored = LLMRuntimeHealthPersistenceCoordinator(
        restored_runtime,
        TransactionalJsonLLMRuntimeHealthSnapshotPersistence(backend),
    )
    restored_agent = AutonomousAgent(object(), restored_runtime, runtime_health_persistence=restored)
    restored_snapshot = restored_agent.restore_transport_health()

    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert restored_runtime.provider_status("offline-operations")["circuit"] == "open"
    assert restored_runtime.provider_status("offline-coding")["attempts"] == 1
    assert restored_runtime.model_status("offline-coding", "model-coding")["successes"] == 1
    assert restored_runtime.model_status("offline-operations", "model-operations")["failures"] == 1


def test_python_runtime_health_persistence_is_digest_bound_and_cas_fenced() -> None:
    backend = _CasTextStore()
    source_runtime = LLMRuntime()
    _populate_runtime(source_runtime)
    source = LLMRuntimeHealthPersistenceCoordinator(
        source_runtime,
        TransactionalJsonLLMRuntimeHealthSnapshotPersistence(backend),
    )
    source.flush()

    stale_runtime = LLMRuntime()
    _register_domain_providers(stale_runtime)
    stale = LLMRuntimeHealthPersistenceCoordinator(
        stale_runtime,
        TransactionalJsonLLMRuntimeHealthSnapshotPersistence(backend),
    )
    stale.restore()
    source_runtime.invoke("offline-coding", _request("model-coding"))
    source.flush()
    with pytest.raises(ProviderError, match="compare-and-swap conflict"):
        stale.flush()

    assert backend.value is not None
    tampered = json.loads(backend.value)
    tampered["secret_material"] = "provider-secret"
    backend.value = json.dumps(tampered, separators=(",", ":"), sort_keys=True)
    with pytest.raises(ProviderError, match="markers|digest"):
        JsonLLMRuntimeHealthSnapshotPersistence(backend).read()


def test_python_agent_runtime_health_lifecycle_fails_closed_and_enforces_runtime_identity() -> None:
    runtime = LLMRuntime()
    with pytest.raises(BrainRunError, match="runtime health persistence is not configured"):
        AutonomousAgent(object(), runtime).flush_runtime_health()
    with pytest.raises(BrainRunError, match="runtime health persistence is not configured"):
        AutonomousAgent(object(), runtime).restore_runtime_health()
    with pytest.raises(BrainRunError, match="LLMRuntimeHealthPersistenceCoordinator"):
        AutonomousAgent(object(), runtime, runtime_health_persistence=object())

    other_runtime = LLMRuntime()
    store = _CasTextStore()
    coordinator = LLMRuntimeHealthPersistenceCoordinator(
        other_runtime,
        TransactionalJsonLLMRuntimeHealthSnapshotPersistence(store),
    )
    with pytest.raises(BrainRunError, match="bound to the supplied runtime"):
        AutonomousAgent(object(), runtime, runtime_health_persistence=coordinator)
