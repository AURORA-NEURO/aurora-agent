"""Typed atlasx coverage-debt and failure-browse surface audits.

The atlas surface is intentionally different from the base atlas report.  It reads a
CapabilityGrid as a denominator-bearing publication surface, optionally compares it with a
later grid, and browses FailureRecord values without treating the browse as a trial rate.  This
module validates the transport envelope and preserves the server's explicit refusal states; it
does not reimplement atlasx semantics in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ATLAS_SURFACE_SCHEMA = "bioprism-mcp/atlas-surface-audit/0.1"
ATLAS_SURFACE_FACETS = frozenset(
    {"mechanism", "first_divergence_stage", "severity", "inducement", "architecture_component"}
)
ATLAS_SURFACE_MAX_INPUT_BYTES = 20_000_000
ATLAS_SURFACE_MAX_FAILURES = 8_192
ATLAS_SURFACE_MAX_VISIBILITY = 8_192
ATLAS_SURFACE_MAX_RATE_CAPABILITIES = 4_096
ATLAS_SURFACE_MAX_ITEMS = 1_000


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _texts(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("atlas surface response", value)
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
                        raise ArgumentError(f"atlas surface response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == ATLAS_SURFACE_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an atlas surface projection")


@dataclass(frozen=True)
class AtlasSurfaceAuditArgs:
    """Bounded serialized grids, failure records, and publication-surface policies."""

    grid: dict[str, Any]
    later_grid: dict[str, Any] | None = None
    failures: tuple[dict[str, Any], ...] = ()
    failure_subject: str | None = None
    facet: str = "mechanism"
    visibility: tuple[dict[str, Any], ...] = ()
    rate_capabilities: tuple[str, ...] = ()
    require_no_holes: bool = False
    require_no_blocking_debt: bool = False
    require_no_withheld: bool = False
    require_sound_surfaces: bool = False
    max_items: int = 100

    def __init__(
        self,
        grid: Mapping[str, Any],
        later_grid: Mapping[str, Any] | None = None,
        failures: Sequence[Mapping[str, Any]] = (),
        failure_subject: str | None = None,
        facet: str = "mechanism",
        visibility: Sequence[Mapping[str, Any]] = (),
        rate_capabilities: Sequence[str] = (),
        require_no_holes: bool = False,
        require_no_blocking_debt: bool = False,
        require_no_withheld: bool = False,
        require_sound_surfaces: bool = False,
        max_items: int = 100,
    ) -> None:
        normalized_grid = _mapping("atlas surface grid", grid)
        normalized_later = None if later_grid is None else _mapping("atlas surface later_grid", later_grid)
        normalized_failures = tuple(
            _mapping(f"atlas surface failures[{index}]", item)
            for index, item in enumerate(_sequence("atlas surface failures", failures))
        )
        normalized_visibility = tuple(
            _mapping(f"atlas surface visibility[{index}]", item)
            for index, item in enumerate(_sequence("atlas surface visibility", visibility))
        )
        normalized_rates = tuple(
            _route_text(f"atlas surface rate_capabilities[{index}]", item)
            for index, item in enumerate(_sequence("atlas surface rate_capabilities", rate_capabilities))
        )
        if len(normalized_failures) > ATLAS_SURFACE_MAX_FAILURES:
            raise ArgumentError("atlas surface failures are bounded at 8192 records")
        if len(normalized_visibility) > ATLAS_SURFACE_MAX_VISIBILITY:
            raise ArgumentError("atlas surface visibility is bounded at 8192 declarations")
        if len(normalized_rates) > ATLAS_SURFACE_MAX_RATE_CAPABILITIES:
            raise ArgumentError("atlas surface rate_capabilities are bounded at 4096 identifiers")
        normalized_subject = None if failure_subject is None else _route_text(
            "atlas surface failure_subject", failure_subject
        )
        if normalized_subject is not None and len(normalized_subject.encode("utf-8")) > 4_096:
            raise ArgumentError("atlas surface failure_subject exceeds 4096 UTF-8 bytes")
        normalized_facet = _route_text("atlas surface facet", facet)
        if normalized_facet not in ATLAS_SURFACE_FACETS:
            raise ArgumentError(f"atlas surface facet {normalized_facet!r} is not recognized")
        for name, flag in (
            ("require_no_holes", require_no_holes),
            ("require_no_blocking_debt", require_no_blocking_debt),
            ("require_no_withheld", require_no_withheld),
            ("require_sound_surfaces", require_sound_surfaces),
        ):
            if not isinstance(flag, bool):
                raise ArgumentError(f"atlas surface {name} must be a boolean")
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= ATLAS_SURFACE_MAX_ITEMS:
            raise ArgumentError(f"atlas surface max_items must be between 1 and {ATLAS_SURFACE_MAX_ITEMS}")
        try:
            encoded_size = len(
                json.dumps(
                    {
                        "grid": normalized_grid,
                        "later_grid": normalized_later,
                        "failures": normalized_failures,
                        "visibility": normalized_visibility,
                    },
                    separators=(",", ":"),
                ).encode("utf-8")
            )
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"atlas surface arguments must be JSON serializable: {error}") from error
        if encoded_size > ATLAS_SURFACE_MAX_INPUT_BYTES:
            raise ArgumentError(
                f"atlas surface input exceeds the {ATLAS_SURFACE_MAX_INPUT_BYTES}-byte safety bound"
            )
        object.__setattr__(self, "grid", normalized_grid)
        object.__setattr__(self, "later_grid", normalized_later)
        object.__setattr__(self, "failures", normalized_failures)
        object.__setattr__(self, "failure_subject", normalized_subject)
        object.__setattr__(self, "facet", normalized_facet)
        object.__setattr__(self, "visibility", normalized_visibility)
        object.__setattr__(self, "rate_capabilities", normalized_rates)
        object.__setattr__(self, "require_no_holes", require_no_holes)
        object.__setattr__(self, "require_no_blocking_debt", require_no_blocking_debt)
        object.__setattr__(self, "require_no_withheld", require_no_withheld)
        object.__setattr__(self, "require_sound_surfaces", require_sound_surfaces)
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasSurfaceAuditArgs":
        raw = _mapping("atlas surface audit arguments", value)
        return cls(
            raw.get("grid"),
            raw.get("later_grid"),
            _sequence("atlas surface failures", raw.get("failures", [])),
            raw.get("failure_subject"),
            raw.get("facet", "mechanism"),
            _sequence("atlas surface visibility", raw.get("visibility", [])),
            _sequence("atlas surface rate_capabilities", raw.get("rate_capabilities", [])),
            raw.get("require_no_holes", False),
            raw.get("require_no_blocking_debt", False),
            raw.get("require_no_withheld", False),
            raw.get("require_sound_surfaces", False),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "grid": self.grid,
            "failures": list(self.failures),
            "facet": self.facet,
            "visibility": list(self.visibility),
            "rate_capabilities": list(self.rate_capabilities),
            "require_no_holes": self.require_no_holes,
            "require_no_blocking_debt": self.require_no_blocking_debt,
            "require_no_withheld": self.require_no_withheld,
            "require_sound_surfaces": self.require_sound_surfaces,
            "max_items": self.max_items,
        }
        if self.later_grid is not None:
            result["later_grid"] = self.later_grid
        if self.failure_subject is not None:
            result["failure_subject"] = self.failure_subject
        return result


@dataclass(frozen=True)
class AtlasSurfaceCoverageReport:
    raw: dict[str, Any]
    subject: str
    total_capabilities: int
    measured: int
    unmeasured: int
    blocking: int
    closed_by_declaration: int
    vacuous: bool
    holes: tuple[Mapping[str, Any], ...]
    omitted_holes: int
    profile_coverage: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasSurfaceCoverageReport":
        raw = _mapping("atlas surface coverage", value)
        return cls(
            raw,
            _route_text("atlas surface subject", raw.get("subject")),
            _integer("atlas surface total capabilities", raw.get("total_capabilities")),
            _integer("atlas surface measured", raw.get("measured")),
            _integer("atlas surface unmeasured", raw.get("unmeasured")),
            _integer("atlas surface blocking", raw.get("blocking")),
            _integer("atlas surface closed by declaration", raw.get("closed_by_declaration")),
            _bool("atlas surface vacuous", raw.get("vacuous")),
            tuple(_mapping("atlas surface hole", item) for item in _sequence("atlas surface holes", raw.get("holes", []))),
            _integer("atlas surface omitted holes", raw.get("omitted_holes")),
            _mapping("atlas surface profile coverage", raw.get("profile_coverage")),
        )

    @property
    def has_holes(self) -> bool:
        return self.unmeasured > 0

    @property
    def all_holes_visible(self) -> bool:
        return self.omitted_holes == 0


@dataclass(frozen=True)
class AtlasSurfaceBrowseReport:
    raw: dict[str, Any]
    subject: str
    facet: str
    taxonomy_version: str
    records_browsed: int
    visible: int
    withheld: int
    contested: int
    undiagnosed: int
    evaluator_induced: int
    distinct_families: int
    shares_sum_to_one: bool
    buckets: tuple[Mapping[str, Any], ...]
    omitted_buckets: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasSurfaceBrowseReport":
        raw = _mapping("atlas surface failure browse", value)
        return cls(
            raw,
            _route_text("atlas surface browse subject", raw.get("subject")),
            _route_text("atlas surface browse facet", raw.get("facet")),
            _route_text("atlas surface taxonomy version", raw.get("taxonomy_version")),
            _integer("atlas surface records browsed", raw.get("records_browsed")),
            _integer("atlas surface visible records", raw.get("visible")),
            _integer("atlas surface withheld records", raw.get("withheld")),
            _integer("atlas surface contested records", raw.get("contested")),
            _integer("atlas surface undiagnosed records", raw.get("undiagnosed")),
            _integer("atlas surface evaluator-induced records", raw.get("evaluator_induced")),
            _integer("atlas surface distinct families", raw.get("distinct_families")),
            _bool("atlas surface shares sum to one", raw.get("shares_sum_to_one")),
            tuple(_mapping("atlas surface bucket", item) for item in _sequence("atlas surface buckets", raw.get("buckets", []))),
            _integer("atlas surface omitted buckets", raw.get("omitted_buckets")),
        )


@dataclass(frozen=True)
class AtlasSurfaceAuditReport:
    """Validated coverage debt, failure browsing, rate checks, and surface soundness."""

    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    coverage: AtlasSurfaceCoverageReport | None
    debt_discharge: Mapping[str, Any] | None
    failure_browse: AtlasSurfaceBrowseReport | None
    rate_checks: Mapping[str, Any] | None
    surface_audits: Mapping[str, Any] | None
    policies: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AtlasSurfaceAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("atlas surface refusals must be fail-closed")
            return cls(
                raw,
                False,
                raw.get("schema"),
                raw.get("workflow"),
                None,
                None,
                None,
                None,
                None,
                None,
                _route_text("atlas surface refusal stage", raw.get("stage")),
                _route_text("atlas surface refusal", raw.get("refusal")),
                _route_strings("atlas surface refusal guarantees", raw.get("guarantees", [])),
                _route_strings("atlas surface refusal limitations", raw.get("limitations", [])),
                True,
            )
        if raw.get("ok") is not True or raw.get("schema") != ATLAS_SURFACE_SCHEMA:
            raise ArgumentError("atlas surface projection has an invalid schema")
        coverage = AtlasSurfaceCoverageReport.from_wire(raw.get("coverage"))
        browse = AtlasSurfaceBrowseReport.from_wire(raw.get("failure_browse"))
        return cls(
            raw,
            True,
            ATLAS_SURFACE_SCHEMA,
            _route_text("atlas surface workflow", raw.get("workflow")),
            coverage,
            None if raw.get("debt_discharge") is None else _mapping("atlas surface debt discharge", raw.get("debt_discharge")),
            browse,
            _mapping("atlas surface rate checks", raw.get("rate_checks")),
            _mapping("atlas surface audits", raw.get("surface_audits")),
            _mapping("atlas surface policies", raw.get("policies")),
            None,
            None,
            _route_strings("atlas surface guarantees", raw.get("guarantees", [])),
            _route_strings("atlas surface limitations", raw.get("limitations", [])),
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def measured(self) -> int | None:
        return None if self.coverage is None else self.coverage.measured

    @property
    def unmeasured(self) -> int | None:
        return None if self.coverage is None else self.coverage.unmeasured

    @property
    def withheld(self) -> int | None:
        return None if self.failure_browse is None else self.failure_browse.withheld

    @property
    def surface_sound(self) -> bool | None:
        if self.surface_audits is None:
            return None
        value = self.surface_audits.get("sound")
        return value if isinstance(value, bool) else None

    @property
    def has_evidence_discharge(self) -> bool | None:
        if self.debt_discharge is None:
            return None
        value = self.debt_discharge.get("any_evidence")
        return value if isinstance(value, bool) else None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def atlas_surface_audit_report(value: Mapping[str, Any]) -> AtlasSurfaceAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return AtlasSurfaceAuditReport.from_wire(value)


__all__ = [
    "ATLAS_SURFACE_SCHEMA",
    "ATLAS_SURFACE_FACETS",
    "ATLAS_SURFACE_MAX_INPUT_BYTES",
    "ATLAS_SURFACE_MAX_FAILURES",
    "ATLAS_SURFACE_MAX_VISIBILITY",
    "ATLAS_SURFACE_MAX_RATE_CAPABILITIES",
    "ATLAS_SURFACE_MAX_ITEMS",
    "AtlasSurfaceAuditArgs",
    "AtlasSurfaceCoverageReport",
    "AtlasSurfaceBrowseReport",
    "AtlasSurfaceAuditReport",
    "atlas_surface_audit_report",
]
