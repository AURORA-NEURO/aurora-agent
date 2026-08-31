"""Conservative analytics over verified autonomous run traces.

The trace journal answers *what happened in one run*.  Deployments also need to answer a
different question: which domains, providers, and model routes are being observed, how often do
provider attempts fail, how much latency and token usage is actually measured, and which runs have
not reached a terminal state?  This module answers that question without becoming a metrics
oracle.

Only metadata already present in :mod:`autonomous_run_trace` is aggregated.  Missing latency or
token measurements remain ``None`` at the rate/quantile boundary; they are never converted into
zeroes.  Domain rows are emitted for the complete reviewed domain catalogue, so an unobserved
domain is visibly ``unmeasured`` rather than looking like a domain with no failures.  The report is
bound to the exact verified snapshot and policy digests, and contains no task text, prompts,
responses, tool arguments, credentials, source values, or cost claims.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
from .autonomous_run_trace import (
    AUTONOMOUS_RUN_TRACE_PHASES,
    AUTONOMOUS_RUN_TRACE_STATUSES,
    AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
    AutonomousRunTraceEvent,
    AutonomousRunTraceSnapshot,
    validate_autonomous_run_trace_snapshot,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA = "bioprism-python-autonomous-run-trace-analytics/0.1"
AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION = "metadata_only_no_prompts_responses_tool_payloads_or_cost_claims"
AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY = "verified_trace_aggregation_only;not_task_correctness_or_external_health"
AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES = ("unmeasured", "observed", "degraded", "attention_required")
AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES = ("measured", "unmeasured")
AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES = ("info", "warning", "critical")
MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_RUNS = 10_000
MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_EVENTS = 100_000
MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ROWS = 512
MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS = 10_000
MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES = 20_000_000


def _text(name: str, value: Any, *, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > maximum or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any, *, maximum: int = 256) -> str:
    value = _text(name, value, maximum=maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-" for character in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _finite_ratio(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0.0 <= float(value) <= 1.0:
        raise ArgumentError(f"{name} must be finite and within [0, 1]")
    return float(value)


def _optional_threshold(name: str, value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0.0 < float(value) <= 86_400_000:
        raise ArgumentError(f"{name} must be a finite positive millisecond threshold or None")
    return float(value)


def _nonnegative_integer(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _safe_metadata(value: Any, *, depth: int = 0) -> None:
    if depth > 20:
        raise ArgumentError("autonomous trace analytics metadata is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                raise ArgumentError("autonomous trace analytics metadata keys must be strings")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in {
                "task", "prompt", "response", "messages", "credential", "credentials", "secret",
                "token", "apikey", "authorization", "arguments", "argument", "payload", "output",
                "input", "sourcevalue", "rawvalue", "cost", "price",
            }:
                raise ArgumentError("autonomous trace analytics contains transient, secret, or cost-shaped metadata")
            _safe_metadata(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _safe_metadata(child, depth=depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError("autonomous trace analytics contains a non-finite number")


def _ordered_domains(values: Sequence[str]) -> tuple[str, ...]:
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        raise ArgumentError("autonomous trace analytics expected_domains must be a sequence")
    if not 1 <= len(values) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("autonomous trace analytics expected_domains is outside its bounds")
    selected = tuple(values)
    if any(value not in AUTONOMOUS_DOMAIN_NAMES for value in selected) or len(set(selected)) != len(selected):
        raise ArgumentError("autonomous trace analytics expected_domains contains an unsupported or duplicate domain")
    return tuple(domain for domain in AUTONOMOUS_DOMAIN_NAMES if domain in selected)


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceAnalyticsPolicy:
    """Caller-owned thresholds for interpreting observed trace metadata.

    Thresholds produce attention rows; they do not certify provider quality or task correctness.
    ``None`` latency thresholds disable latency alerts, while missing observations remain distinct
    from measured zero latency.
    """

    expected_domains: tuple[str, ...] = AUTONOMOUS_DOMAIN_NAMES
    failure_rate_warning: float = 0.25
    failure_rate_critical: float = 0.50
    p95_latency_warning_ms: float | None = 10_000.0
    p95_latency_critical_ms: float | None = 60_000.0
    warn_on_incomplete_runs: bool = True
    warn_on_unmeasured_domains: bool = False

    def __post_init__(self) -> None:
        object.__setattr__(self, "expected_domains", _ordered_domains(self.expected_domains))
        warning = _finite_ratio("failure_rate_warning", self.failure_rate_warning)
        critical = _finite_ratio("failure_rate_critical", self.failure_rate_critical)
        if warning > critical:
            raise ArgumentError("failure_rate_warning cannot exceed failure_rate_critical")
        object.__setattr__(self, "failure_rate_warning", warning)
        object.__setattr__(self, "failure_rate_critical", critical)
        latency_warning = _optional_threshold("p95_latency_warning_ms", self.p95_latency_warning_ms)
        latency_critical = _optional_threshold("p95_latency_critical_ms", self.p95_latency_critical_ms)
        if latency_warning is not None and latency_critical is not None and latency_warning > latency_critical:
            raise ArgumentError("p95_latency_warning_ms cannot exceed p95_latency_critical_ms")
        object.__setattr__(self, "p95_latency_warning_ms", latency_warning)
        object.__setattr__(self, "p95_latency_critical_ms", latency_critical)
        if not isinstance(self.warn_on_incomplete_runs, bool) or not isinstance(self.warn_on_unmeasured_domains, bool):
            raise ArgumentError("autonomous trace analytics policy booleans must be boolean")

    def to_dict(self) -> dict[str, Any]:
        return {
            "expected_domains": list(self.expected_domains),
            "failure_rate_warning": self.failure_rate_warning,
            "failure_rate_critical": self.failure_rate_critical,
            "p95_latency_warning_ms": self.p95_latency_warning_ms,
            "p95_latency_critical_ms": self.p95_latency_critical_ms,
            "warn_on_incomplete_runs": self.warn_on_incomplete_runs,
            "warn_on_unmeasured_domains": self.warn_on_unmeasured_domains,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousRunTraceAnalyticsPolicy":
        if not isinstance(value, Mapping):
            raise ArgumentError("autonomous trace analytics policy must be a mapping")
        expected = {
            "expected_domains", "failure_rate_warning", "failure_rate_critical",
            "p95_latency_warning_ms", "p95_latency_critical_ms", "warn_on_incomplete_runs",
            "warn_on_unmeasured_domains",
        }
        if set(value) != expected:
            raise ArgumentError("autonomous trace analytics policy contains unsupported or missing fields")
        return cls(
            expected_domains=tuple(value["expected_domains"]),
            failure_rate_warning=value["failure_rate_warning"],
            failure_rate_critical=value["failure_rate_critical"],
            p95_latency_warning_ms=value["p95_latency_warning_ms"],
            p95_latency_critical_ms=value["p95_latency_critical_ms"],
            warn_on_incomplete_runs=value["warn_on_incomplete_runs"],
            warn_on_unmeasured_domains=value["warn_on_unmeasured_domains"],
        )


def _status_counts() -> dict[str, int]:
    return {status: 0 for status in AUTONOMOUS_RUN_TRACE_STATUSES}


def _phase_counts() -> dict[str, int]:
    return {phase: 0 for phase in AUTONOMOUS_RUN_TRACE_PHASES}


def _quantile(values: Sequence[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(float(value) for value in values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * fraction) - 1))
    return ordered[index]


def _mean(values: Sequence[float]) -> float | None:
    return None if not values else round(sum(values) / len(values), 6)


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceAnalyticsDimension:
    """One domain/provider/model rollup with explicit measurement state."""

    kind: str
    identity: str
    expected: bool
    observed: bool
    measurement_state: str
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
            "input_token_observation_count": self.input_token_observation_count,
            "output_token_observation_count": self.output_token_observation_count,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "tool_calls": self.tool_calls,
            "failure_codes": list(self.failure_codes),
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousRunTraceAnalyticsDimension":
        if not isinstance(value, Mapping):
            raise ArgumentError("autonomous trace analytics dimension must be a mapping")
        expected = {
            "kind", "identity", "expected", "observed", "measurement_state", "run_count", "event_count",
            "terminal_run_count", "incomplete_run_count", "status_counts", "provider_invocations",
            "provider_failures", "failure_rate", "latency_observation_count", "latency_mean_ms",
            "latency_p50_ms", "latency_p95_ms", "input_token_observation_count", "output_token_observation_count",
            "input_tokens", "output_tokens", "tool_calls", "failure_codes",
        }
        if set(value) != expected:
            raise ArgumentError("autonomous trace analytics dimension contains unsupported or missing fields")
        kind = _identifier("autonomous trace analytics dimension kind", value["kind"])
        identity = _text("autonomous trace analytics dimension identity", value["identity"], maximum=512)
        if kind not in {"domain", "provider", "model"}:
            raise ArgumentError("autonomous trace analytics dimension kind is invalid")
        if not isinstance(value["expected"], bool) or not isinstance(value["observed"], bool):
            raise ArgumentError("autonomous trace analytics dimension booleans are invalid")
        measurement = value["measurement_state"]
        if measurement not in AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES:
            raise ArgumentError("autonomous trace analytics dimension measurement state is invalid")
        counts: dict[str, int] = {}
        if not isinstance(value["status_counts"], Mapping) or set(value["status_counts"]) != set(AUTONOMOUS_RUN_TRACE_STATUSES):
            raise ArgumentError("autonomous trace analytics dimension status counts are malformed")
        for status in AUTONOMOUS_RUN_TRACE_STATUSES:
            counts[status] = _nonnegative_integer(f"status_counts.{status}", value["status_counts"][status])
        integers = {
            name: _nonnegative_integer(name, value[name])
            for name in (
                "run_count", "event_count", "terminal_run_count", "incomplete_run_count", "provider_invocations",
                "provider_failures", "latency_observation_count", "input_token_observation_count",
                "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls",
            )
        }
        if integers["provider_failures"] > integers["provider_invocations"]:
            raise ArgumentError("autonomous trace analytics provider failures exceed invocations")
        if integers["terminal_run_count"] + integers["incomplete_run_count"] != integers["run_count"]:
            raise ArgumentError("autonomous trace analytics terminal and incomplete runs exceed run count")
        if integers["event_count"] == 0 and value["observed"]:
            raise ArgumentError("observed analytics dimensions require events")
        if integers["event_count"] > 0 and not value["observed"]:
            raise ArgumentError("analytics dimensions with events must be observed")
        if sum(counts.values()) != integers["run_count"]:
            raise ArgumentError("autonomous trace analytics dimension status counts do not reconcile")
        failure_rate = value["failure_rate"]
        if failure_rate is not None:
            failure_rate = _finite_ratio("autonomous trace analytics failure_rate", failure_rate)
            if integers["provider_invocations"] == 0:
                raise ArgumentError("unmeasured failure rate must be null")
            expected_failure_rate = round(integers["provider_failures"] / integers["provider_invocations"], 12)
            if not math.isclose(failure_rate, expected_failure_rate, rel_tol=0.0, abs_tol=1e-12):
                raise ArgumentError("autonomous trace analytics failure rate does not reconcile")
        elif integers["provider_invocations"] != 0:
            raise ArgumentError("measured provider invocations require a failure rate")
        latencies: list[float | None] = []
        for name in ("latency_mean_ms", "latency_p50_ms", "latency_p95_ms"):
            raw = value[name]
            if raw is not None:
                if isinstance(raw, bool) or not isinstance(raw, (int, float)) or not math.isfinite(float(raw)) or float(raw) < 0:
                    raise ArgumentError(f"autonomous trace analytics {name} is invalid")
                latencies.append(float(raw))
            else:
                latencies.append(None)
        if (integers["latency_observation_count"] == 0) != all(item is None for item in latencies):
            raise ArgumentError("autonomous trace analytics latency observations do not reconcile")
        raw_codes = value["failure_codes"]
        if not isinstance(raw_codes, Sequence) or isinstance(raw_codes, (str, bytes, bytearray)):
            raise ArgumentError("autonomous trace analytics failure_codes must be a sequence")
        codes = tuple(_text("autonomous trace analytics failure code", item) for item in raw_codes)
        if tuple(sorted(set(codes))) != codes:
            raise ArgumentError("autonomous trace analytics failure_codes must be unique and sorted")
        if measurement == "unmeasured" and value["observed"]:
            raise ArgumentError("observed analytics dimensions cannot be unmeasured")
        if measurement == "measured" and not value["observed"]:
            raise ArgumentError("unobserved analytics dimensions cannot be measured")
        return cls(
            kind=kind, identity=identity, expected=value["expected"], observed=value["observed"],
            measurement_state=measurement, status_counts=counts, failure_codes=codes,
            failure_rate=failure_rate, **integers,
            latency_mean_ms=latencies[0], latency_p50_ms=latencies[1], latency_p95_ms=latencies[2],
        )


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceAnalyticsAlert:
    code: str
    severity: str
    scope: str
    identity: str
    detail: str
    observed_value: float | None = None
    threshold: float | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "severity": self.severity,
            "scope": self.scope,
            "identity": self.identity,
            "detail": self.detail,
            "observed_value": self.observed_value,
            "threshold": self.threshold,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousRunTraceAnalyticsAlert":
        if not isinstance(value, Mapping) or set(value) != {"code", "severity", "scope", "identity", "detail", "observed_value", "threshold"}:
            raise ArgumentError("autonomous trace analytics alert is malformed")
        code = _identifier("autonomous trace analytics alert code", value["code"])
        severity = value["severity"]
        if severity not in AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES:
            raise ArgumentError("autonomous trace analytics alert severity is invalid")
        scope = _identifier("autonomous trace analytics alert scope", value["scope"])
        identity = _text("autonomous trace analytics alert identity", value["identity"], maximum=512)
        detail = _text("autonomous trace analytics alert detail", value["detail"], maximum=512)
        observed = value["observed_value"]
        threshold = value["threshold"]
        for name, item in (("observed_value", observed), ("threshold", threshold)):
            if item is not None and (isinstance(item, bool) or not isinstance(item, (int, float)) or not math.isfinite(float(item))):
                raise ArgumentError(f"autonomous trace analytics alert {name} is invalid")
        return cls(code, severity, scope, identity, detail, None if observed is None else float(observed), None if threshold is None else float(threshold))


@dataclass(frozen=True, slots=True)
class AutonomousRunTraceAnalyticsReport:
    schema: str
    source_snapshot_digest: str
    policy_digest: str
    status: str
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
    latency_p50_ms: float | None
    latency_p95_ms: float | None
    first_recorded_at: int | None
    last_recorded_at: int | None
    status_counts: Mapping[str, int]
    phase_counts: Mapping[str, int]
    domains: tuple[AutonomousRunTraceAnalyticsDimension, ...]
    providers: tuple[AutonomousRunTraceAnalyticsDimension, ...]
    models: tuple[AutonomousRunTraceAnalyticsDimension, ...]
    alerts: tuple[AutonomousRunTraceAnalyticsAlert, ...]
    unattributed_provider_events: int
    unattributed_model_events: int
    cost_posture: str
    authority: str
    retention: str
    secret_material: str
    report_digest: str

    def to_dict(self, *, include_digest: bool = True) -> dict[str, Any]:
        body: dict[str, Any] = {
            "schema": self.schema,
            "source_snapshot_digest": self.source_snapshot_digest,
            "policy_digest": self.policy_digest,
            "status": self.status,
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
            "first_recorded_at": self.first_recorded_at,
            "last_recorded_at": self.last_recorded_at,
            "status_counts": dict(self.status_counts),
            "phase_counts": dict(self.phase_counts),
            "domains": [row.to_dict() for row in self.domains],
            "providers": [row.to_dict() for row in self.providers],
            "models": [row.to_dict() for row in self.models],
            "alerts": [alert.to_dict() for alert in self.alerts],
            "unattributed_provider_events": self.unattributed_provider_events,
            "unattributed_model_events": self.unattributed_model_events,
            "cost_posture": self.cost_posture,
            "authority": self.authority,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        if include_digest:
            body["report_digest"] = self.report_digest
        return body


def _dimension(
    kind: str,
    identity: str,
    expected: bool,
    events: Sequence[AutonomousRunTraceEvent],
    runs: Mapping[str, tuple[str, tuple[str, ...], str, Sequence[AutonomousRunTraceEvent]]],
) -> AutonomousRunTraceAnalyticsDimension:
    run_ids = {event.run_id for event in events}
    selected_runs = [runs[run_id] for run_id in sorted(run_ids)]
    status_counts = _status_counts()
    terminal = 0
    incomplete = 0
    for _run_id, _task_digest, _domains, run_events in selected_runs:
        final = run_events[-1]
        status_counts[final.status] += 1
        if final.status in {"completed", "partial", "paused", "refused", "failed"}:
            terminal += 1
        else:
            incomplete += 1
    finished = [event for event in events if event.phase == "provider_invocation_finished"]
    failures = [event for event in finished if event.failure_code is not None or event.failure_class is not None]
    latencies = [float(event.latency_ms) for event in finished if event.latency_ms is not None]
    input_observed = [event.input_tokens for event in finished if event.input_tokens is not None]
    output_observed = [event.output_tokens for event in finished if event.output_tokens is not None]
    failure_codes = tuple(sorted({event.failure_code for event in failures if event.failure_code is not None}))
    observed = bool(events)
    return AutonomousRunTraceAnalyticsDimension(
        kind=kind,
        identity=identity,
        expected=expected,
        observed=observed,
        measurement_state="measured" if observed else "unmeasured",
        run_count=len(selected_runs),
        event_count=len(events),
        terminal_run_count=terminal,
        incomplete_run_count=incomplete,
        status_counts=status_counts,
        provider_invocations=len(finished),
        provider_failures=len(failures),
        failure_rate=None if not finished else round(len(failures) / len(finished), 12),
        latency_observation_count=len(latencies),
        latency_mean_ms=_mean(latencies),
        latency_p50_ms=_quantile(latencies, 0.50),
        latency_p95_ms=_quantile(latencies, 0.95),
        input_token_observation_count=len(input_observed),
        output_token_observation_count=len(output_observed),
        input_tokens=sum(input_observed),
        output_tokens=sum(output_observed),
        tool_calls=sum(event.tool_count or 0 for event in finished),
        failure_codes=failure_codes,
    )


def _alert_for_dimension(row: AutonomousRunTraceAnalyticsDimension, policy: AutonomousRunTraceAnalyticsPolicy) -> list[AutonomousRunTraceAnalyticsAlert]:
    alerts: list[AutonomousRunTraceAnalyticsAlert] = []
    if row.kind in {"domain", "provider", "model"} and row.failure_rate is not None:
        if row.failure_rate >= policy.failure_rate_critical:
            alerts.append(AutonomousRunTraceAnalyticsAlert("provider_failure_rate", "critical", row.kind, row.identity, f"{row.kind} provider failure rate reached the critical threshold", row.failure_rate, policy.failure_rate_critical))
        elif row.failure_rate >= policy.failure_rate_warning:
            alerts.append(AutonomousRunTraceAnalyticsAlert("provider_failure_rate", "warning", row.kind, row.identity, f"{row.kind} provider failure rate reached the warning threshold", row.failure_rate, policy.failure_rate_warning))
    if row.latency_p95_ms is not None:
        if policy.p95_latency_critical_ms is not None and row.latency_p95_ms >= policy.p95_latency_critical_ms:
            alerts.append(AutonomousRunTraceAnalyticsAlert("p95_latency", "critical", row.kind, row.identity, f"{row.kind} p95 latency reached the critical threshold", row.latency_p95_ms, policy.p95_latency_critical_ms))
        elif policy.p95_latency_warning_ms is not None and row.latency_p95_ms >= policy.p95_latency_warning_ms:
            alerts.append(AutonomousRunTraceAnalyticsAlert("p95_latency", "warning", row.kind, row.identity, f"{row.kind} p95 latency reached the warning threshold", row.latency_p95_ms, policy.p95_latency_warning_ms))
    if row.kind == "domain" and row.expected and not row.observed and policy.warn_on_unmeasured_domains:
        alerts.append(AutonomousRunTraceAnalyticsAlert("domain_unmeasured", "info", "domain", row.identity, "the expected domain has no trace observations", None, None))
    if policy.warn_on_incomplete_runs and row.incomplete_run_count:
        alerts.append(AutonomousRunTraceAnalyticsAlert("run_not_terminal", "warning", row.kind, row.identity, f"{row.kind} has runs without a terminal trace status", float(row.incomplete_run_count), None))
    return alerts


def _validate_report(value: Mapping[str, Any]) -> AutonomousRunTraceAnalyticsReport:
    if not isinstance(value, Mapping):
        raise ArgumentError("autonomous trace analytics report must be a mapping")
    expected = {
        "schema", "source_snapshot_digest", "policy_digest", "status", "event_count", "run_count",
        "terminal_run_count", "incomplete_run_count", "terminal_coverage", "provider_invocations",
        "provider_failures", "provider_failure_rate", "input_tokens", "output_tokens", "tool_calls",
        "latency_observation_count", "latency_mean_ms", "latency_p50_ms", "latency_p95_ms",
        "first_recorded_at", "last_recorded_at", "status_counts", "phase_counts", "domains", "providers",
        "models", "alerts", "unattributed_provider_events", "unattributed_model_events", "cost_posture",
        "authority", "retention", "secret_material", "report_digest",
    }
    if set(value) != expected:
        raise ArgumentError("autonomous trace analytics report contains unsupported or missing fields")
    if value["schema"] != AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA:
        raise ArgumentError("autonomous trace analytics report schema is invalid")
    _digest("autonomous trace analytics source snapshot digest", value["source_snapshot_digest"])
    _digest("autonomous trace analytics policy digest", value["policy_digest"])
    if value["status"] not in AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES:
        raise ArgumentError("autonomous trace analytics report status is invalid")
    integers = {name: _nonnegative_integer(name, value[name]) for name in ("event_count", "run_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "input_tokens", "output_tokens", "tool_calls", "latency_observation_count", "unattributed_provider_events", "unattributed_model_events")}
    if integers["terminal_run_count"] + integers["incomplete_run_count"] != integers["run_count"] or integers["provider_failures"] > integers["provider_invocations"]:
        raise ArgumentError("autonomous trace analytics report counts do not reconcile")
    coverage = value["terminal_coverage"]
    if coverage is None:
        if integers["run_count"]:
            raise ArgumentError("measured trace runs require terminal coverage")
    else:
        coverage = _finite_ratio("autonomous trace analytics terminal_coverage", coverage)
        if not integers["run_count"]:
            raise ArgumentError("empty trace cannot have terminal coverage")
        expected_coverage = round(integers["terminal_run_count"] / integers["run_count"], 12)
        if not math.isclose(coverage, expected_coverage, rel_tol=0.0, abs_tol=1e-12):
            raise ArgumentError("autonomous trace analytics terminal coverage does not reconcile")
    failure_rate = value["provider_failure_rate"]
    if failure_rate is None:
        if integers["provider_invocations"]:
            raise ArgumentError("measured provider invocations require provider_failure_rate")
    else:
        failure_rate = _finite_ratio("autonomous trace analytics provider_failure_rate", failure_rate)
        if not integers["provider_invocations"]:
            raise ArgumentError("empty provider observations cannot have provider_failure_rate")
        expected_failure_rate = round(integers["provider_failures"] / integers["provider_invocations"], 12)
        if not math.isclose(failure_rate, expected_failure_rate, rel_tol=0.0, abs_tol=1e-12):
            raise ArgumentError("autonomous trace analytics provider failure rate does not reconcile")
    latencies: list[float | None] = []
    for name in ("latency_mean_ms", "latency_p50_ms", "latency_p95_ms"):
        item = value[name]
        if item is not None:
            if isinstance(item, bool) or not isinstance(item, (int, float)) or not math.isfinite(float(item)) or float(item) < 0:
                raise ArgumentError(f"autonomous trace analytics {name} is invalid")
            latencies.append(float(item))
        else:
            latencies.append(None)
    if (integers["latency_observation_count"] == 0) != all(item is None for item in latencies):
        raise ArgumentError("autonomous trace analytics report latency observations do not reconcile")
    for field, names in (("status_counts", AUTONOMOUS_RUN_TRACE_STATUSES), ("phase_counts", AUTONOMOUS_RUN_TRACE_PHASES)):
        raw = value[field]
        if not isinstance(raw, Mapping) or set(raw) != set(names):
            raise ArgumentError(f"autonomous trace analytics {field} are malformed")
        for name in names:
            _nonnegative_integer(f"{field}.{name}", raw[name])
        expected_total = integers["run_count"] if field == "status_counts" else integers["event_count"]
        if sum(raw[name] for name in names) != expected_total:
            raise ArgumentError(f"autonomous trace analytics {field} do not reconcile with aggregate counts")
    dimensions: dict[str, tuple[AutonomousRunTraceAnalyticsDimension, ...]] = {}
    for field, kind in (("domains", "domain"), ("providers", "provider"), ("models", "model")):
        raw_rows = value[field]
        if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes, bytearray)) or len(raw_rows) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ROWS:
            raise ArgumentError(f"autonomous trace analytics {field} are malformed")
        rows = tuple(AutonomousRunTraceAnalyticsDimension.from_dict(row) for row in raw_rows)
        if any(row.kind != kind for row in rows) or len({row.identity for row in rows}) != len(rows):
            raise ArgumentError(f"autonomous trace analytics {field} contain duplicate or mismatched rows")
        dimensions[field] = rows
    raw_alerts = value["alerts"]
    if not isinstance(raw_alerts, Sequence) or isinstance(raw_alerts, (str, bytes, bytearray)) or len(raw_alerts) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS:
        raise ArgumentError("autonomous trace analytics alerts are malformed")
    alerts = tuple(AutonomousRunTraceAnalyticsAlert.from_dict(item) for item in raw_alerts)
    if value["cost_posture"] != "not_measured_by_trace":
        raise ArgumentError("autonomous trace analytics cost posture is invalid")
    if value["authority"] != AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY or value["retention"] != AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION or value["secret_material"] != "never_returned":
        raise ArgumentError("autonomous trace analytics authority or retention marker is invalid")
    first_recorded_at = value["first_recorded_at"]
    last_recorded_at = value["last_recorded_at"]
    if (first_recorded_at is None) != (integers["event_count"] == 0) or (last_recorded_at is None) != (integers["event_count"] == 0):
        raise ArgumentError("autonomous trace analytics recorded-at bounds do not reconcile")
    if first_recorded_at is not None and last_recorded_at is not None and first_recorded_at > last_recorded_at:
        raise ArgumentError("autonomous trace analytics recorded-at bounds do not reconcile")
    expected_status = (
        "unmeasured" if not integers["event_count"]
        else "attention_required" if any(alert.severity == "critical" for alert in alerts)
        else "degraded" if any(alert.severity == "warning" for alert in alerts)
        else "observed"
    )
    if value["status"] != expected_status:
        raise ArgumentError("autonomous trace analytics status does not reconcile with alerts")
    if integers["unattributed_provider_events"] > integers["provider_invocations"] or integers["unattributed_model_events"] > integers["provider_invocations"]:
        raise ArgumentError("autonomous trace analytics attribution counts exceed provider invocations")
    for name in ("first_recorded_at", "last_recorded_at"):
        item = value[name]
        if item is not None:
            _nonnegative_integer(name, item)
    body = dict(value)
    supplied = body.pop("report_digest")
    if _digest("autonomous trace analytics report digest", supplied) != supplied or content_digest(body) != supplied:
        raise ArgumentError("autonomous trace analytics report digest is invalid")
    _safe_metadata(body)
    return AutonomousRunTraceAnalyticsReport(
        schema=value["schema"], source_snapshot_digest=value["source_snapshot_digest"], policy_digest=value["policy_digest"],
        status=value["status"], **integers, terminal_coverage=coverage, provider_failure_rate=failure_rate,
        latency_mean_ms=latencies[0], latency_p50_ms=latencies[1], latency_p95_ms=latencies[2],
        first_recorded_at=value["first_recorded_at"], last_recorded_at=value["last_recorded_at"],
        status_counts=dict(value["status_counts"]), phase_counts=dict(value["phase_counts"]),
        domains=dimensions["domains"], providers=dimensions["providers"], models=dimensions["models"], alerts=alerts,
        cost_posture=value["cost_posture"], authority=value["authority"], retention=value["retention"],
        secret_material=value["secret_material"], report_digest=supplied,
    )


def analyze_autonomous_run_trace(
    snapshot: Mapping[str, Any] | AutonomousRunTraceSnapshot,
    policy: AutonomousRunTraceAnalyticsPolicy | Mapping[str, Any] | None = None,
) -> AutonomousRunTraceAnalyticsReport:
    """Aggregate one verified trace snapshot without inventing missing measurements."""

    verified = validate_autonomous_run_trace_snapshot(snapshot)
    if verified.schema not in {AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA, "bioprism-python-autonomous-run-trace-snapshot/0.1"}:
        raise ArgumentError("autonomous trace analytics requires a supported trace snapshot")
    resolved_policy = (
        AutonomousRunTraceAnalyticsPolicy()
        if policy is None
        else policy if isinstance(policy, AutonomousRunTraceAnalyticsPolicy)
        else AutonomousRunTraceAnalyticsPolicy.from_dict(policy)
    )
    if len(verified.events) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_EVENTS:
        raise ArgumentError("autonomous trace analytics event capacity is exceeded")
    runs: dict[str, tuple[str, tuple[str, ...], str, list[AutonomousRunTraceEvent]]] = {}
    for event in verified.events:
        prior = runs.get(event.run_id)
        if prior is None:
            runs[event.run_id] = (event.task_digest, tuple(event.domains), event.run_id, [event])
        else:
            task_digest, domains, run_id, events = prior
            if task_digest != event.task_digest:
                raise ArgumentError(f"autonomous trace analytics run {event.run_id!r} changes task identity")
            runs[event.run_id] = (task_digest, tuple(sorted(set(domains).union(event.domains), key=AUTONOMOUS_DOMAIN_NAMES.index)), run_id, [*events, event])
    if len(runs) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_RUNS:
        raise ArgumentError("autonomous trace analytics run capacity is exceeded")
    run_view = {run_id: (task, domains, run_id, tuple(events)) for run_id, (task, domains, _run_id, events) in runs.items()}
    status_counts = _status_counts()
    phase_counts = _phase_counts()
    for event in verified.events:
        phase_counts[event.phase] += 1
    terminal_statuses = {"completed", "partial", "paused", "refused", "failed"}
    for _task, _domains, _run_id, events in run_view.values():
        status_counts[events[-1].status] += 1
    terminal_run_count = sum(events[-1].status in terminal_statuses for _task, _domains, _run_id, events in run_view.values())
    incomplete_run_count = len(run_view) - terminal_run_count
    finished = [event for event in verified.events if event.phase == "provider_invocation_finished"]
    failures = [event for event in finished if event.failure_code is not None or event.failure_class is not None]
    latencies = [float(event.latency_ms) for event in finished if event.latency_ms is not None]
    input_observed = [event.input_tokens for event in finished if event.input_tokens is not None]
    output_observed = [event.output_tokens for event in finished if event.output_tokens is not None]
    observed_domains = {domain for event in verified.events for domain in event.domains}
    all_domains = tuple(domain for domain in AUTONOMOUS_DOMAIN_NAMES if domain in set(resolved_policy.expected_domains).union(observed_domains))
    domains = tuple(
        _dimension("domain", domain, domain in resolved_policy.expected_domains, [event for event in verified.events if domain in event.domains], run_view)
        for domain in all_domains
    )
    provider_names = tuple(sorted({event.provider for event in verified.events if event.provider is not None}))
    providers = tuple(_dimension("provider", provider, True, [event for event in verified.events if event.provider == provider], run_view) for provider in provider_names)
    model_names = tuple(sorted({f"{event.provider}/{event.model}" for event in verified.events if event.provider is not None and event.model is not None}))
    models = tuple(_dimension("model", model, True, [event for event in verified.events if event.provider is not None and event.model is not None and f"{event.provider}/{event.model}" == model], run_view) for model in model_names)
    alerts: list[AutonomousRunTraceAnalyticsAlert] = []
    for row in (*domains, *providers, *models):
        alerts.extend(_alert_for_dimension(row, resolved_policy))
    if resolved_policy.warn_on_incomplete_runs:
        for run_id, (_task, domains_for_run, _run, events) in sorted(run_view.items()):
            if events[-1].status not in terminal_statuses:
                alerts.append(AutonomousRunTraceAnalyticsAlert("run_not_terminal", "warning", "run", run_id, "run has no terminal trace status", None, None))
    if any(run[3][-1].status == "unknown" for run in run_view.values()):
        alerts.append(AutonomousRunTraceAnalyticsAlert("unknown_terminal_status", "warning", "trace", "snapshot", "at least one run ended with an unknown status", None, None))
    severity_order = {"critical": 0, "warning": 1, "info": 2}
    alerts = sorted(alerts, key=lambda alert: (severity_order[alert.severity], alert.code, alert.scope, alert.identity))
    if len(alerts) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS:
        raise ArgumentError("autonomous trace analytics alert capacity is exceeded")
    status = "unmeasured" if not verified.events else "attention_required" if any(alert.severity == "critical" for alert in alerts) else "degraded" if any(alert.severity == "warning" for alert in alerts) else "observed"
    descriptor = {
        "schema": AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA,
        "source_snapshot_digest": verified.snapshot_digest,
        "policy_digest": content_digest(resolved_policy.to_dict()),
        "status": status,
        "event_count": len(verified.events),
        "run_count": len(run_view),
        "terminal_run_count": terminal_run_count,
        "incomplete_run_count": incomplete_run_count,
        "terminal_coverage": None if not run_view else round(terminal_run_count / len(run_view), 12),
        "provider_invocations": len(finished),
        "provider_failures": len(failures),
        "provider_failure_rate": None if not finished else round(len(failures) / len(finished), 12),
        "input_tokens": sum(input_observed),
        "output_tokens": sum(output_observed),
        "tool_calls": sum(event.tool_count or 0 for event in finished),
        "latency_observation_count": len(latencies),
        "latency_mean_ms": _mean(latencies),
        "latency_p50_ms": _quantile(latencies, 0.50),
        "latency_p95_ms": _quantile(latencies, 0.95),
        "first_recorded_at": min((event.recorded_at for event in verified.events), default=None),
        "last_recorded_at": max((event.recorded_at for event in verified.events), default=None),
        "status_counts": status_counts,
        "phase_counts": phase_counts,
        "domains": [row.to_dict() for row in domains],
        "providers": [row.to_dict() for row in providers],
        "models": [row.to_dict() for row in models],
        "alerts": [alert.to_dict() for alert in alerts],
        "unattributed_provider_events": sum(event.phase == "provider_invocation_finished" and event.provider is None for event in verified.events),
        "unattributed_model_events": sum(event.phase == "provider_invocation_finished" and (event.provider is None or event.model is None) for event in verified.events),
        "cost_posture": "not_measured_by_trace",
        "authority": AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY,
        "retention": AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
        "secret_material": "never_returned",
    }
    _safe_metadata(descriptor)
    if len(canonical_json(descriptor).encode("utf-8")) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES:
        raise ArgumentError("autonomous trace analytics report exceeds its byte capacity")
    return _validate_report({**descriptor, "report_digest": content_digest(descriptor)})


def validate_autonomous_run_trace_analytics_report(value: Mapping[str, Any] | AutonomousRunTraceAnalyticsReport) -> AutonomousRunTraceAnalyticsReport:
    """Validate a report before using it as an operational observation or persisted artifact."""

    raw = value.to_dict() if isinstance(value, AutonomousRunTraceAnalyticsReport) else value
    report = _validate_report(raw)
    if len(canonical_json(report.to_dict()).encode("utf-8")) > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES:
        raise ArgumentError("autonomous trace analytics report exceeds its byte capacity")
    return report


__all__ = [
    "AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA",
    "AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION",
    "AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY",
    "AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES",
    "MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_RUNS",
    "MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_EVENTS",
    "MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ROWS",
    "MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS",
    "MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES",
    "AutonomousRunTraceAnalyticsPolicy",
    "AutonomousRunTraceAnalyticsDimension",
    "AutonomousRunTraceAnalyticsAlert",
    "AutonomousRunTraceAnalyticsReport",
    "analyze_autonomous_run_trace",
    "validate_autonomous_run_trace_analytics_report",
]
