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
class DomainWorkflowInstantiateRequest:
    """Caller-owned selection for one capability-group workflow template."""

    workflow_id: str
    mission_id: str
    goal: str
    steps: Sequence[Mapping[str, Any]]
    policy: Mapping[str, Any] | None = None
    claim_requests: Sequence[Mapping[str, Any]] = ()
    evaluator_review: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        for name, value in (("workflow_id", self.workflow_id), ("mission_id", self.mission_id), ("goal", self.goal)):
            _route_text(f"workflow {name}", value)
        if not isinstance(self.steps, Sequence) or isinstance(self.steps, (str, bytes)) or not 1 <= len(self.steps) <= 128:
            raise ArgumentError("workflow steps must contain between 1 and 128 objects")
        for index, step in enumerate(self.steps):
            _route_mapping(f"workflow steps[{index}]", step)
        if self.policy is not None:
            _route_mapping("workflow policy", self.policy)
        if not isinstance(self.claim_requests, Sequence) or isinstance(self.claim_requests, (str, bytes)) or len(self.claim_requests) > 64:
            raise ArgumentError("workflow claim_requests must contain at most 64 objects")
        for index, claim in enumerate(self.claim_requests):
            _route_mapping(f"workflow claim_requests[{index}]", claim)
        if self.evaluator_review is not None:
            _route_mapping("workflow evaluator_review", self.evaluator_review)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "workflow_id": self.workflow_id,
            "mission_id": self.mission_id,
            "goal": self.goal,
            "steps": [dict(step) for step in self.steps],
            "claim_requests": [dict(claim) for claim in self.claim_requests],
        }
        if self.policy is not None:
            result["policy"] = dict(self.policy)
        if self.evaluator_review is not None:
            result["evaluator_review"] = dict(self.evaluator_review)
        return result


@dataclass(frozen=True)
class DomainWorkflowReconcileRequest:
    """Retained execution evidence to reconcile against one workflow instantiation."""

    instantiation: Mapping[str, Any]
    mission_report: Mapping[str, Any] | None = None
    evidence_bundle: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _route_mapping("workflow instantiation", self.instantiation)
        if self.mission_report is None and self.evidence_bundle is None:
            raise ArgumentError("workflow reconciliation requires mission_report or evidence_bundle")
        if self.mission_report is not None:
            _route_mapping("workflow mission_report", self.mission_report)
        if self.evidence_bundle is not None:
            _route_mapping("workflow evidence_bundle", self.evidence_bundle)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"instantiation": dict(self.instantiation)}
        if self.mission_report is not None:
            result["mission_report"] = dict(self.mission_report)
        if self.evidence_bundle is not None:
            result["evidence_bundle"] = dict(self.evidence_bundle)
        return result


@dataclass(frozen=True)
class DomainWorkflowCatalogueReport:
    """Typed deterministic catalogue of all capability-group workflow templates."""

    raw: dict[str, Any]
    catalog_digest: str
    workflow_catalog_digest: str
    workflow_count: int
    workflows: tuple[Mapping[str, Any], ...]
    coverage: Mapping[str, Any]
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowCatalogueReport":
        raw = _tool_payload(value, "domain_workflow_catalogue")
        workflows = raw.get("workflows", [])
        if not isinstance(workflows, Sequence) or isinstance(workflows, (str, bytes)):
            raise ArgumentError("domain workflow catalogue workflows must be an array")
        count = _route_count("domain workflow count", raw.get("workflow_count"))
        if count != len(workflows):
            raise ArgumentError("domain workflow count does not match workflows")
        return cls(
            raw=raw,
            catalog_digest=_route_text("domain workflow catalog digest", raw.get("catalog_digest")),
            workflow_catalog_digest=_route_text("domain workflow catalogue digest", raw.get("workflow_catalog_digest")),
            workflow_count=count,
            workflows=tuple(_route_mapping("domain workflow", item) for item in workflows),
            coverage=_route_mapping("domain workflow coverage", raw.get("coverage", {})),
            execution=_route_text("domain workflow catalogue execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainWorkflowInstantiationReport:
    """Typed group-scoped mission, domain contract, evidence plan, and preflight projection."""

    raw: dict[str, Any]
    workflow_id: str
    workflow_digest: str
    catalog_digest: str
    mission: Mapping[str, Any]
    selection: Mapping[str, Any]
    domain_contract: Mapping[str, Any]
    domain_contract_digest: str
    evidence_plan: Mapping[str, Any]
    preflight: Mapping[str, Any]
    preflight_report: Mapping[str, Any] | None
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowInstantiationReport":
        raw = _tool_payload(value, "domain_workflow_instantiate")
        preflight_report = raw.get("preflight_report")
        if preflight_report is not None:
            preflight_report = _route_mapping("domain workflow preflight report", preflight_report)
        return cls(
            raw=raw,
            workflow_id=_route_text("domain workflow id", raw.get("workflow_id")),
            workflow_digest=_route_text("domain workflow digest", raw.get("workflow_digest")),
            catalog_digest=_route_text("domain workflow catalog digest", raw.get("catalog_digest")),
            mission=_route_mapping("domain workflow mission", raw.get("mission")),
            selection=_route_mapping("domain workflow selection", raw.get("selection")),
            domain_contract=_route_mapping("domain workflow domain contract", raw.get("domain_contract", {})),
            domain_contract_digest=_route_text(
                "domain workflow domain contract digest",
                raw.get("domain_contract_digest", raw.get("workflow_digest")),
            ),
            evidence_plan=_route_mapping("domain workflow evidence plan", raw.get("evidence_plan", {})),
            preflight=_route_mapping("domain workflow preflight", raw.get("preflight")),
            preflight_report=preflight_report,
            execution=_route_text("domain workflow execution", raw.get("execution")),
        )

    @property
    def selected_tools(self) -> tuple[str, ...]:
        return _route_strings("selected workflow tools", self.selection.get("selected_tools", []))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_workflow_catalogue_report(value: Mapping[str, Any]) -> DomainWorkflowCatalogueReport:
    """Parse a direct REST/MCP workflow catalogue result."""

    return DomainWorkflowCatalogueReport.from_wire(value)


def domain_workflow_instantiation_report(value: Mapping[str, Any]) -> DomainWorkflowInstantiationReport:
    """Parse a direct REST/MCP workflow instantiation result."""

    return DomainWorkflowInstantiationReport.from_wire(value)


@dataclass(frozen=True)
class DomainWorkflowReconciliationReport:
    """Typed structural completion/evidence reconciliation for one domain workflow."""

    raw: dict[str, Any]
    workflow_id: str
    workflow_digest: str
    catalog_digest: str
    mission_id: str
    mission_plan_digest: str
    reconciliation_digest: str
    source: str
    report: Mapping[str, Any]
    evidence: Mapping[str, Any]
    completion: Mapping[str, Any]
    integrity: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationReport":
        raw = _tool_payload(value, "domain_workflow_reconcile")
        return cls(
            raw=raw,
            workflow_id=_route_text("workflow reconciliation id", raw.get("workflow_id")),
            workflow_digest=_route_text("workflow reconciliation workflow digest", raw.get("workflow_digest")),
            catalog_digest=_route_text("workflow reconciliation catalog digest", raw.get("catalog_digest")),
            mission_id=_route_text("workflow reconciliation mission id", raw.get("mission_id")),
            mission_plan_digest=_route_text("workflow reconciliation mission plan digest", raw.get("mission_plan_digest")),
            reconciliation_digest=_route_text("workflow reconciliation digest", raw.get("reconciliation_digest")),
            source=_route_text("workflow reconciliation source", raw.get("source")),
            report=_route_mapping("workflow reconciliation report", raw.get("report", {})),
            evidence=_route_mapping("workflow reconciliation evidence", raw.get("evidence", {})),
            completion=_route_mapping("workflow reconciliation completion", raw.get("completion", {})),
            integrity=_route_mapping("workflow reconciliation integrity", raw.get("integrity", {})),
        )

    @property
    def ready(self) -> bool:
        return self.completion.get("ready") is True

    @property
    def completion_status(self) -> str:
        return _route_text("workflow reconciliation completion status", self.completion.get("status"))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_workflow_reconciliation_report(value: Mapping[str, Any]) -> DomainWorkflowReconciliationReport:
    """Parse a direct REST/MCP workflow reconciliation result."""

    return DomainWorkflowReconciliationReport.from_wire(value)


@dataclass(frozen=True)
class DomainWorkflowReconciliationImportRequest:
    """Import one canonical domain workflow reconciliation report into a registry."""

    record: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "record", dict(_route_mapping("workflow reconciliation record", self.record)))

    def to_arguments(self) -> dict[str, Any]:
        return {"record": dict(self.record)}

    def to_http_body(self) -> dict[str, Any]:
        return self.to_arguments()


@dataclass(frozen=True)
class DomainWorkflowReconciliationQueryRequest:
    """Bounded registry query for retained workflow reconciliation posture."""

    mission_id: str | None = None
    workflow_id: str | None = None
    mission_plan_digest: str | None = None
    completion_status: str | None = None
    after: str | None = None
    max_items: int = 100
    include_records: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("mission_id", self.mission_id),
            ("workflow_id", self.workflow_id),
            ("mission_plan_digest", self.mission_plan_digest),
            ("completion_status", self.completion_status),
            ("after", self.after),
        ):
            if value is not None:
                _route_text(f"workflow reconciliation query {name}", value)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("workflow reconciliation query max_items must be between 1 and 256")
        if not isinstance(self.include_records, bool):
            raise ArgumentError("workflow reconciliation query include_records must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params: dict[str, str] = {
            "limit": str(self.max_items),
            "include_records": "true" if self.include_records else "false",
        }
        for name, value in (
            ("mission_id", self.mission_id),
            ("workflow_id", self.workflow_id),
            ("mission_plan_digest", self.mission_plan_digest),
            ("completion_status", self.completion_status),
            ("after", self.after),
        ):
            if value is not None:
                params[name] = value
        return params

    def to_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "max_items": self.max_items,
            "include_records": self.include_records,
        }
        for name, value in (
            ("mission_id", self.mission_id),
            ("workflow_id", self.workflow_id),
            ("mission_plan_digest", self.mission_plan_digest),
            ("completion_status", self.completion_status),
            ("after", self.after),
        ):
            if value is not None:
                arguments[name] = value
        return arguments


@dataclass(frozen=True)
class DomainWorkflowReconciliationImportReport:
    """Typed idempotent workflow reconciliation registry import result."""

    raw: dict[str, Any]
    reconciliation_digest: str
    created: bool
    already_present: bool
    registry_generation: int
    registry_size: int
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationImportReport":
        raw = _tool_payload(value, "domain_workflow_reconciliation_import")
        if raw.get("workflow") != "domain_workflow_reconciliation_import":
            raise ArgumentError("workflow reconciliation import workflow is invalid")
        created = raw.get("created")
        already_present = raw.get("already_present")
        if not isinstance(created, bool) or not isinstance(already_present, bool):
            raise ArgumentError("workflow reconciliation import flags must be booleans")
        return cls(
            raw=raw,
            reconciliation_digest=_route_text("workflow reconciliation import digest", raw.get("reconciliation_digest")),
            created=created,
            already_present=already_present,
            registry_generation=_route_count("workflow reconciliation import generation", raw.get("registry_generation")),
            registry_size=_route_count("workflow reconciliation import size", raw.get("registry_size")),
            execution=_route_text("workflow reconciliation import execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainWorkflowReconciliationQueryReport:
    """Typed deterministic workflow reconciliation registry index page."""

    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationQueryReport":
        raw = _tool_payload(value, "domain_workflow_reconciliation_query")
        if raw.get("workflow") != "domain_workflow_reconciliation_query":
            raise ArgumentError("workflow reconciliation query workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("workflow reconciliation query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _route_text("workflow reconciliation query next cursor", next_after)
        has_more = raw.get("has_more")
        if not isinstance(has_more, bool):
            raise ArgumentError("workflow reconciliation query has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_route_mapping("workflow reconciliation query row", row) for row in rows),
            next_after=next_after,
            has_more=has_more,
            registry_generation=_route_count("workflow reconciliation query generation", raw.get("registry_generation")),
            registry_size=_route_count("workflow reconciliation query size", raw.get("registry_size")),
            execution=_route_text("workflow reconciliation query execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainWorkflowReconciliationSummaryReport:
    """Typed compact operator posture derived from retained reconciliation reports."""

    raw: dict[str, Any]
    registry_generation: int
    registry_size: int
    completion_status_counts: Mapping[str, int]
    ready_count: int
    review_required_count: int
    integrity_invalid_count: int
    evidence_invalid_count: int
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationSummaryReport":
        raw = _route_mapping("workflow reconciliation summary", value)
        if raw.get("workflow") != "domain_workflow_reconciliation_summary":
            raise ArgumentError("workflow reconciliation summary workflow is invalid")
        counts_raw = _route_mapping(
            "workflow reconciliation summary completion_status_counts",
            raw.get("completion_status_counts"),
        )
        counts = {
            _route_text("workflow reconciliation summary status", status): _route_count(
                f"workflow reconciliation summary status count {status}", count
            )
            for status, count in counts_raw.items()
        }
        readiness_claimed = raw.get("readiness_claimed")
        if readiness_claimed is not False:
            raise ArgumentError("workflow reconciliation summary readiness_claimed must be false")
        return cls(
            raw=raw,
            registry_generation=_route_count("workflow reconciliation summary generation", raw.get("registry_generation")),
            registry_size=_route_count("workflow reconciliation summary size", raw.get("registry_size")),
            completion_status_counts=counts,
            ready_count=_route_count("workflow reconciliation summary ready count", raw.get("ready_count")),
            review_required_count=_route_count("workflow reconciliation summary review count", raw.get("review_required_count")),
            integrity_invalid_count=_route_count("workflow reconciliation summary integrity count", raw.get("integrity_invalid_count")),
            evidence_invalid_count=_route_count("workflow reconciliation summary evidence count", raw.get("evidence_invalid_count")),
            readiness_claimed=readiness_claimed,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainWorkflowReconciliationPersistenceStatus:
    """Typed restart/checkpoint posture for the reconciliation registry."""

    raw: dict[str, Any]
    enabled: bool
    file_present: bool
    file_bytes: int | None
    schema: str
    state_digest: str | None
    integrity_verified: bool | None
    registry_size: int
    registry_generation: int
    max_reconciliations: int
    max_file_bytes: int
    recovery_policy: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationPersistenceStatus":
        raw = _route_mapping("workflow reconciliation persistence", value)
        enabled = raw.get("enabled")
        file_present = raw.get("file_present")
        if not isinstance(enabled, bool) or not isinstance(file_present, bool):
            raise ArgumentError("workflow reconciliation persistence enabled and file_present must be booleans")
        file_bytes = raw.get("file_bytes")
        if file_bytes is not None:
            file_bytes = _route_count("workflow reconciliation persistence file_bytes", file_bytes)
        state_digest = raw.get("state_digest")
        if state_digest is not None:
            state_digest = _route_text("workflow reconciliation persistence state_digest", state_digest)
            if len(state_digest) != 64 or any(character not in "0123456789abcdef" for character in state_digest):
                raise ArgumentError("workflow reconciliation persistence state_digest must be a lowercase SHA-256 digest")
        integrity_verified = raw.get("integrity_verified")
        if integrity_verified is not None and not isinstance(integrity_verified, bool):
            raise ArgumentError("workflow reconciliation persistence integrity_verified must be boolean or null")
        return cls(
            raw=raw,
            enabled=enabled,
            file_present=file_present,
            file_bytes=file_bytes,
            schema=_route_text("workflow reconciliation persistence schema", raw.get("schema")),
            state_digest=state_digest,
            integrity_verified=integrity_verified,
            registry_size=_route_count("workflow reconciliation persistence size", raw.get("registry_size")),
            registry_generation=_route_count("workflow reconciliation persistence generation", raw.get("registry_generation")),
            max_reconciliations=_route_count("workflow reconciliation persistence max_reconciliations", raw.get("max_reconciliations")),
            max_file_bytes=_route_count("workflow reconciliation persistence max_file_bytes", raw.get("max_file_bytes")),
            recovery_policy=_route_text("workflow reconciliation persistence recovery_policy", raw.get("recovery_policy")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainWorkflowReconciliationGetRequest:
    """Fetch one workflow reconciliation record by its content hash."""

    reconciliation_digest: str

    def __post_init__(self) -> None:
        _route_text("workflow reconciliation digest", self.reconciliation_digest)

    def to_arguments(self) -> dict[str, Any]:
        return {"reconciliation_digest": self.reconciliation_digest}


@dataclass(frozen=True)
class DomainWorkflowReconciliationGetReport:
    """Typed lookup result for one stored workflow reconciliation report."""

    raw: dict[str, Any]
    reconciliation_digest: str
    record: Mapping[str, Any]
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainWorkflowReconciliationGetReport":
        raw = _tool_payload(value, "domain_workflow_reconciliation_get")
        if raw.get("workflow") != "domain_workflow_reconciliation_get":
            raise ArgumentError("workflow reconciliation get workflow is invalid")
        return cls(
            raw=raw,
            reconciliation_digest=_route_text("workflow reconciliation get digest", raw.get("reconciliation_digest")),
            record=_route_mapping("workflow reconciliation get record", raw.get("record")),
            execution=_route_text("workflow reconciliation get execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


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
    catalogue_snapshot: Mapping[str, Any]
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
            catalogue_snapshot=_route_mapping(
                "mission evaluator catalogue snapshot", raw.get("catalogue_snapshot", {})
            ),
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

    @property
    def snapshot_retained(self) -> bool:
        retention = self.catalogue_snapshot.get("retention")
        return isinstance(retention, Mapping) and retention.get("rows_retained") is True

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_review_report(value: Mapping[str, Any]) -> MissionEvaluatorReviewReport:
    """Parse a direct MCP result or HTTP envelope from mission evaluator review."""

    return MissionEvaluatorReviewReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvaluatorReplayRequest:
    """Request a structural replay of one retained agent mission report."""

    mission: Mapping[str, Any]
    include_fixtures: bool = True
    max_items: int = 128

    def __post_init__(self) -> None:
        object.__setattr__(self, "mission", dict(_route_mapping("mission evaluator replay mission", self.mission)))
        if not isinstance(self.include_fixtures, bool):
            raise ArgumentError("mission evaluator replay include_fixtures must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 512:
            raise ArgumentError("mission evaluator replay max_items must be between 1 and 512")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "mission": dict(self.mission),
            "include_fixtures": self.include_fixtures,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class MissionEvaluatorReplayReport:
    """Typed structural replay, coverage, fixture, and refusal evidence for a mission report."""

    raw: dict[str, Any]
    mission_id: str
    mission_digest: str
    catalog_digest: str
    binding_count: int
    omitted_bindings: int
    state_counts: Mapping[str, int]
    claims: tuple[dict[str, Any], ...]
    bindings: tuple[dict[str, Any], ...]
    coverage: Mapping[str, Any]
    findings: tuple[dict[str, Any], ...]
    replay_status: str
    execution: str
    fixtures: tuple[dict[str, Any], ...]
    omitted_fixtures: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorReplayReport":
        raw = _tool_payload(value, "mission_evaluator_replay")
        if raw.get("workflow") != "mission_evaluator_replay":
            raise ArgumentError("mission evaluator replay workflow is invalid")
        claims = raw.get("claims", [])
        bindings = raw.get("bindings", [])
        fixtures = raw.get("fixtures", [])
        findings = raw.get("findings", [])
        for name, candidate in (("claims", claims), ("bindings", bindings), ("fixtures", fixtures), ("findings", findings)):
            if not isinstance(candidate, Sequence) or isinstance(candidate, (str, bytes)):
                raise ArgumentError(f"mission evaluator replay {name} must be an array")
        status = _route_text("mission evaluator replay status", raw.get("replay_status"))
        if status not in {"ready", "blocked"}:
            raise ArgumentError(f"unknown mission evaluator replay status: {status}")
        state_counts = raw.get("state_counts", {})
        coverage = raw.get("coverage", {})
        if not isinstance(state_counts, Mapping) or not isinstance(coverage, Mapping):
            raise ArgumentError("mission evaluator replay state_counts and coverage must be objects")
        return cls(
            raw=raw,
            mission_id=_route_text("mission evaluator replay mission id", raw.get("mission_id")),
            mission_digest=_route_text("mission evaluator replay mission digest", raw.get("mission_digest")),
            catalog_digest=_route_text("mission evaluator replay catalog digest", raw.get("catalog_digest")),
            binding_count=_route_count("mission evaluator replay binding count", raw.get("binding_count")),
            omitted_bindings=_route_count("mission evaluator replay omitted bindings", raw.get("omitted_bindings", 0)),
            state_counts={str(key): _route_count(f"mission evaluator replay state {key}", value) for key, value in state_counts.items()},
            claims=tuple(_route_mapping("mission evaluator replay claim", item) for item in claims),
            bindings=tuple(_route_mapping("mission evaluator replay binding", item) for item in bindings),
            coverage=dict(coverage),
            findings=tuple(_route_mapping("mission evaluator replay finding", item) for item in findings),
            replay_status=status,
            execution=_route_text("mission evaluator replay execution", raw.get("execution")),
            fixtures=tuple(_route_mapping("mission evaluator replay fixture", item) for item in fixtures),
            omitted_fixtures=_route_count("mission evaluator replay omitted fixtures", raw.get("omitted_fixtures", 0)),
        )

    @property
    def ready(self) -> bool:
        return self.replay_status == "ready" and not self.findings

    @property
    def catalogue_complete(self) -> bool:
        return self.coverage.get("complete") is True

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_replay_report(value: Mapping[str, Any]) -> MissionEvaluatorReplayReport:
    """Parse a direct MCP result or HTTP envelope from mission evaluator replay."""

    return MissionEvaluatorReplayReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvaluatorReplayCompareRequest:
    """Request a non-executing current-catalogue comparison for one mission report."""

    mission: Mapping[str, Any]
    include_fixtures: bool = True
    max_items: int = 128

    def __post_init__(self) -> None:
        object.__setattr__(self, "mission", dict(_route_mapping("mission evaluator replay comparison mission", self.mission)))
        if not isinstance(self.include_fixtures, bool):
            raise ArgumentError("mission evaluator replay comparison include_fixtures must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 512:
            raise ArgumentError("mission evaluator replay comparison max_items must be between 1 and 512")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "mission": dict(self.mission),
            "include_fixtures": self.include_fixtures,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class MissionEvaluatorReplayComparisonReport:
    """Typed digest-drift and current-binding compatibility evidence."""

    raw: dict[str, Any]
    mission_id: str
    replay: Mapping[str, Any]
    catalog_drift: Mapping[str, Any]
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorReplayComparisonReport":
        raw = _tool_payload(value, "mission_evaluator_replay_compare")
        if raw.get("workflow") != "mission_evaluator_replay_compare":
            raise ArgumentError("mission evaluator replay comparison workflow is invalid")
        return cls(
            raw=raw,
            mission_id=_route_text("mission evaluator replay comparison mission id", raw.get("mission_id")),
            replay=_route_mapping("mission evaluator replay comparison replay", raw.get("replay")),
            catalog_drift=_route_mapping("mission evaluator replay catalog drift", raw.get("catalog_drift")),
            execution=_route_text("mission evaluator replay comparison execution", raw.get("execution")),
            guarantees=_route_strings("mission evaluator replay comparison guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("mission evaluator replay comparison limitations", raw.get("limitations", [])),
        )

    @property
    def status(self) -> str:
        return str(self.catalog_drift.get("status", "not_recorded"))

    @property
    def drifted(self) -> bool:
        return self.status in {"drifted", "drifted_with_missing_bindings", "invalid_recorded_digest"}

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_replay_comparison_report(value: Mapping[str, Any]) -> MissionEvaluatorReplayComparisonReport:
    """Parse a direct MCP result or HTTP envelope from replay comparison."""

    return MissionEvaluatorReplayComparisonReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvaluatorReplayQueryRequest:
    """Bounded REST query for durable full or summary-only evaluator replay evidence."""

    mission_id: str
    include_fixtures: bool = False
    max_items: int = 128

    def __post_init__(self) -> None:
        _route_text("mission evaluator replay query mission id", self.mission_id)
        if not isinstance(self.include_fixtures, bool):
            raise ArgumentError("mission evaluator replay query include_fixtures must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 512:
            raise ArgumentError("mission evaluator replay query max_items must be between 1 and 512")

    def to_query_params(self) -> dict[str, str]:
        return {
            "include_fixtures": "true" if self.include_fixtures else "false",
            "max_items": str(self.max_items),
        }


@dataclass(frozen=True)
class MissionEvaluatorReplayQueryReport:
    """Typed durable REST projection that distinguishes full and summary-only replay."""

    raw: dict[str, Any]
    mission_id: str
    query: Mapping[str, Any]
    retention: Mapping[str, Any]
    replay: Mapping[str, Any]
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    links: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvaluatorReplayQueryReport":
        raw = _tool_payload(value, "mission_evaluator_replay_query")
        query = _route_mapping("mission evaluator replay query", raw.get("query"))
        retention = _route_mapping("mission evaluator replay retention", raw.get("retention"))
        replay = _route_mapping("mission evaluator replay query payload", raw.get("replay"))
        mode = _route_text("mission evaluator replay retention mode", retention.get("mode"))
        if mode not in {"full", "summary_only"}:
            raise ArgumentError(f"unknown mission evaluator replay retention mode: {mode}")
        return cls(
            raw=raw,
            mission_id=_route_text("mission evaluator replay query mission id", raw.get("mission_id")),
            query=query,
            retention=retention,
            replay=replay,
            execution=_route_text("mission evaluator replay query execution", raw.get("execution")),
            guarantees=_route_strings("mission evaluator replay query guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("mission evaluator replay query limitations", raw.get("limitations", [])),
            links=_route_mapping("mission evaluator replay query links", raw.get("links", {})),
        )

    @property
    def summary_only(self) -> bool:
        return self.retention.get("mode") == "summary_only"

    @property
    def full_result_retained(self) -> bool:
        return self.retention.get("result_retained") is True

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evaluator_replay_query_report(value: Mapping[str, Any]) -> MissionEvaluatorReplayQueryReport:
    """Parse the durable REST evaluator replay query response."""

    return MissionEvaluatorReplayQueryReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvidenceBundleRequest:
    """Bounded REST export options for a durable mission evidence bundle."""

    mission_id: str
    include_result: bool = False
    include_trace: bool = True
    include_fixtures: bool = False
    max_items: int = 128

    def __post_init__(self) -> None:
        _route_text("mission evidence bundle mission id", self.mission_id)
        for name, value in (
            ("include_result", self.include_result),
            ("include_trace", self.include_trace),
            ("include_fixtures", self.include_fixtures),
        ):
            if not isinstance(value, bool):
                raise ArgumentError(f"mission evidence bundle {name} must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 512:
            raise ArgumentError("mission evidence bundle max_items must be between 1 and 512")

    def to_query_params(self) -> dict[str, str]:
        return {
            "include_result": "true" if self.include_result else "false",
            "include_trace": "true" if self.include_trace else "false",
            "include_fixtures": "true" if self.include_fixtures else "false",
            "max_items": str(self.max_items),
        }


@dataclass(frozen=True)
class MissionEvidenceBundleVerifyRequest:
    """Request verification of one exported, content-addressed mission bundle."""

    bundle: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "bundle", dict(_route_mapping("mission evidence bundle", self.bundle)))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"bundle": dict(self.bundle)}

    def to_http_body(self) -> dict[str, Any]:
        return {"bundle": dict(self.bundle)}


@dataclass(frozen=True)
class MissionEvidenceBundleImportRequest:
    """Import one independently verified bundle into the bounded evidence registry."""

    bundle: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "bundle", dict(_route_mapping("mission evidence bundle", self.bundle)))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"bundle": dict(self.bundle)}

    def to_http_body(self) -> dict[str, Any]:
        return {"bundle": dict(self.bundle)}


@dataclass(frozen=True)
class MissionEvidenceBundleQueryRequest:
    """Bounded mission/domain index query for the evidence registry."""

    mission_id: str | None = None
    domain: str | None = None
    after: str | None = None
    max_items: int = 100
    include_bundles: bool = False

    def __post_init__(self) -> None:
        for name, value in (("mission_id", self.mission_id), ("domain", self.domain), ("after", self.after)):
            if value is not None:
                _route_text(f"mission evidence bundle query {name}", value)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("mission evidence bundle query max_items must be between 1 and 256")
        if not isinstance(self.include_bundles, bool):
            raise ArgumentError("mission evidence bundle query include_bundles must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params: dict[str, str] = {"max_items": str(self.max_items), "include_bundles": "true" if self.include_bundles else "false"}
        if self.mission_id is not None:
            params["mission_id"] = self.mission_id
        if self.domain is not None:
            params["domain"] = self.domain
        if self.after is not None:
            params["after"] = self.after
        return params

    def to_mcp_arguments(self) -> dict[str, Any]:
        return self.to_query_params()


@dataclass(frozen=True)
class MissionEvidenceBundleImportReport:
    """Typed idempotent registry import result."""

    raw: dict[str, Any]
    bundle_digest: str
    created: bool
    already_present: bool
    registry_generation: int
    registry_size: int
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvidenceBundleImportReport":
        raw = _tool_payload(value, "mission_evidence_bundle_import")
        if raw.get("workflow") != "mission_evidence_bundle_import":
            raise ArgumentError("mission evidence bundle import workflow is invalid")
        created = raw.get("created")
        already_present = raw.get("already_present")
        if not isinstance(created, bool) or not isinstance(already_present, bool):
            raise ArgumentError("mission evidence bundle import flags must be booleans")
        return cls(
            raw=raw,
            bundle_digest=_route_text("mission evidence bundle import digest", raw.get("bundle_digest")),
            created=created,
            already_present=already_present,
            registry_generation=_route_count("mission evidence bundle import generation", raw.get("registry_generation")),
            registry_size=_route_count("mission evidence bundle import size", raw.get("registry_size")),
            execution=_route_text("mission evidence bundle import execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class MissionEvidenceBundleQueryReport:
    """Typed deterministic evidence registry index page."""

    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvidenceBundleQueryReport":
        raw = _tool_payload(value, "mission_evidence_bundle_query")
        if raw.get("workflow") != "mission_evidence_bundle_query":
            raise ArgumentError("mission evidence bundle query workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("mission evidence bundle query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _route_text("mission evidence bundle query next cursor", next_after)
        has_more = raw.get("has_more")
        if not isinstance(has_more, bool):
            raise ArgumentError("mission evidence bundle query has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_route_mapping("mission evidence bundle query row", row) for row in rows),
            next_after=next_after,
            has_more=has_more,
            registry_generation=_route_count("mission evidence bundle query generation", raw.get("registry_generation")),
            registry_size=_route_count("mission evidence bundle query size", raw.get("registry_size")),
            execution=_route_text("mission evidence bundle query execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class MissionEvidenceBundleGetRequest:
    """Fetch one registry bundle by its content hash."""

    bundle_digest: str

    def __post_init__(self) -> None:
        _route_text("mission evidence bundle digest", self.bundle_digest)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"bundle_digest": self.bundle_digest}


@dataclass(frozen=True)
class MissionEvidenceBundleGetReport:
    """Typed lookup result for one verified registry bundle."""

    raw: dict[str, Any]
    bundle_digest: str
    bundle: Mapping[str, Any]
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvidenceBundleGetReport":
        raw = _tool_payload(value, "mission_evidence_bundle_get")
        if raw.get("workflow") != "mission_evidence_bundle_get":
            raise ArgumentError("mission evidence bundle get workflow is invalid")
        return cls(
            raw=raw,
            bundle_digest=_route_text("mission evidence bundle get digest", raw.get("bundle_digest")),
            bundle=_route_mapping("mission evidence bundle get bundle", raw.get("bundle")),
            execution=_route_text("mission evidence bundle get execution", raw.get("execution")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class MissionEvidenceBundleVerificationReport:
    """Typed digest, retention, and result-integrity verification evidence."""

    raw: dict[str, Any]
    valid: bool
    verification_status: str
    bundle_digest: str
    recomputed_bundle_digest: str
    result_digest: str | None
    recomputed_result_digest: str | None
    checks: Mapping[str, Any]
    failures: tuple[str, ...]
    execution: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvidenceBundleVerificationReport":
        raw = _tool_payload(value, "mission_evidence_bundle_verify")
        if raw.get("workflow") != "mission_evidence_bundle_verify":
            raise ArgumentError("mission evidence bundle verification workflow is invalid")
        valid = raw.get("valid")
        if not isinstance(valid, bool):
            raise ArgumentError("mission evidence bundle verification valid must be a boolean")
        status = _route_text("mission evidence bundle verification status", raw.get("verification_status"))
        if status not in {"verified", "failed"}:
            raise ArgumentError(f"unknown mission evidence bundle verification status: {status}")
        failures = raw.get("failures", [])
        if not isinstance(failures, Sequence) or isinstance(failures, (str, bytes)):
            raise ArgumentError("mission evidence bundle verification failures must be an array")
        result_digest = raw.get("result_digest")
        recomputed_result_digest = raw.get("recomputed_result_digest")
        if result_digest is not None:
            result_digest = _route_text("mission evidence bundle result digest", result_digest)
        if recomputed_result_digest is not None:
            recomputed_result_digest = _route_text(
                "mission evidence bundle recomputed result digest", recomputed_result_digest
            )
        return cls(
            raw=raw,
            valid=valid,
            verification_status=status,
            bundle_digest=_route_text("mission evidence bundle digest", raw.get("bundle_digest")),
            recomputed_bundle_digest=_route_text(
                "mission evidence bundle recomputed digest", raw.get("recomputed_bundle_digest")
            ),
            result_digest=result_digest,
            recomputed_result_digest=recomputed_result_digest,
            checks=_route_mapping("mission evidence bundle verification checks", raw.get("checks")),
            failures=tuple(_route_text("mission evidence bundle verification failure", item) for item in failures),
            execution=_route_text("mission evidence bundle verification execution", raw.get("execution")),
        )

    @property
    def digest_matches(self) -> bool:
        return self.checks.get("bundle_digest") is True

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evidence_bundle_verification_report(
    value: Mapping[str, Any],
) -> MissionEvidenceBundleVerificationReport:
    """Parse a direct MCP result or HTTP envelope from bundle verification."""

    return MissionEvidenceBundleVerificationReport.from_wire(value)


@dataclass(frozen=True)
class MissionEvidenceBundleReport:
    """Typed content-addressed mission evidence export."""

    raw: dict[str, Any]
    mission_id: str
    retention: Mapping[str, Any]
    result: Mapping[str, Any] | None
    result_digest: str | None
    evaluator_replay: Mapping[str, Any]
    catalog_drift: Mapping[str, Any]
    trace: tuple[Mapping[str, Any], ...]
    bundle_digest: str
    execution: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    links: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MissionEvidenceBundleReport":
        raw = _tool_payload(value, "mission_evidence_bundle_export")
        if raw.get("workflow") != "mission_evidence_bundle_export":
            raise ArgumentError("mission evidence bundle workflow is invalid")
        result = raw.get("result")
        if result is not None and not isinstance(result, Mapping):
            raise ArgumentError("mission evidence bundle result must be an object or null")
        trace = raw.get("trace", [])
        if not isinstance(trace, Sequence) or isinstance(trace, (str, bytes)):
            raise ArgumentError("mission evidence bundle trace must be an array")
        digest = _route_text("mission evidence bundle digest", raw.get("bundle_digest"))
        if len(digest) != 64:
            raise ArgumentError("mission evidence bundle digest must be a 64-character digest")
        export = _route_mapping("mission evidence bundle export", raw.get("export"))
        result_digest = raw.get("result_digest")
        if result_digest is not None:
            result_digest = _route_text("mission evidence bundle result digest", result_digest)
        return cls(
            raw=raw,
            mission_id=_route_text("mission evidence bundle mission id", raw.get("mission_id")),
            retention=_route_mapping("mission evidence bundle retention", raw.get("retention")),
            result=dict(result) if isinstance(result, Mapping) else None,
            result_digest=result_digest,
            evaluator_replay=_route_mapping("mission evidence bundle replay", raw.get("evaluator_replay")),
            catalog_drift=_route_mapping("mission evidence bundle catalog drift", raw.get("catalog_drift")),
            trace=tuple(_route_mapping("mission evidence bundle trace row", row) for row in trace),
            bundle_digest=digest,
            execution=_route_text("mission evidence bundle execution", export.get("execution")),
            guarantees=_route_strings("mission evidence bundle guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("mission evidence bundle limitations", raw.get("limitations", [])),
            links=_route_mapping("mission evidence bundle links", raw.get("links", {})),
        )

    @property
    def summary_only(self) -> bool:
        return self.retention.get("mode") == "summary_only"

    @property
    def result_included(self) -> bool:
        return self.retention.get("result_included") is True

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def mission_evidence_bundle_report(value: Mapping[str, Any]) -> MissionEvidenceBundleReport:
    """Parse the durable REST mission evidence bundle response."""

    return MissionEvidenceBundleReport.from_wire(value)


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
    "DomainWorkflowInstantiateRequest",
    "DomainWorkflowReconcileRequest",
    "DomainWorkflowCatalogueReport",
    "DomainWorkflowInstantiationReport",
    "DomainWorkflowReconciliationReport",
    "DomainWorkflowReconciliationImportRequest",
    "DomainWorkflowReconciliationQueryRequest",
    "DomainWorkflowReconciliationGetRequest",
    "DomainWorkflowReconciliationImportReport",
    "DomainWorkflowReconciliationQueryReport",
    "DomainWorkflowReconciliationSummaryReport",
    "DomainWorkflowReconciliationPersistenceStatus",
    "DomainWorkflowReconciliationGetReport",
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
    "MissionEvaluatorReplayRequest",
    "MissionEvaluatorReplayReport",
    "MissionEvaluatorReplayCompareRequest",
    "MissionEvaluatorReplayComparisonReport",
    "MissionEvaluatorReplayQueryRequest",
    "MissionEvaluatorReplayQueryReport",
    "MissionEvidenceBundleRequest",
    "MissionEvidenceBundleImportRequest",
    "MissionEvidenceBundleQueryRequest",
    "MissionEvidenceBundleGetRequest",
    "MissionEvidenceBundleVerifyRequest",
    "MissionEvidenceBundleReport",
    "MissionEvidenceBundleImportReport",
    "MissionEvidenceBundleQueryReport",
    "MissionEvidenceBundleGetReport",
    "MissionEvidenceBundleVerificationReport",
    "MissionEvaluatorAdapterReport",
    "MissionEvaluatorMatchReport",
    "MissionEvaluatorCoverageReport",
    "MissionEvaluatorSearchReport",
    "capability_route_report",
    "domain_workflow_catalogue_report",
    "domain_workflow_instantiation_report",
    "domain_workflow_reconciliation_report",
    "capability_route_review_report",
    "capability_discover_report",
    "capability_audit_report",
    "mission_evaluator_discover_report",
    "mission_evaluator_review_report",
    "mission_evaluator_replay_report",
    "mission_evaluator_replay_comparison_report",
    "mission_evaluator_replay_query_report",
    "mission_evidence_bundle_report",
    "mission_evidence_bundle_verification_report",
]
