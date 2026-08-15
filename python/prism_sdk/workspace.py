"""High-level helpers for the most important cross-domain MCP workflows."""

from __future__ import annotations

from typing import Any, Mapping, Sequence

from .async_client import AsyncClient
from .analytics import (
    AnalyticsRequest,
    CalibrationObservation,
    MetricObservation,
    PairedObservation,
    analytics_request,
)
from .authoring import PackArtifact
from .biological import AdapterPlanRequest
from .bioql import BioQlCompileRequest
from .client import Client
from .capability import (
    CapabilityQuery,
    CapabilityRouteNeed,
    CapabilityRouteReport,
    CapabilityRouteReviewReport,
    CapabilityRouteReviewRequest,
    CapabilityRouteRequest,
    capability_route_report,
    capability_route_review_report,
)
from .context_requests import (
    ContextLayer,
    FiberCompileRequest,
    FiberExplainRequest,
    FiberRefineRequest,
    FiberVerifyRequest,
    ProjectionBundleRequest,
)
from .errors import ArgumentError
from .evidence import BioCapabilityEvidenceAuditRequest
from .domain_requests import LabPlanRequest, RoutingDecisionRequest, WorldClaimCheckRequest
from .mission import (
    MissionAssembly,
    MissionPolicy,
    MissionPreflight,
    MissionRequest,
    MissionRouteSelection,
    MissionStep,
    mission_from_route as assemble_mission_from_route,
    preflight_mission,
)
from .repository_requests import (
    RepositoryBundleRequest,
    RepositoryCatalogRequest,
    RepositoryImpactRequest,
    RepositoryTraversalPolicy,
    TelemetryProjectRequest,
)
from .tooling import ToolCallPlan, ToolCatalogue
from .oracle import (
    EvidenceTier,
    EvaluationReproductionRequest,
    EvaluationTrajectoryRequest,
    EvaluationWorldlineRequest,
    MissingnessAuditRequest,
    OracleCombineRequest,
    ReferencePanelRequest,
    ReferenceStandardAuditRequest,
)
from .workbench import WorkbenchRequest


def _targets(request_id: str | None, targets: Sequence[str] | None) -> dict[str, Any] | None:
    if request_id is None and targets is None:
        return None
    if not isinstance(request_id, str) or not request_id:
        raise ArgumentError("request_id is required when targets are supplied")
    if not targets:
        raise ArgumentError("targets must contain at least one target")
    return {"id": request_id, "targets": list(targets)}


class Workspace:
    """Typed convenience facade over an initialized synchronous MCP client."""

    def __init__(self, client: Client) -> None:
        self.client = client

    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return self.client.call_tool(name, arguments).require_ok()

    def tool_catalogue(self) -> ToolCatalogue:
        """Snapshot the authoritative live ``tools/list`` catalogue for checked calls."""

        return ToolCatalogue.from_definitions(self.client.list_tools())

    def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate a cross-domain tool call without executing it or interpreting its result."""

        snapshot = catalogue if catalogue is not None else self.tool_catalogue()
        if not isinstance(snapshot, ToolCatalogue):
            raise ArgumentError("catalogue must be a ToolCatalogue")
        return snapshot.plan(name, arguments)

    def tool_checked(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> dict[str, Any]:
        """Run any live MCP tool after conservative schema preflight.

        The preflight checks only the transport JSON shape.  Remote refusals still raise
        ``ToolRefusal`` and remain distinct from a successful domain result.
        """

        plan = self.plan_tool(name, arguments, catalogue=catalogue)
        return self.tool(plan.tool, plan.to_mcp_arguments())

    def pack_health_assess(
        self,
        pack: PackArtifact | Mapping[str, Any],
        observations: Mapping[str, Any],
        *,
        policy: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Run the authoritative Rust pack-health gate over a locally authored PackIr."""

        artifact = pack if isinstance(pack, PackArtifact) else PackArtifact.from_document(pack)
        return self.tool("pack_health_assess", artifact.to_mcp_arguments(observations, policy))

    def pack_catalogue(self, *, section: str | None = None, max_items: int | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        if section is not None:
            arguments["section"] = section
        if max_items is not None:
            arguments["max_items"] = max_items
        return self.tool("pack_catalogue", arguments)

    def mutation_family(
        self,
        world: str,
        *,
        include_worlds: bool = False,
        max_worlds: int | None = None,
    ) -> dict[str, Any]:
        """Run the server's standard metamorphic suite with explicit bounded disclosure."""

        from .authoring import MutationPlan

        arguments = MutationPlan.standard().standard_tool_arguments(
            world, include_worlds=include_worlds, max_worlds=max_worlds
        )
        return self.tool("mutation_family", arguments)

    def metrics_analytics_audit(
        self,
        observations: Sequence[MetricObservation | Mapping[str, Any]],
        *,
        pairs: Sequence[PairedObservation | Mapping[str, Any]] = (),
        calibration: Sequence[CalibrationObservation | Mapping[str, Any]] = (),
        calibration_bins: int = 10,
    ) -> dict[str, Any]:
        """Run bounded descriptive metrics analytics in the Rust kernel."""

        request = analytics_request(
            observations,
            pairs=pairs,
            calibration=calibration,
            calibration_bins=calibration_bins,
        )
        return self.tool("metrics_analytics_audit", request.to_mcp_arguments())

    def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Audit evidence prerequisites before making cross-domain capability claims."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return self.tool("biocapability_evidence_audit", request.to_mcp_arguments())

    def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Compile BioQL against an explicit schema without executing the query."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return self.tool("bioql_compile", request.to_mcp_arguments())

    def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Check a serialized claim against world provenance without inventing support."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return self.tool("world_claim_check", request.to_mcp_arguments())

    def lab_plan(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Plan evidence acquisition while preserving the no-execution boundary."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return self.tool("lab_plan", request.to_mcp_arguments())

    def routing_decide(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> dict[str, Any]:
        """Route an unseen task using an approved architecture policy and evidence ledger."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return self.tool("routing_decide", request.to_mcp_arguments())

    def developer_workbench(
        self,
        session: Mapping[str, Any],
        *,
        dashboard: Mapping[str, Any] | None = None,
        ci: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit an authoring/notebook session and optional dashboard/CI projections in Rust."""

        request = WorkbenchRequest(session, dashboard, ci)
        return self.tool("developer_workbench", request.to_mcp_arguments())

    def agent_mission(
        self,
        mission_id: str,
        goal: str,
        steps: Sequence[MissionStep | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Preview or execute a bounded dependency graph of existing domain tools."""

        request = MissionRequest(mission_id, goal, steps, policy)
        return self.tool("agent_mission", request.to_mcp_arguments())

    def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Review mission graph, execution policy, and every step schema without dispatching."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else self.tool_catalogue()
        return preflight_mission(request, snapshot)

    def mission_from_route(
        self,
        route: Mapping[str, Any],
        mission_id: str,
        selections: Sequence[MissionRouteSelection | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> MissionAssembly:
        """Assemble an explicit mission from one reviewed capability-route response without transport."""

        return assemble_mission_from_route(route, mission_id, selections, policy=policy)

    def capability_discover(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Search every catalogued domain and optionally return authoritative tool schemas."""

        if isinstance(query, CapabilityQuery):
            if (
                any(value is not None for value in (text, domain, tool, group_id))
                or max_items != 50
                or include_tools
            ):
                raise ArgumentError("query cannot be combined with individual capability filters")
            request = query
        elif isinstance(query, str):
            if text is not None:
                raise ArgumentError("query cannot be combined with text")
            request = CapabilityQuery(query, group_id, domain, tool, max_items, include_tools)
        elif query is not None:
            raise ArgumentError("query must be a CapabilityQuery or string")
        else:
            request = CapabilityQuery(text, group_id, domain, tool, max_items, include_tools)
        return self.tool("capability_discover", request.to_mcp_arguments())

    def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        """Verify catalogue membership against the authoritative MCP schema set."""

        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return self.tool("capability_audit", {"include_groups": include_groups})

    def capability_route(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Batch named cross-domain needs into a reviewed, non-executing route proposal."""

        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return self.tool("capability_route", request.to_mcp_arguments())

    def capability_route_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Return a validated typed view over a non-executing route proposal."""

        return capability_route_report(
            self.capability_route(
                goal,
                needs,
                max_candidates_per_need=max_candidates_per_need,
                max_tools=max_tools,
                include_tools=include_tools,
            )
        )

    def capability_route_review(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> dict[str, Any]:
        """Review explicit route selections and return a non-executing mission handoff."""

        request = CapabilityRouteReviewRequest(route, selections, validate_schemas)
        return self.tool("capability_route_review", request.to_mcp_arguments())

    def capability_route_review_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Return typed diagnostics for a route-to-mission handoff review."""

        return capability_route_review_report(
            self.capability_route_review(route, selections, validate_schemas=validate_schemas)
        )

    def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Plan native and Python-delegated adapters before any source bytes are read."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return self.tool("adapter_plan", request.to_mcp_arguments())

    def oracle_combine(
        self,
        subject: str,
        at: str,
        judgements: Sequence[Mapping[str, Any] | Any],
        *,
        minimum_deciding_tier: EvidenceTier | str = EvidenceTier.JUDGE,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Combine retained oracle judgements under Rust's tiered, set-valued mesh policy."""

        tier = minimum_deciding_tier if isinstance(minimum_deciding_tier, EvidenceTier) else EvidenceTier(minimum_deciding_tier)
        request = OracleCombineRequest(subject, at, tuple(judgements), tier, max_items)
        return self.tool("oracle_combine", request.to_mcp_arguments())

    def oracle_reference_panel(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        request = ReferencePanelRequest(panel, rule, model_call, max_items)
        return self.tool("oracle_reference_panel", request.to_mcp_arguments())

    def oracle_missingness(
        self,
        pattern: Mapping[str, Any],
        field: Mapping[str, Any],
        boundary: Mapping[str, Any],
        small_cell_floor: int,
        *,
        mechanism: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        request = MissingnessAuditRequest(pattern, field, boundary, small_cell_floor, mechanism)
        return self.tool("oracle_missingness", request.to_mcp_arguments())

    def bioeval_reference_audit(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> dict[str, Any]:
        return self.tool("bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments())

    def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return self.tool("evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments())

    def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        return self.tool(
            "evaluation_reproduction_check",
            EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments(),
        )

    def evaluation_trajectory_check(
        self,
        trajectory: Mapping[str, Any],
        *,
        step: int | None = None,
        horizon: int | None = None,
    ) -> dict[str, Any]:
        return self.tool(
            "evaluation_trajectory_check",
            EvaluationTrajectoryRequest(trajectory, step, horizon).to_mcp_arguments(),
        )

    def developer_delivery_audit(
        self,
        *,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        platform: Mapping[str, Any] | None = None,
        repository: Mapping[str, Any] | None = None,
        repository_impact: Mapping[str, Any] | None = None,
        sdk: Mapping[str, Any] | None = None,
        conformance: Mapping[str, Any] | None = None,
        provider: Mapping[str, Any] | None = None,
        governance: Mapping[str, Any] | None = None,
        release: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        for key, value in (
            ("platform", platform),
            ("repository", repository),
            ("repository_impact", repository_impact),
            ("sdk", sdk),
            ("conformance", conformance),
            ("provider", provider),
            ("governance", governance),
            ("release", release),
        ):
            if value is not None:
                arguments[key] = dict(value)
        release_request = _targets(request_id, targets)
        if release_request is not None:
            arguments["release_request"] = release_request
        return self.tool("developer_delivery_audit", arguments)

    def bioatlas_publication_audit(
        self,
        atlas: Mapping[str, Any],
        *,
        weighting: Mapping[str, Any] | None = None,
        evidence_audit: Mapping[str, Any] | None = None,
        card: Mapping[str, Any] | None = None,
        leaderboard: Mapping[str, Any] | None = None,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        max_items: int | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"atlas": dict(atlas)}
        for key, value in (
            ("weighting", weighting),
            ("evidence_audit", evidence_audit),
            ("card", card),
            ("leaderboard", leaderboard),
        ):
            if value is not None:
                arguments[key] = dict(value)
        release_request = _targets(request_id, targets)
        if release_request is not None:
            arguments["release_request"] = release_request
        if max_items is not None:
            arguments["max_items"] = max_items
        return self.tool("bioatlas_publication_audit", arguments)

    def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Discover repository modules without dumping document bodies by default."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return self.tool("repository_catalog", request.to_mcp_arguments())

    def repository_bundle(
        self,
        route: Mapping[str, Any] | RepositoryBundleRequest,
        *,
        policy: RepositoryTraversalPolicy | str = RepositoryTraversalPolicy.NORMATIVE,
        max_depth: int | None = None,
        denied_labels: Sequence[str] = (),
        follow: Sequence[str] = (),
        include_markdown: bool = False,
        max_markdown_chars: int | None = None,
    ) -> dict[str, Any]:
        """Compile a route-specific, omission-aware repository context."""

        if isinstance(route, RepositoryBundleRequest):
            if (
                policy not in (RepositoryTraversalPolicy.NORMATIVE, "normative")
                or max_depth is not None
                or denied_labels
                or follow
                or include_markdown
                or max_markdown_chars is not None
            ):
                raise ArgumentError("bundle options must be omitted when passing a RepositoryBundleRequest")
            request = route
        else:
            request = RepositoryBundleRequest(route, policy, max_depth, denied_labels, follow, include_markdown, max_markdown_chars)
        return self.tool("repository_bundle", request.to_mcp_arguments())

    def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compute conservative documentation impact for one changed module."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return self.tool("repository_impact", request.to_mcp_arguments())

    def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Project a domain event through explicit redaction and optional metric policy."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return self.tool("telemetry_project", request.to_mcp_arguments())

    def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Compile a typed world/query pair into a bounded decision-sufficient context."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return self.tool("fiber_compile", request.to_mcp_arguments())

    def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Descend a compiled context through a verified handle or explicit source paths."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return self.tool("fiber_refine", request.to_mcp_arguments())

    def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Return the compile plan and omission explanation before trusting compact context."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return self.tool("fiber_explain", request.to_mcp_arguments())

    def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Recompute a context certificate digest before consuming a remotely produced result."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return self.tool("fiber_verify", request.to_mcp_arguments())

    def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Generate bounded graph, hypergraph, timeline, and table projections."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return self.tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    def compile_context(
        self,
        world: Mapping[str, Any],
        query: Mapping[str, Any],
        *,
        policy: str | None = None,
        profile: str | None = None,
        include_views: bool | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"world": dict(world), "query": dict(query)}
        if policy is not None:
            arguments["policy"] = policy
        if profile is not None:
            arguments["profile"] = profile
        if include_views is not None:
            arguments["include_views"] = include_views
        return self.tool("fiber_compile", arguments)

    def trace_otel_ingest(
        self,
        trace_id: str,
        *,
        otlp_json: str | None = None,
        document: str | None = None,
        succeeded: bool | None = None,
        include_events: bool | None = None,
        max_items: int | None = None,
        max_spans: int | None = None,
        max_bytes: int | None = None,
    ) -> dict[str, Any]:
        arguments = _otel_arguments(
            trace_id,
            otlp_json=otlp_json,
            document=document,
            succeeded=succeeded,
            include_events=include_events,
            max_items=max_items,
            max_spans=max_spans,
            max_bytes=max_bytes,
        )
        return self.tool("trace_otel_ingest", arguments)


class AsyncWorkspace:
    """Async convenience facade mirroring :class:`Workspace`."""

    def __init__(self, client: AsyncClient) -> None:
        self.client = client

    async def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return (await self.client.call_tool(name, arguments)).require_ok()

    async def tool_catalogue(self) -> ToolCatalogue:
        """Async snapshot of the authoritative live ``tools/list`` catalogue."""

        return ToolCatalogue.from_definitions(await self.client.list_tools())

    async def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate an arbitrary cross-domain tool call without executing it."""

        snapshot = catalogue if catalogue is not None else await self.tool_catalogue()
        if not isinstance(snapshot, ToolCatalogue):
            raise ArgumentError("catalogue must be a ToolCatalogue")
        return snapshot.plan(name, arguments)

    async def tool_checked(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> dict[str, Any]:
        """Run any live MCP tool after conservative schema preflight."""

        plan = await self.plan_tool(name, arguments, catalogue=catalogue)
        return await self.tool(plan.tool, plan.to_mcp_arguments())

    async def pack_health_assess(
        self,
        pack: PackArtifact | Mapping[str, Any],
        observations: Mapping[str, Any],
        *,
        policy: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.pack_health_assess`."""

        artifact = pack if isinstance(pack, PackArtifact) else PackArtifact.from_document(pack)
        return await self.tool("pack_health_assess", artifact.to_mcp_arguments(observations, policy))

    async def pack_catalogue(self, *, section: str | None = None, max_items: int | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        if section is not None:
            arguments["section"] = section
        if max_items is not None:
            arguments["max_items"] = max_items
        return await self.tool("pack_catalogue", arguments)

    async def mutation_family(
        self,
        world: str,
        *,
        include_worlds: bool = False,
        max_worlds: int | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.mutation_family`."""

        from .authoring import MutationPlan

        arguments = MutationPlan.standard().standard_tool_arguments(
            world, include_worlds=include_worlds, max_worlds=max_worlds
        )
        return await self.tool("mutation_family", arguments)

    async def metrics_analytics_audit(
        self,
        observations: Sequence[MetricObservation | Mapping[str, Any]],
        *,
        pairs: Sequence[PairedObservation | Mapping[str, Any]] = (),
        calibration: Sequence[CalibrationObservation | Mapping[str, Any]] = (),
        calibration_bins: int = 10,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.metrics_analytics_audit`."""

        request: AnalyticsRequest = analytics_request(
            observations,
            pairs=pairs,
            calibration=calibration,
            calibration_bins=calibration_bins,
        )
        return await self.tool("metrics_analytics_audit", request.to_mcp_arguments())

    async def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.biocapability_evidence_audit`."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return await self.tool("biocapability_evidence_audit", request.to_mcp_arguments())

    async def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.bioql_compile`."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return await self.tool("bioql_compile", request.to_mcp_arguments())

    async def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.world_claim_check`."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return await self.tool("world_claim_check", request.to_mcp_arguments())

    async def lab_plan(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.lab_plan`."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return await self.tool("lab_plan", request.to_mcp_arguments())

    async def routing_decide(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.routing_decide`."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return await self.tool("routing_decide", request.to_mcp_arguments())

    async def developer_workbench(
        self,
        session: Mapping[str, Any],
        *,
        dashboard: Mapping[str, Any] | None = None,
        ci: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.developer_workbench`."""

        request = WorkbenchRequest(session, dashboard, ci)
        return await self.tool("developer_workbench", request.to_mcp_arguments())

    async def agent_mission(
        self,
        mission_id: str,
        goal: str,
        steps: Sequence[MissionStep | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.agent_mission`."""

        request = MissionRequest(mission_id, goal, steps, policy)
        return await self.tool("agent_mission", request.to_mcp_arguments())

    async def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Async review of mission graph, policy, and step schemas without dispatching."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else await self.tool_catalogue()
        return preflight_mission(request, snapshot)

    async def mission_from_route(
        self,
        route: Mapping[str, Any],
        mission_id: str,
        selections: Sequence[MissionRouteSelection | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> MissionAssembly:
        """Async facade for route-to-mission assembly; no network call is made."""

        return assemble_mission_from_route(route, mission_id, selections, policy=policy)

    async def capability_discover(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.capability_discover`."""

        if isinstance(query, CapabilityQuery):
            if (
                any(value is not None for value in (text, domain, tool, group_id))
                or max_items != 50
                or include_tools
            ):
                raise ArgumentError("query cannot be combined with individual capability filters")
            request = query
        elif isinstance(query, str):
            if text is not None:
                raise ArgumentError("query cannot be combined with text")
            request = CapabilityQuery(query, group_id, domain, tool, max_items, include_tools)
        elif query is not None:
            raise ArgumentError("query must be a CapabilityQuery or string")
        else:
            request = CapabilityQuery(text, group_id, domain, tool, max_items, include_tools)
        return await self.tool("capability_discover", request.to_mcp_arguments())

    async def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.capability_audit`."""

        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return await self.tool("capability_audit", {"include_groups": include_groups})

    async def capability_route(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.capability_route`."""

        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return await self.tool("capability_route", request.to_mcp_arguments())

    async def capability_route_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Async counterpart to :meth:`Workspace.capability_route_report`."""

        return capability_route_report(
            await self.capability_route(
                goal,
                needs,
                max_candidates_per_need=max_candidates_per_need,
                max_tools=max_tools,
                include_tools=include_tools,
            )
        )

    async def capability_route_review(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.capability_route_review`."""

        request = CapabilityRouteReviewRequest(route, selections, validate_schemas)
        return await self.tool("capability_route_review", request.to_mcp_arguments())

    async def capability_route_review_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Async typed diagnostics for a route-to-mission handoff review."""

        return capability_route_review_report(
            await self.capability_route_review(route, selections, validate_schemas=validate_schemas)
        )

    async def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.adapter_plan`."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return await self.tool("adapter_plan", request.to_mcp_arguments())

    async def oracle_combine(
        self,
        subject: str,
        at: str,
        judgements: Sequence[Mapping[str, Any] | Any],
        *,
        minimum_deciding_tier: EvidenceTier | str = EvidenceTier.JUDGE,
        max_items: int = 100,
    ) -> dict[str, Any]:
        tier = minimum_deciding_tier if isinstance(minimum_deciding_tier, EvidenceTier) else EvidenceTier(minimum_deciding_tier)
        request = OracleCombineRequest(subject, at, tuple(judgements), tier, max_items)
        return await self.tool("oracle_combine", request.to_mcp_arguments())

    async def oracle_reference_panel(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        request = ReferencePanelRequest(panel, rule, model_call, max_items)
        return await self.tool("oracle_reference_panel", request.to_mcp_arguments())

    async def oracle_missingness(
        self,
        pattern: Mapping[str, Any],
        field: Mapping[str, Any],
        boundary: Mapping[str, Any],
        small_cell_floor: int,
        *,
        mechanism: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        request = MissingnessAuditRequest(pattern, field, boundary, small_cell_floor, mechanism)
        return await self.tool("oracle_missingness", request.to_mcp_arguments())

    async def bioeval_reference_audit(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> dict[str, Any]:
        return await self.tool(
            "bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments()
        )

    async def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return await self.tool(
            "evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments()
        )

    async def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        return await self.tool(
            "evaluation_reproduction_check",
            EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments(),
        )

    async def evaluation_trajectory_check(
        self,
        trajectory: Mapping[str, Any],
        *,
        step: int | None = None,
        horizon: int | None = None,
    ) -> dict[str, Any]:
        return await self.tool(
            "evaluation_trajectory_check",
            EvaluationTrajectoryRequest(trajectory, step, horizon).to_mcp_arguments(),
        )

    async def developer_delivery_audit(
        self,
        *,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        platform: Mapping[str, Any] | None = None,
        repository: Mapping[str, Any] | None = None,
        repository_impact: Mapping[str, Any] | None = None,
        sdk: Mapping[str, Any] | None = None,
        conformance: Mapping[str, Any] | None = None,
        provider: Mapping[str, Any] | None = None,
        governance: Mapping[str, Any] | None = None,
        release: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        arguments = _developer_delivery_arguments(
            {
                "request_id": request_id,
                "targets": targets,
                "platform": platform,
                "repository": repository,
                "repository_impact": repository_impact,
                "sdk": sdk,
                "conformance": conformance,
                "provider": provider,
                "governance": governance,
                "release": release,
            }
        )
        return (await self.client.call_tool("developer_delivery_audit", arguments)).require_ok()

    async def bioatlas_publication_audit(self, atlas: Mapping[str, Any], **kwargs: Any) -> dict[str, Any]:
        arguments: dict[str, Any] = {"atlas": dict(atlas)}
        for key in ("weighting", "evidence_audit", "card", "leaderboard"):
            if kwargs.get(key) is not None:
                arguments[key] = dict(kwargs[key])
        release_request = _targets(kwargs.get("request_id"), kwargs.get("targets"))
        if release_request is not None:
            arguments["release_request"] = release_request
        if kwargs.get("max_items") is not None:
            arguments["max_items"] = kwargs["max_items"]
        return (await self.client.call_tool("bioatlas_publication_audit", arguments)).require_ok()

    async def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.repository_catalog`."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return await self.tool("repository_catalog", request.to_mcp_arguments())

    async def repository_bundle(
        self,
        route: Mapping[str, Any] | RepositoryBundleRequest,
        *,
        policy: RepositoryTraversalPolicy | str = RepositoryTraversalPolicy.NORMATIVE,
        max_depth: int | None = None,
        denied_labels: Sequence[str] = (),
        follow: Sequence[str] = (),
        include_markdown: bool = False,
        max_markdown_chars: int | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.repository_bundle`."""

        if isinstance(route, RepositoryBundleRequest):
            if (
                policy not in (RepositoryTraversalPolicy.NORMATIVE, "normative")
                or max_depth is not None
                or denied_labels
                or follow
                or include_markdown
                or max_markdown_chars is not None
            ):
                raise ArgumentError("bundle options must be omitted when passing a RepositoryBundleRequest")
            request = route
        else:
            request = RepositoryBundleRequest(route, policy, max_depth, denied_labels, follow, include_markdown, max_markdown_chars)
        return await self.tool("repository_bundle", request.to_mcp_arguments())

    async def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.repository_impact`."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return await self.tool("repository_impact", request.to_mcp_arguments())

    async def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.telemetry_project`."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return await self.tool("telemetry_project", request.to_mcp_arguments())

    async def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.fiber_compile`."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return await self.tool("fiber_compile", request.to_mcp_arguments())

    async def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.fiber_refine`."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return await self.tool("fiber_refine", request.to_mcp_arguments())

    async def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.fiber_explain`."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return await self.tool("fiber_explain", request.to_mcp_arguments())

    async def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.fiber_verify`."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return await self.tool("fiber_verify", request.to_mcp_arguments())

    async def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.projection_bundle`."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return await self.tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    async def compile_context(
        self,
        world: Mapping[str, Any],
        query: Mapping[str, Any],
        *,
        policy: str | None = None,
        profile: str | None = None,
        include_views: bool | None = None,
    ) -> dict[str, Any]:
        arguments: dict[str, Any] = {"world": dict(world), "query": dict(query)}
        if policy is not None:
            arguments["policy"] = policy
        if profile is not None:
            arguments["profile"] = profile
        if include_views is not None:
            arguments["include_views"] = include_views
        return (await self.client.call_tool("fiber_compile", arguments)).require_ok()

    async def trace_otel_ingest(
        self,
        trace_id: str,
        *,
        otlp_json: str | None = None,
        document: str | None = None,
        succeeded: bool | None = None,
        include_events: bool | None = None,
        max_items: int | None = None,
        max_spans: int | None = None,
        max_bytes: int | None = None,
    ) -> dict[str, Any]:
        arguments = _otel_arguments(
            trace_id,
            otlp_json=otlp_json,
            document=document,
            succeeded=succeeded,
            include_events=include_events,
            max_items=max_items,
            max_spans=max_spans,
            max_bytes=max_bytes,
        )
        return (await self.client.call_tool("trace_otel_ingest", arguments)).require_ok()


def _developer_delivery_arguments(kwargs: Mapping[str, Any]) -> dict[str, Any]:
    arguments: dict[str, Any] = {}
    for key in (
        "platform",
        "repository",
        "repository_impact",
        "sdk",
        "conformance",
        "provider",
        "governance",
        "release",
    ):
        if kwargs.get(key) is not None:
            arguments[key] = dict(kwargs[key])
    release_request = _targets(kwargs.get("request_id"), kwargs.get("targets"))
    if release_request is not None:
        arguments["release_request"] = release_request
    return arguments


def _otel_arguments(
    trace_id: str,
    *,
    otlp_json: str | None,
    document: str | None,
    succeeded: bool | None,
    include_events: bool | None,
    max_items: int | None,
    max_spans: int | None,
    max_bytes: int | None,
) -> dict[str, Any]:
    if not isinstance(trace_id, str) or not trace_id.strip():
        raise ArgumentError("trace_id must be a non-empty string")
    if (otlp_json is None) == (document is None):
        raise ArgumentError("provide exactly one of otlp_json or document")
    arguments: dict[str, Any] = {"trace_id": trace_id}
    if otlp_json is not None:
        if not isinstance(otlp_json, str):
            raise ArgumentError("otlp_json must be a string")
        arguments["otlp_json"] = otlp_json
    if document is not None:
        if not isinstance(document, str) or not document:
            raise ArgumentError("document must be a non-empty string")
        arguments["document"] = document
    for key, value in (
        ("succeeded", succeeded),
        ("include_events", include_events),
        ("max_items", max_items),
        ("max_spans", max_spans),
        ("max_bytes", max_bytes),
    ):
        if value is not None:
            arguments[key] = value
    return arguments
