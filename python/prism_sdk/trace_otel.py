"""Typed bounded OTLP JSON ingestion requests and reports.

The OTLP adapter is intentionally a one-way import into the trajectory Event IR.  These models
keep the normalized event preview, source-to-IR mapping counts, every semantic-loss category, and
compilation readiness visible without interpreting vendor conventions or claiming export.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_text
from .errors import ArgumentError


TRACE_OTEL_INGEST_SCHEMA = "bioprism-mcp/trace-otel-ingest/0.1"
TRACE_OTEL_EVENT_KINDS = frozenset({"goal", "observation", "choice", "action", "result", "claim", "termination"})
TRACE_OTEL_MAX_SPANS = 100_000
TRACE_OTEL_MAX_ITEMS = 1_000
TRACE_OTEL_MAX_BYTES = 10_000_000


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _text(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _texts(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a trace ingestion report from direct MCP output or an HTTP REST envelope."""

    raw = _route_mapping("trace OTLP response", value)
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
                        raise ArgumentError(f"trace OTLP response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == TRACE_OTEL_INGEST_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a trace OTLP ingestion report")


@dataclass(frozen=True)
class TraceOtelIngestArgs:
    """Bounded inline or root-confined OTLP JSON import request."""

    trace_id: str
    otlp_json: str | None = None
    document: str | None = None
    succeeded: bool = False
    include_events: bool = False
    max_items: int = 100
    max_spans: int = TRACE_OTEL_MAX_SPANS
    max_bytes: int = TRACE_OTEL_MAX_BYTES

    def __post_init__(self) -> None:
        object.__setattr__(self, "trace_id", _text("trace_id", self.trace_id))
        if (self.otlp_json is None) == (self.document is None):
            raise ArgumentError("provide exactly one of otlp_json or document")
        for name, value, maximum in (("max_items", self.max_items, TRACE_OTEL_MAX_ITEMS), ("max_spans", self.max_spans, TRACE_OTEL_MAX_SPANS), ("max_bytes", self.max_bytes, TRACE_OTEL_MAX_BYTES)):
            if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
                raise ArgumentError(f"{name} must be between 1 and {maximum}")
        if self.otlp_json is not None:
            if not isinstance(self.otlp_json, str):
                raise ArgumentError("otlp_json must be a string")
            if len(self.otlp_json.encode("utf-8")) > self.max_bytes:
                raise ArgumentError("otlp_json exceeds max_bytes")
        if self.document is not None:
            object.__setattr__(self, "document", _text("document", self.document))
        if not isinstance(self.succeeded, bool) or not isinstance(self.include_events, bool):
            raise ArgumentError("succeeded and include_events must be booleans")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelIngestArgs":
        raw = _route_mapping("trace OTLP arguments", value)
        return cls(raw.get("trace_id"), raw.get("otlp_json"), raw.get("document"), raw.get("succeeded", False), raw.get("include_events", False), raw.get("max_items", 100), raw.get("max_spans", TRACE_OTEL_MAX_SPANS), raw.get("max_bytes", TRACE_OTEL_MAX_BYTES))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"trace_id": self.trace_id, "succeeded": self.succeeded, "include_events": self.include_events, "max_items": self.max_items, "max_spans": self.max_spans, "max_bytes": self.max_bytes}
        if self.otlp_json is not None:
            result["otlp_json"] = self.otlp_json
        if self.document is not None:
            result["document"] = self.document
        return result


@dataclass(frozen=True)
class TraceOtelFieldLossReport:
    raw: dict[str, Any]
    path: str
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelFieldLossReport":
        raw = _route_mapping("trace OTLP field loss", value)
        return cls(raw, _text("trace OTLP loss path", raw.get("path")), _text("trace OTLP loss detail", raw.get("detail")))


@dataclass(frozen=True)
class TraceOtelDroppedSpanReport:
    raw: dict[str, Any]
    path: str
    name: str | None
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelDroppedSpanReport":
        raw = _route_mapping("trace OTLP dropped span", value)
        return cls(raw, _text("trace OTLP dropped span path", raw.get("path")), _optional_text("trace OTLP dropped span name", raw.get("name")), _text("trace OTLP dropped span detail", raw.get("detail")))


@dataclass(frozen=True)
class TraceOtelLossReport:
    raw: dict[str, Any]
    dropped_spans: tuple[TraceOtelDroppedSpanReport, ...]
    dropped_span_events: tuple[TraceOtelFieldLossReport, ...]
    unmapped_fields: tuple[TraceOtelFieldLossReport, ...]
    duplicate_attributes: tuple[TraceOtelFieldLossReport, ...]
    inferred_kinds: tuple[TraceOtelFieldLossReport, ...]
    missing_start_times: tuple[TraceOtelFieldLossReport, ...]
    unresolved_parents: tuple[TraceOtelFieldLossReport, ...]
    multiple_trace_ids: tuple[TraceOtelFieldLossReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelLossReport":
        raw = _route_mapping("trace OTLP loss", value)
        fields = lambda name: tuple(TraceOtelFieldLossReport.from_wire(item) for item in _sequence(name, raw.get(name, [])))
        return cls(raw, tuple(TraceOtelDroppedSpanReport.from_wire(item) for item in _sequence("dropped_spans", raw.get("dropped_spans", []))), fields("dropped_span_events"), fields("unmapped_fields"), fields("duplicate_attributes"), fields("inferred_kinds"), fields("missing_start_times"), fields("unresolved_parents"), fields("multiple_trace_ids"))

    @property
    def lossless(self) -> bool:
        return not any((self.dropped_spans, self.dropped_span_events, self.unmapped_fields, self.duplicate_attributes, self.inferred_kinds, self.missing_start_times, self.unresolved_parents, self.multiple_trace_ids))

    @property
    def dropped_events(self) -> int:
        return len(self.dropped_spans) + len(self.dropped_span_events)


@dataclass(frozen=True)
class TraceOtelMappingReport:
    raw: dict[str, Any]
    format: str
    resource_count: int
    scope_count: int
    source_span_count: int
    accepted_span_count: int
    span_event_count: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelMappingReport":
        raw = _route_mapping("trace OTLP mapping", value)
        return cls(raw, _text("trace OTLP mapping format", raw.get("format")), _integer("trace OTLP resource count", raw.get("resource_count")), _integer("trace OTLP scope count", raw.get("scope_count")), _integer("trace OTLP source span count", raw.get("source_span_count")), _integer("trace OTLP accepted span count", raw.get("accepted_span_count")), _integer("trace OTLP span event count", raw.get("span_event_count")))


@dataclass(frozen=True)
class TraceOtelEventReport:
    raw: dict[str, Any]
    step: int
    kind: str
    payload: dict[str, Any]
    caused_by: int | None
    visible: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelEventReport":
        raw = _route_mapping("trace OTLP event", value)
        kind = _text("trace OTLP event kind", raw.get("kind"))
        if kind not in TRACE_OTEL_EVENT_KINDS:
            raise ArgumentError(f"unknown trace OTLP event kind {kind!r}")
        caused_by = None if raw.get("caused_by") is None else _integer("trace OTLP caused_by", raw.get("caused_by"))
        return cls(raw, _integer("trace OTLP event step", raw.get("step")), kind, _route_mapping("trace OTLP event payload", raw.get("payload")), caused_by, _texts("trace OTLP visible fields", raw.get("visible", [])))


@dataclass(frozen=True)
class TraceOtelIngestReport:
    raw: dict[str, Any]
    ok: bool
    schema: str
    trace_id: str
    event_count: int
    succeeded: bool
    trace_sha256: str
    valid: bool
    validation_error: str | None
    mapping: TraceOtelMappingReport
    loss: TraceOtelLossReport
    lossless: bool
    dropped_events: int
    compilable: bool
    events_included: bool
    events: tuple[TraceOtelEventReport, ...] | None
    omitted_events: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TraceOtelIngestReport":
        raw = _payload(value)
        ok = _bool("trace OTLP ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("trace OTLP structured reports must be successful; import errors remain transport refusals")
        schema = _text("trace OTLP schema", raw.get("schema"))
        if schema != TRACE_OTEL_INGEST_SCHEMA:
            raise ArgumentError(f"unsupported trace OTLP schema {schema!r}")
        validation_error = _optional_text("trace OTLP validation error", raw.get("validation_error"))
        valid = _bool("trace OTLP valid", raw.get("valid"))
        if valid != (validation_error is None):
            raise ArgumentError("trace OTLP valid does not reconcile with validation_error")
        loss = TraceOtelLossReport.from_wire(raw.get("loss"))
        lossless = _bool("trace OTLP lossless", raw.get("lossless"))
        if lossless != loss.lossless:
            raise ArgumentError("trace OTLP lossless does not reconcile with its loss report")
        event_count = _integer("trace OTLP event count", raw.get("event_count"))
        dropped_events = _integer("trace OTLP dropped events", raw.get("dropped_events"))
        if dropped_events != loss.dropped_events:
            raise ArgumentError("trace OTLP dropped_events does not reconcile with its loss report")
        events_included = _bool("trace OTLP events_included", raw.get("events_included"))
        events_raw = raw.get("events")
        events = None if events_raw is None else tuple(TraceOtelEventReport.from_wire(item) for item in _sequence("trace OTLP events", events_raw))
        omitted_events = _integer("trace OTLP omitted events", raw.get("omitted_events"))
        if events_included != (events is not None):
            raise ArgumentError("trace OTLP events_included does not reconcile with events")
        if events is None and omitted_events != 0:
            raise ArgumentError("trace OTLP omitted events must be zero when events are not included")
        if events is not None and len(events) + omitted_events != event_count:
            raise ArgumentError("trace OTLP event preview does not reconcile with event_count")
        compilable = _bool("trace OTLP compilable", raw.get("compilable"))
        if compilable and (not valid or not lossless or event_count == 0):
            raise ArgumentError("compilable OTLP traces must be valid, lossless, and non-empty")
        mapping = TraceOtelMappingReport.from_wire(raw.get("mapping"))
        if mapping.accepted_span_count != event_count or mapping.accepted_span_count > mapping.source_span_count:
            raise ArgumentError("trace OTLP mapping counts do not reconcile with the normalized trace")
        return cls(raw, True, schema, _text("trace OTLP trace_id", raw.get("trace_id")), event_count, _bool("trace OTLP succeeded", raw.get("succeeded")), _text("trace OTLP trace_sha256", raw.get("trace_sha256")), valid, validation_error, mapping, loss, lossless, dropped_events, compilable, events_included, events, omitted_events, tuple(_text(f"trace OTLP guarantee[{index}]", item) for index, item in enumerate(_sequence("trace OTLP guarantees", raw.get("guarantees", [])))), tuple(_text(f"trace OTLP limitation[{index}]", item) for index, item in enumerate(_sequence("trace OTLP limitations", raw.get("limitations", [])))))

    @property
    def ready_for_compilation(self) -> bool:
        return self.compilable

    @property
    def semantic_loss_is_explicit(self) -> bool:
        return self.loss is not None and self.dropped_events == self.loss.dropped_events

    @property
    def network_export_is_not_claimed(self) -> bool:
        return any("not an OTLP exporter" in limitation or "network export" in limitation for limitation in self.limitations)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def trace_otel_ingest(value: Mapping[str, Any]) -> TraceOtelIngestReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return TraceOtelIngestReport.from_wire(value)


__all__ = [
    "TRACE_OTEL_INGEST_SCHEMA",
    "TRACE_OTEL_EVENT_KINDS",
    "TRACE_OTEL_MAX_SPANS",
    "TRACE_OTEL_MAX_ITEMS",
    "TRACE_OTEL_MAX_BYTES",
    "TraceOtelIngestArgs",
    "TraceOtelFieldLossReport",
    "TraceOtelDroppedSpanReport",
    "TraceOtelLossReport",
    "TraceOtelMappingReport",
    "TraceOtelEventReport",
    "TraceOtelIngestReport",
    "trace_otel_ingest",
]
