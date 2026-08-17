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


def _review_selection(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("route selection", value)
    for name in ("need_id", "tool", "domain", "capability", "objective"):
        _route_text(f"route selection.{name}", raw.get(name))
    if not isinstance(raw.get("arguments"), Mapping):
        raise ArgumentError("route selection.arguments must be an object")
    depends_on = raw.get("depends_on", [])
    if not isinstance(depends_on, Sequence) or isinstance(depends_on, (str, bytes)):
        raise ArgumentError("route selection.depends_on must be an array")
    for dependency in depends_on:
        _route_text("route selection dependency", dependency)
    required = raw.get("required", True)
    if not isinstance(required, bool):
        raise ArgumentError("route selection.required must be a boolean")
    bindings = raw.get("bindings", [])
    if not isinstance(bindings, Sequence) or isinstance(bindings, (str, bytes)):
        raise ArgumentError("route selection.bindings must be an array")
    for binding in bindings:
        _route_mapping("route selection binding", binding)
    return {
        "need_id": raw["need_id"],
        "tool": raw["tool"],
        "domain": raw["domain"],
        "capability": raw["capability"],
        "objective": raw["objective"],
        "arguments": dict(raw["arguments"]),
        "depends_on": list(depends_on),
        "required": required,
        "bindings": [dict(binding) for binding in bindings],
    }


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


@dataclass(frozen=True)
class CapabilityRouteReviewRequest:
    """Explicit caller selections for the route-to-mission handoff review."""

    route: Mapping[str, Any]
    selections: Sequence[Mapping[str, Any]]
    validate_schemas: bool = False

    def __post_init__(self) -> None:
        route = _route_mapping("review route", self.route)
        if route.get("workflow") != "capability_route":
            raise ArgumentError("review route.workflow must be capability_route")
        if not isinstance(self.selections, Sequence) or isinstance(self.selections, (str, bytes)):
            raise ArgumentError("review selections must be an array")
        if not 1 <= len(self.selections) <= 32:
            raise ArgumentError("review selections must contain between 1 and 32 choices")
        if not isinstance(self.validate_schemas, bool):
            raise ArgumentError("validate_schemas must be a boolean")
        normalized = tuple(_review_selection(value) for value in self.selections)
        object.__setattr__(self, "route", route)
        object.__setattr__(self, "selections", normalized)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "route": dict(self.route),
            "selections": [dict(value) for value in self.selections],
            "validate_schemas": self.validate_schemas,
        }


@dataclass(frozen=True)
class CapabilityRouteReviewReport:
    """Validated non-executing handoff diagnostics for a reviewed route."""

    raw: dict[str, Any]
    review_id: str
    route_id: str
    catalog_digest: str
    goal: str
    review_status: str
    handoff_status: str
    need_count: int
    selection_count: int
    missing_needs: tuple[str, ...]
    selected_tools: tuple[str, ...]
    selected_domains: tuple[str, ...]
    dependency_waves: tuple[tuple[str, ...], ...]
    findings: tuple[dict[str, Any], ...]
    route_coverage: dict[str, Any]
    schema_review: dict[str, Any]
    mission_draft: dict[str, Any] | None
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityRouteReviewReport":
        raw = _route_mapping("capability route review report", value)
        if raw.get("workflow") != "capability_route_review":
            raise ArgumentError("review.workflow must be capability_route_review")
        review_status = _route_text("review_status", raw.get("review_status"))
        if review_status not in {"ready", "blocked"}:
            raise ArgumentError("review_status must be ready or blocked")
        handoff_status = _route_text("handoff_status", raw.get("handoff_status"))
        if handoff_status not in {"mission_preflight_required", "requires_caller_correction"}:
            raise ArgumentError("unknown handoff_status")
        need_count = _route_count("need_count", raw.get("need_count"))
        selection_count = _route_count("selection_count", raw.get("selection_count"))
        missing_needs = _route_strings("missing_needs", raw.get("missing_needs", []))
        selected_tools = _route_strings("selected_tools", raw.get("selected_tools", []))
        selected_domains = _route_strings("selected_domains", raw.get("selected_domains", []))
        raw_waves = raw.get("dependency_waves")
        if not isinstance(raw_waves, Sequence) or isinstance(raw_waves, (str, bytes)):
            raise ArgumentError("dependency_waves must be an array")
        dependency_waves = tuple(
            _route_strings(f"dependency_waves[{index}]", wave)
            for index, wave in enumerate(raw_waves)
        )
        raw_findings = raw.get("findings")
        if not isinstance(raw_findings, Sequence) or isinstance(raw_findings, (str, bytes)):
            raise ArgumentError("findings must be an array")
        findings = tuple(_route_mapping("route finding", finding) for finding in raw_findings)
        mission_draft_value = raw.get("mission_draft")
        mission_draft = None if mission_draft_value is None else _route_mapping("mission_draft", mission_draft_value)
        if review_status == "ready":
            if findings or mission_draft is None or handoff_status != "mission_preflight_required":
                raise ArgumentError("ready route review must have no findings and a mission draft")
        elif handoff_status != "requires_caller_correction":
            raise ArgumentError("blocked route review requires caller correction")
        return cls(
            raw=raw,
            review_id=_route_text("review review_id", raw.get("review_id")),
            route_id=_route_text("review route_id", raw.get("route_id")),
            catalog_digest=_route_text("review catalog_digest", raw.get("catalog_digest")),
            goal=_route_text("review goal", raw.get("goal")),
            review_status=review_status,
            handoff_status=handoff_status,
            need_count=need_count,
            selection_count=selection_count,
            missing_needs=missing_needs,
            selected_tools=selected_tools,
            selected_domains=selected_domains,
            dependency_waves=dependency_waves,
            findings=findings,
            route_coverage=_route_mapping("review route_coverage", raw.get("route_coverage", {})),
            schema_review=_route_mapping("review schema_review", raw.get("schema_review", {})),
            mission_draft=mission_draft,
            execution=_route_text("review execution", raw.get("execution")),
        )

    @property
    def ready(self) -> bool:
        return self.review_status == "ready"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _tool_payload(value: Mapping[str, Any], workflow: str) -> dict[str, Any]:
    """Extract a JSON projection from either a decoded MCP payload or an HTTP REST envelope."""

    raw = _route_mapping("capability tool response", value)
    if raw.get("workflow") == workflow:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and structured.get("workflow") == workflow:
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(f"route response text is not JSON: {error}") from error
                        decoded_mapping = _route_mapping("decoded capability tool response", decoded)
                        if decoded_mapping.get("workflow") == workflow:
                            return decoded_mapping
    raise ArgumentError(f"response does not contain a {workflow} JSON projection")


def capability_route_report(value: Mapping[str, Any]) -> CapabilityRouteReport:
    """Parse either a direct route payload or an HTTP tool envelope into a typed report."""

    return CapabilityRouteReport.from_wire(_tool_payload(value, "capability_route"))


def capability_route_review_report(value: Mapping[str, Any]) -> CapabilityRouteReviewReport:
    """Parse either a direct review payload or an HTTP tool envelope into diagnostics."""

    return CapabilityRouteReviewReport.from_wire(_tool_payload(value, "capability_route_review"))


@dataclass(frozen=True)
class CapabilityGroupReport:
    """Validated cross-domain catalogue metadata for one ranked group."""

    raw: dict[str, Any]
    id: str
    domains: tuple[str, ...]
    crates: tuple[str, ...]
    mcp_tools: tuple[str, ...]
    cli_entrypoints: tuple[str, ...]
    python_artifacts: tuple[str, ...]
    status: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityGroupReport":
        raw = _route_mapping("capability group", value)
        return cls(
            raw=raw,
            id=_route_text("capability group id", raw.get("id")),
            domains=_route_strings("capability group domains", raw.get("domains", [])),
            crates=_route_strings("capability group crates", raw.get("crates", [])),
            mcp_tools=_route_strings("capability group mcp_tools", raw.get("mcp_tools", [])),
            cli_entrypoints=_route_strings(
                "capability group cli_entrypoints", raw.get("cli_entrypoints", [])
            ),
            python_artifacts=_route_strings(
                "capability group python_artifacts", raw.get("python_artifacts", [])
            ),
            status=_route_text("capability group status", raw.get("status")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CapabilityMatchReport:
    """One deterministic ranked match with its complete cross-domain context."""

    raw: dict[str, Any]
    group: CapabilityGroupReport
    score: int
    matched_fields: tuple[str, ...]
    matched_tools: tuple[str, ...]
    tool_schemas: tuple[dict[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityMatchReport":
        raw = _route_mapping("capability match", value)
        group = CapabilityGroupReport.from_wire(raw.get("group"))
        matched_tools = _route_strings("capability matched_tools", raw.get("matched_tools", []))
        if not set(matched_tools).issubset(set(group.mcp_tools)):
            raise ArgumentError("capability matched_tools must belong to the matched group")
        raw_schemas = raw.get("tool_schemas", [])
        if not isinstance(raw_schemas, Sequence) or isinstance(raw_schemas, (str, bytes)):
            raise ArgumentError("capability tool_schemas must be an array")
        tool_schemas = tuple(_route_mapping("capability tool schema", schema) for schema in raw_schemas)
        return cls(
            raw=raw,
            group=group,
            score=_route_count("capability match score", raw.get("score")),
            matched_fields=_route_strings(
                "capability matched_fields", raw.get("matched_fields", [])
            ),
            matched_tools=matched_tools,
            tool_schemas=tool_schemas,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CapabilitySearchReport:
    """Validated digest-bound search projection over every advertised domain group."""

    raw: dict[str, Any]
    schema_version: str
    catalog_digest: str
    total_groups: int
    query: dict[str, Any]
    result_count: int
    matches: tuple[CapabilityMatchReport, ...]
    schema_attachment: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilitySearchReport":
        raw = _route_mapping("capability discovery report", value)
        if raw.get("ok") is False:
            raise ArgumentError("capability discovery report is not successful")
        if raw.get("workflow") != "capability_discover":
            raise ArgumentError("capability discovery workflow is invalid")
        raw_matches = raw.get("matches")
        if not isinstance(raw_matches, Sequence) or isinstance(raw_matches, (str, bytes)):
            raise ArgumentError("capability discovery matches must be an array")
        matches = tuple(CapabilityMatchReport.from_wire(match) for match in raw_matches)
        total_groups = _route_count("capability total_groups", raw.get("total_groups"))
        result_count = _route_count("capability result_count", raw.get("result_count"))
        if result_count != len(matches) or result_count > total_groups:
            raise ArgumentError("capability discovery result counts do not reconcile")
        group_ids = tuple(match.group.id for match in matches)
        if len(group_ids) != len(set(group_ids)):
            raise ArgumentError("capability discovery matches must contain unique groups")
        return cls(
            raw=raw,
            schema_version=_route_text("capability schema_version", raw.get("schema_version")),
            catalog_digest=_route_text("capability catalog_digest", raw.get("catalog_digest")),
            total_groups=total_groups,
            query=_route_mapping("capability discovery query", raw.get("query", {})),
            result_count=result_count,
            matches=matches,
            schema_attachment=_route_mapping(
                "capability discovery schema_attachment", raw.get("schema_attachment", {})
            ),
        )

    @property
    def domains(self) -> tuple[str, ...]:
        return tuple(sorted({domain for match in self.matches for domain in match.group.domains}))

    @property
    def tools(self) -> tuple[str, ...]:
        return tuple(
            sorted({tool for match in self.matches for tool in match.matched_tools})
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def capability_discover_report(value: Mapping[str, Any]) -> CapabilitySearchReport:
    """Parse a direct capability discovery result or an HTTP tool envelope."""

    return CapabilitySearchReport.from_wire(_tool_payload(value, "capability_discover"))


@dataclass(frozen=True)
class CapabilityAuditGroupReport:
    """Validated per-group coverage from the authoritative capability audit."""

    raw: dict[str, Any]
    id: str
    domains: tuple[str, ...]
    status: str
    declared_tool_memberships: int
    unique_tools: int
    schemas_found: int
    missing_schemas: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityAuditGroupReport":
        raw = _route_mapping("capability audit group", value)
        declared = _route_count(
            "capability audit declared_tool_memberships",
            raw.get("declared_tool_memberships"),
        )
        unique_tools = _route_count("capability audit unique_tools", raw.get("unique_tools"))
        schemas_found = _route_count("capability audit schemas_found", raw.get("schemas_found"))
        if unique_tools > declared or schemas_found > unique_tools:
            raise ArgumentError("capability audit group counts do not reconcile")
        missing_schemas = _route_strings(
            "capability audit missing_schemas", raw.get("missing_schemas", [])
        )
        if len(missing_schemas) != unique_tools - schemas_found:
            raise ArgumentError("capability audit missing schema count does not reconcile")
        return cls(
            raw=raw,
            id=_route_text("capability audit group id", raw.get("id")),
            domains=_route_strings("capability audit group domains", raw.get("domains", [])),
            status=_route_text("capability audit group status", raw.get("status")),
            declared_tool_memberships=declared,
            unique_tools=unique_tools,
            schemas_found=schemas_found,
            missing_schemas=missing_schemas,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CapabilitySchemaQualityReport:
    """Schema validation totals and bounded findings from a capability audit."""

    raw: dict[str, Any]
    checked: int
    valid: int
    total_bytes: int
    maximum_schema_bytes: int
    findings: tuple[dict[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilitySchemaQualityReport":
        raw = _route_mapping("capability schema quality", value)
        checked = _route_count("capability schema quality checked", raw.get("checked"))
        valid = _route_count("capability schema quality valid", raw.get("valid"))
        total_bytes = _route_count("capability schema quality total_bytes", raw.get("total_bytes"))
        maximum_schema_bytes = _route_count(
            "capability schema quality maximum_schema_bytes",
            raw.get("maximum_schema_bytes"),
        )
        if valid > checked:
            raise ArgumentError("capability schema quality valid count exceeds checked count")
        raw_findings = raw.get("findings", [])
        if not isinstance(raw_findings, Sequence) or isinstance(raw_findings, (str, bytes)):
            raise ArgumentError("capability schema quality findings must be an array")
        findings = tuple(
            _route_mapping("capability schema quality finding", finding)
            for finding in raw_findings
        )
        return cls(
            raw=raw,
            checked=checked,
            valid=valid,
            total_bytes=total_bytes,
            maximum_schema_bytes=maximum_schema_bytes,
            findings=findings,
        )

    @property
    def fully_valid(self) -> bool:
        return self.checked == self.valid and not self.findings

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CapabilityAuditReport:
    """Validated parity, schema quality, and per-group coverage diagnostics."""

    raw: dict[str, Any]
    capability_schema_version: str
    catalog_digest: str
    healthy: bool
    total_groups: int
    catalog_tool_memberships: int
    unique_catalog_tools: int
    advertised_tool_count: int
    catalog_only_tools: tuple[str, ...]
    advertised_only_tools: tuple[str, ...]
    duplicate_schema_names: tuple[str, ...]
    duplicate_group_memberships: tuple[dict[str, Any], ...]
    schema_quality: CapabilitySchemaQualityReport
    invariants: dict[str, Any]
    groups: tuple[CapabilityAuditGroupReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CapabilityAuditReport":
        raw = _route_mapping("capability audit report", value)
        if raw.get("ok") is False:
            raise ArgumentError("capability audit report is not successful")
        if raw.get("workflow") != "capability_audit":
            raise ArgumentError("capability audit workflow is invalid")
        raw_memberships = raw.get("duplicate_group_memberships", [])
        if not isinstance(raw_memberships, Sequence) or isinstance(raw_memberships, (str, bytes)):
            raise ArgumentError("capability audit duplicate_group_memberships must be an array")
        duplicate_group_memberships = tuple(
            _route_mapping("capability audit duplicate membership", membership)
            for membership in raw_memberships
        )
        raw_groups = raw.get("groups", [])
        if not isinstance(raw_groups, Sequence) or isinstance(raw_groups, (str, bytes)):
            raise ArgumentError("capability audit groups must be an array")
        groups = tuple(CapabilityAuditGroupReport.from_wire(group) for group in raw_groups)
        total_groups = _route_count("capability audit total_groups", raw.get("total_groups"))
        if groups and len(groups) != total_groups:
            raise ArgumentError("capability audit group count does not reconcile")
        return cls(
            raw=raw,
            capability_schema_version=_route_text(
                "capability audit schema version", raw.get("capability_schema_version")
            ),
            catalog_digest=_route_text("capability audit catalog_digest", raw.get("catalog_digest")),
            healthy=raw.get("healthy") is True,
            total_groups=total_groups,
            catalog_tool_memberships=_route_count(
                "capability audit catalog_tool_memberships", raw.get("catalog_tool_memberships")
            ),
            unique_catalog_tools=_route_count(
                "capability audit unique_catalog_tools", raw.get("unique_catalog_tools")
            ),
            advertised_tool_count=_route_count(
                "capability audit advertised_tool_count", raw.get("advertised_tool_count")
            ),
            catalog_only_tools=_route_strings(
                "capability audit catalog_only_tools", raw.get("catalog_only_tools", [])
            ),
            advertised_only_tools=_route_strings(
                "capability audit advertised_only_tools", raw.get("advertised_only_tools", [])
            ),
            duplicate_schema_names=_route_strings(
                "capability audit duplicate_schema_names", raw.get("duplicate_schema_names", [])
            ),
            duplicate_group_memberships=duplicate_group_memberships,
            schema_quality=CapabilitySchemaQualityReport.from_wire(raw.get("schema_quality", {})),
            invariants=_route_mapping("capability audit invariants", raw.get("invariants", {})),
            groups=groups,
        )

    @property
    def catalogue_complete(self) -> bool:
        return not self.catalog_only_tools and not self.advertised_only_tools

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def capability_audit_report(value: Mapping[str, Any]) -> CapabilityAuditReport:
    """Parse a direct capability audit result or an HTTP tool envelope."""

    return CapabilityAuditReport.from_wire(_tool_payload(value, "capability_audit"))


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


@dataclass(frozen=True)
class MissionEvaluatorQuery:
    """Bounded discovery filters for explicit, non-executing mission evaluators."""

    query: str | None = None
    group_id: str | None = None
    domain: str | None = None
    level: str | None = None
    adapter_id: str | None = None
    max_items: int = 32

    def __post_init__(self) -> None:
        for name, value in (
            ("query", self.query),
            ("group_id", self.group_id),
            ("domain", self.domain),
            ("level", self.level),
            ("adapter_id", self.adapter_id),
        ):
            _optional_text(name, value)
        if self.level is not None and self.level not in {"observation", "evaluation", "operational", "release"}:
            raise ArgumentError("level must be observation, evaluation, operational, or release")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= 256:
            raise ArgumentError("max_items must be between 1 and 256")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            key: value
            for key, value in {
                "query": self.query,
                "group_id": self.group_id,
                "domain": self.domain,
                "level": self.level,
                "adapter_id": self.adapter_id,
                "max_items": self.max_items,
            }.items()
            if value is not None
        }


def _evaluator_selection(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("mission evaluator selection", value)
    for name, maximum in (
        ("id", 128),
        ("claim_id", 128),
        ("adapter_id", 256),
        ("domain", 256),
        ("step_id", 128),
    ):
        text = raw.get(name)
        if not isinstance(text, str) or not text.strip() or len(text) > maximum:
            raise ArgumentError(f"mission evaluator selection.{name} must be a visible string of at most {maximum} characters")
    pointer = raw.get("output_pointer")
    if not isinstance(pointer, str) or "\x00" in pointer or "\n" in pointer or "\r" in pointer:
        raise ArgumentError("mission evaluator selection.output_pointer must be an RFC 6901 pointer string")
    required = raw.get("required", True)
    if not isinstance(required, bool):
        raise ArgumentError("mission evaluator selection.required must be a boolean")
    return {
        "id": raw["id"],
        "claim_id": raw["claim_id"],
        "adapter_id": raw["adapter_id"],
        "domain": raw["domain"],
        "step_id": raw["step_id"],
        "output_pointer": pointer,
        "required": required,
    }


@dataclass(frozen=True)
class MissionEvaluatorReviewRequest:
    """Explicit selections to review before adding evaluator bindings to a mission claim."""

    discovery: Mapping[str, Any]
    selections: Sequence[Mapping[str, Any]]

    def __post_init__(self) -> None:
        if not isinstance(self.discovery, Mapping):
            raise ArgumentError("mission evaluator discovery must be an object")
        if not isinstance(self.selections, Sequence) or isinstance(self.selections, (str, bytes)):
            raise ArgumentError("mission evaluator selections must be an array")
        if not 1 <= len(self.selections) <= 64:
            raise ArgumentError("mission evaluator selections must contain between 1 and 64 rows")
        normalized = tuple(_evaluator_selection(value) for value in self.selections)
        ids = [value["id"] for value in normalized]
        if len(ids) != len(set(ids)):
            raise ArgumentError("mission evaluator selection ids must be unique")
        object.__setattr__(self, "discovery", dict(self.discovery))
        object.__setattr__(self, "selections", normalized)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"discovery": dict(self.discovery), "selections": [dict(value) for value in self.selections]}


@dataclass(frozen=True)
class MissionEvaluatorBindingReport:
    """One candidate-to-claim binding row from the non-executing review."""

    id: str
    claim_id: str
    adapter_id: str
    domain: str
    step_id: str
    output_pointer: str
    required: bool
    candidate_found: bool
    domain_supported: bool
    binding_posture: str
    candidate_tools: tuple[str, ...]
    output_pointer_examples: tuple[str, ...]
    proposed_binding: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorBindingReport":
        raw = _route_mapping("mission evaluator binding review", value)
        candidate_found = raw.get("candidate_found")
        domain_supported = raw.get("domain_supported")
        required = raw.get("required")
        if not isinstance(candidate_found, bool) or not isinstance(domain_supported, bool) or not isinstance(required, bool):
            raise ArgumentError("mission evaluator binding review booleans are invalid")
        proposed = raw.get("proposed_binding")
        return cls(
            id=_route_text("review binding id", raw.get("id")),
            claim_id=_route_text("review binding claim_id", raw.get("claim_id")),
            adapter_id=_route_text("review binding adapter_id", raw.get("adapter_id")),
            domain=_route_text("review binding domain", raw.get("domain")),
            step_id=_route_text("review binding step_id", raw.get("step_id")),
            output_pointer=raw.get("output_pointer") if isinstance(raw.get("output_pointer"), str) else "",
            required=required,
            candidate_found=candidate_found,
            domain_supported=domain_supported,
            binding_posture=_route_text("review binding posture", raw.get("binding_posture")),
            candidate_tools=_route_strings("review candidate tools", raw.get("candidate_tools", [])),
            output_pointer_examples=_route_strings(
                "review output pointer examples", raw.get("output_pointer_examples", [])
            ),
            proposed_binding=dict(proposed) if isinstance(proposed, Mapping) else None,
        )


@dataclass(frozen=True)
class MissionEvaluatorReviewReport:
    """Typed, non-executing evaluator-to-claim binding review."""

    raw: dict[str, Any]
    review_id: str
    catalog_digest: str
    discovery_digest: str
    selection_count: int
    claim_count: int
    bindings: tuple[MissionEvaluatorBindingReport, ...]
    findings: tuple[dict[str, Any], ...]
    review_status: str
    binding_posture: str
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorReviewReport":
        raw = _tool_payload(value, "mission_evaluator_review")
        findings_value = raw.get("findings", [])
        if not isinstance(findings_value, Sequence) or isinstance(findings_value, (str, bytes)):
            raise ArgumentError("mission evaluator review findings must be an array")
        bindings_value = raw.get("bindings", [])
        if not isinstance(bindings_value, Sequence) or isinstance(bindings_value, (str, bytes)):
            raise ArgumentError("mission evaluator review bindings must be an array")
        review_status = _route_text("mission evaluator review status", raw.get("review_status"))
        if review_status not in {"ready", "blocked"}:
            raise ArgumentError(f"unknown mission evaluator review status: {review_status}")
        return cls(
            raw=raw,
            review_id=_route_text("mission evaluator review id", raw.get("review_id")),
            catalog_digest=_route_text("mission evaluator review catalog digest", raw.get("catalog_digest")),
            discovery_digest=_route_text("mission evaluator discovery digest", raw.get("discovery_digest")),
            selection_count=_route_count("mission evaluator selection count", raw.get("selection_count")),
            claim_count=_route_count("mission evaluator claim count", raw.get("claim_count")),
            bindings=tuple(MissionEvaluatorBindingReport.from_wire(item) for item in bindings_value),
            findings=tuple(_route_mapping("mission evaluator review finding", item) for item in findings_value),
            review_status=review_status,
            binding_posture=_route_text("mission evaluator binding posture", raw.get("binding_posture")),
            execution=_route_text("mission evaluator review execution", raw.get("execution")),
            guarantees=_route_strings("mission evaluator review guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("mission evaluator review limitations", raw.get("limitations", [])),
        )

    @property
    def ready(self) -> bool:
        return self.review_status == "ready" and not self.findings

    @property
    def proposed_bindings(self) -> tuple[dict[str, Any], ...]:
        return tuple(
            binding.proposed_binding
            for binding in self.bindings
            if binding.proposed_binding is not None
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_review_report(value: Mapping[str, Any]) -> MissionEvaluatorReviewReport:
    """Parse a direct MCP result or HTTP envelope from mission evaluator review."""

    return MissionEvaluatorReviewReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvaluatorAdapterReport:
    """One typed candidate row from mission evaluator discovery."""

    id: str
    group_id: str
    domains: tuple[str, ...]
    levels: tuple[str, ...]
    purpose: str
    candidate_tools: tuple[str, ...]
    output_pointer_examples: tuple[str, ...]
    status: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorAdapterReport":
        raw = _route_mapping("mission evaluator adapter", value)
        return cls(
            id=_route_text("evaluator adapter id", raw.get("id")),
            group_id=_route_text("evaluator group id", raw.get("group_id")),
            domains=_route_strings("evaluator domains", raw.get("domains", [])),
            levels=_route_strings("evaluator levels", raw.get("levels", [])),
            purpose=_route_text("evaluator purpose", raw.get("purpose")),
            candidate_tools=_route_strings("evaluator candidate tools", raw.get("candidate_tools", [])),
            output_pointer_examples=_route_strings(
                "evaluator output pointer examples", raw.get("output_pointer_examples", [])
            ),
            status=_route_text("evaluator status", raw.get("status")),
        )


@dataclass(frozen=True)
class MissionEvaluatorMatchReport:
    adapter: MissionEvaluatorAdapterReport
    score: int
    matched_fields: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorMatchReport":
        raw = _route_mapping("mission evaluator match", value)
        score = _route_count("evaluator score", raw.get("score"))
        return cls(
            adapter=MissionEvaluatorAdapterReport.from_wire(raw.get("adapter", {})),
            score=score,
            matched_fields=_route_strings("evaluator matched fields", raw.get("matched_fields", [])),
        )


@dataclass(frozen=True)
class MissionEvaluatorCoverageReport:
    """Coverage reconciliation between capability groups and evaluator candidates."""

    capability_group_count: int
    evaluator_group_count: int
    uncovered_groups: tuple[str, ...]
    unbound_groups: tuple[str, ...]
    complete: bool
    posture: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorCoverageReport":
        raw = _route_mapping("mission evaluator coverage", value)
        capability_group_count = _route_count(
            "evaluator coverage capability_group_count", raw.get("capability_group_count")
        )
        evaluator_group_count = _route_count(
            "evaluator coverage evaluator_group_count", raw.get("evaluator_group_count")
        )
        uncovered_groups = _route_strings("evaluator coverage uncovered_groups", raw.get("uncovered_groups", []))
        unbound_groups = _route_strings("evaluator coverage unbound_groups", raw.get("unbound_groups", []))
        complete = raw.get("complete")
        if not isinstance(complete, bool):
            raise ArgumentError("evaluator coverage complete must be a boolean")
        if complete != (not uncovered_groups and not unbound_groups):
            raise ArgumentError("evaluator coverage complete does not reconcile with group gaps")
        return cls(
            capability_group_count=capability_group_count,
            evaluator_group_count=evaluator_group_count,
            uncovered_groups=uncovered_groups,
            unbound_groups=unbound_groups,
            complete=complete,
            posture=_route_text("evaluator coverage posture", raw.get("posture")),
        )


@dataclass(frozen=True)
class MissionEvaluatorSearchReport:
    """Validated typed view over cross-domain evaluator candidate discovery."""

    raw: dict[str, Any]
    catalog_digest: str
    total_adapters: int
    query: dict[str, Any]
    matches: tuple[MissionEvaluatorMatchReport, ...]
    coverage: MissionEvaluatorCoverageReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorSearchReport":
        raw = _route_mapping("mission evaluator search", value)
        if raw.get("ok") is False:
            raise ArgumentError("mission evaluator search is not successful")
        if raw.get("workflow") != "mission_evaluator_discover":
            raise ArgumentError("mission evaluator workflow must be mission_evaluator_discover")
        if raw.get("selection_posture") != "candidate_only":
            raise ArgumentError("mission evaluator selection posture must be candidate_only")
        matches_value = raw.get("matches", [])
        if not isinstance(matches_value, Sequence) or isinstance(matches_value, (str, bytes)):
            raise ArgumentError("mission evaluator matches must be an array")
        return cls(
            raw=raw,
            catalog_digest=_route_text("mission evaluator catalog digest", raw.get("catalog_digest")),
            total_adapters=_route_count("mission evaluator total adapters", raw.get("total_adapters")),
            query=_route_mapping("mission evaluator query", raw.get("query", {})),
            matches=tuple(MissionEvaluatorMatchReport.from_wire(item) for item in matches_value),
            coverage=MissionEvaluatorCoverageReport.from_wire(raw.get("coverage", {})),
            guarantees=_route_strings("mission evaluator guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("mission evaluator limitations", raw.get("limitations", [])),
        )

    @property
    def adapters(self) -> tuple[MissionEvaluatorAdapterReport, ...]:
        return tuple(match.adapter for match in self.matches)

    @property
    def all_candidates_only(self) -> bool:
        return bool(self.matches) and all(adapter.status == "candidate_only" for adapter in self.adapters)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_discover_report(value: Mapping[str, Any]) -> MissionEvaluatorSearchReport:
    """Parse a mission evaluator discovery response into a typed report."""

    return MissionEvaluatorSearchReport.from_wire(_tool_payload(value, "mission_evaluator_discover"))


__all__ = [
    "CapabilityQuery",
    "CapabilityGroupReport",
    "CapabilityMatchReport",
    "CapabilitySearchReport",
    "CapabilityAuditGroupReport",
    "CapabilitySchemaQualityReport",
    "CapabilityAuditReport",
    "CapabilityRouteNeed",
    "CapabilityRouteRequest",
    "CapabilityRouteNeedReport",
    "CapabilityRouteCoverage",
    "CapabilityRouteReport",
    "CapabilityRouteReviewRequest",
    "CapabilityRouteReviewReport",
    "MissionEvaluatorQuery",
    "MissionEvaluatorReviewRequest",
    "MissionEvaluatorBindingReport",
    "MissionEvaluatorReviewReport",
    "MissionEvaluatorAdapterReport",
    "MissionEvaluatorMatchReport",
    "MissionEvaluatorCoverageReport",
    "MissionEvaluatorSearchReport",
    "capability_route_report",
    "capability_route_review_report",
    "capability_discover_report",
    "capability_audit_report",
    "mission_evaluator_discover_report",
    "mission_evaluator_review_report",
]
