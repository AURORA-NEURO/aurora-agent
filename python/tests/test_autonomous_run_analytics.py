from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousRunTraceAnalyticsPolicy,
    AutonomousRunTraceSession,
    InMemoryAutonomousRunTraceStore,
    analyze_autonomous_run_trace,
    validate_autonomous_run_trace_analytics_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.autonomy import AutonomousAgent


def _trace_snapshot(*, include_failure: bool = True) -> object:
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 100)

    healthy = AutonomousRunTraceSession(store, run_id="healthy", task_digest="a" * 64, domains=("coding",))
    healthy.started()
    healthy.record(
        phase="provider_invocation_finished",
        status="running",
        provider="offline",
        model="reasoner-v1",
        latency_ms=12.0,
        input_tokens=20,
        output_tokens=8,
        tool_count=2,
    )
    healthy.complete(status="completed")

    if include_failure:
        failed = AutonomousRunTraceSession(store, run_id="failed", task_digest="b" * 64, domains=("science",))
        failed.started()
        failed.record(
            phase="provider_invocation_finished",
            status="running",
            provider="offline",
            model="reasoner-v1",
            latency_ms=None,
            input_tokens=11,
            output_tokens=None,
            tool_count=0,
            failure_class="ProviderRuntimeError",
            failure_code="provider_timeout",
        )
        failed.complete(status="failed", failure_code="execution_error")

    incomplete = AutonomousRunTraceSession(store, run_id="incomplete", task_digest="c" * 64, domains=("browser",))
    incomplete.started()
    incomplete.record(phase="plan_compiled", status="running")
    return store.snapshot()


def test_analytics_preserves_unmeasured_state_and_aggregates_dimensions() -> None:
    snapshot = _trace_snapshot()
    report = analyze_autonomous_run_trace(snapshot)

    assert report.status == "attention_required"
    assert report.run_count == 3
    assert report.terminal_run_count == 2
    assert report.incomplete_run_count == 1
    assert report.terminal_coverage == pytest.approx(2 / 3, abs=1e-12)
    assert report.provider_invocations == 2
    assert report.provider_failures == 1
    assert report.provider_failure_rate == 0.5
    assert report.input_tokens == 31
    assert report.output_tokens == 8
    assert report.tool_calls == 2
    assert report.latency_observation_count == 1
    assert report.latency_p95_ms == 12.0
    assert len(report.domains) == len(AUTONOMOUS_DOMAIN_NAMES)

    by_domain = {row.identity: row for row in report.domains}
    assert by_domain["coding"].measurement_state == "measured"
    assert by_domain["coding"].failure_rate == 0.0
    assert by_domain["browser"].measurement_state == "measured"
    assert by_domain["browser"].failure_rate is None
    assert by_domain["browser"].latency_p95_ms is None
    assert by_domain["evaluation"].measurement_state == "unmeasured"
    assert by_domain["evaluation"].failure_rate is None
    assert by_domain["evaluation"].latency_p95_ms is None

    provider = {row.identity: row for row in report.providers}["offline"]
    assert provider.run_count == 2
    assert provider.failure_codes == ("provider_timeout",)
    model = {row.identity: row for row in report.models}["offline/reasoner-v1"]
    assert model.input_token_observation_count == 2
    assert model.output_token_observation_count == 1
    assert any(alert.code == "provider_failure_rate" and alert.severity == "critical" for alert in report.alerts)
    assert any(alert.code == "run_not_terminal" for alert in report.alerts)


def test_analytics_policy_controls_alerts_and_is_digest_bound() -> None:
    snapshot = _trace_snapshot(include_failure=False)
    quiet = AutonomousRunTraceAnalyticsPolicy(
        failure_rate_warning=1.0,
        failure_rate_critical=1.0,
        p95_latency_warning_ms=None,
        p95_latency_critical_ms=None,
        warn_on_incomplete_runs=False,
        warn_on_unmeasured_domains=True,
    )
    report = analyze_autonomous_run_trace(snapshot, quiet)
    assert report.status == "observed"
    assert any(alert.code == "domain_unmeasured" and alert.severity == "info" for alert in report.alerts)
    assert not any(alert.code == "run_not_terminal" for alert in report.alerts)

    wire = report.to_dict()
    assert validate_autonomous_run_trace_analytics_report(wire).report_digest == report.report_digest
    wire["status"] = "degraded"
    with pytest.raises(ArgumentError):
        validate_autonomous_run_trace_analytics_report(wire)

    tampered_snapshot = snapshot.to_dict()
    tampered_snapshot["events"][0]["status"] = "failed"
    with pytest.raises(ArgumentError):
        analyze_autonomous_run_trace(tampered_snapshot)

    with pytest.raises(ArgumentError):
        AutonomousRunTraceAnalyticsPolicy(failure_rate_warning=0.9, failure_rate_critical=0.1)
    with pytest.raises(ArgumentError):
        AutonomousRunTraceAnalyticsPolicy.from_dict({"expected_domains": list(AUTONOMOUS_DOMAIN_NAMES)})


def test_analytics_report_is_value_free_and_available_on_agent_facade() -> None:
    agent = AutonomousAgent.__new__(AutonomousAgent)
    report = agent.analyze_run_trace(_trace_snapshot())
    wire = report.to_dict()
    serialized = json.dumps(wire, sort_keys=True)
    assert "healthy" not in serialized
    assert "reasoner-v1" in serialized
    assert set(wire).isdisjoint({"task", "prompt", "response", "messages", "credentials", "arguments", "payload"})
    assert wire["cost_posture"] == "not_measured_by_trace"
    assert wire["secret_material"] == "never_returned"
