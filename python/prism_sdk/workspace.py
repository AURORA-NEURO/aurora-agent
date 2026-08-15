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
from .client import Client
from .errors import ArgumentError
from .mission import MissionPolicy, MissionRequest, MissionStep
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
