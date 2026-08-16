"""Typed cross-domain capability dashboard requests and reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import (
    _optional_text,
    _route_count,
    _route_mapping,
    _route_strings,
    _route_text,
    _tool_payload,
)
from .errors import ArgumentError

CAPABILITY_DASHBOARD_SCHEMA = "bioprism-devplat-capability-dashboard/0.1"
MAX_DASHBOARD_GROUPS = 512
DEFAULT_DASHBOARD_GROUPS = 128


@dataclass(frozen=True)
class CapabilityDashboardQueryArgs:
    group_id: str | None = None
    domain: str | None = None
    status: str | None = None
    max_groups: int = DEFAULT_DASHBOARD_GROUPS
    include_tools: bool = False
    include_gaps: bool = True

    def __post_init__(self) -> None:
        for name, value in (("group_id", self.group_id), ("domain", self.domain), ("status", self.status)):
            _optional_text(f"dashboard.{name}", value)
            if value is not None and len(value.encode("utf-8")) > 512:
                raise ArgumentError(f"dashboard.{name} exceeds 512 UTF-8 bytes")
        if not isinstance(self.max_groups, int) or isinstance(self.max_groups, bool) or not 1 <= self.max_groups <= MAX_DASHBOARD_GROUPS:
            raise ArgumentError(f"dashboard.max_groups must be between 1 and {MAX_DASHBOARD_GROUPS}")
        for name in ("include_tools", "include_gaps"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"dashboard.{name} must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "max_groups": self.max_groups,
            "include_tools": self.include_tools,
            "include_gaps": self.include_gaps,
        }
        for name in ("group_id", "domain", "status"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class CapabilityDashboardGroupReport:
    raw: dict[str, Any]
    id: str
    domains: tuple[str, ...]
    status: str
    readiness: str
    surfaces: dict[str, int]
    tool_count: int
    callable_tool_count: int
    schema_backed_tool_count: int
    missing_transport_schemas: tuple[str, ...]
    invalid_transport_schemas: tuple[str, ...]
    tools: tuple[str, ...]
    gaps: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityDashboardGroupReport":
        raw = _route_mapping("capability dashboard group", value)
        surfaces_raw = _route_mapping("capability dashboard surfaces", raw.get("surfaces"))
        surfaces = {name: _route_count(f"dashboard surfaces.{name}", surfaces_raw.get(name)) for name in ("crates", "mcp_tools", "cli_entrypoints", "python_artifacts")}
        tool_count = _route_count("dashboard group tool_count", raw.get("tool_count"))
        callable_count = _route_count("dashboard group callable_tool_count", raw.get("callable_tool_count"))
        schema_count = _route_count("dashboard group schema_backed_tool_count", raw.get("schema_backed_tool_count"))
        if callable_count > tool_count or schema_count > callable_count:
            raise ArgumentError("dashboard group tool counts do not reconcile")
        return cls(
            raw=raw,
            id=_route_text("dashboard group id", raw.get("id")),
            domains=_route_strings("dashboard group domains", raw.get("domains", [])),
            status=_route_text("dashboard group status", raw.get("status")),
            readiness=_route_text("dashboard group readiness", raw.get("readiness")),
            surfaces=surfaces,
            tool_count=tool_count,
            callable_tool_count=callable_count,
            schema_backed_tool_count=schema_count,
            missing_transport_schemas=_route_strings("dashboard missing schemas", raw.get("missing_transport_schemas", [])),
            invalid_transport_schemas=_route_strings("dashboard invalid schemas", raw.get("invalid_transport_schemas", [])),
            tools=_route_strings("dashboard group tools", raw.get("tools", [])),
            gaps=_route_strings("dashboard group gaps", raw.get("gaps", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CapabilityDashboardReport:
    raw: dict[str, Any]
    schema: str
    catalog_digest: str
    dashboard_digest: str
    ready: bool
    query: dict[str, Any]
    total_group_count: int
    selected_group_count: int
    available_group_count: int
    callable_group_count: int
    partial_group_count: int
    declared_only_group_count: int
    selected_tool_memberships: int
    selected_unique_tools: int
    schema_backed_unique_tools: int
    readiness_counts: dict[str, int]
    gap_counts: dict[str, int]
    groups: tuple[CapabilityDashboardGroupReport, ...]
    warnings: tuple[str, ...]
    duplicate_schema_names: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityDashboardReport":
        raw = _tool_payload(value, "capability_dashboard")
        if raw.get("ok") is not True:
            raise ArgumentError("capability dashboard report is not successful")
        audit = _route_mapping("capability dashboard audit", raw.get("audit"))
        groups_raw = audit.get("groups", [])
        if not isinstance(groups_raw, list):
            raise ArgumentError("capability dashboard groups must be an array")
        groups = tuple(CapabilityDashboardGroupReport.from_wire(group) for group in groups_raw)
        selected = _route_count("dashboard selected_group_count", audit.get("selected_group_count"))
        if selected != len(groups):
            raise ArgumentError("dashboard selected group count does not reconcile")
        return cls(
            raw=raw,
            schema=_route_text("dashboard schema", raw.get("schema")),
            catalog_digest=_route_text("dashboard catalog_digest", raw.get("catalog_digest")),
            dashboard_digest=_route_text("dashboard dashboard_digest", raw.get("dashboard_digest")),
            ready=raw.get("capability_dashboard_ready") is True,
            query=_route_mapping("dashboard query", audit.get("query")),
            total_group_count=_route_count("dashboard total_group_count", audit.get("total_group_count")),
            selected_group_count=selected,
            available_group_count=_route_count("dashboard available_group_count", audit.get("available_group_count")),
            callable_group_count=_route_count("dashboard callable_group_count", audit.get("callable_group_count")),
            partial_group_count=_route_count("dashboard partial_group_count", audit.get("partial_group_count")),
            declared_only_group_count=_route_count("dashboard declared_only_group_count", audit.get("declared_only_group_count")),
            selected_tool_memberships=_route_count("dashboard selected_tool_memberships", audit.get("selected_tool_memberships")),
            selected_unique_tools=_route_count("dashboard selected_unique_tools", audit.get("selected_unique_tools")),
            schema_backed_unique_tools=_route_count("dashboard schema_backed_unique_tools", audit.get("schema_backed_unique_tools")),
            readiness_counts={key: _route_count(f"dashboard readiness_counts.{key}", value) for key, value in _route_mapping("dashboard readiness_counts", audit.get("readiness_counts")).items()},
            gap_counts={key: _route_count(f"dashboard gap_counts.{key}", value) for key, value in _route_mapping("dashboard gap_counts", audit.get("gap_counts")).items()},
            groups=groups,
            warnings=_route_strings("dashboard warnings", audit.get("warnings", [])),
            duplicate_schema_names=_route_strings("dashboard duplicate schema names", raw.get("duplicate_schema_names", [])),
        )

    @property
    def callable(self) -> tuple[CapabilityDashboardGroupReport, ...]:
        return tuple(group for group in self.groups if group.readiness == "callable")

    @property
    def gap_labels(self) -> tuple[str, ...]:
        return tuple(sorted(self.gap_counts))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def capability_dashboard_report(value: Mapping[str, Any]) -> CapabilityDashboardReport:
    """Parse a direct MCP projection or HTTP REST tool envelope."""

    return CapabilityDashboardReport.from_wire(value)


__all__ = [
    "CAPABILITY_DASHBOARD_SCHEMA",
    "MAX_DASHBOARD_GROUPS",
    "DEFAULT_DASHBOARD_GROUPS",
    "CapabilityDashboardQueryArgs",
    "CapabilityDashboardGroupReport",
    "CapabilityDashboardReport",
    "capability_dashboard_report",
]
