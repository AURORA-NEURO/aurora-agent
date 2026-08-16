"""Typed capability-atlas coverage, failure-debt, and composite reports.

The atlas report is a coverage object rather than a leaderboard.  This module keeps measured
scores attached to their depth and effective evidence, preserves every bounded hole and its
claim-blocking influence, and represents composite eligibility as either an explicit value or a
fail-closed refusal.  It validates projections without recomputing atlas semantics in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ATLAS_REPORT_SCHEMA = "bioprism-mcp/atlas-report/0.1"
ATLAS_MAX_INPUT_BYTES = 10_000_000
ATLAS_MAX_ITEMS = 1_000


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _texts(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract an atlas report from direct MCP output or an HTTP REST envelope."""

    raw = _mapping("atlas report response", value)
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
                        raise ArgumentError(f"atlas report response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == ATLAS_REPORT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an atlas report")


@dataclass(frozen=True)
class AtlasReportArgs:
    """Bounded serialized Atlas with an optional predeclared composite weighting policy."""

    atlas: dict[str, Any]
    weighting: dict[str, Any] | None = None
    max_items: int = 100

    def __init__(
        self,
        atlas: Mapping[str, Any],
        weighting: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> None:
        normalized_atlas = _mapping("atlas", atlas)
        normalized_weighting = None if weighting is None else _mapping("atlas weighting", weighting)
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= ATLAS_MAX_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {ATLAS_MAX_ITEMS}")
        try:
            encoded_size = len(json.dumps(normalized_atlas, separators=(",", ":")).encode("utf-8"))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"atlas must be JSON serializable: {error}") from error
        if encoded_size > ATLAS_MAX_INPUT_BYTES:
            raise ArgumentError(f"atlas exceeds the {ATLAS_MAX_INPUT_BYTES}-byte safety bound")
        object.__setattr__(self, "atlas", normalized_atlas)
        object.__setattr__(self, "weighting", normalized_weighting)
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasReportArgs":
        raw = _mapping("atlas report arguments", value)
        return cls(raw.get("atlas"), raw.get("weighting"), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"atlas": self.atlas, "max_items": self.max_items}
        if self.weighting is not None:
            result["weighting"] = self.weighting
        return result


@dataclass(frozen=True)
class AtlasMeasuredEntryReport:
    raw: dict[str, Any]
    capability: str
    family: str
    score: float
    depth: str
    evaluable: int
    excluded: int
    effective_size: int
    generated_instances: int
    permitted_claim: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasMeasuredEntryReport":
        raw = _mapping("atlas measured entry", value)
        return cls(
            raw,
            _route_text("atlas measured capability", raw.get("capability")),
            _route_text("atlas measured family", raw.get("family")),
            _number("atlas measured score", raw.get("score")),
            _route_text("atlas measured depth", raw.get("depth")),
            _integer("atlas measured evaluable", raw.get("evaluable")),
            _integer("atlas measured excluded", raw.get("excluded")),
            _integer("atlas measured effective size", raw.get("effective_size")),
            _integer("atlas measured generated instances", raw.get("generated_instances")),
            _route_text("atlas measured permitted claim", raw.get("permitted_claim")),
        )


@dataclass(frozen=True)
class AtlasHoleReport:
    raw: dict[str, Any]
    capability: str
    family: str
    reason: str
    influence: str
    aggregate: bool
    blocks_claims_for: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasHoleReport":
        raw = _mapping("atlas hole", value)
        return cls(
            raw,
            _route_text("atlas hole capability", raw.get("capability")),
            _route_text("atlas hole family", raw.get("family")),
            _route_text("atlas hole reason", raw.get("reason")),
            _route_text("atlas hole influence", raw.get("influence")),
            _bool("atlas hole aggregate", raw.get("aggregate")),
            _texts("atlas hole blocking claims", raw.get("blocks_claims_for", [])),
        )


@dataclass(frozen=True)
class AtlasFamilyCoverageReport:
    raw: dict[str, Any]
    family: str
    total: int
    measured: int
    holes: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasFamilyCoverageReport":
        raw = _mapping("atlas family coverage", value)
        return cls(raw, _route_text("atlas family", raw.get("family")), _integer("atlas family total", raw.get("total")), _integer("atlas family measured", raw.get("measured")), _integer("atlas family holes", raw.get("holes")))

    @property
    def is_dark(self) -> bool:
        return self.total > 0 and self.measured == 0


@dataclass(frozen=True)
class AtlasHistogramEntryReport:
    raw: dict[str, Any]
    label: str
    count: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], label_field: str) -> "AtlasHistogramEntryReport":
        raw = _mapping("atlas histogram entry", value)
        return cls(raw, _route_text(f"atlas histogram {label_field}", raw.get(label_field)), _integer("atlas histogram count", raw.get("count")))


@dataclass(frozen=True)
class AtlasCoverageDebtReport:
    raw: dict[str, Any]
    total_capabilities: int
    measured: int
    unmeasured: int
    closed_by_declaration: int
    dark_families: tuple[str, ...]
    unclassified_failures: int
    undiagnosed_failures: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasCoverageDebtReport":
        raw = _mapping("atlas coverage debt", value)
        return cls(
            raw,
            _integer("atlas debt total capabilities", raw.get("total_capabilities")),
            _integer("atlas debt measured", raw.get("measured")),
            _integer("atlas debt unmeasured", raw.get("unmeasured")),
            _integer("atlas debt closed by declaration", raw.get("closed_by_declaration")),
            _texts("atlas debt dark families", raw.get("dark_families", [])),
            _integer("atlas debt unclassified failures", raw.get("unclassified_failures")),
            _integer("atlas debt undiagnosed failures", raw.get("undiagnosed_failures")),
        )


@dataclass(frozen=True)
class AtlasInconsistencyReport:
    raw: dict[str, Any]
    kind: str
    capability: str | None
    failure_id: str | None
    failures_recorded: int | None
    failed_trials: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasInconsistencyReport":
        raw = _mapping("atlas inconsistency", value)
        return cls(
            raw,
            _route_text("atlas inconsistency kind", raw.get("kind")),
            _optional_text("atlas inconsistency capability", raw.get("capability")),
            _optional_text("atlas inconsistency failure id", raw.get("failure_id")),
            None if raw.get("failures_recorded") is None else _integer("atlas failures recorded", raw.get("failures_recorded")),
            None if raw.get("failed_trials") is None else _integer("atlas failed trials", raw.get("failed_trials")),
        )


@dataclass(frozen=True)
class AtlasCompositeReport:
    """An eligible composite or the explicit refusal that prevented one."""

    raw: dict[str, Any]
    state: str
    intended_use: str | None
    value: float | None
    weighted_capabilities: int | None
    tier: str | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasCompositeReport":
        raw = _mapping("atlas composite", value)
        ok = _bool("atlas composite ok", raw.get("ok"))
        if ok:
            composite = _mapping("atlas composite value", raw.get("value"))
            return cls(
                raw,
                "eligible",
                _route_text("atlas composite intended use", composite.get("intended_use")),
                _number("atlas composite value", composite.get("value")),
                _integer("atlas weighted capabilities", composite.get("weighted_capabilities")),
                _route_text("atlas composite tier", composite.get("tier")),
                None,
                None,
            )
        return cls(
            raw,
            "refused",
            None,
            None,
            None,
            None,
            _route_text("atlas composite refusal", raw.get("refusal")),
            _bool("atlas composite fail_closed", raw.get("fail_closed")),
        )

    @property
    def eligible(self) -> bool:
        return self.state == "eligible"


@dataclass(frozen=True)
class AtlasSummaryReport:
    raw: dict[str, Any]
    measured: int
    holes: int
    families: int
    inconsistencies: int
    coverage_debt_ratio: float
    has_holes: bool
    coverage_supports_aggregation: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasSummaryReport":
        raw = _mapping("atlas summary", value)
        return cls(
            raw,
            _integer("atlas summary measured", raw.get("measured")),
            _integer("atlas summary holes", raw.get("holes")),
            _integer("atlas summary families", raw.get("families")),
            _integer("atlas summary inconsistencies", raw.get("inconsistencies")),
            _number("atlas coverage debt ratio", raw.get("coverage_debt_ratio")),
            _bool("atlas summary has holes", raw.get("has_holes")),
            _bool("atlas summary aggregation support", raw.get("coverage_supports_aggregation")),
        )


@dataclass(frozen=True)
class AtlasReport:
    """Validated capability coverage and failure-debt projection."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    ontology_version: str
    summary: AtlasSummaryReport
    debt: AtlasCoverageDebtReport
    measured: tuple[AtlasMeasuredEntryReport, ...]
    omitted_measured: int
    holes: tuple[AtlasHoleReport, ...]
    omitted_holes: int
    family_coverage: tuple[AtlasFamilyCoverageReport, ...]
    omitted_families: int
    depth_histogram: tuple[AtlasHistogramEntryReport, ...]
    stage_histogram: tuple[AtlasHistogramEntryReport, ...]
    inconsistencies: tuple[AtlasInconsistencyReport, ...]
    omitted_inconsistencies: int
    composite: AtlasCompositeReport | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasReport":
        raw = _payload(value)
        ok = _bool("atlas report ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("atlas reports must be successful; refusals remain transport errors")
        schema = _route_text("atlas report schema", raw.get("schema"))
        if schema != ATLAS_REPORT_SCHEMA:
            raise ArgumentError(f"unsupported atlas report schema {schema!r}")
        summary = AtlasSummaryReport.from_wire(raw.get("summary"))
        debt = AtlasCoverageDebtReport.from_wire(raw.get("debt"))
        measured = tuple(AtlasMeasuredEntryReport.from_wire(item) for item in _sequence("atlas measured", raw.get("measured", [])))
        holes = tuple(AtlasHoleReport.from_wire(item) for item in _sequence("atlas holes", raw.get("holes", [])))
        family_coverage = tuple(AtlasFamilyCoverageReport.from_wire(item) for item in _sequence("atlas family coverage", raw.get("family_coverage", [])))
        depth_histogram = tuple(AtlasHistogramEntryReport.from_wire(item, "depth") for item in _sequence("atlas depth histogram", raw.get("depth_histogram", [])))
        stage_histogram = tuple(AtlasHistogramEntryReport.from_wire(item, "stage") for item in _sequence("atlas stage histogram", raw.get("stage_histogram", [])))
        inconsistencies = tuple(AtlasInconsistencyReport.from_wire(item) for item in _sequence("atlas inconsistencies", raw.get("inconsistencies", [])))
        omitted_measured = _integer("atlas omitted measured", raw.get("omitted_measured"))
        omitted_holes = _integer("atlas omitted holes", raw.get("omitted_holes"))
        omitted_families = _integer("atlas omitted families", raw.get("omitted_families"))
        omitted_inconsistencies = _integer("atlas omitted inconsistencies", raw.get("omitted_inconsistencies"))
        if len(measured) + omitted_measured != summary.measured:
            raise ArgumentError("atlas measured projection does not reconcile with its summary")
        if len(holes) + omitted_holes != summary.holes:
            raise ArgumentError("atlas hole projection does not reconcile with its summary")
        if len(family_coverage) + omitted_families != summary.families:
            raise ArgumentError("atlas family projection does not reconcile with its summary")
        if len(inconsistencies) + omitted_inconsistencies != summary.inconsistencies:
            raise ArgumentError("atlas inconsistency projection does not reconcile with its summary")
        if debt.measured != summary.measured or debt.unmeasured != summary.holes:
            raise ArgumentError("atlas coverage debt does not reconcile with its summary")
        composite_raw = raw.get("composite")
        composite = None if composite_raw is None else AtlasCompositeReport.from_wire(composite_raw)
        return cls(
            raw,
            True,
            schema,
            _route_text("atlas ontology version", raw.get("ontology_version")),
            summary,
            debt,
            measured,
            omitted_measured,
            holes,
            omitted_holes,
            family_coverage,
            omitted_families,
            depth_histogram,
            stage_histogram,
            inconsistencies,
            omitted_inconsistencies,
            composite,
            _texts("atlas guarantees", raw.get("guarantees", [])),
            _texts("atlas limitations", raw.get("limitations", [])),
        )

    @property
    def has_holes(self) -> bool:
        return self.summary.has_holes

    @property
    def coverage_supports_aggregation(self) -> bool:
        return self.summary.coverage_supports_aggregation

    @property
    def composite_is_eligible(self) -> bool:
        return self.composite is not None and self.composite.eligible

    @property
    def all_holes_visible(self) -> bool:
        return self.omitted_holes == 0

    @property
    def dark_families(self) -> tuple[str, ...]:
        return self.debt.dark_families

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def atlas_report(value: Mapping[str, Any]) -> AtlasReport:
    """Parse a direct MCP result or HTTP envelope into a typed atlas report."""

    return AtlasReport.from_wire(value)


__all__ = [
    "ATLAS_REPORT_SCHEMA",
    "ATLAS_MAX_INPUT_BYTES",
    "ATLAS_MAX_ITEMS",
    "AtlasReportArgs",
    "AtlasMeasuredEntryReport",
    "AtlasHoleReport",
    "AtlasFamilyCoverageReport",
    "AtlasHistogramEntryReport",
    "AtlasCoverageDebtReport",
    "AtlasInconsistencyReport",
    "AtlasCompositeReport",
    "AtlasSummaryReport",
    "AtlasReport",
    "atlas_report",
]
