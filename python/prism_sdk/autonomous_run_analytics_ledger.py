"""Bounded longitudinal storage for verified autonomous run analytics.

Individual run-trace analytics answer what one verified snapshot observed.  This module adds the
smallest useful longitudinal boundary: validated reports can be ingested idempotently, retained
under an explicit cap, and aggregated by domain/provider/model without re-reading a prompt or
invoking a provider. Source-digest deduplication is bounded to the currently retained window;
an unbounded identity index would violate the ledger's explicit memory and persistence cap.
Quantiles are intentionally not combined from per-report quantiles; the ledger exposes that
limitation instead of manufacturing a p95.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
import time
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_run_analytics import (
    AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY,
    AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES,
    AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
    AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES,
    AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES,
    AutonomousRunTraceAnalyticsAlert,
    AutonomousRunTraceAnalyticsDimension,
    AutonomousRunTraceAnalyticsPolicy,
    AutonomousRunTraceAnalyticsReport,
    validate_autonomous_run_trace_analytics_report,
)
from .autonomous_run_trace import AUTONOMOUS_RUN_TRACE_STATUSES
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA = "bioprism-python-autonomous-run-analytics-ledger/0.1"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA = "bioprism-python-autonomous-run-analytics-ledger-entry/0.1"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA = "bioprism-python-autonomous-run-analytics-ledger-ingest/0.1"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA = "bioprism-python-autonomous-run-analytics-ledger-summary/0.1"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION = "metadata_only_validated_reports_no_prompts_responses_tool_payloads_or_cost_claims"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY = "verified_report_aggregation_only;not_task_correctness_or_external_health"
AUTONOMOUS_RUN_ANALYTICS_LEDGER_STATUSES = AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES
AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_STATUSES = ("accepted", "duplicate", "conflict")
AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE = "not_aggregated_from_report_quantiles"
MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS = 256
MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES = 512
MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES = 50_000_000
MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_DIMENSIONS = 512


def _text(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > maximum or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, *, maximum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0 or (maximum is not None and value > maximum):
        raise ArgumentError(f"{name} must be a bounded non-negative integer")
    return value


def _timestamp(name: str, value: Any) -> int:
    return _integer(name, value, maximum=253_402_300_799_999)


def _mapping(name: str, value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise ArgumentError(f"{name} must be a string-keyed mapping")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerPolicy:
    """Retention and domain coverage controls for a longitudinal analytics ledger."""

    expected_domains: tuple[str, ...] = AUTONOMOUS_DOMAIN_NAMES
    max_reports: int = MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS

    def __post_init__(self) -> None:
        analytics_policy = AutonomousRunTraceAnalyticsPolicy(expected_domains=tuple(self.expected_domains))
        object.__setattr__(self, "expected_domains", analytics_policy.expected_domains)
        object.__setattr__(self, "max_reports", _integer("analytics ledger max_reports", self.max_reports, maximum=MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS))
        if self.max_reports < 1:
            raise ArgumentError("analytics ledger max_reports must be positive")

    def to_dict(self) -> dict[str, Any]:
        return {"expected_domains": list(self.expected_domains), "max_reports": self.max_reports}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousRunAnalyticsLedgerPolicy":
        raw = _mapping("analytics ledger policy", value)
        if set(raw) != {"expected_domains", "max_reports"}:
            raise ArgumentError("analytics ledger policy contains unsupported or missing fields")
        expected = raw["expected_domains"]
        if not isinstance(expected, Sequence) or isinstance(expected, (str, bytes, bytearray)):
            raise ArgumentError("analytics ledger policy expected_domains must be a sequence")
        return cls(expected_domains=tuple(expected), max_reports=raw["max_reports"])


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerEntry:
    """One retained, validated report and its caller/ledger ingestion timestamp."""

    report: AutonomousRunTraceAnalyticsReport
    ingested_at: int
    entry_digest: str

    def __post_init__(self) -> None:
        if not isinstance(self.report, AutonomousRunTraceAnalyticsReport):
            raise ArgumentError("analytics ledger entry report is invalid")
        ingested_at = _timestamp("analytics ledger entry ingested_at", self.ingested_at)
        entry_digest = _digest("analytics ledger entry entry_digest", self.entry_digest)
        expected = content_digest({"report_digest": self.report.report_digest, "ingested_at": ingested_at})
        if entry_digest != expected:
            raise ArgumentError("analytics ledger entry digest does not match its report")
        object.__setattr__(self, "ingested_at", ingested_at)

    @classmethod
    def create(cls, report: AutonomousRunTraceAnalyticsReport, ingested_at: int) -> "AutonomousRunAnalyticsLedgerEntry":
        ingested_at = _timestamp("analytics ledger entry ingested_at", ingested_at)
        return cls(report, ingested_at, content_digest({"report_digest": report.report_digest, "ingested_at": ingested_at}))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA,
            "report": self.report.to_dict(),
            "ingested_at": self.ingested_at,
            "entry_digest": self.entry_digest,
            "retention": AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION,
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousRunAnalyticsLedgerEntry":
        raw = _mapping("analytics ledger entry", value)
        expected = {"schema", "report", "ingested_at", "entry_digest", "retention", "secret_material"}
        if set(raw) != expected or raw["schema"] != AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA:
            raise ArgumentError("analytics ledger entry contains unsupported or missing fields")
        if raw["retention"] != AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION or raw["secret_material"] != "never_returned":
            raise ArgumentError("analytics ledger entry retention markers are invalid")
        if not isinstance(raw["report"], Mapping):
            raise ArgumentError("analytics ledger entry report is malformed")
        return cls(
            report=validate_autonomous_run_trace_analytics_report(raw["report"]),
            ingested_at=raw["ingested_at"],
            entry_digest=raw["entry_digest"],
        )


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerIngestResult:
    status: str
    report_digest: str
    source_snapshot_digest: str
    retained_report_count: int
    evicted_report_count: int

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_STATUSES:
            raise ArgumentError("analytics ledger ingest status is invalid")
        _digest("analytics ledger ingest report_digest", self.report_digest)
        _digest("analytics ledger ingest source_snapshot_digest", self.source_snapshot_digest)
        _integer("analytics ledger retained_report_count", self.retained_report_count)
        _integer("analytics ledger evicted_report_count", self.evicted_report_count)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA,
            "status": self.status,
            "report_digest": self.report_digest,
            "source_snapshot_digest": self.source_snapshot_digest,
            "retained_report_count": self.retained_report_count,
            "evicted_report_count": self.evicted_report_count,
            "retention": AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION,
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerDimension:
    """Additive cross-report rollup with explicit non-aggregated quantile posture."""

    kind: str
    identity: str
    expected: bool
    observed: bool
    measurement_state: str
    report_count: int
    run_count: int
    event_count: int
    terminal_run_count: int
    incomplete_run_count: int
    status_counts: Mapping[str, int]
    provider_invocations: int
    provider_failures: int
    failure_rate: float | None
    latency_observation_count: int
    latency_mean_ms: float | None
    latency_p50_ms: float | None
    latency_p95_ms: float | None
    latency_quantile_posture: str
    input_token_observation_count: int
    output_token_observation_count: int
    input_tokens: int
    output_tokens: int
    tool_calls: int
    failure_codes: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "identity": self.identity,
            "expected": self.expected,
            "observed": self.observed,
            "measurement_state": self.measurement_state,
            "report_count": self.report_count,
            "run_count": self.run_count,
            "event_count": self.event_count,
            "terminal_run_count": self.terminal_run_count,
            "incomplete_run_count": self.incomplete_run_count,
            "status_counts": dict(self.status_counts),
            "provider_invocations": self.provider_invocations,
            "provider_failures": self.provider_failures,
            "failure_rate": self.failure_rate,
            "latency_observation_count": self.latency_observation_count,
            "latency_mean_ms": self.latency_mean_ms,
            "latency_p50_ms": self.latency_p50_ms,
            "latency_p95_ms": self.latency_p95_ms,
            "latency_quantile_posture": self.latency_quantile_posture,
            "input_token_observation_count": self.input_token_observation_count,
            "output_token_observation_count": self.output_token_observation_count,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "tool_calls": self.tool_calls,
            "failure_codes": list(self.failure_codes),
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerAlert:
    code: str
    severity: str
    scope: str
    identity: str
    occurrences: int
    last_report_digest: str
    detail: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "scope": self.scope,
            "identity": self.identity,
            "occurrences": self.occurrences,
            "last_report_digest": self.last_report_digest,
            "detail": self.detail,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRunAnalyticsLedgerSummary:
    """Digest-bound current view over the retained report window."""

    schema: str
    status: str
    report_count: int
    source_snapshot_count: int
    accepted_report_count: int
    evicted_report_count: int
    first_ingested_at: int | None
    last_ingested_at: int | None
    event_count: int
    run_count: int
    terminal_run_count: int
    incomplete_run_count: int
    terminal_coverage: float | None
    provider_invocations: int
    provider_failures: int
    provider_failure_rate: float | None
    input_tokens: int
    output_tokens: int
    tool_calls: int
    latency_observation_count: int
    latency_mean_ms: float | None
    latency_p50_ms: None
    latency_p95_ms: None
    latency_quantile_posture: str
    status_counts: Mapping[str, int]
    alert_counts: Mapping[str, int]
    domains: tuple[AutonomousRunAnalyticsLedgerDimension, ...]
    providers: tuple[AutonomousRunAnalyticsLedgerDimension, ...]
    models: tuple[AutonomousRunAnalyticsLedgerDimension, ...]
    alerts: tuple[AutonomousRunAnalyticsLedgerAlert, ...]
    cost_posture: str
    authority: str
    retention: str
    secret_material: str
    summary_digest: str

    def to_dict(self, *, include_digest: bool = True) -> dict[str, Any]:
        body: dict[str, Any] = {
            "schema": self.schema,
            "status": self.status,
            "report_count": self.report_count,
            "source_snapshot_count": self.source_snapshot_count,
            "accepted_report_count": self.accepted_report_count,
            "evicted_report_count": self.evicted_report_count,
            "first_ingested_at": self.first_ingested_at,
            "last_ingested_at": self.last_ingested_at,
            "event_count": self.event_count,
            "run_count": self.run_count,
            "terminal_run_count": self.terminal_run_count,
            "incomplete_run_count": self.incomplete_run_count,
            "terminal_coverage": self.terminal_coverage,
            "provider_invocations": self.provider_invocations,
            "provider_failures": self.provider_failures,
            "provider_failure_rate": self.provider_failure_rate,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "tool_calls": self.tool_calls,
            "latency_observation_count": self.latency_observation_count,
            "latency_mean_ms": self.latency_mean_ms,
            "latency_p50_ms": self.latency_p50_ms,
            "latency_p95_ms": self.latency_p95_ms,
            "latency_quantile_posture": AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE,
            "status_counts": dict(self.status_counts),
            "alert_counts": dict(self.alert_counts),
            "domains": [row.to_dict() for row in self.domains],
            "providers": [row.to_dict() for row in self.providers],
            "models": [row.to_dict() for row in self.models],
            "alerts": [alert.to_dict() for alert in self.alerts],
            "cost_posture": self.cost_posture,
            "authority": self.authority,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        if include_digest:
            body["summary_digest"] = self.summary_digest
        return body


def _validate_dimension(value: AutonomousRunAnalyticsLedgerDimension) -> AutonomousRunAnalyticsLedgerDimension:
    if value.kind not in {"domain", "provider", "model"}:
        raise ArgumentError("analytics ledger dimension kind is invalid")
    _text("analytics ledger dimension identity", value.identity)
    if value.measurement_state not in AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES:
        raise ArgumentError("analytics ledger dimension measurement state is invalid")
    if value.observed != (value.measurement_state == "measured"):
        raise ArgumentError("analytics ledger dimension measurement state does not reconcile")
    for name in (
        "report_count", "run_count", "event_count", "terminal_run_count", "incomplete_run_count",
        "provider_invocations", "provider_failures", "latency_observation_count",
        "input_token_observation_count", "output_token_observation_count", "input_tokens",
        "output_tokens", "tool_calls",
    ):
        _integer(f"analytics ledger dimension {name}", getattr(value, name))
    if value.terminal_run_count + value.incomplete_run_count != value.run_count or value.provider_failures > value.provider_invocations:
        raise ArgumentError("analytics ledger dimension counts do not reconcile")
    if value.failure_rate is None:
        if value.provider_invocations:
            raise ArgumentError("analytics ledger dimension failure rate cannot be null")
    elif not math.isfinite(value.failure_rate) or not 0 <= value.failure_rate <= 1 or not value.provider_invocations or not math.isclose(value.failure_rate, round(value.provider_failures / value.provider_invocations, 12), abs_tol=1e-12, rel_tol=0):
        raise ArgumentError("analytics ledger dimension failure rate does not reconcile")
    if value.latency_quantile_posture != AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE or value.latency_p50_ms is not None or value.latency_p95_ms is not None:
        raise ArgumentError("analytics ledger dimension quantile posture is invalid")
    if value.latency_mean_ms is not None and (not math.isfinite(value.latency_mean_ms) or value.latency_mean_ms < 0):
        raise ArgumentError("analytics ledger dimension latency mean is invalid")
    if (value.latency_observation_count == 0) != (value.latency_mean_ms is None):
        raise ArgumentError("analytics ledger dimension latency observations do not reconcile")
    if set(value.status_counts) != set(AUTONOMOUS_RUN_TRACE_STATUSES) or any(_integer("analytics ledger status count", item) < 0 for item in value.status_counts.values()) or sum(value.status_counts.values()) != value.run_count:
        raise ArgumentError("analytics ledger dimension status counts are invalid")
    if tuple(sorted(set(value.failure_codes))) != value.failure_codes:
        raise ArgumentError("analytics ledger dimension failure codes are not sorted and unique")
    return value


def _aggregate_dimensions(
    entries: Sequence[AutonomousRunAnalyticsLedgerEntry],
    field: str,
    kind: str,
    expected_domains: Sequence[str],
) -> tuple[AutonomousRunAnalyticsLedgerDimension, ...]:
    accumulators: dict[str, dict[str, Any]] = {}
    if kind == "domain":
        for domain in expected_domains:
            accumulators[domain] = {"expected": True}
    for entry in entries:
        report_rows = getattr(entry.report, field)
        for row in report_rows:
            if row.kind != kind:
                continue
            accumulator = accumulators.setdefault(row.identity, {"expected": kind == "domain" and row.identity in expected_domains})
            if "report_count" not in accumulator:
                accumulator.update({
                    "report_count": 0, "run_count": 0, "event_count": 0, "terminal_run_count": 0,
                        "incomplete_run_count": 0, "status_counts": {status: 0 for status in AUTONOMOUS_RUN_TRACE_STATUSES},
                    "provider_invocations": 0, "provider_failures": 0, "latency_observation_count": 0,
                    "latency_weighted_sum": 0.0, "input_token_observation_count": 0,
                    "output_token_observation_count": 0, "input_tokens": 0, "output_tokens": 0,
                    "tool_calls": 0, "failure_codes": set(), "observed": False,
                })
            accumulator["report_count"] += 1
            for name in ("run_count", "event_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "latency_observation_count", "input_token_observation_count", "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls"):
                accumulator[name] += getattr(row, name)
            for status, count in row.status_counts.items():
                accumulator["status_counts"][status] += count
            if row.latency_mean_ms is not None:
                accumulator["latency_weighted_sum"] += row.latency_mean_ms * row.latency_observation_count
            accumulator["failure_codes"].update(row.failure_codes)
            accumulator["observed"] = accumulator["observed"] or row.observed
    rows: list[AutonomousRunAnalyticsLedgerDimension] = []
    for identity in sorted(accumulators):
        accumulator = accumulators[identity]
        count = accumulator.get("report_count", 0)
        invocations = accumulator.get("provider_invocations", 0)
        latency_count = accumulator.get("latency_observation_count", 0)
        row = AutonomousRunAnalyticsLedgerDimension(
            kind=kind,
            identity=identity,
            expected=bool(accumulator.get("expected", False)),
            observed=bool(accumulator.get("observed", False)),
            measurement_state="measured" if accumulator.get("observed", False) else "unmeasured",
            report_count=count,
            run_count=accumulator.get("run_count", 0),
            event_count=accumulator.get("event_count", 0),
            terminal_run_count=accumulator.get("terminal_run_count", 0),
            incomplete_run_count=accumulator.get("incomplete_run_count", 0),
            status_counts=accumulator.get("status_counts", {status: 0 for status in AUTONOMOUS_RUN_TRACE_STATUSES}),
            provider_invocations=invocations,
            provider_failures=accumulator.get("provider_failures", 0),
            failure_rate=None if not invocations else round(accumulator["provider_failures"] / invocations, 12),
            latency_observation_count=latency_count,
            latency_mean_ms=None if not latency_count else round(accumulator["latency_weighted_sum"] / latency_count, 6),
            latency_p50_ms=None,
            latency_p95_ms=None,
            latency_quantile_posture=AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE,
            input_token_observation_count=accumulator.get("input_token_observation_count", 0),
            output_token_observation_count=accumulator.get("output_token_observation_count", 0),
            input_tokens=accumulator.get("input_tokens", 0),
            output_tokens=accumulator.get("output_tokens", 0),
            tool_calls=accumulator.get("tool_calls", 0),
            failure_codes=tuple(sorted(accumulator.get("failure_codes", set()))),
        )
        rows.append(_validate_dimension(row))
    if len(rows) > MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_DIMENSIONS:
        raise ArgumentError("analytics ledger dimension capacity is exceeded")
    return tuple(rows)


def _summary_for_entries(
    entries: Sequence[AutonomousRunAnalyticsLedgerEntry],
    policy: AutonomousRunAnalyticsLedgerPolicy,
    accepted_report_count: int,
    evicted_report_count: int,
) -> AutonomousRunAnalyticsLedgerSummary:
    status_counts = {status: 0 for status in AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES}
    alert_counts = {severity: 0 for severity in AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES}
    alerts: dict[tuple[str, str, str, str], AutonomousRunAnalyticsLedgerAlert] = {}
    event_count = run_count = terminal_run_count = incomplete_run_count = 0
    provider_invocations = provider_failures = input_tokens = output_tokens = tool_calls = latency_observation_count = 0
    latency_weighted_sum = 0.0
    source_digests: set[str] = set()
    for entry in entries:
        report = entry.report
        source_digests.add(report.source_snapshot_digest)
        status_counts[report.status] += 1
        event_count += report.event_count
        run_count += report.run_count
        terminal_run_count += report.terminal_run_count
        incomplete_run_count += report.incomplete_run_count
        provider_invocations += report.provider_invocations
        provider_failures += report.provider_failures
        input_tokens += report.input_tokens
        output_tokens += report.output_tokens
        tool_calls += report.tool_calls
        latency_observation_count += report.latency_observation_count
        if report.latency_mean_ms is not None:
            latency_weighted_sum += report.latency_mean_ms * report.latency_observation_count
        for alert in report.alerts:
            alert_counts[alert.severity] += 1
            key = (alert.code, alert.severity, alert.scope, alert.identity)
            previous = alerts.get(key)
            alerts[key] = AutonomousRunAnalyticsLedgerAlert(
                code=alert.code,
                severity=alert.severity,
                scope=alert.scope,
                identity=alert.identity,
                occurrences=(previous.occurrences + 1 if previous else 1),
                last_report_digest=report.report_digest,
                detail=alert.detail,
            )
    critical = alert_counts["critical"] > 0
    warning = alert_counts["warning"] > 0
    status = "unmeasured" if not entries else "attention_required" if critical else "degraded" if warning else "observed"
    descriptor = {
        "schema": AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA,
        "status": status,
        "report_count": len(entries),
        "source_snapshot_count": len(source_digests),
        "accepted_report_count": accepted_report_count,
        "evicted_report_count": evicted_report_count,
        "first_ingested_at": min((entry.ingested_at for entry in entries), default=None),
        "last_ingested_at": max((entry.ingested_at for entry in entries), default=None),
        "event_count": event_count,
        "run_count": run_count,
        "terminal_run_count": terminal_run_count,
        "incomplete_run_count": incomplete_run_count,
        "terminal_coverage": None if not run_count else round(terminal_run_count / run_count, 12),
        "provider_invocations": provider_invocations,
        "provider_failures": provider_failures,
        "provider_failure_rate": None if not provider_invocations else round(provider_failures / provider_invocations, 12),
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "tool_calls": tool_calls,
        "latency_observation_count": latency_observation_count,
        "latency_mean_ms": None if not latency_observation_count else round(latency_weighted_sum / latency_observation_count, 6),
        "latency_p50_ms": None,
        "latency_p95_ms": None,
        "latency_quantile_posture": AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE,
        "status_counts": status_counts,
        "alert_counts": alert_counts,
        "domains": [row.to_dict() for row in _aggregate_dimensions(entries, "domains", "domain", policy.expected_domains)],
        "providers": [row.to_dict() for row in _aggregate_dimensions(entries, "providers", "provider", policy.expected_domains)],
        "models": [row.to_dict() for row in _aggregate_dimensions(entries, "models", "model", policy.expected_domains)],
        "alerts": [alert.to_dict() for alert in sorted(alerts.values(), key=lambda item: (item.severity, item.code, item.scope, item.identity))],
        "cost_posture": "not_measured_by_trace",
        "authority": AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY,
        "retention": AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION,
        "secret_material": "never_returned",
    }
    return AutonomousRunAnalyticsLedgerSummary(
        **{key: value for key, value in descriptor.items() if key not in {"domains", "providers", "models", "alerts"}},
        domains=tuple(_aggregate_dimensions(entries, "domains", "domain", policy.expected_domains)),
        providers=tuple(_aggregate_dimensions(entries, "providers", "provider", policy.expected_domains)),
        models=tuple(_aggregate_dimensions(entries, "models", "model", policy.expected_domains)),
        alerts=tuple(sorted(alerts.values(), key=lambda item: (item.severity, item.code, item.scope, item.identity))),
        summary_digest=content_digest(descriptor),
    )


def _validate_snapshot(value: Mapping[str, Any], policy: AutonomousRunAnalyticsLedgerPolicy | None = None) -> dict[str, Any]:
    raw = _mapping("analytics ledger snapshot", value)
    expected = {"schema", "policy", "entries", "accepted_report_count", "evicted_report_count", "generation", "previous_snapshot_digest", "snapshot_digest", "retention", "secret_material"}
    if set(raw) != expected or raw["schema"] != AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA:
        raise ArgumentError("analytics ledger snapshot contains unsupported or missing fields")
    if raw["retention"] != AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION or raw["secret_material"] != "never_returned":
        raise ArgumentError("analytics ledger snapshot retention markers are invalid")
    snapshot_policy = AutonomousRunAnalyticsLedgerPolicy.from_dict(raw["policy"])
    if policy is not None and snapshot_policy != policy:
        raise ArgumentError("analytics ledger snapshot policy does not match the ledger")
    entries_raw = raw["entries"]
    if not isinstance(entries_raw, Sequence) or isinstance(entries_raw, (str, bytes, bytearray)) or len(entries_raw) > snapshot_policy.max_reports:
        raise ArgumentError("analytics ledger snapshot entries are outside their bound")
    entries = tuple(AutonomousRunAnalyticsLedgerEntry.from_dict(item) for item in entries_raw)
    if len({entry.report.source_snapshot_digest for entry in entries}) != len(entries) or len({entry.report.report_digest for entry in entries}) != len(entries):
        raise ArgumentError("analytics ledger snapshot contains duplicate report identities")
    if tuple(sorted(entries, key=lambda entry: (entry.ingested_at, entry.report.report_digest))) != entries:
        raise ArgumentError("analytics ledger snapshot entries are not deterministically ordered")
    _integer("analytics ledger accepted_report_count", raw["accepted_report_count"])
    _integer("analytics ledger evicted_report_count", raw["evicted_report_count"])
    generation = _integer("analytics ledger generation", raw["generation"])
    previous = raw["previous_snapshot_digest"]
    if previous is not None:
        _digest("analytics ledger previous_snapshot_digest", previous)
    if (generation == 1) != (previous is None):
        raise ArgumentError("analytics ledger generation and previous snapshot digest are inconsistent")
    snapshot_digest = _digest("analytics ledger snapshot_digest", raw["snapshot_digest"])
    body = {key: raw[key] for key in expected if key not in {"snapshot_digest"}}
    if content_digest(body) != snapshot_digest:
        raise ArgumentError("analytics ledger snapshot digest does not match its contents")
    if raw["accepted_report_count"] != len(entries) + raw["evicted_report_count"]:
        raise ArgumentError("analytics ledger accepted count does not reconcile with retained and evicted reports")
    if len(canonical_json(raw).encode("utf-8")) > MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES:
        raise ArgumentError("analytics ledger snapshot exceeds its byte capacity")
    return dict(raw)


class AutonomousRunAnalyticsLedger:
    """In-memory bounded ledger with idempotent ingestion within the retained window."""

    def __init__(self, policy: AutonomousRunAnalyticsLedgerPolicy | None = None, *, clock: Callable[[], float] = time.time) -> None:
        self.policy = policy or AutonomousRunAnalyticsLedgerPolicy()
        if not callable(clock):
            raise ArgumentError("analytics ledger clock must be callable")
        self.clock = clock
        self._entries: list[AutonomousRunAnalyticsLedgerEntry] = []
        self._accepted_report_count = 0
        self._evicted_report_count = 0
        self._generation = 0
        self._previous_snapshot_digest: str | None = None
        self._cached_snapshot: dict[str, Any] | None = None
        self._cached_signature: tuple[str, ...] | None = None

    @property
    def entries(self) -> tuple[AutonomousRunAnalyticsLedgerEntry, ...]:
        return tuple(self._entries)

    def _invalidate(self) -> None:
        self._cached_snapshot = None
        self._cached_signature = None

    def ingest(self, report: Mapping[str, Any] | AutonomousRunTraceAnalyticsReport, *, ingested_at: int | None = None) -> AutonomousRunAnalyticsLedgerIngestResult:
        validated = validate_autonomous_run_trace_analytics_report(report)
        source_digest = validated.source_snapshot_digest
        for entry in self._entries:
            if entry.report.source_snapshot_digest == source_digest:
                if entry.report.report_digest == validated.report_digest:
                    return AutonomousRunAnalyticsLedgerIngestResult("duplicate", validated.report_digest, source_digest, len(self._entries), self._evicted_report_count)
                return AutonomousRunAnalyticsLedgerIngestResult("conflict", validated.report_digest, source_digest, len(self._entries), self._evicted_report_count)
        stamp = int(self.clock() * 1000) if ingested_at is None else _timestamp("analytics ledger ingested_at", ingested_at)
        entry = AutonomousRunAnalyticsLedgerEntry.create(validated, stamp)
        self._entries.append(entry)
        self._entries.sort(key=lambda item: (item.ingested_at, item.report.report_digest))
        self._accepted_report_count += 1
        while len(self._entries) > self.policy.max_reports:
            self._entries.pop(0)
            self._evicted_report_count += 1
        self._invalidate()
        return AutonomousRunAnalyticsLedgerIngestResult("accepted", validated.report_digest, source_digest, len(self._entries), self._evicted_report_count)

    def history(self, *, limit: int = 100, status: str | None = None) -> tuple[AutonomousRunAnalyticsLedgerEntry, ...]:
        limit = _integer("analytics ledger history limit", limit, maximum=MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES)
        if not 1 <= limit <= MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES:
            raise ArgumentError("analytics ledger history limit must be positive")
        if status is not None and status not in AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES:
            raise ArgumentError("analytics ledger history status is invalid")
        selected = [entry for entry in reversed(self._entries) if status is None or entry.report.status == status]
        return tuple(selected[:limit])

    def summary(self) -> AutonomousRunAnalyticsLedgerSummary:
        return _summary_for_entries(self._entries, self.policy, self._accepted_report_count, self._evicted_report_count)

    def snapshot(self) -> dict[str, Any]:
        signature = tuple(entry.entry_digest for entry in self._entries)
        if self._cached_snapshot is not None and self._cached_signature == signature:
            return dict(self._cached_snapshot)
        descriptor = {
            "schema": AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA,
            "policy": self.policy.to_dict(),
            "entries": [entry.to_dict() for entry in self._entries],
            "accepted_report_count": self._accepted_report_count,
            "evicted_report_count": self._evicted_report_count,
            "generation": self._generation + 1,
            "previous_snapshot_digest": self._previous_snapshot_digest,
            "retention": AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION,
            "secret_material": "never_returned",
        }
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES:
            raise ArgumentError("analytics ledger snapshot exceeds its byte capacity")
        self._generation = snapshot["generation"]
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        self._cached_snapshot = snapshot
        self._cached_signature = signature
        return dict(snapshot)

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        raw = _validate_snapshot(snapshot, self.policy)
        entries = [AutonomousRunAnalyticsLedgerEntry.from_dict(item) for item in raw["entries"]]
        self._entries = entries
        self._accepted_report_count = raw["accepted_report_count"]
        self._evicted_report_count = raw["evicted_report_count"]
        self._generation = raw["generation"]
        self._previous_snapshot_digest = raw["snapshot_digest"]
        self._cached_snapshot = dict(raw)
        self._cached_signature = tuple(entry.entry_digest for entry in entries)


class JsonAutonomousRunAnalyticsLedgerPersistence:
    """Canonical text persistence for the ledger."""

    def __init__(self, store: Any, *, max_bytes: int = MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("analytics ledger JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _integer("analytics ledger persistence max_bytes", max_bytes, maximum=MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES)

    def read(self) -> dict[str, Any] | None:
        value = self.store.read()
        if value is None:
            return None
        if not isinstance(value, str) or len(value.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("analytics ledger JSON exceeds its byte bound")
        import json
        try:
            raw = json.loads(value)
        except (TypeError, json.JSONDecodeError) as error:
            raise ArgumentError("analytics ledger JSON is invalid") from error
        if canonical_json(raw) != value:
            raise ArgumentError("analytics ledger JSON is not canonical")
        return _validate_snapshot(raw)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _validate_snapshot(snapshot)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("analytics ledger JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(JsonAutonomousRunAnalyticsLedgerPersistence):
    def __init__(self, store: Any, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional analytics ledger persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None:
            _digest("analytics ledger expected_snapshot_digest", expected_snapshot_digest)
        normalized = _validate_snapshot(snapshot)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized)))


class AutonomousRunAnalyticsLedgerPersistenceCoordinator:
    """Ordered restore/flush coordinator with optional stale-writer fencing."""

    def __init__(self, ledger: AutonomousRunAnalyticsLedger, persistence: Any) -> None:
        if not isinstance(ledger, AutonomousRunAnalyticsLedger):
            raise ArgumentError("analytics ledger coordinator requires an analytics ledger")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("analytics ledger coordinator persistence is malformed")
        self.ledger = ledger
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            self._expected_snapshot_digest = None
            return None
        self.ledger.restore(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.ledger.snapshot()
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self._expected_snapshot_digest, snapshot):
                raise ArgumentError("analytics ledger persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


def validate_autonomous_run_analytics_ledger_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Public strict validator for digest-bound longitudinal analytics snapshots."""

    return _validate_snapshot(value)


__all__ = [
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_STATUSES",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_STATUSES",
    "AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE",
    "MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS",
    "MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES",
    "MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES",
    "MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_DIMENSIONS",
    "AutonomousRunAnalyticsLedgerPolicy",
    "AutonomousRunAnalyticsLedgerEntry",
    "AutonomousRunAnalyticsLedgerIngestResult",
    "AutonomousRunAnalyticsLedgerDimension",
    "AutonomousRunAnalyticsLedgerAlert",
    "AutonomousRunAnalyticsLedgerSummary",
    "AutonomousRunAnalyticsLedger",
    "JsonAutonomousRunAnalyticsLedgerPersistence",
    "TransactionalJsonAutonomousRunAnalyticsLedgerPersistence",
    "AutonomousRunAnalyticsLedgerPersistenceCoordinator",
    "validate_autonomous_run_analytics_ledger_snapshot",
]
