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
from .biological import AdapterPlanReport, AdapterPlanRequest, adapter_plan_report
from .bioql import BioQlCompileRequest
from .client import Client
from .capability import (
    CapabilityAuditReport,
    CapabilitySearchReport,
    CapabilityQuery,
    CapabilityRouteNeed,
    CapabilityRouteReport,
    CapabilityRouteReviewReport,
    CapabilityRouteReviewRequest,
    CapabilityRouteRequest,
    capability_audit_report,
    capability_discover_report,
    capability_route_report,
    capability_route_review_report,
)
from .conformance import ConformanceRunArgs, ConformanceRunReport, conformance_run_report
from .context_requests import (
    ContextLayer,
    FiberCompileRequest,
    FiberExplainRequest,
    FiberRefineRequest,
    FiberVerifyRequest,
    ProjectionBundleRequest,
)
from .delivery import DeveloperDeliveryAuditReport, developer_delivery_audit_report
from .errors import ArgumentError
from .evidence import (
    BioCapabilityEvidenceAuditReport,
    BioCapabilityEvidenceAuditRequest,
    biocapability_evidence_audit_report,
)
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
from .publication import BioAtlasPublicationAuditReport, bioatlas_publication_audit_report
from .release import ReleaseAuditArgs, ReleaseAuditReport, release_audit_report
from .tabular import TabularIngestReport, TabularIngestRequest, tabular_ingest_report
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
from .operations import (
    OpsAcceptanceArgs,
    OpsAcceptanceReport,
    OperationsCatalogArgs,
    OperationsCatalogReport,
    ops_acceptance_report,
    operations_catalog_report,
)
from .safety import (
    MedicalBoundaryReport,
    MedicalBoundaryRequest,
    RiskAssessmentRequest,
    SafetyPostureArgs,
    SafetyPostureReport,
    SafetyReleaseGateArgs,
    SafetyReleaseGateReport,
    medical_boundary_report,
    safety_posture_report,
    safety_release_gate_report,
)
from .hub import (
    HubLockArgs,
    HubLockReport,
    HubResolveArgs,
    HubResolveReport,
    HubSearchArgs,
    HubSearchReport,
    hub_lock_report,
    hub_resolve_report,
    hub_search_report,
)
from .lineage import LineageAuditArgs, LineageAuditReport, lineage_audit_report
from .preanalytic import PreanalyticApplyArgs, PreanalyticApplyReport, preanalytic_apply_report
from .contradiction import ContradictionReviewArgs, ContradictionReviewReport, contradiction_review_report
from .lab import LabPlanReport, lab_plan_report
from .evaluation import (
    BioevalReferenceAuditReport,
    EvaluationReproductionReport,
    EvaluationTrajectoryReport,
    EvaluationWorldlineReport,
    OracleCombineReport,
    OracleMissingnessReport,
    OracleReferencePanelReport,
    bioeval_reference_audit_report,
    evaluation_reproduction_check_report,
    evaluation_trajectory_check_report,
    evaluation_worldline_audit_report,
    oracle_combine_report,
    oracle_missingness_report,
    oracle_reference_panel_report,
)
from .runtime import (
    RuntimeEffectCheckArgs,
    RuntimeEffectReport,
    RuntimeExecutionSimulateArgs,
    RuntimeExecutionSimulateReport,
    RuntimeTapeVerifyArgs,
    RuntimeTapeVerifyReport,
    runtime_effect_check_report,
    runtime_execution_simulate_report,
    runtime_tape_verify_report,
)
from .bioethics import (
    BioethicsActionReviewArgs,
    BioethicsActionReviewReport,
    BioethicsDualUseReviewArgs,
    BioethicsDualUseReviewReport,
    BioethicsRepresentationAuditArgs,
    BioethicsRepresentationAuditReport,
    BioethicsValidationCheckArgs,
    BioethicsValidationCheckReport,
    HumanSubjectScreenArgs,
    HumanSubjectScreenReport,
    bioethics_action_review_report,
    bioethics_dual_use_review_report,
    bioethics_representation_audit_report,
    bioethics_validation_check_report,
    human_subject_screen_report,
)
from .oncology import (
    OncoBoundaryArgs,
    OncoBoundaryReport,
    OncoClassificationArgs,
    OncoClassificationReport,
    OncoIdentityJoinArgs,
    OncoIdentityJoinReport,
    OncoOutcomeAnalyzeArgs,
    OncoOutcomeReport,
    OncoResponseAssessArgs,
    OncoResponseReport,
    OncoWorldlineReport,
    OncoWorldlineViewArgs,
    onco_boundary_report,
    onco_classification_report,
    onco_identity_join_report,
    onco_outcome_report,
    onco_response_report,
    onco_worldline_report,
)
from .oncoworlds import (
    OncoWorldsClonalHistoryCheckArgs,
    OncoWorldsClonalHistoryCheckReport,
    OncoWorldsMethylationClassifyArgs,
    OncoWorldsMethylationClassifyReport,
    OncoWorldsMethylationCompareArgs,
    OncoWorldsMethylationCompareReport,
    OncoWorldsModelTransportArgs,
    OncoWorldsModelTransportReport,
    OncoWorldsRadiogenomicCheckArgs,
    OncoWorldsRadiogenomicCheckReport,
    oncoworlds_clonal_history_check_report,
    oncoworlds_methylation_classify_report,
    oncoworlds_methylation_compare_report,
    oncoworlds_model_transport_report,
    oncoworlds_radiogenomic_check_report,
)
from .stress import (
    StressProfileArgs,
    StressProfileReport,
    StressReportArgs,
    StressReportProjection,
    stress_profile_report,
    stress_report_projection,
)
from .influence import InfluenceAnalysisReport, InfluenceAnalyzeArgs, influence_analysis_report
from .standards import MeasurementCompareArgs, MeasurementCompareReport, measurement_compare_report
from .workbench import WorkbenchRequest
from .world import (
    ObservedWorldDeclareArgs,
    ObservedWorldDeclareReport,
    WorldClaimCheckReport,
    observed_world_declare_report,
    world_claim_check_report,
)


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

    def biocapability_evidence_audit_report(
        self, request: BioCapabilityEvidenceAuditRequest
    ) -> BioCapabilityEvidenceAuditReport:
        """Return typed evidence states, claim blockers, and release posture."""

        return biocapability_evidence_audit_report(self.biocapability_evidence_audit(request))

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
        result = self.client.call_tool("world_claim_check", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def observed_world_declare(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and seal a pinned observed-world declaration."""

        normalized = request if isinstance(request, ObservedWorldDeclareArgs) else ObservedWorldDeclareArgs.from_wire(request)
        return self.tool("observed_world_declare", normalized.to_mcp_arguments())

    def observed_world_declare_report(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> ObservedWorldDeclareReport:
        """Return typed observed-world sources, design, provenance, and boundary counts."""

        return observed_world_declare_report(self.observed_world_declare(request))

    def world_claim_check_report(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> WorldClaimCheckReport:
        """Return typed grounded evidence or the kernel's fail-closed refusal."""

        return world_claim_check_report(self.world_claim_check(provenance, claim))

    def lineage_audit(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit specimen ancestry, material, artifact, and identity evidence with bounded output."""

        normalized = request if isinstance(request, LineageAuditArgs) else LineageAuditArgs.from_wire(request)
        return self.tool("lineage_audit", normalized.to_mcp_arguments())

    def lineage_audit_report(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> LineageAuditReport:
        """Return typed lineage findings while keeping missing identity evidence non-passing."""

        return lineage_audit_report(self.lineage_audit(request))

    def preanalytic_apply(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Apply a declared pre-analytic fault and preserve biological/refusal postconditions."""

        normalized = request if isinstance(request, PreanalyticApplyArgs) else PreanalyticApplyArgs.from_wire(request)
        result = self.client.call_tool("preanalytic_apply", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def preanalytic_apply_report(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> PreanalyticApplyReport:
        """Return typed admitted fault evidence or a fail-closed pre-analytic refusal."""

        return preanalytic_apply_report(self.preanalytic_apply(request))

    def contradiction_review(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Pose and review a contradiction without selecting a modality as the winner."""

        normalized = request if isinstance(request, ContradictionReviewArgs) else ContradictionReviewArgs.from_wire(request)
        result = self.client.call_tool("contradiction_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def contradiction_review_report(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> ContradictionReviewReport:
        """Return typed hypotheses, resolution state, next actions, cues, or refusal."""

        return contradiction_review_report(self.contradiction_review(request))

    def onco_boundary_check(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Apply the research-only oncology boundary, preserving partial release/refusal."""

        normalized = request if isinstance(request, OncoBoundaryArgs) else OncoBoundaryArgs.from_wire(request)
        result = self.client.call_tool("onco_boundary_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def onco_boundary_report(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> OncoBoundaryReport:
        """Return typed oncology research release, clinical refusal, and escalation evidence."""

        return onco_boundary_report(self.onco_boundary_check(request))

    def onco_response_assess(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Assess longitudinal response without turning unconfirmed change into progression."""

        normalized = request if isinstance(request, OncoResponseAssessArgs) else OncoResponseAssessArgs.from_wire(request)
        result = self.client.call_tool("onco_response_assess", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def onco_response_report(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> OncoResponseReport:
        return onco_response_report(self.onco_response_assess(request))

    def onco_worldline_view(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render biological/record order and apply an explicit agent-visibility cutoff."""

        normalized = request if isinstance(request, OncoWorldlineViewArgs) else OncoWorldlineViewArgs.from_wire(request)
        result = self.client.call_tool("onco_worldline_view", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def onco_worldline_report(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> OncoWorldlineReport:
        return onco_worldline_report(self.onco_worldline_view(request))

    def onco_classification_check(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run integrated classification while retaining unresolved assay obligations."""

        normalized = request if isinstance(request, OncoClassificationArgs) else OncoClassificationArgs.from_wire(request)
        result = self.client.call_tool("onco_classification_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def onco_classification_report(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> OncoClassificationReport:
        return onco_classification_report(self.onco_classification_check(request))

    def oncoworlds_identity_join(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Check identity evidence without collapsing a declined join into an exception."""

        normalized = request if isinstance(request, OncoIdentityJoinArgs) else OncoIdentityJoinArgs.from_wire(request)
        result = self.client.call_tool("oncoworlds_identity_join", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def oncoworlds_identity_join_report(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> OncoIdentityJoinReport:
        return onco_identity_join_report(self.oncoworlds_identity_join(request))

    def onco_outcome_analyze(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Analyze one follow-up record under a predeclared estimand."""

        normalized = request if isinstance(request, OncoOutcomeAnalyzeArgs) else OncoOutcomeAnalyzeArgs.from_wire(request)
        result = self.client.call_tool("onco_outcome_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def onco_outcome_report(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> OncoOutcomeReport:
        return onco_outcome_report(self.onco_outcome_analyze(request))

    def oncoworlds_model_transport(
        self,
        request: OncoWorldsModelTransportArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsModelTransportArgs) else OncoWorldsModelTransportArgs.from_wire(request)
        result = self.client.call_tool("oncoworlds_model_transport", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def oncoworlds_model_transport_report(
        self,
        request: OncoWorldsModelTransportArgs | Mapping[str, Any],
    ) -> OncoWorldsModelTransportReport:
        return oncoworlds_model_transport_report(self.oncoworlds_model_transport(request))

    def oncoworlds_methylation_classify(
        self,
        request: OncoWorldsMethylationClassifyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsMethylationClassifyArgs) else OncoWorldsMethylationClassifyArgs.from_wire(request)
        result = self.client.call_tool("oncoworlds_methylation_classify", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def oncoworlds_methylation_classify_report(
        self,
        request: OncoWorldsMethylationClassifyArgs | Mapping[str, Any],
    ) -> OncoWorldsMethylationClassifyReport:
        return oncoworlds_methylation_classify_report(self.oncoworlds_methylation_classify(request))

    def oncoworlds_methylation_compare(
        self,
        request: OncoWorldsMethylationCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsMethylationCompareArgs) else OncoWorldsMethylationCompareArgs.from_wire(request)
        return self.tool("oncoworlds_methylation_compare", normalized.to_mcp_arguments())

    def oncoworlds_methylation_compare_report(
        self,
        request: OncoWorldsMethylationCompareArgs | Mapping[str, Any],
    ) -> OncoWorldsMethylationCompareReport:
        return oncoworlds_methylation_compare_report(self.oncoworlds_methylation_compare(request))

    def oncoworlds_radiogenomic_check(
        self,
        request: OncoWorldsRadiogenomicCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsRadiogenomicCheckArgs) else OncoWorldsRadiogenomicCheckArgs.from_wire(request)
        result = self.client.call_tool("oncoworlds_radiogenomic_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def oncoworlds_radiogenomic_check_report(
        self,
        request: OncoWorldsRadiogenomicCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsRadiogenomicCheckReport:
        return oncoworlds_radiogenomic_check_report(self.oncoworlds_radiogenomic_check(request))

    def oncoworlds_clonal_history_check(
        self,
        request: OncoWorldsClonalHistoryCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsClonalHistoryCheckArgs) else OncoWorldsClonalHistoryCheckArgs.from_wire(request)
        return self.tool("oncoworlds_clonal_history_check", normalized.to_mcp_arguments())

    def oncoworlds_clonal_history_check_report(
        self,
        request: OncoWorldsClonalHistoryCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsClonalHistoryCheckReport:
        return oncoworlds_clonal_history_check_report(self.oncoworlds_clonal_history_check(request))

    def stress_profile(
        self,
        request: StressProfileArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, StressProfileArgs) else StressProfileArgs.from_wire(request)
        result = self.client.call_tool("stress_profile", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def stress_profile_report(
        self,
        request: StressProfileArgs | Mapping[str, Any],
    ) -> StressProfileReport:
        return stress_profile_report(self.stress_profile(request))

    def stress_report(
        self,
        request: StressReportArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, StressReportArgs) else StressReportArgs.from_wire(request)
        result = self.client.call_tool("stress_report", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def stress_report_projection(
        self,
        request: StressReportArgs | Mapping[str, Any],
    ) -> StressReportProjection:
        return stress_report_projection(self.stress_report(request))

    def influence_analyze(
        self,
        request: InfluenceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, InfluenceAnalyzeArgs) else InfluenceAnalyzeArgs.from_wire(request)
        result = self.client.call_tool("influence_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def influence_analysis_report(
        self,
        request: InfluenceAnalyzeArgs | Mapping[str, Any],
    ) -> InfluenceAnalysisReport:
        return influence_analysis_report(self.influence_analyze(request))

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

    def lab_plan_report(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> LabPlanReport:
        """Return typed ordered/excluded acquisition evidence and stop/escalation state."""

        return lab_plan_report(
            self.lab_plan(
                graph,
                actions,
                budget,
                marginal_value_floor=marginal_value_floor,
                hypotheses=hypotheses,
                observations=observations,
                max_items=max_items,
            )
        )

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

    def capability_audit_report(self, *, include_groups: bool = True) -> CapabilityAuditReport:
        """Return validated parity and schema-quality diagnostics for the capability catalogue."""

        return capability_audit_report(self.capability_audit(include_groups=include_groups))

    def capability_discover_report(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> CapabilitySearchReport:
        """Return a validated ranked projection with domains, tools, and schema attachments."""

        return capability_discover_report(
            self.capability_discover(
                query=query,
                text=text,
                domain=domain,
                tool=tool,
                group_id=group_id,
                max_items=max_items,
                include_tools=include_tools,
            )
        )

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

    def adapter_plan_report(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> AdapterPlanReport:
        """Return typed adapter candidates, dependencies, conformance, and loss boundaries."""

        return adapter_plan_report(
            self.adapter_plan(
                source_id,
                source_kind,
                declared_format=declared_format,
                required_conformance=required_conformance,
                available_dependencies=available_dependencies,
            )
        )

    def tabular_ingest(self, request: TabularIngestRequest) -> dict[str, Any]:
        """Execute the Rust CSV/TSV adapter with independent conformance and loss accounting."""

        if not isinstance(request, TabularIngestRequest):
            raise ArgumentError("request must be a TabularIngestRequest")
        return self.tool("tabular_ingest", request.to_mcp_arguments())

    def tabular_ingest_report(self, request: TabularIngestRequest) -> TabularIngestReport:
        """Return typed manifest, conformance, semantic-loss, and bounded fact evidence."""

        return tabular_ingest_report(self.tabular_ingest(request))

    def conformance_run(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Run the shipped fixture-verified conformance suite without mutating artifacts."""

        request = ConformanceRunArgs(include_details, max_items)
        return self.tool("conformance_run", request.to_mcp_arguments())

    def conformance_run_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> ConformanceRunReport:
        """Return typed suite, pyramid, case, and noncompensatory release evidence."""

        return conformance_run_report(
            self.conformance_run(include_details=include_details, max_items=max_items)
        )

    def release_audit(self, request: ReleaseAuditArgs) -> dict[str, Any]:
        """Compose bounded release gates while preserving advisory/refusal evidence."""

        if not isinstance(request, ReleaseAuditArgs):
            raise ArgumentError("request must be a ReleaseAuditArgs")
        return self.tool("release_audit", request.to_mcp_arguments())

    def release_audit_report(self, request: ReleaseAuditArgs) -> ReleaseAuditReport:
        """Return typed noncompensatory release readiness and delegated check evidence."""

        return release_audit_report(self.release_audit(request))

    def operations_catalog(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Inspect storage promises, service contracts, SLO names, and metric debt."""

        request = OperationsCatalogArgs(include_details, max_items)
        return self.tool("operations_catalog", request.to_mcp_arguments())

    def operations_catalog_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> OperationsCatalogReport:
        """Return typed operations topology, service, and metric evidence."""

        return operations_catalog_report(
            self.operations_catalog(include_details=include_details, max_items=max_items)
        )

    def ops_acceptance(self, *, max_items: int = 100) -> dict[str, Any]:
        """Run the closed alpha-acceptance predicate set with explicit unverifiable findings."""

        request = OpsAcceptanceArgs(max_items)
        return self.tool("ops_acceptance", request.to_mcp_arguments())

    def ops_acceptance_report(self, *, max_items: int = 100) -> OpsAcceptanceReport:
        """Return typed met/refuted/unverifiable operational acceptance evidence."""

        return ops_acceptance_report(self.ops_acceptance(max_items=max_items))

    def safety_release_gate(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Evaluate a complete reviewer-labelled safety release assessment."""

        if isinstance(assessment, SafetyReleaseGateArgs):
            request = assessment
        else:
            request = SafetyReleaseGateArgs(
                assessment if isinstance(assessment, RiskAssessmentRequest) else RiskAssessmentRequest.from_wire(assessment)
            )
        return self.tool("safety_release_gate", request.to_mcp_arguments())

    def safety_release_gate_report(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> SafetyReleaseGateReport:
        """Return typed fail-closed safety-gate evidence."""

        return safety_release_gate_report(self.safety_release_gate(assessment))

    def medical_boundary_check(self, request: MedicalBoundaryRequest) -> dict[str, Any]:
        """Check a research-only medical boundary and preserve structured clinical refusal."""

        if not isinstance(request, MedicalBoundaryRequest):
            raise ArgumentError("request must be a MedicalBoundaryRequest")
        result = self.client.call_tool("medical_boundary_check", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def medical_boundary_report(self, request: MedicalBoundaryRequest) -> MedicalBoundaryReport:
        """Return typed research admission or unconditional clinical refusal evidence."""

        return medical_boundary_report(self.medical_boundary_check(request))

    def safety_posture(self, *, include_threats: bool = False) -> dict[str, Any]:
        """Summarize section-13 threat populations without claiming runtime enforcement."""

        request = SafetyPostureArgs(include_threats)
        return self.tool("safety_posture", request.to_mcp_arguments())

    def safety_posture_report(self, *, include_threats: bool = False) -> SafetyPostureReport:
        """Return typed mitigated/declared-only/unmitigated and residual threat evidence."""

        return safety_posture_report(self.safety_posture(include_threats=include_threats))

    def measurement_compare(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compare standards-declared measurements without silent unit or ontology coercion."""

        normalized = request if isinstance(request, MeasurementCompareArgs) else MeasurementCompareArgs.from_wire(request)
        return self.tool("measurement_compare", normalized.to_mcp_arguments())

    def measurement_compare_report(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> MeasurementCompareReport:
        """Return typed comparability, conversion receipt, caveats, and blocking reason."""

        return measurement_compare_report(self.measurement_compare(request))

    def hub_search(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Search bounded federated catalogs while preserving authority and freshness evidence."""

        normalized = request if isinstance(request, HubSearchArgs) else HubSearchArgs.from_wire(request)
        return self.tool("hub_search", normalized.to_mcp_arguments())

    def hub_search_report(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> HubSearchReport:
        """Return typed matches, near misses, facet reasons, and provenance."""

        return hub_search_report(self.hub_search(request))

    def hub_resolve(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Resolve one federated pack while retaining exact subject and provenance."""

        normalized = request if isinstance(request, HubResolveArgs) else HubResolveArgs.from_wire(request)
        return self.tool("hub_resolve", normalized.to_mcp_arguments())

    def hub_resolve_report(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> HubResolveReport:
        """Return typed resolution subject, authority, freshness, policy, and lifecycle notes."""

        return hub_resolve_report(self.hub_resolve(request))

    def hub_lock(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Resolve a bounded transitive dependency closure with required-by provenance."""

        normalized = request if isinstance(request, HubLockArgs) else HubLockArgs.from_wire(request)
        return self.tool("hub_lock", normalized.to_mcp_arguments())

    def hub_lock_report(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> HubLockReport:
        """Return typed lock entries, digest provenance, lifecycle notes, and omission counts."""

        return hub_lock_report(self.hub_lock(request))

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
        result = self.client.call_tool("oracle_reference_panel", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def oracle_combine_report(
        self,
        subject: str,
        at: str,
        judgements: Sequence[Mapping[str, Any] | Any],
        *,
        minimum_deciding_tier: EvidenceTier | str = EvidenceTier.JUDGE,
        max_items: int = 100,
    ) -> OracleCombineReport:
        return oracle_combine_report(self.oracle_combine(subject, at, judgements, minimum_deciding_tier=minimum_deciding_tier, max_items=max_items))

    def oracle_reference_panel_report(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> OracleReferencePanelReport:
        return oracle_reference_panel_report(self.oracle_reference_panel(panel, rule=rule, model_call=model_call, max_items=max_items))

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

    def oracle_missingness_report(
        self,
        pattern: Mapping[str, Any],
        field: Mapping[str, Any],
        boundary: Mapping[str, Any],
        small_cell_floor: int,
        *,
        mechanism: Mapping[str, Any] | None = None,
    ) -> OracleMissingnessReport:
        return oracle_missingness_report(self.oracle_missingness(pattern, field, boundary, small_cell_floor, mechanism=mechanism))

    def bioeval_reference_audit(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> dict[str, Any]:
        return self.tool("bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments())

    def bioeval_reference_audit_report(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> BioevalReferenceAuditReport:
        return bioeval_reference_audit_report(self.bioeval_reference_audit(reference, state=state))

    def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return self.tool("evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments())

    def evaluation_worldline_audit_report(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> EvaluationWorldlineReport:
        return evaluation_worldline_audit_report(self.evaluation_worldline_audit(worldline, at=at))

    def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        result = self.client.call_tool(
            "evaluation_reproduction_check",
            EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments(),
        )
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def evaluation_reproduction_check_report(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> EvaluationReproductionReport:
        return evaluation_reproduction_check_report(self.evaluation_reproduction_check(reexecution, biological_claim=biological_claim))

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

    def evaluation_trajectory_check_report(
        self,
        trajectory: Mapping[str, Any],
        *,
        step: int | None = None,
        horizon: int | None = None,
    ) -> EvaluationTrajectoryReport:
        return evaluation_trajectory_check_report(self.evaluation_trajectory_check(trajectory, step=step, horizon=horizon))

    def runtime_effect_check(
        self,
        request: RuntimeEffectCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Inspect one runtime effect without touching a filesystem, network, or process."""

        normalized = request if isinstance(request, RuntimeEffectCheckArgs) else RuntimeEffectCheckArgs.from_wire(request)
        result = self.client.call_tool("runtime_effect_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def runtime_effect_check_report(
        self,
        request: RuntimeEffectCheckArgs | Mapping[str, Any],
    ) -> RuntimeEffectReport:
        return runtime_effect_check_report(self.runtime_effect_check(request))

    def runtime_tape_verify(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a hash-chained tape and retain checkpoint, artifact, and simulation evidence."""

        normalized = request if isinstance(request, RuntimeTapeVerifyArgs) else RuntimeTapeVerifyArgs.from_wire(request)
        result = self.client.call_tool("runtime_tape_verify", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def runtime_tape_verify_report(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> RuntimeTapeVerifyReport:
        return runtime_tape_verify_report(self.runtime_tape_verify(request))

    def runtime_execution_simulate(
        self,
        request: RuntimeExecutionSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Record and replay a bounded deterministic effect program with optional counterfactual fork."""

        normalized = request if isinstance(request, RuntimeExecutionSimulateArgs) else RuntimeExecutionSimulateArgs.from_wire(request)
        return self.tool("runtime_execution_simulate", normalized.to_mcp_arguments())

    def runtime_execution_simulate_report(
        self,
        request: RuntimeExecutionSimulateArgs | Mapping[str, Any],
    ) -> RuntimeExecutionSimulateReport:
        return runtime_execution_simulate_report(self.runtime_execution_simulate(request))

    def bioethics_action_review(
        self,
        request: BioethicsActionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsActionReviewArgs) else BioethicsActionReviewArgs.from_wire(request)
        result = self.client.call_tool("bioethics_action_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioethics_action_review_report(
        self,
        request: BioethicsActionReviewArgs | Mapping[str, Any],
    ) -> BioethicsActionReviewReport:
        return bioethics_action_review_report(self.bioethics_action_review(request))

    def human_subject_screen(
        self,
        request: HumanSubjectScreenArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, HumanSubjectScreenArgs) else HumanSubjectScreenArgs.from_wire(request)
        return self.tool("bioethics_human_subject_screen", normalized.to_mcp_arguments())

    def human_subject_screen_report(
        self,
        request: HumanSubjectScreenArgs | Mapping[str, Any],
    ) -> HumanSubjectScreenReport:
        return human_subject_screen_report(self.human_subject_screen(request))

    def bioethics_dual_use_review(
        self,
        request: BioethicsDualUseReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsDualUseReviewArgs) else BioethicsDualUseReviewArgs.from_wire(request)
        result = self.client.call_tool("bioethics_dual_use_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioethics_dual_use_review_report(
        self,
        request: BioethicsDualUseReviewArgs | Mapping[str, Any],
    ) -> BioethicsDualUseReviewReport:
        return bioethics_dual_use_review_report(self.bioethics_dual_use_review(request))

    def bioethics_validation_check(
        self,
        request: BioethicsValidationCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsValidationCheckArgs) else BioethicsValidationCheckArgs.from_wire(request)
        return self.tool("bioethics_validation_check", normalized.to_mcp_arguments())

    def bioethics_validation_check_report(
        self,
        request: BioethicsValidationCheckArgs | Mapping[str, Any],
    ) -> BioethicsValidationCheckReport:
        return bioethics_validation_check_report(self.bioethics_validation_check(request))

    def bioethics_representation_audit(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsRepresentationAuditArgs) else BioethicsRepresentationAuditArgs.from_wire(request)
        result = self.client.call_tool("bioethics_representation_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioethics_representation_audit_report(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> BioethicsRepresentationAuditReport:
        return bioethics_representation_audit_report(self.bioethics_representation_audit(request))

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

    def developer_delivery_audit_report(
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
    ) -> DeveloperDeliveryAuditReport:
        """Return typed cross-domain delivery gates and explicit release-target blockers."""

        return developer_delivery_audit_report(
            self.developer_delivery_audit(
                request_id=request_id,
                targets=targets,
                platform=platform,
                repository=repository,
                repository_impact=repository_impact,
                sdk=sdk,
                conformance=conformance,
                provider=provider,
                governance=governance,
                release=release,
            )
        )

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

    def bioatlas_publication_audit_report(
        self,
        atlas: Mapping[str, Any],
        **kwargs: Any,
    ) -> BioAtlasPublicationAuditReport:
        """Return typed atlas, evidence, card, leaderboard, and publication gates."""

        return bioatlas_publication_audit_report(
            self.bioatlas_publication_audit(atlas, **kwargs)
        )

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

    async def biocapability_evidence_audit_report(
        self, request: BioCapabilityEvidenceAuditRequest
    ) -> BioCapabilityEvidenceAuditReport:
        """Async typed evidence states, claim blockers, and release posture."""

        return biocapability_evidence_audit_report(
            await self.biocapability_evidence_audit(request)
        )

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
        result = await self.client.call_tool("world_claim_check", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def observed_world_declare(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.observed_world_declare`."""

        normalized = request if isinstance(request, ObservedWorldDeclareArgs) else ObservedWorldDeclareArgs.from_wire(request)
        return await self.tool("observed_world_declare", normalized.to_mcp_arguments())

    async def observed_world_declare_report(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> ObservedWorldDeclareReport:
        """Return typed async observed-world declaration evidence."""

        return observed_world_declare_report(await self.observed_world_declare(request))

    async def world_claim_check_report(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> WorldClaimCheckReport:
        """Return typed async grounded evidence or refusal."""

        return world_claim_check_report(await self.world_claim_check(provenance, claim))

    async def lineage_audit(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.lineage_audit`."""

        normalized = request if isinstance(request, LineageAuditArgs) else LineageAuditArgs.from_wire(request)
        return await self.tool("lineage_audit", normalized.to_mcp_arguments())

    async def lineage_audit_report(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> LineageAuditReport:
        return lineage_audit_report(await self.lineage_audit(request))

    async def preanalytic_apply(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.preanalytic_apply`."""

        normalized = request if isinstance(request, PreanalyticApplyArgs) else PreanalyticApplyArgs.from_wire(request)
        result = await self.client.call_tool("preanalytic_apply", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def preanalytic_apply_report(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> PreanalyticApplyReport:
        return preanalytic_apply_report(await self.preanalytic_apply(request))

    async def contradiction_review(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.contradiction_review`."""

        normalized = request if isinstance(request, ContradictionReviewArgs) else ContradictionReviewArgs.from_wire(request)
        result = await self.client.call_tool("contradiction_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def contradiction_review_report(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> ContradictionReviewReport:
        return contradiction_review_report(await self.contradiction_review(request))

    async def onco_boundary_check(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.onco_boundary_check`."""

        normalized = request if isinstance(request, OncoBoundaryArgs) else OncoBoundaryArgs.from_wire(request)
        result = await self.client.call_tool("onco_boundary_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def onco_boundary_report(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> OncoBoundaryReport:
        return onco_boundary_report(await self.onco_boundary_check(request))

    async def onco_response_assess(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.onco_response_assess`."""

        normalized = request if isinstance(request, OncoResponseAssessArgs) else OncoResponseAssessArgs.from_wire(request)
        result = await self.client.call_tool("onco_response_assess", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def onco_response_report(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> OncoResponseReport:
        return onco_response_report(await self.onco_response_assess(request))

    async def onco_worldline_view(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.onco_worldline_view`."""

        normalized = request if isinstance(request, OncoWorldlineViewArgs) else OncoWorldlineViewArgs.from_wire(request)
        result = await self.client.call_tool("onco_worldline_view", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def onco_worldline_report(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> OncoWorldlineReport:
        return onco_worldline_report(await self.onco_worldline_view(request))

    async def onco_classification_check(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.onco_classification_check`."""

        normalized = request if isinstance(request, OncoClassificationArgs) else OncoClassificationArgs.from_wire(request)
        result = await self.client.call_tool("onco_classification_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def onco_classification_report(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> OncoClassificationReport:
        return onco_classification_report(await self.onco_classification_check(request))

    async def oncoworlds_identity_join(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.oncoworlds_identity_join`."""

        normalized = request if isinstance(request, OncoIdentityJoinArgs) else OncoIdentityJoinArgs.from_wire(request)
        result = await self.client.call_tool("oncoworlds_identity_join", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def oncoworlds_identity_join_report(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> OncoIdentityJoinReport:
        return onco_identity_join_report(await self.oncoworlds_identity_join(request))

    async def onco_outcome_analyze(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.onco_outcome_analyze`."""

        normalized = request if isinstance(request, OncoOutcomeAnalyzeArgs) else OncoOutcomeAnalyzeArgs.from_wire(request)
        result = await self.client.call_tool("onco_outcome_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def onco_outcome_report(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> OncoOutcomeReport:
        return onco_outcome_report(await self.onco_outcome_analyze(request))

    async def oncoworlds_model_transport(
        self,
        request: OncoWorldsModelTransportArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsModelTransportArgs) else OncoWorldsModelTransportArgs.from_wire(request)
        result = await self.client.call_tool("oncoworlds_model_transport", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def oncoworlds_model_transport_report(
        self,
        request: OncoWorldsModelTransportArgs | Mapping[str, Any],
    ) -> OncoWorldsModelTransportReport:
        return oncoworlds_model_transport_report(await self.oncoworlds_model_transport(request))

    async def oncoworlds_methylation_classify(
        self,
        request: OncoWorldsMethylationClassifyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsMethylationClassifyArgs) else OncoWorldsMethylationClassifyArgs.from_wire(request)
        result = await self.client.call_tool("oncoworlds_methylation_classify", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def oncoworlds_methylation_classify_report(
        self,
        request: OncoWorldsMethylationClassifyArgs | Mapping[str, Any],
    ) -> OncoWorldsMethylationClassifyReport:
        return oncoworlds_methylation_classify_report(await self.oncoworlds_methylation_classify(request))

    async def oncoworlds_methylation_compare(
        self,
        request: OncoWorldsMethylationCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsMethylationCompareArgs) else OncoWorldsMethylationCompareArgs.from_wire(request)
        return await self.tool("oncoworlds_methylation_compare", normalized.to_mcp_arguments())

    async def oncoworlds_methylation_compare_report(
        self,
        request: OncoWorldsMethylationCompareArgs | Mapping[str, Any],
    ) -> OncoWorldsMethylationCompareReport:
        return oncoworlds_methylation_compare_report(await self.oncoworlds_methylation_compare(request))

    async def oncoworlds_radiogenomic_check(
        self,
        request: OncoWorldsRadiogenomicCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsRadiogenomicCheckArgs) else OncoWorldsRadiogenomicCheckArgs.from_wire(request)
        result = await self.client.call_tool("oncoworlds_radiogenomic_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def oncoworlds_radiogenomic_check_report(
        self,
        request: OncoWorldsRadiogenomicCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsRadiogenomicCheckReport:
        return oncoworlds_radiogenomic_check_report(await self.oncoworlds_radiogenomic_check(request))

    async def oncoworlds_clonal_history_check(
        self,
        request: OncoWorldsClonalHistoryCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsClonalHistoryCheckArgs) else OncoWorldsClonalHistoryCheckArgs.from_wire(request)
        return await self.tool("oncoworlds_clonal_history_check", normalized.to_mcp_arguments())

    async def oncoworlds_clonal_history_check_report(
        self,
        request: OncoWorldsClonalHistoryCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsClonalHistoryCheckReport:
        return oncoworlds_clonal_history_check_report(await self.oncoworlds_clonal_history_check(request))

    async def stress_profile(
        self,
        request: StressProfileArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, StressProfileArgs) else StressProfileArgs.from_wire(request)
        result = await self.client.call_tool("stress_profile", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def stress_profile_report(
        self,
        request: StressProfileArgs | Mapping[str, Any],
    ) -> StressProfileReport:
        return stress_profile_report(await self.stress_profile(request))

    async def stress_report(
        self,
        request: StressReportArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, StressReportArgs) else StressReportArgs.from_wire(request)
        result = await self.client.call_tool("stress_report", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def stress_report_projection(
        self,
        request: StressReportArgs | Mapping[str, Any],
    ) -> StressReportProjection:
        return stress_report_projection(await self.stress_report(request))

    async def influence_analyze(
        self,
        request: InfluenceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, InfluenceAnalyzeArgs) else InfluenceAnalyzeArgs.from_wire(request)
        result = await self.client.call_tool("influence_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def influence_analysis_report(
        self,
        request: InfluenceAnalyzeArgs | Mapping[str, Any],
    ) -> InfluenceAnalysisReport:
        return influence_analysis_report(await self.influence_analyze(request))

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

    async def lab_plan_report(
        self,
        graph: Mapping[str, Any] | LabPlanRequest,
        actions: Sequence[Mapping[str, Any]] | None = None,
        budget: Mapping[str, Any] | None = None,
        *,
        marginal_value_floor: float = 0.0,
        hypotheses: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
        max_items: int = 100,
    ) -> LabPlanReport:
        return lab_plan_report(
            await self.lab_plan(
                graph,
                actions,
                budget,
                marginal_value_floor=marginal_value_floor,
                hypotheses=hypotheses,
                observations=observations,
                max_items=max_items,
            )
        )

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

    async def capability_audit_report(self, *, include_groups: bool = True) -> CapabilityAuditReport:
        """Async typed parity and schema-quality diagnostics for the capability catalogue."""

        return capability_audit_report(
            await self.capability_audit(include_groups=include_groups)
        )

    async def capability_discover_report(
        self,
        *,
        query: CapabilityQuery | str | None = None,
        text: str | None = None,
        domain: str | None = None,
        tool: str | None = None,
        group_id: str | None = None,
        max_items: int = 50,
        include_tools: bool = False,
    ) -> CapabilitySearchReport:
        """Async typed ranked projection over the complete capability catalogue."""

        return capability_discover_report(
            await self.capability_discover(
                query=query,
                text=text,
                domain=domain,
                tool=tool,
                group_id=group_id,
                max_items=max_items,
                include_tools=include_tools,
            )
        )

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

    async def adapter_plan_report(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> AdapterPlanReport:
        """Async typed adapter candidates, dependencies, conformance, and loss boundaries."""

        return adapter_plan_report(
            await self.adapter_plan(
                source_id,
                source_kind,
                declared_format=declared_format,
                required_conformance=required_conformance,
                available_dependencies=available_dependencies,
            )
        )

    async def tabular_ingest(self, request: TabularIngestRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.tabular_ingest`."""

        if not isinstance(request, TabularIngestRequest):
            raise ArgumentError("request must be a TabularIngestRequest")
        return await self.tool("tabular_ingest", request.to_mcp_arguments())

    async def tabular_ingest_report(self, request: TabularIngestRequest) -> TabularIngestReport:
        """Return typed async tabular conformance and loss evidence."""

        return tabular_ingest_report(await self.tabular_ingest(request))

    async def conformance_run(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.conformance_run`."""

        request = ConformanceRunArgs(include_details, max_items)
        return await self.tool("conformance_run", request.to_mcp_arguments())

    async def conformance_run_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> ConformanceRunReport:
        """Return typed async conformance and release evidence."""

        return conformance_run_report(
            await self.conformance_run(include_details=include_details, max_items=max_items)
        )

    async def release_audit(self, request: ReleaseAuditArgs) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.release_audit`."""

        if not isinstance(request, ReleaseAuditArgs):
            raise ArgumentError("request must be a ReleaseAuditArgs")
        return await self.tool("release_audit", request.to_mcp_arguments())

    async def release_audit_report(self, request: ReleaseAuditArgs) -> ReleaseAuditReport:
        """Return typed async release gates and delegated evidence."""

        return release_audit_report(await self.release_audit(request))

    async def operations_catalog(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.operations_catalog`."""

        request = OperationsCatalogArgs(include_details, max_items)
        return await self.tool("operations_catalog", request.to_mcp_arguments())

    async def operations_catalog_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> OperationsCatalogReport:
        """Return typed async operations topology and metric evidence."""

        return operations_catalog_report(
            await self.operations_catalog(include_details=include_details, max_items=max_items)
        )

    async def ops_acceptance(self, *, max_items: int = 100) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.ops_acceptance`."""

        request = OpsAcceptanceArgs(max_items)
        return await self.tool("ops_acceptance", request.to_mcp_arguments())

    async def ops_acceptance_report(self, *, max_items: int = 100) -> OpsAcceptanceReport:
        """Return typed async acceptance evidence and decidability state."""

        return ops_acceptance_report(await self.ops_acceptance(max_items=max_items))

    async def safety_release_gate(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.safety_release_gate`."""

        if isinstance(assessment, SafetyReleaseGateArgs):
            request = assessment
        else:
            request = SafetyReleaseGateArgs(
                assessment if isinstance(assessment, RiskAssessmentRequest) else RiskAssessmentRequest.from_wire(assessment)
            )
        return await self.tool("safety_release_gate", request.to_mcp_arguments())

    async def safety_release_gate_report(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> SafetyReleaseGateReport:
        """Return typed async fail-closed safety-gate evidence."""

        return safety_release_gate_report(await self.safety_release_gate(assessment))

    async def medical_boundary_check(self, request: MedicalBoundaryRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.medical_boundary_check`."""

        if not isinstance(request, MedicalBoundaryRequest):
            raise ArgumentError("request must be a MedicalBoundaryRequest")
        result = await self.client.call_tool("medical_boundary_check", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def medical_boundary_report(self, request: MedicalBoundaryRequest) -> MedicalBoundaryReport:
        """Return typed async medical research admission/refusal evidence."""

        return medical_boundary_report(await self.medical_boundary_check(request))

    async def safety_posture(self, *, include_threats: bool = False) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.safety_posture`."""

        request = SafetyPostureArgs(include_threats)
        return await self.tool("safety_posture", request.to_mcp_arguments())

    async def safety_posture_report(self, *, include_threats: bool = False) -> SafetyPostureReport:
        """Return typed async section-13 threat posture evidence."""

        return safety_posture_report(await self.safety_posture(include_threats=include_threats))

    async def measurement_compare(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.measurement_compare`."""

        normalized = request if isinstance(request, MeasurementCompareArgs) else MeasurementCompareArgs.from_wire(request)
        return await self.tool("measurement_compare", normalized.to_mcp_arguments())

    async def measurement_compare_report(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> MeasurementCompareReport:
        """Return typed async measurement-comparability evidence."""

        return measurement_compare_report(await self.measurement_compare(request))

    async def hub_search(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.hub_search`."""

        normalized = request if isinstance(request, HubSearchArgs) else HubSearchArgs.from_wire(request)
        return await self.tool("hub_search", normalized.to_mcp_arguments())

    async def hub_search_report(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> HubSearchReport:
        """Return typed async federated hub-search evidence."""

        return hub_search_report(await self.hub_search(request))

    async def hub_resolve(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.hub_resolve`."""

        normalized = request if isinstance(request, HubResolveArgs) else HubResolveArgs.from_wire(request)
        return await self.tool("hub_resolve", normalized.to_mcp_arguments())

    async def hub_resolve_report(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> HubResolveReport:
        """Return typed async federated resolution evidence."""

        return hub_resolve_report(await self.hub_resolve(request))

    async def hub_lock(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.hub_lock`."""

        normalized = request if isinstance(request, HubLockArgs) else HubLockArgs.from_wire(request)
        return await self.tool("hub_lock", normalized.to_mcp_arguments())

    async def hub_lock_report(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> HubLockReport:
        """Return typed async dependency-lock evidence."""

        return hub_lock_report(await self.hub_lock(request))

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
        result = await self.client.call_tool("oracle_reference_panel", request.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def oracle_combine_report(
        self,
        subject: str,
        at: str,
        judgements: Sequence[Mapping[str, Any] | Any],
        *,
        minimum_deciding_tier: EvidenceTier | str = EvidenceTier.JUDGE,
        max_items: int = 100,
    ) -> OracleCombineReport:
        return oracle_combine_report(await self.oracle_combine(subject, at, judgements, minimum_deciding_tier=minimum_deciding_tier, max_items=max_items))

    async def oracle_reference_panel_report(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> OracleReferencePanelReport:
        return oracle_reference_panel_report(await self.oracle_reference_panel(panel, rule=rule, model_call=model_call, max_items=max_items))

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

    async def oracle_missingness_report(
        self,
        pattern: Mapping[str, Any],
        field: Mapping[str, Any],
        boundary: Mapping[str, Any],
        small_cell_floor: int,
        *,
        mechanism: Mapping[str, Any] | None = None,
    ) -> OracleMissingnessReport:
        return oracle_missingness_report(await self.oracle_missingness(pattern, field, boundary, small_cell_floor, mechanism=mechanism))

    async def bioeval_reference_audit(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> dict[str, Any]:
        return await self.tool(
            "bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments()
        )

    async def bioeval_reference_audit_report(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> BioevalReferenceAuditReport:
        return bioeval_reference_audit_report(await self.bioeval_reference_audit(reference, state=state))

    async def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return await self.tool(
            "evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments()
        )

    async def evaluation_worldline_audit_report(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> EvaluationWorldlineReport:
        return evaluation_worldline_audit_report(await self.evaluation_worldline_audit(worldline, at=at))

    async def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        result = await self.client.call_tool(
            "evaluation_reproduction_check",
            EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments(),
        )
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def evaluation_reproduction_check_report(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> EvaluationReproductionReport:
        return evaluation_reproduction_check_report(await self.evaluation_reproduction_check(reexecution, biological_claim=biological_claim))

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

    async def evaluation_trajectory_check_report(
        self,
        trajectory: Mapping[str, Any],
        *,
        step: int | None = None,
        horizon: int | None = None,
    ) -> EvaluationTrajectoryReport:
        return evaluation_trajectory_check_report(await self.evaluation_trajectory_check(trajectory, step=step, horizon=horizon))

    async def runtime_effect_check(
        self,
        request: RuntimeEffectCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, RuntimeEffectCheckArgs) else RuntimeEffectCheckArgs.from_wire(request)
        result = await self.client.call_tool("runtime_effect_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def runtime_effect_check_report(
        self,
        request: RuntimeEffectCheckArgs | Mapping[str, Any],
    ) -> RuntimeEffectReport:
        return runtime_effect_check_report(await self.runtime_effect_check(request))

    async def runtime_tape_verify(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, RuntimeTapeVerifyArgs) else RuntimeTapeVerifyArgs.from_wire(request)
        result = await self.client.call_tool("runtime_tape_verify", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def runtime_tape_verify_report(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> RuntimeTapeVerifyReport:
        return runtime_tape_verify_report(await self.runtime_tape_verify(request))

    async def runtime_execution_simulate(
        self,
        request: RuntimeExecutionSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, RuntimeExecutionSimulateArgs) else RuntimeExecutionSimulateArgs.from_wire(request)
        return await self.tool("runtime_execution_simulate", normalized.to_mcp_arguments())

    async def runtime_execution_simulate_report(
        self,
        request: RuntimeExecutionSimulateArgs | Mapping[str, Any],
    ) -> RuntimeExecutionSimulateReport:
        return runtime_execution_simulate_report(await self.runtime_execution_simulate(request))

    async def bioethics_action_review(
        self,
        request: BioethicsActionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsActionReviewArgs) else BioethicsActionReviewArgs.from_wire(request)
        result = await self.client.call_tool("bioethics_action_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioethics_action_review_report(
        self,
        request: BioethicsActionReviewArgs | Mapping[str, Any],
    ) -> BioethicsActionReviewReport:
        return bioethics_action_review_report(await self.bioethics_action_review(request))

    async def human_subject_screen(
        self,
        request: HumanSubjectScreenArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, HumanSubjectScreenArgs) else HumanSubjectScreenArgs.from_wire(request)
        return await self.tool("bioethics_human_subject_screen", normalized.to_mcp_arguments())

    async def human_subject_screen_report(
        self,
        request: HumanSubjectScreenArgs | Mapping[str, Any],
    ) -> HumanSubjectScreenReport:
        return human_subject_screen_report(await self.human_subject_screen(request))

    async def bioethics_dual_use_review(
        self,
        request: BioethicsDualUseReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsDualUseReviewArgs) else BioethicsDualUseReviewArgs.from_wire(request)
        result = await self.client.call_tool("bioethics_dual_use_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioethics_dual_use_review_report(
        self,
        request: BioethicsDualUseReviewArgs | Mapping[str, Any],
    ) -> BioethicsDualUseReviewReport:
        return bioethics_dual_use_review_report(await self.bioethics_dual_use_review(request))

    async def bioethics_validation_check(
        self,
        request: BioethicsValidationCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsValidationCheckArgs) else BioethicsValidationCheckArgs.from_wire(request)
        return await self.tool("bioethics_validation_check", normalized.to_mcp_arguments())

    async def bioethics_validation_check_report(
        self,
        request: BioethicsValidationCheckArgs | Mapping[str, Any],
    ) -> BioethicsValidationCheckReport:
        return bioethics_validation_check_report(await self.bioethics_validation_check(request))

    async def bioethics_representation_audit(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, BioethicsRepresentationAuditArgs) else BioethicsRepresentationAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioethics_representation_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioethics_representation_audit_report(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> BioethicsRepresentationAuditReport:
        return bioethics_representation_audit_report(await self.bioethics_representation_audit(request))

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

    async def developer_delivery_audit_report(
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
    ) -> DeveloperDeliveryAuditReport:
        """Async typed cross-domain delivery gates and release-target blockers."""

        return developer_delivery_audit_report(
            await self.developer_delivery_audit(
                request_id=request_id,
                targets=targets,
                platform=platform,
                repository=repository,
                repository_impact=repository_impact,
                sdk=sdk,
                conformance=conformance,
                provider=provider,
                governance=governance,
                release=release,
            )
        )

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

    async def bioatlas_publication_audit_report(
        self,
        atlas: Mapping[str, Any],
        **kwargs: Any,
    ) -> BioAtlasPublicationAuditReport:
        """Async typed atlas, evidence, card, leaderboard, and publication gates."""

        return bioatlas_publication_audit_report(
            await self.bioatlas_publication_audit(atlas, **kwargs)
        )

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
