from __future__ import annotations

import copy
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousRunAnalyticsLedger,
    AutonomousRunAnalyticsController,
    AutonomousRunTraceRegistryController,
    AutonomousRunObservabilityController,
    AutonomousRunTraceRegistry,
    TransactionalJsonAutonomousRunTraceRegistryPersistence,
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


def test_agent_analytics_controller_is_restart_safe_and_persistence_explicit() -> None:
    source = InMemoryAutonomousRunTraceStore(clock=lambda: 5_000)
    session = AutonomousRunTraceSession(
        source,
        run_id="facade-analytics-all-domains",
        task_digest="d" * 64,
        domains=tuple(AUTONOMOUS_DOMAIN_NAMES),
    )
    session.started()
    session.record(
        phase="provider_invocation_finished",
        status="running",
        provider="offline",
        model="offline-model",
        input_tokens=7,
        output_tokens=5,
        tool_count=3,
    )
    session.complete(status="completed")

    agent = AutonomousAgent.__new__(AutonomousAgent)
    store = _TransactionalTextStore()
    controller = agent.create_run_analytics_controller(
        AutonomousRunAnalyticsLedger(),
        TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(store),
    )
    with pytest.raises(ArgumentError, match="must restore before use"):
        controller.summary()
    assert controller.restore().status == "empty"

    analyzed = controller.analyze_and_ingest(source.snapshot(), ingested_at=7_000)
    assert analyzed.ingest.status == "accepted"
    assert analyzed.persisted is True
    assert analyzed.controller.status == "ingested"
    assert analyzed.report.run_count == 1
    summary = controller.summary()
    assert summary.report_count == 1
    assert len(summary.domains) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert {row.identity for row in summary.domains if row.observed} == set(AUTONOMOUS_DOMAIN_NAMES)
    assert next(row for row in summary.providers if row.identity == "offline").provider_invocations == 1
    assert next(row for row in summary.models if row.identity == "offline/offline-model").provider_invocations == 1
    assert controller.history(limit=1)[0].report.report_digest == analyzed.report.report_digest
    assert controller.verify_integrity().verified is True
    wire = controller.snapshot()
    assert "facade-analytics-all-domains" not in str(wire)
    assert "prompt" not in wire and "response" not in wire and "credentials" not in wire

    duplicate = controller.ingest(analyzed.report, ingested_at=99_999)
    assert duplicate.ingest.status == "duplicate"
    assert duplicate.persisted is True

    restored_controller = AutonomousRunAnalyticsController(
        agent,
        AutonomousRunAnalyticsLedger(),
        TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(store),
    )
    restored = restored_controller.restore()
    assert restored.status == "restored"
    assert restored.persisted is True
    assert restored.summary.report_count == 1
    assert restored_controller.verify_integrity().verified is True

    class _FailingStore:
        def read(self) -> None:
            return None

        def write(self, _value: str) -> None:
            raise RuntimeError("analytics persistence unavailable")

    failing = agent.create_run_analytics_controller(
        AutonomousRunAnalyticsLedger(),
        JsonAutonomousRunAnalyticsLedgerPersistence(_FailingStore()),
    )
    failing.restore()
    failed = failing.ingest(analyzed.report)
    assert failed.ingest.status == "accepted"
    assert failed.persisted is False
    assert failed.controller.status == "persistence_failed"
    assert failing.summary().report_count == 1


def test_agent_coordinates_one_trace_snapshot_across_registry_and_analytics() -> None:
    source = InMemoryAutonomousRunTraceStore(clock=lambda: 8_000)
    session = AutonomousRunTraceSession(
        source,
        run_id="facade-observability-all-domains",
        task_digest="1" * 64,
        domains=tuple(AUTONOMOUS_DOMAIN_NAMES),
    )
    session.started()
    session.record(
        phase="provider_invocation_finished",
        status="running",
        provider="offline",
        model="offline-model",
        input_tokens=8,
        output_tokens=6,
        tool_count=4,
    )
    session.complete(status="completed")

    agent = AutonomousAgent.__new__(AutonomousAgent)
    trace_store = _TransactionalTextStore()
    analytics_store = _TransactionalTextStore()
    trace_registry = agent.create_trace_registry_controller(
        AutonomousRunTraceRegistry({"max_runs": 16, "max_events": 128, "max_bytes": 250_000}),
        TransactionalJsonAutonomousRunTraceRegistryPersistence(trace_store, max_bytes=250_000),
    )
    run_analytics = agent.create_run_analytics_controller(
        AutonomousRunAnalyticsLedger(),
        TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(analytics_store),
    )
    observability = agent.create_run_observability_controller(trace_registry, run_analytics)
    with pytest.raises(ArgumentError, match="must restore before use"):
        observability.verify_integrity()
    assert observability.restore().controller.status == "empty"

    reads = 0

    class _SingleReadSource:
        def snapshot(self):
            nonlocal reads
            reads += 1
            return source.snapshot()

    run = observability.publish_and_analyze(
        _SingleReadSource(),
        "facade-observability-all-domains",
        ingested_at=9_000,
    )
    assert reads == 1
    assert run.errors == ()
    assert run.controller.status == "published_and_analyzed"
    assert run.trace_registry is not None and run.trace_registry.publication.status == "published"
    assert run.run_analytics is not None and run.run_analytics.ingest.status == "accepted"
    assert run.source_snapshot_digest == run.trace_registry.publication.source_snapshot_digest
    assert run.source_snapshot_digest == run.run_analytics.report.source_snapshot_digest
    assert run.controller.persisted is True
    assert trace_registry.query({"domain": "neuroscience"}).total_matches == 1
    assert len([row for row in run_analytics.summary().domains if row.observed]) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert observability.verify_integrity()["verified"] is True
    wire = run.to_dict()
    assert "private task" not in str(wire)
    assert "offline provider output" not in str(wire) and "sk-" not in str(wire)

    restored_trace = agent.create_trace_registry_controller(
        AutonomousRunTraceRegistry({"max_runs": 16, "max_events": 128, "max_bytes": 250_000}),
        TransactionalJsonAutonomousRunTraceRegistryPersistence(trace_store, max_bytes=250_000),
    )
    restored_analytics = agent.create_run_analytics_controller(
        AutonomousRunAnalyticsLedger(),
        TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(analytics_store),
    )
    restarted = agent.create_run_observability_controller(restored_trace, restored_analytics)
    restored = restarted.restore()
    assert restored.controller.status == "restored"
    assert restored.controller.persisted is True
    assert restored.controller.trace_registry is not None and restored.controller.trace_registry.runs == 1
    assert restored.controller.run_analytics is not None and restored.controller.run_analytics.summary.report_count == 1

    class _FailingStore:
        def read(self):
            return None

        def write(self, _value):
            raise RuntimeError("analytics persistence unavailable")

    partial = agent.create_run_observability_controller(
        agent.create_trace_registry_controller(
            AutonomousRunTraceRegistry({"max_runs": 16, "max_events": 128, "max_bytes": 250_000}),
            TransactionalJsonAutonomousRunTraceRegistryPersistence(_TransactionalTextStore(), max_bytes=250_000),
        ),
        agent.create_run_analytics_controller(
            AutonomousRunAnalyticsLedger(),
            JsonAutonomousRunAnalyticsLedgerPersistence(_FailingStore()),
        ),
    )
    partial.restore()
    failed = partial.publish_and_analyze(source, "facade-observability-all-domains")
    assert failed.controller.status == "persistence_partial"
    assert failed.trace_registry is not None and failed.trace_registry.persisted is True
    assert failed.run_analytics is not None and failed.run_analytics.persisted is False
    assert any(error["scope"] == "analytics_persistence" for error in failed.errors)
    with pytest.raises(ArgumentError, match="bounded identifier"):
        partial.publish_and_analyze(source, "untrusted run id")
