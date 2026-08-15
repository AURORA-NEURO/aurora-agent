"""Typed builders for cross-domain capability discovery."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from typing import Any, Mapping, Sequence

from .errors import ArgumentError


def _optional_text(name: str, value: str | None) -> None:
    if value is not None and (not isinstance(value, str) or not value.strip()):
        raise ArgumentError(f"{name} must be a non-empty string when supplied")


def _route_text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    return value


def _route_strings(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array of strings")
    values = tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(values) != len(set(values)):
        raise ArgumentError(f"{name} must contain unique strings")
    return values


def _route_mapping(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


def _route_count(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


@dataclass(frozen=True)
class CapabilityRouteNeedReport:
    """Validated evidence for one named need returned by ``capability_route``."""

    id: str
    resolution: str
    candidate_groups: tuple[str, ...]
    candidate_domains: tuple[str, ...]
    candidate_tools: tuple[str, ...]
    search: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityRouteNeedReport":
        raw = _route_mapping("route need report", value)
        resolution = _route_text("need resolution", raw.get("resolution"))
        if resolution not in {"explicit", "ranked_candidates", "unresolved"}:
            raise ArgumentError(f"unknown route need resolution: {resolution}")
        return cls(
            id=_route_text("need id", raw.get("id")),
            resolution=resolution,
            candidate_groups=_route_strings("candidate_groups", raw.get("candidate_groups", [])),
            candidate_domains=_route_strings("candidate_domains", raw.get("candidate_domains", [])),
            candidate_tools=_route_strings("candidate_tools", raw.get("candidate_tools", [])),
            search=_route_mapping("need search", raw.get("search", {})),
        )


@dataclass(frozen=True)
class CapabilityRouteCoverage:
    """Aggregate domain/group/tool coverage evidence for one route."""

    needs_total: int
    needs_resolved: int
    needs_unresolved: int
    candidate_group_count: int
    candidate_groups: tuple[str, ...]
    candidate_domain_count: int
    candidate_domains: tuple[str, ...]
    candidate_tool_count: int
    posture: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityRouteCoverage":
        raw = _route_mapping("route coverage", value)
        needs_total = _route_count("route coverage needs_total", raw.get("needs_total"))
        needs_resolved = _route_count("route coverage needs_resolved", raw.get("needs_resolved"))
        needs_unresolved = _route_count("route coverage needs_unresolved", raw.get("needs_unresolved"))
        candidate_group_count = _route_count(
            "route coverage candidate_group_count", raw.get("candidate_group_count")
        )
        candidate_groups = _route_strings("route coverage candidate_groups", raw.get("candidate_groups", []))
        candidate_domain_count = _route_count(
            "route coverage candidate_domain_count", raw.get("candidate_domain_count")
        )
        candidate_domains = _route_strings("route coverage candidate_domains", raw.get("candidate_domains", []))
        candidate_tool_count = _route_count(
            "route coverage candidate_tool_count", raw.get("candidate_tool_count")
        )
        if needs_resolved + needs_unresolved != needs_total:
            raise ArgumentError("route coverage need counts do not reconcile")
        if candidate_group_count != len(candidate_groups):
            raise ArgumentError("route coverage group count does not match candidate_groups")
        if candidate_domain_count != len(candidate_domains):
            raise ArgumentError("route coverage domain count does not match candidate_domains")
        return cls(
            needs_total=needs_total,
            needs_resolved=needs_resolved,
            needs_unresolved=needs_unresolved,
            candidate_group_count=candidate_group_count,
            candidate_groups=candidate_groups,
            candidate_domain_count=candidate_domain_count,
            candidate_domains=candidate_domains,
            candidate_tool_count=candidate_tool_count,
            posture=_route_text("route coverage posture", raw.get("posture")),
        )

    @property
    def fully_resolved(self) -> bool:
        """Whether every named need has at least one route candidate."""

        return self.needs_total > 0 and self.needs_unresolved == 0


@dataclass(frozen=True)
class CapabilityRouteReport:
    """Validated typed view over a non-executing cross-domain route proposal."""

    raw: dict[str, Any]
    route_id: str
    catalog_digest: str
    goal: str
    needs: tuple[CapabilityRouteNeedReport, ...]
    unresolved_needs: tuple[str, ...]
    recommended_tools: tuple[str, ...]
    recommended_tool_count: int
    recommended_tool_overflow: int
    route_coverage: CapabilityRouteCoverage
    schema_attachment: dict[str, Any]
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityRouteReport":
        raw = _route_mapping("capability route report", value)
        if raw.get("ok") is False:
            raise ArgumentError("capability route report is not successful")
        if raw.get("workflow") != "capability_route":
            raise ArgumentError("route.workflow must be capability_route")
        needs_value = raw.get("needs")
        if not isinstance(needs_value, Sequence) or isinstance(needs_value, (str, bytes)):
            raise ArgumentError("route needs must be an array")
        needs = tuple(CapabilityRouteNeedReport.from_wire(item) for item in needs_value)
        if not 1 <= len(needs) <= 32:
            raise ArgumentError("route needs must contain between 1 and 32 requirements")
        unresolved_needs = _route_strings("unresolved_needs", raw.get("unresolved_needs", []))
        need_ids = tuple(need.id for need in needs)
        if len(need_ids) != len(set(need_ids)):
            raise ArgumentError("route need ids must be unique")
        if set(unresolved_needs) != {need.id for need in needs if need.resolution == "unresolved"}:
            raise ArgumentError("unresolved_needs does not match per-need resolutions")
        coverage = CapabilityRouteCoverage.from_wire(raw.get("route_coverage", {}))
        if coverage.needs_total != len(needs):
            raise ArgumentError("route coverage needs_total does not match needs")
        recommended_tools = _route_strings("recommended_tools", raw.get("recommended_tools", []))
        recommended_tool_count = _route_count(
            "recommended_tool_count", raw.get("recommended_tool_count")
        )
        recommended_tool_overflow = _route_count(
            "recommended_tool_overflow", raw.get("recommended_tool_overflow")
        )
        if recommended_tool_count < len(recommended_tools):
            raise ArgumentError("recommended_tool_count is smaller than recommended_tools")
        if recommended_tool_count - len(recommended_tools) != recommended_tool_overflow:
            raise ArgumentError("recommended_tool_overflow does not match recommended_tools")
        if coverage.candidate_tool_count != recommended_tool_count:
            raise ArgumentError("route coverage candidate_tool_count does not match recommendations")
        return cls(
            raw=raw,
            route_id=_route_text("route_id", raw.get("route_id")),
            catalog_digest=_route_text("catalog_digest", raw.get("catalog_digest")),
            goal=_route_text("route goal", raw.get("goal")),
            needs=needs,
            unresolved_needs=unresolved_needs,
            recommended_tools=recommended_tools,
            recommended_tool_count=recommended_tool_count,
            recommended_tool_overflow=recommended_tool_overflow,
            route_coverage=coverage,
            schema_attachment=_route_mapping("schema_attachment", raw.get("schema_attachment", {})),
            execution=_route_text("route execution", raw.get("execution")),
            guarantees=_route_strings("route guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("route limitations", raw.get("limitations", [])),
        )

    @property
    def resolved_needs(self) -> tuple[CapabilityRouteNeedReport, ...]:
        return tuple(need for need in self.needs if need.resolution != "unresolved")

    @property
    def candidate_domains(self) -> tuple[str, ...]:
        return self.route_coverage.candidate_domains

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _route_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a route JSON projection from either stdio payloads or HTTP REST envelopes."""

    raw = _route_mapping("capability route response", value)
    if raw.get("workflow") == "capability_route":
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"route response text is not JSON: {error}") from error
                        return _route_mapping("decoded capability route response", decoded)
    raise ArgumentError("response does not contain a capability_route JSON projection")


def capability_route_report(value: Mapping[str, Any]) -> CapabilityRouteReport:
    """Parse either a direct route payload or an HTTP tool envelope into a typed report."""

    return CapabilityRouteReport.from_wire(_route_payload(value))


@dataclass(frozen=True)
class CapabilityQuery:
    """Conjunctive filters for the digest-bound workspace capability catalogue."""

    query: str | None = None
    group_id: str | None = None
    domain: str | None = None
    tool: str | None = None
    max_items: int = 50
    include_tools: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("query", self.query),
            ("group_id", self.group_id),
            ("domain", self.domain),
            ("tool", self.tool),
        ):
            _optional_text(name, value)
        if (
            not isinstance(self.max_items, int)
            or isinstance(self.max_items, bool)
            or not 1 <= self.max_items <= 500
        ):
            raise ArgumentError("max_items must be between 1 and 500")
        if not isinstance(self.include_tools, bool):
            raise ArgumentError("include_tools must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "max_items": self.max_items,
            "include_tools": self.include_tools,
        }
        for name in ("query", "group_id", "domain", "tool"):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = value
        return arguments


@dataclass(frozen=True)
class CapabilityRouteNeed:
    """One named requirement in a batched cross-domain route."""

    id: str
    query: CapabilityQuery = field(default_factory=CapabilityQuery)

    def __post_init__(self) -> None:
        _optional_text("need.id", self.id)
        if not isinstance(self.query, CapabilityQuery):
            raise ArgumentError("need.query must be a CapabilityQuery")
        if self.query.include_tools:
            raise ArgumentError("nested need queries cannot request tool schemas")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"id": self.id, **self.query.to_mcp_arguments()}


def _route_need(value: CapabilityRouteNeed | Mapping[str, Any]) -> CapabilityRouteNeed:
    if isinstance(value, CapabilityRouteNeed):
        return value
    if not isinstance(value, Mapping):
        raise ArgumentError("route need must be a CapabilityRouteNeed or mapping")
    raw = dict(value)
    if "id" not in raw:
        raise ArgumentError("route need requires id")
    return CapabilityRouteNeed(
        id=raw["id"],
        query=CapabilityQuery(
            query=raw.get("query"),
            group_id=raw.get("group_id"),
            domain=raw.get("domain"),
            tool=raw.get("tool"),
            max_items=raw.get("max_items", 50),
            include_tools=raw.get("include_tools", False),
        ),
    )


@dataclass(frozen=True)
class CapabilityRouteRequest:
    """Bounded multi-need routing that never executes the returned candidates."""

    goal: str
    needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]]
    max_candidates_per_need: int = 10
    max_tools: int = 128
    include_tools: bool = False

    def __post_init__(self) -> None:
        _optional_text("goal", self.goal)
        if (
            not isinstance(self.needs, Sequence)
            or isinstance(self.needs, (str, bytes))
            or not self.needs
            or len(self.needs) > 32
        ):
            raise ArgumentError("needs must contain between 1 and 32 named requirements")
        ids: set[str] = set()
        for value in self.needs:
            need = _route_need(value)
            if need.id in ids:
                raise ArgumentError(f"duplicate route need id: {need.id}")
            ids.add(need.id)
        for name, value, maximum in (
            ("max_candidates_per_need", self.max_candidates_per_need, 50),
            ("max_tools", self.max_tools, 256),
        ):
            if not isinstance(value, int) or isinstance(value, bool) or not 1 <= value <= maximum:
                raise ArgumentError(f"{name} must be between 1 and {maximum}")
        if not isinstance(self.include_tools, bool):
            raise ArgumentError("include_tools must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "goal": self.goal,
            "needs": [_route_need(value).to_mcp_arguments() for value in self.needs],
            "max_candidates_per_need": self.max_candidates_per_need,
            "max_tools": self.max_tools,
            "include_tools": self.include_tools,
        }


__all__ = [
    "CapabilityQuery",
    "CapabilityRouteNeed",
    "CapabilityRouteRequest",
    "CapabilityRouteNeedReport",
    "CapabilityRouteCoverage",
    "CapabilityRouteReport",
    "capability_route_report",
]
