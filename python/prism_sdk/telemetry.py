"""Typed telemetry-projection reports.

The telemetry tool has two deliberately separate contracts: a canonical domain event is projected
through an explicit redaction policy, and an optional operational metric is evaluated only when its
inputs are observed.  This module validates both branches without manufacturing a reverse event,
turning semantic loss into a success value, or treating asserted samples as observations.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


TELEMETRY_PROJECTION_SCHEMA = "bioprism-mcp/telemetry-projection/0.1"
TELEMETRY_PROJECTION_STAGES = frozenset({"telemetry_projection"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _number(name: str, value: Any) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise ArgumentError(f"{name} must be numeric")
    return float(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract the projection from direct MCP output or an HTTP REST envelope."""

    raw = _route_mapping("telemetry projection response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"telemetry projection response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == TELEMETRY_PROJECTION_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a telemetry projection")


@dataclass(frozen=True)
class TelemetryLossReport:
    """Exact field-level loss emitted beside a projected record."""

    raw: dict[str, Any]
    dropped: tuple[str, ...]
    coarsened: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TelemetryLossReport":
        raw = _route_mapping("telemetry semantic loss", value)
        dropped = _route_strings("telemetry dropped fields", raw.get("dropped", []))
        coarsened = _route_strings("telemetry coarsened fields", raw.get("coarsened", []))
        if set(dropped).intersection(coarsened):
            raise ArgumentError("telemetry fields cannot be both dropped and coarsened")
        return cls(raw, dropped, coarsened)

    @property
    def lossless(self) -> bool:
        return not self.dropped and not self.coarsened


@dataclass(frozen=True)
class TelemetryRecordReport:
    """The one-way, exportable record produced from a canonical event."""

    raw: dict[str, Any]
    event_id: str
    kind: str
    trace: str
    attributes: dict[str, Any]
    epoch: int
    policy: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TelemetryRecordReport":
        raw = _route_mapping("telemetry record", value)
        return cls(
            raw,
            _route_text("telemetry event_id", raw.get("event_id")),
            _route_text("telemetry event_kind", raw.get("kind")),
            _route_text("telemetry trace", raw.get("trace")),
            _route_mapping("telemetry attributes", raw.get("attributes", {})),
            _integer("telemetry epoch", raw.get("epoch")),
            _route_text("telemetry policy", raw.get("policy")),
        )


@dataclass(frozen=True)
class TelemetryMetricValueReport:
    """A metric value whose required signal inputs were all observed."""

    raw: dict[str, Any]
    metric: str
    unit: str
    value: float
    supported_by: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TelemetryMetricValueReport":
        raw = _route_mapping("telemetry metric value", value)
        supported_by = _route_strings("telemetry metric supported_by", raw.get("supported_by", []))
        if not supported_by:
            raise ArgumentError("supported telemetry metric values must retain their input signals")
        return cls(
            raw,
            _route_text("telemetry metric name", raw.get("metric")),
            _route_text("telemetry metric unit", raw.get("unit")),
            _number("telemetry metric value", raw.get("value")),
            supported_by,
        )


@dataclass(frozen=True)
class TelemetryMetricReport:
    """Success or refusal for the optional observed-versus-asserted metric."""

    raw: dict[str, Any]
    ok: bool
    value: TelemetryMetricValueReport | None
    audit_statement: str | None
    refusal: str | None
    asserted_signals: tuple[str, ...]
    observed_sample_count: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TelemetryMetricReport":
        raw = _route_mapping("telemetry metric result", value)
        ok = _bool("telemetry metric ok", raw.get("ok"))
        refusal_raw = raw.get("refusal")
        refusal = None if refusal_raw is None else _route_text("telemetry metric refusal", refusal_raw)
        asserted_signals = _route_strings("telemetry asserted signals", raw.get("asserted_signals", []))
        observed_sample_count = None
        if raw.get("observed_sample_count") is not None:
            observed_sample_count = _integer("telemetry observed sample count", raw.get("observed_sample_count"))
        if ok:
            if refusal is not None:
                raise ArgumentError("successful telemetry metrics cannot retain a refusal")
            metric_value = TelemetryMetricValueReport.from_wire(raw.get("value"))
            audit_statement = _route_text("telemetry metric audit statement", raw.get("audit_statement"))
            if asserted_signals or observed_sample_count is not None:
                raise ArgumentError("successful telemetry metrics cannot retain refusal diagnostics")
            return cls(raw, True, metric_value, audit_statement, None, (), None)
        if refusal is None:
            raise ArgumentError("refused telemetry metrics must retain a refusal")
        if raw.get("value") is not None:
            raise ArgumentError("refused telemetry metrics cannot retain a value")
        return cls(raw, False, None, None, refusal, asserted_signals, observed_sample_count)


@dataclass(frozen=True)
class TelemetryProjectionReport:
    """Validated telemetry projection with explicit loss and metric posture."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    stage: str | None
    event_id: str | None
    event_kind: str | None
    trace: str | None
    policy_version: str | None
    record: TelemetryRecordReport | None
    loss: TelemetryLossReport | None
    lossless: bool | None
    metric: TelemetryMetricReport | None
    refusal: str | None
    fail_closed: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TelemetryProjectionReport":
        raw = _payload(value)
        ok = _bool("telemetry projection ok", raw.get("ok"))
        schema = _route_text("telemetry projection schema", raw.get("schema"))
        if schema != TELEMETRY_PROJECTION_SCHEMA:
            raise ArgumentError(f"unsupported telemetry projection schema {schema!r}")
        stage_raw = raw.get("stage")
        stage = None if stage_raw is None else _route_text("telemetry projection stage", stage_raw)
        if stage is not None and stage not in TELEMETRY_PROJECTION_STAGES:
            raise ArgumentError(f"unknown telemetry projection stage {stage!r}")
        refusal_raw = raw.get("refusal")
        refusal = None if refusal_raw is None else _route_text("telemetry projection refusal", refusal_raw)
        fail_closed = _bool("telemetry projection fail_closed", raw.get("fail_closed", False))
        guarantees = _route_strings("telemetry projection guarantees", raw.get("guarantees", []))
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("failed telemetry projections must be fail-closed")
            if raw.get("record") is not None or raw.get("loss") is not None:
                raise ArgumentError("failed telemetry projections cannot retain a record or loss report")
            return cls(raw, False, schema, stage, None, None, None, None, None, None, None, None, refusal, fail_closed, guarantees)

        if refusal is not None or stage is not None:
            raise ArgumentError("successful telemetry projections cannot retain refusal metadata")
        record = TelemetryRecordReport.from_wire(raw.get("record"))
        loss = TelemetryLossReport.from_wire(raw.get("loss"))
        lossless = _bool("telemetry lossless", raw.get("lossless"))
        if lossless != loss.lossless:
            raise ArgumentError("telemetry lossless does not reconcile with the semantic-loss report")
        event_id = _route_text("telemetry top-level event_id", raw.get("event_id"))
        event_kind = _route_text("telemetry top-level event_kind", raw.get("event_kind"))
        trace = _route_text("telemetry top-level trace", raw.get("trace"))
        policy_version = _route_text("telemetry policy_version", raw.get("policy_version"))
        if (event_id, event_kind, trace, policy_version) != (record.event_id, record.kind, record.trace, record.policy):
            raise ArgumentError("telemetry record metadata does not reconcile with its top-level projection")
        metric_raw = raw.get("metric")
        metric = None if metric_raw is None else TelemetryMetricReport.from_wire(metric_raw)
        return cls(raw, True, schema, None, event_id, event_kind, trace, policy_version, record, loss, lossless, metric, None, False, guarantees)

    @property
    def semantic_loss_is_explicit(self) -> bool:
        return self.ok and self.record is not None and self.loss is not None

    @property
    def metric_supported(self) -> bool:
        return self.metric is not None and self.metric.ok

    @property
    def metric_refused(self) -> bool:
        return self.metric is not None and not self.metric.ok

    @property
    def asserted_signals(self) -> tuple[str, ...]:
        return () if self.metric is None else self.metric.asserted_signals

    @property
    def projection_is_one_way(self) -> bool:
        return any("one-way projection" in guarantee for guarantee in self.guarantees)

    @property
    def network_export_is_not_claimed(self) -> bool:
        return any("no OTLP export" in guarantee and "network" in guarantee for guarantee in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def telemetry_project(value: Mapping[str, Any]) -> TelemetryProjectionReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return TelemetryProjectionReport.from_wire(value)


__all__ = [
    "TELEMETRY_PROJECTION_SCHEMA",
    "TELEMETRY_PROJECTION_STAGES",
    "TelemetryLossReport",
    "TelemetryRecordReport",
    "TelemetryMetricValueReport",
    "TelemetryMetricReport",
    "TelemetryProjectionReport",
    "telemetry_project",
]
