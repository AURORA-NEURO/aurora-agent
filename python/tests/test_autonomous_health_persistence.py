from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    LLMRuntime,
    ProviderHealthLedger,
    ProviderHealthPersistenceCoordinator,
    TransactionalJsonProviderHealthSnapshotPersistence,
)
from prism_sdk.brain import BrainRunError
from prism_sdk.llm_runtime import PROVIDER_OBSERVATION_SCHEMA, ProviderError


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


def _observation(index: int, domain: str) -> dict[str, object]:
    return {
        "schema": PROVIDER_OBSERVATION_SCHEMA,
        "provider": "offline",
        "model": "selection-model",
        "status": "completed" if index % 4 else "provider_refused",
        "outcome": "success" if index % 4 else "failure",
        "latency_ms": index + 1,
        "observed_at": index + 1,
        "domain": domain,
        "capability": "model_selection",
        "risk_class": "bounded_review",
        "failure_class": "provider_error" if index % 4 == 0 else None,
    }


def test_python_agent_health_lifecycle_restarts_all_domains_and_model_priors(tmp_path) -> None:
    backend = _CasTextStore()
    source = ProviderHealthLedger(tmp_path / "source-health.jsonl", max_records=64)
    source_persistence = ProviderHealthPersistenceCoordinator(
        source,
        TransactionalJsonProviderHealthSnapshotPersistence(backend),
    )
    source_agent = AutonomousAgent(
        object(),
        LLMRuntime(),
        health_ledger=source,
        health_persistence=source_persistence,
    )

    for index, domain in enumerate(AUTONOMOUS_DOMAINS, start=1):
        source.record(_observation(index, domain))

    flushed = source_agent.flush_provider_health()
    assert flushed["snapshot_generation"] == 1
    assert flushed["previous_snapshot_digest"] is None
    assert len(flushed["records"]) == len(AUTONOMOUS_DOMAINS)
    assert "provider-secret" not in json.dumps(flushed)
    assert {
        row["domain"] for row in source.records(limit=64)
    } == set(AUTONOMOUS_DOMAINS)

    restored = ProviderHealthLedger(tmp_path / "restored-health.jsonl", max_records=64)
    restored_persistence = ProviderHealthPersistenceCoordinator(
        restored,
        TransactionalJsonProviderHealthSnapshotPersistence(backend),
    )
    restored_agent = AutonomousAgent(
        tmp_path,
        LLMRuntime(),
        health_ledger=restored,
        health_persistence=restored_persistence,
    )
    restored_snapshot = restored_agent.restore_health()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert restored.model_health_snapshot()["offline/selection-model"]["attempts"] == 12

    source.record(_observation(99, AUTONOMOUS_DOMAINS[0]))
    advanced = source_agent.flush_health()
    assert advanced["snapshot_generation"] == 2
    assert advanced["previous_snapshot_digest"] == flushed["snapshot_digest"]

    stale = ProviderHealthLedger(tmp_path / "stale-health.jsonl", max_records=64)
    stale_persistence = ProviderHealthPersistenceCoordinator(
        stale,
        TransactionalJsonProviderHealthSnapshotPersistence(backend),
    )
    stale_agent = AutonomousAgent(
        tmp_path,
        LLMRuntime(),
        health_ledger=stale,
        health_persistence=stale_persistence,
    )
    stale_agent.restore_health()
    source.record(_observation(100, AUTONOMOUS_DOMAINS[1]))
    source_agent.flush_health()
    with pytest.raises(ProviderError, match="compare-and-swap conflict"):
        stale_agent.flush_health()


def test_python_agent_health_lifecycle_fails_closed_without_ledger_or_persistence(tmp_path) -> None:
    without_ledger = AutonomousAgent(object(), LLMRuntime())
    with pytest.raises(BrainRunError, match="has no provider health ledger"):
        without_ledger.restore_health()
    with pytest.raises(BrainRunError, match="has no provider health ledger"):
        without_ledger.flush_provider_health()

    ledger = ProviderHealthLedger(tmp_path / "unconfigured-health.jsonl")
    agent = AutonomousAgent(object(), LLMRuntime(), health_ledger=ledger)
    with pytest.raises(BrainRunError, match="health persistence is not configured"):
        agent.restore_health()
    with pytest.raises(BrainRunError, match="health persistence is not configured"):
        agent.flush_provider_health()


def test_python_agent_rejects_non_coordinator_health_persistence(tmp_path) -> None:
    ledger = ProviderHealthLedger(tmp_path / "health.jsonl")
    with pytest.raises(BrainRunError, match="ProviderHealthPersistenceCoordinator"):
        AutonomousAgent(
            object(),
            LLMRuntime(),
            health_ledger=ledger,
            health_persistence=object(),
        )
