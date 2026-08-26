from __future__ import annotations

import copy
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousRunAnalyticsLedger,
    AutonomousRunAnalyticsLedgerPersistenceCoordinator,
    AutonomousRunAnalyticsLedgerPolicy,
    InMemoryAutonomousRunTraceStore,
    JsonAutonomousRunAnalyticsLedgerPersistence,
    TransactionalJsonAutonomousRunAnalyticsLedgerPersistence,
    AutonomousRunTraceSession,
    analyze_autonomous_run_trace,
    validate_autonomous_run_analytics_ledger_snapshot,
)
from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError


def _report(marker: str, domain: str):
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 100)
    session = AutonomousRunTraceSession(
        store,
        run_id=f"run-{marker}",
        task_digest=marker * 64,
        domains=(domain,),
    )
    session.started()
    session.record(
        phase="provider_invocation_finished",
        status="running",
        provider=f"provider-{marker}",
        model=f"model-{marker}",
        latency_ms=10.0 + (1 if marker == "b" else 0),
        input_tokens=20,
        output_tokens=8,
        tool_count=1,
    )
    session.complete(status="completed")
    return analyze_autonomous_run_trace(store.snapshot())


class _TransactionalTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected: str | None, value: str) -> bool:
        current = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if current != expected:
            return False
        self.value = value
        return True


def test_ledger_aggregates_reports_across_all_domain_dimensions_and_deduplicates() -> None:
    first = _report("a", "coding")
    second = _report("b", "science")
    ledger = AutonomousRunAnalyticsLedger(clock=lambda: 123.456)

    accepted = ledger.ingest(first, ingested_at=1_000)
    assert accepted.status == "accepted"
    duplicate = ledger.ingest(first, ingested_at=9_999)
    assert duplicate.status == "duplicate"
    assert duplicate.retained_report_count == 1

    conflict = first.to_dict()
    conflict["policy_digest"] = "f" * 64
    conflict["report_digest"] = content_digest({key: value for key, value in conflict.items() if key != "report_digest"})
    assert ledger.ingest(conflict).status == "conflict"

    ledger.ingest(second, ingested_at=2_000)
    summary = ledger.summary()
    assert summary.report_count == 2
    assert summary.source_snapshot_count == 2
    assert summary.accepted_report_count == 2
    assert summary.evicted_report_count == 0
    assert summary.event_count == 6
    assert summary.run_count == 2
    assert summary.provider_invocations == 2
    assert summary.latency_mean_ms == pytest.approx(10.5)
    assert summary.latency_p50_ms is None
    assert summary.latency_p95_ms is None
    assert summary.latency_quantile_posture == "not_aggregated_from_report_quantiles"
    assert len(summary.domains) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert {row.identity for row in summary.domains if row.observed} == {"coding", "science"}
    assert {row.identity for row in summary.providers} == {"provider-a", "provider-b"}
    assert {row.identity for row in summary.models} == {"provider-a/model-a", "provider-b/model-b"}
    assert next(row for row in summary.domains if row.identity == "evaluation").measurement_state == "unmeasured"

    wire = summary.to_dict()
    assert wire["summary_digest"] == summary.summary_digest
    assert set(wire).isdisjoint({"prompt", "response", "messages", "credentials", "arguments", "payload"})


def test_ledger_is_bounded_digest_validated_and_restart_safe() -> None:
    ledger = AutonomousRunAnalyticsLedger(AutonomousRunAnalyticsLedgerPolicy(max_reports=1))
    first = _report("a", "coding")
    second = _report("b", "science")
    ledger.ingest(first, ingested_at=10)
    ledger.ingest(second, ingested_at=20)
    assert len(ledger.entries) == 1
    assert ledger.summary().evicted_report_count == 1
    assert ledger.history(limit=1)[0].report.source_snapshot_digest == second.source_snapshot_digest

    snapshot = ledger.snapshot()
    assert validate_autonomous_run_analytics_ledger_snapshot(snapshot)["snapshot_digest"] == snapshot["snapshot_digest"]
    restored = AutonomousRunAnalyticsLedger(AutonomousRunAnalyticsLedgerPolicy(max_reports=1))
    restored.restore(snapshot)
    assert restored.summary().to_dict() == ledger.summary().to_dict()

    tampered = copy.deepcopy(snapshot)
    tampered["entries"][0]["report"]["event_count"] += 1
    with pytest.raises(ArgumentError):
        restored.restore(tampered)

    with pytest.raises(ArgumentError):
        AutonomousRunAnalyticsLedgerPolicy(max_reports=0)
    with pytest.raises(ArgumentError):
        AutonomousRunAnalyticsLedgerPolicy(expected_domains=("not-a-domain",))


def test_ledger_json_persistence_uses_cas_and_agent_facade() -> None:
    store = _TransactionalTextStore()
    persistence = TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(store)
    ledger = AutonomousRunAnalyticsLedger()
    ledger.ingest(_report("a", "coding"), ingested_at=10)
    coordinator = AutonomousRunAnalyticsLedgerPersistenceCoordinator(ledger, persistence)
    saved = coordinator.flush()
    assert saved["snapshot_digest"] == json.loads(store.value)["snapshot_digest"]

    restarted = AutonomousRunAnalyticsLedger()
    restarted_coordinator = AutonomousRunAnalyticsLedgerPersistenceCoordinator(restarted, persistence)
    assert restarted_coordinator.restore() is not None
    assert restarted.summary().report_count == 1

    stale = AutonomousRunAnalyticsLedgerPersistenceCoordinator(AutonomousRunAnalyticsLedger(), persistence)
    assert stale.restore() is not None
    restarted.ingest(_report("b", "science"), ingested_at=20)
    restarted_coordinator.flush()
    with pytest.raises(ArgumentError, match="compare-and-swap"):
        stale.flush()

    plain = JsonAutonomousRunAnalyticsLedgerPersistence(store)
    assert plain.read() is not None
    agent = AutonomousAgent.__new__(AutonomousAgent)
    assert agent.create_run_analytics_ledger({"max_reports": 1}).policy.max_reports == 1
