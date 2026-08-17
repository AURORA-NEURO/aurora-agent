"""High-level helpers for the most important cross-domain MCP workflows."""

from __future__ import annotations

import asyncio
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
from .artifacts import (
    ArtifactCrossStoreAuditReport,
    ArtifactGetReport,
    ArtifactGetRequest,
    ArtifactLineageReport,
    ArtifactQueryReport,
    ArtifactQueryRequest,
    ArtifactRegistrationReport,
    ArtifactRegistrationRequest,
)
from .domain_reports import (
    DomainReportCoverageReport,
    DomainReportCoverageRequest,
    DomainReportProjectReport,
    DomainReportProjectRequest,
)
from .domain_evidence import (
    DomainEvidenceHarmonizationReport,
    DomainEvidenceHarmonizeRequest,
)
from .domain_evidence_intake import (
    DomainEvidenceIntakeCoverageReport,
    DomainEvidenceIntakeCoverageRequest,
    DomainEvidenceIntakeReport,
    DomainEvidenceIntakeRequest,
)
from .domain_evidence_source import (
    DomainEvidenceSourceExecutionReport,
    DomainEvidenceSourceExecutionRequest,
    DomainEvidenceSourcePlanReport,
    DomainEvidenceSourcePlanRequest,
)
from .adapter_runtime import AdapterRuntime
from .source_adapter import (
    SourceAdapterProjectionRequest,
    SourceAdapterProjectionResult,
    project_source_execution,
)
from .domain_evidence_pipeline import (
    DomainEvidencePipelineRequest,
    DomainEvidencePipelineResult,
    project_domain_source_execution,
)
from .domain_evidence_provider import (
    DomainEvidenceProviderNormalizationReport,
    DomainEvidenceProviderNormalizationRequest,
    DomainEvidenceProviderReplayRequest,
    DomainEvidenceProviderReplayVerificationReport,
    domain_evidence_provider_normalization_report,
    domain_evidence_provider_replay_verification_report,
)
from .domain_acquisition import (
    DOMAIN_ACQUISITION_WORKFLOW,
    DomainAcquisitionQuery,
    DomainAcquisitionReport,
    domain_acquisition_report,
)
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
    DomainWorkflowCatalogueReport,
    DomainWorkflowInstantiateRequest,
    DomainWorkflowInstantiationReport,
    DomainWorkflowScaffoldRequest,
    DomainWorkflowScaffoldReport,
    DomainWorkflowReconcileRequest,
    DomainWorkflowReconciliationReport,
    DomainWorkflowReconciliationImportRequest,
    DomainWorkflowReconciliationQueryRequest,
    DomainWorkflowReconciliationGetRequest,
    DomainWorkflowReconciliationImportReport,
    DomainWorkflowReconciliationQueryReport,
    DomainWorkflowReconciliationGetReport,
    MissionEvaluatorQuery,
    MissionEvaluatorReplayCompareRequest,
    MissionEvaluatorReplayComparisonReport,
    MissionEvaluatorReplayReport,
    MissionEvaluatorReplayRequest,
    MissionEvaluatorReviewReport,
    MissionEvaluatorReviewRequest,
    MissionEvidenceBundleVerificationReport,
    MissionEvidenceBundleVerifyRequest,
    MissionEvidenceBundleImportReport,
    MissionEvidenceBundleImportRequest,
    MissionEvidenceBundleQueryReport,
    MissionEvidenceBundleQueryRequest,
    MissionEvidenceBundleGetReport,
    MissionEvidenceBundleGetRequest,
    MissionEvaluatorSearchReport,
    capability_audit_report,
    capability_discover_report,
    capability_route_report,
    capability_route_review_report,
    domain_workflow_catalogue_report,
    domain_workflow_instantiation_report,
    domain_workflow_scaffold_report,
    domain_workflow_reconciliation_report,
    mission_evaluator_discover_report,
    mission_evaluator_review_report,
    mission_evaluator_replay_report,
    mission_evaluator_replay_comparison_report,
    mission_evidence_bundle_verification_report,
)
from .capability_dashboard import (
    CapabilityDashboardQueryArgs,
    CapabilityDashboardReport,
    capability_dashboard_report,
)
from .ci_evidence import (
    CiExecutionEvidenceReport,
    CiExecutionEvidenceRequest,
    ci_execution_evidence_report,
)
from .ci_provider import (
    CiProviderNormalizationReport,
    CiProviderNormalizationRequest,
    ci_provider_normalization_report,
)
from .ci_provider_evidence import (
    CiProviderEvidenceReport,
    CiProviderEvidenceRequest,
    ci_provider_evidence_report,
)
from .execution_provenance import (
    ExecutionProvenanceReport,
    ExecutionProvenanceRequest,
    execution_provenance_report,
)
from .delivery_receipt import (
    DeveloperDeliveryReceiptReport,
    DeveloperDeliveryReceiptRequest,
    DeveloperDeliveryReceiptVerificationReport,
    DeveloperDeliveryReceiptVerificationRequest,
    developer_delivery_receipt_report,
    developer_delivery_receipt_verification_report,
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
from .developer_platform import (
    DeveloperPlatformStatusArgs,
    DeveloperPlatformStatusReport,
    developer_platform_status_report,
)
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
from .telemetry import TelemetryProjectionReport, telemetry_project as telemetry_project_report
from .ledger import LedgerIngestArgs, LedgerIngestReport, ledger_ingest as ledger_ingest_report
from .trace_otel import TraceOtelIngestArgs, TraceOtelIngestReport, trace_otel_ingest as trace_otel_ingest_report
from .quality_gate import QualityGateRunArgs, QualityGateRunReport, quality_gate_run as quality_gate_run_report
from .atlas_report import AtlasReport, AtlasReportArgs, atlas_report as atlas_report_parser
from .atlas_surface import (
    AtlasSurfaceAuditArgs,
    AtlasSurfaceAuditReport,
    atlas_surface_audit_report,
)
from .engineering_manifest import (
    EngineeringAuditReport,
    EngineeringManifestArgs,
    engineering_manifest_audit_report,
)
from .engineering_plan import (
    EngineeringPlanReport,
    EngineeringPlanRequestArgs,
    engineering_execution_plan_report,
)
from .release_pipeline import (
    ReleasePipelineAuditReport,
    ReleasePipelineManifestArgs,
    release_pipeline_audit_report,
)
from .operational_readiness import (
    OperationalReadinessAuditReport,
    OperationalReadinessManifestArgs,
    operational_readiness_audit_report,
)
from .security_privacy import (
    SecurityPrivacyAuditReport,
    SecurityPrivacyManifestArgs,
    security_privacy_audit_report,
)
from .sandbox_admission import (
    SandboxAuditReport,
    SandboxManifestArgs,
    sandbox_admission_audit_report,
)
from .sandbox_runtime import (
    SandboxRuntimeAuditReport,
    SandboxRuntimeManifestArgs,
    sandbox_runtime_simulate_report,
)
from .security_program import (
    SecurityProgramAuditReport,
    SecurityProgramManifestArgs,
    security_program_audit_report,
)
from .adaptive_panel import AdaptivePanelReport, AdaptivePanelRunArgs, adaptive_panel_report
from .posterior_gate import PosteriorGateArgs, PosteriorGateReport, posterior_gate_report
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
from .obligation import ObligationGateCheckArgs, ObligationGateCheckReport, obligation_gate_check_report
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
    OncoClonalEvidenceCheckArgs,
    OncoWorldsClonalEvidenceCheckReport,
    OncoWorldsClonalHistoryCheckArgs,
    OncoWorldsClonalHistoryCheckReport,
    OncoWorldsEraShiftCheckArgs,
    OncoWorldsEraShiftCheckReport,
    OncoWorldsEquityCheckArgs,
    OncoWorldsEquityCheckReport,
    OncoWorldsEntityWorldCheckArgs,
    OncoWorldsEntityWorldCheckReport,
    OncoWorldsMethylationClassifyArgs,
    OncoWorldsMethylationClassifyReport,
    OncoWorldsMethylationCompareArgs,
    OncoWorldsMethylationCompareReport,
    OncoWorldsModelTransportArgs,
    OncoWorldsModelTransportReport,
    OncoWorldsRadiogenomicCheckArgs,
    OncoWorldsRadiogenomicCheckReport,
    oncoworlds_clonal_evidence_check_report,
    oncoworlds_clonal_history_check_report,
    oncoworlds_era_shift_check_report,
    oncoworlds_equity_check_report,
    oncoworlds_entity_world_check_report,
    oncoworlds_methylation_classify_report,
    oncoworlds_methylation_compare_report,
    oncoworlds_model_transport_report,
    oncoworlds_radiogenomic_check_report,
)
from .literature import LiteratureBindCheckArgs, LiteratureBindCheckReport, literature_bind_check_report
from .modality import ModalitySupportCheckArgs, ModalitySupportCheckReport, modality_support_check_report
from .transport import ModalityTransportCheckArgs, ModalityTransportCheckReport, modality_transport_check_report
from .comparability import ModalityComparabilityCheckArgs, ModalityComparabilityCheckReport, modality_comparability_check_report
from .stress import (
    StressProfileArgs,
    StressProfileReport,
    StressReportArgs,
    StressReportProjection,
    stress_profile_report,
    stress_report_projection,
)
from .influence import InfluenceAnalysisReport, InfluenceAnalyzeArgs, influence_analysis_report
from .routing import RoutingDecisionReport, routing_decision_report
from .routing_lab import RoutingLabRunArgs, RoutingLabRunReport, routing_lab_run_report
from .lab_pareto import LabParetoAuditArgs, LabParetoAuditReport, lab_pareto_audit_report
from .lab_branch import LabBranchAuditArgs, LabBranchAuditReport, lab_branch_audit_report
from .lab_holdout import LabHoldoutAuditArgs, LabHoldoutAuditReport, lab_holdout_audit_report
from .lab_evolution import LabEvolutionAuditArgs, LabEvolutionAuditReport, lab_evolution_audit_report
from .lab_space import LabSpaceAuditArgs, LabSpaceAuditReport, lab_space_audit_report
from .provider import ProviderCapabilityGateArgs, ProviderCapabilityGateReport, provider_capability_gate_report
from .sdk_registry import SdkRegistryCheckArgs, SdkRegistryCheckReport, sdk_registry_check_report
from .token_context import (
    TokenContextPlanArgs,
    TokenContextPlanningReport,
    token_context_plan_report,
)
from .weavelang import WeaveLangCompileArgs, WeaveLangCompileReport, weavelang_compile_report
from .epistemic import EpistemicVoiArgs, EpistemicVoiReport, epistemic_voi_report
from .epistemic_context import EpistemicContextAuditArgs, EpistemicContextAuditReport, epistemic_context_audit_report
from .epistemic_selection import EpistemicSelectionAuditArgs, EpistemicSelectionAuditReport, epistemic_selection_audit_report
from .bioeval_acquisition import BioevalAcquisitionAuditArgs, BioevalAcquisitionAuditReport, bioeval_acquisition_audit_report
from .bioeval_grounding import BioevalGroundingAuditArgs, BioevalGroundingAuditReport, bioeval_grounding_audit_report
from .bioeval_estimand import BioevalEstimandAuditArgs, BioevalEstimandAuditReport, bioeval_estimand_audit_report
from .bioeval_evaluator import BioevalEvaluatorAuditArgs, BioevalEvaluatorAuditReport, bioeval_evaluator_audit_report
from .bioeval_plane import BioevalPlaneAuditArgs, BioevalPlaneAuditReport, bioeval_plane_audit_report
from .bioeval_metamorphic import BioevalMetamorphicAuditArgs, BioevalMetamorphicAuditReport, bioeval_metamorphic_audit_report
from .bioeval_waiver import BioevalWaiverAuditArgs, BioevalWaiverAuditReport, bioeval_waiver_audit_report
from .bioeval_design import BioevalDesignAuditArgs, BioevalDesignAuditReport, bioeval_design_audit_report
from .bioeval_mesh import BioevalMeshAuditArgs, BioevalMeshAuditReport, bioeval_mesh_audit_report
from .bioeval_burden import BioevalBurdenAuditArgs, BioevalBurdenAuditReport, bioeval_burden_audit_report
from .bioeval_reveal import BioevalRevealAuditArgs, BioevalRevealAuditReport, bioeval_reveal_audit_report
from .bioeval_boundary import BioevalBoundaryAuditArgs, BioevalBoundaryAuditReport, bioeval_boundary_audit_report
from .benchmark_trace import BenchmarkTraceAnalyzeArgs, BenchmarkTraceAnalysisReport, benchmark_trace_analysis_report
from .benchmark_decision import BenchmarkDecisionAuditArgs, BenchmarkDecisionAuditReport, benchmark_decision_audit_report
from .benchmark_integrity import BenchmarkIntegrityAuditArgs, BenchmarkIntegrityAuditReport, benchmark_integrity_audit_report
from .benchmark_counterfactual import BenchmarkCounterfactualCheckArgs, BenchmarkCounterfactualCheckReport, benchmark_counterfactual_check_report
from .benchmark_oracle import BenchmarkOracleReviewArgs, BenchmarkOracleReviewReport, benchmark_oracle_review_report
from .benchmark_compile import BenchmarkCompileArgs, BenchmarkCompileReport, benchmark_compile_report
from .benchmark_compile_review import BenchmarkCompileReviewArgs, BenchmarkCompileReviewReport, benchmark_compile_review_report
from .foundation import FoundationContractCheckArgs, FoundationContractCheckReport, foundation_contract_check_report
from .pack_catalogue import PackCatalogueArgs, PackCatalogueReport, pack_catalogue_report
from .pack_coverage import PackCoverageAuditArgs, PackCoverageAuditReport, pack_coverage_audit_report
from .pack_release import PackReleaseAuditArgs, PackReleaseAuditReport, pack_release_audit_report
from .pack_health import PackHealthAssessArgs, PackHealthAssessmentReport, pack_health_assessment_report
from .security_redteam import SecurityRedteamReport, SecurityRedteamSimulateArgs, security_redteam_simulate_report
from .world_generation import WorldGenerateArgs, WorldGenerateReport, world_generate_report
from .factory_lifecycle import FactoryLifecycleReport, FactoryLifecycleSimulateArgs, factory_lifecycle_report
from .storage_lifecycle import StorageLifecycleReport, StorageLifecycleSimulateArgs, storage_lifecycle_report
from .registry_lifecycle import RegistryLifecycleReport, RegistryLifecycleSimulateArgs, registry_lifecycle_report
from .cache_invalidation import CacheInvalidationReport, CacheInvalidationSimulateArgs, cache_invalidation_report
from .hub_disclosure import HubDisclosureReviewArgs, HubDisclosureReviewReport, hub_disclosure_review
from .hub_card import HubCardRenderArgs, HubCardRenderReport, hub_card_render
from .hub_publication import BioAtlasPublicationAuditArgs, BioAtlasPublicationAuditReport, HubLeaderboardRenderArgs, HubLeaderboardRenderReport, bioatlas_publication_audit, hub_leaderboard_render
from .hub_submission import HubSubmissionReviewArgs, HubSubmissionReviewReport, hub_submission_review
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

    def pack_health_assess_report(
        self,
        pack: PackArtifact | Mapping[str, Any],
        observations: Mapping[str, Any],
        *,
        policy: Mapping[str, Any] | None = None,
    ) -> PackHealthAssessmentReport:
        """Return typed calibration, health findings, digest binding, and score-gate evidence."""

        artifact = pack if isinstance(pack, PackArtifact) else PackArtifact.from_document(pack)
        request = PackHealthAssessArgs(artifact.document, observations, policy)
        result = self.client.call_tool("pack_health_assess", request.to_mcp_arguments())
        return pack_health_assessment_report(result.require_object())

    def pack_catalogue(self, *, section: str | None = None, max_items: int | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        if section is not None:
            arguments["section"] = section
        if max_items is not None:
            arguments["max_items"] = max_items
        return self.tool("pack_catalogue", arguments)

    def security_redteam_simulate(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Replay the bounded section-13 safety workflow with explicit evidence planes."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        return self.tool("security_redteam_simulate", normalized.to_mcp_arguments())

    def security_redteam_simulate_report(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> SecurityRedteamReport:
        """Return typed regression, disclosure, boundary, incident, audit, and attestation evidence."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        result = self.client.call_tool("security_redteam_simulate", normalized.to_mcp_arguments())
        return security_redteam_simulate_report(result.require_object())

    def world_generate(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Generate a deterministic synthetic world/query pair through the Rust authority."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        return self.tool("world_generate", normalized.to_mcp_arguments())

    def world_generate_report(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> WorldGenerateReport:
        """Return digest, validation, structural-count, and optional-document evidence."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        result = self.client.call_tool("world_generate", normalized.to_mcp_arguments())
        return world_generate_report(result.require_object())

    def factory_lifecycle_simulate(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay the bounded typed factory lifecycle through the MCP authority."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        return self.tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())

    def factory_lifecycle_simulate_report(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> FactoryLifecycleReport:
        """Return ordered action refusals, recovery branches, and final job visibility."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        result = self.client.call_tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())
        return factory_lifecycle_report(result.require_object())

    def storage_lifecycle_simulate(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan and optionally apply deterministic storage tiering and quota accounting."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        return self.tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())

    def storage_lifecycle_simulate_report(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> StorageLifecycleReport:
        """Return typed tier transitions, quota usage, and fail-closed accounting rows."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        result = self.client.call_tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())
        return storage_lifecycle_report(result.require_object())

    def registry_lifecycle_simulate(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Replay the bounded content-addressed registry lifecycle."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        return self.tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())

    def registry_lifecycle_simulate_report(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> RegistryLifecycleReport:
        """Return typed pack preflight, integrity, append-only action, and continuation evidence."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        result = self.client.call_tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())
        return registry_lifecycle_report(result.require_object())

    def cache_invalidation_simulate(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan and optionally apply cache invalidation through the MCP authority."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        return self.tool("cache_invalidation_simulate", normalized.to_mcp_arguments())

    def cache_invalidation_simulate_report(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> CacheInvalidationReport:
        """Return typed cache keys, completeness, lookup misses, application, and reproof evidence."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        result = self.client.call_tool("cache_invalidation_simulate", normalized.to_mcp_arguments())
        return cache_invalidation_report(result.require_object())

    def hub_disclosure_review(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay digest-bound disclosure and headline-eligibility actions."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        return self.tool("hub_disclosure_review", normalized.to_mcp_arguments())

    def hub_disclosure_review_report(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> HubDisclosureReviewReport:
        """Return typed ratchets, contamination witnesses, caveats, and withheld headlines."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        result = self.client.call_tool("hub_disclosure_review", normalized.to_mcp_arguments())
        return hub_disclosure_review(result.require_object())

    def hub_card_render(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render a typed public-hub card with its score withheld by default."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        return self.tool("hub_card_render", normalized.to_mcp_arguments())

    def hub_card_render_report(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> HubCardRenderReport:
        """Return typed card state, score display, provenance, limitations, and gates."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        result = self.client.call_tool("hub_card_render", normalized.to_mcp_arguments())
        return hub_card_render(result.require_object())

    def hub_leaderboard_render(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render rankable and explicitly unranked public-hub entries."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        return self.tool("hub_leaderboard_render", normalized.to_mcp_arguments())

    def hub_leaderboard_render_report(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> HubLeaderboardRenderReport:
        """Return typed rankability, reasons, labels, counts, and headline nonclaims."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        result = self.client.call_tool("hub_leaderboard_render", normalized.to_mcp_arguments())
        return hub_leaderboard_render(result.require_object())

    def hub_submission_review(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Accept a public-hub submission and optionally replay moderation actions."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        return self.tool("hub_submission_review", normalized.to_mcp_arguments())

    def hub_submission_review_report(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> HubSubmissionReviewReport:
        """Return typed acceptance, moderation events, tombstones, and refusal stages."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        result = self.client.call_tool("hub_submission_review", normalized.to_mcp_arguments())
        return hub_submission_review(result.require_object())

    def bioatlas_publication_audit(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compose atlas, evidence, card, leaderboard, and explicit release gates."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        return self.tool("bioatlas_publication_audit", normalized.to_mcp_arguments())

    def bioatlas_publication_audit_report(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> BioAtlasPublicationAuditReport:
        """Return typed cross-layer publication readiness without implying network release."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        result = self.client.call_tool("bioatlas_publication_audit", normalized.to_mcp_arguments())
        return bioatlas_publication_audit(result.require_object())

    def pack_catalogue_report(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> PackCatalogueReport:
        """Return typed pack declarations, oracle ceilings, release waves, and duplicate reviews."""

        if request is not None:
            if section != "all" or max_items != 100:
                raise ArgumentError("request cannot be combined with section or max_items")
            normalized = request if isinstance(request, PackCatalogueArgs) else PackCatalogueArgs.from_wire(request)
        else:
            normalized = PackCatalogueArgs(section, max_items)
        return pack_catalogue_report(self.tool("pack_catalogue", normalized.to_mcp_arguments()))

    def pack_coverage_audit(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit benchmark-pack capability-family and domain coverage through workspace MCP."""

        normalized = request if isinstance(request, PackCoverageAuditArgs) else PackCoverageAuditArgs.from_wire(request or {})
        return self.tool("pack_coverage_audit", normalized.to_mcp_arguments())

    def pack_coverage_audit_report(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackCoverageAuditReport:
        """Return typed selected-portfolio coverage and gap evidence."""

        return pack_coverage_audit_report(self.pack_coverage_audit(request))

    def pack_release_audit(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit pack release sequencing through workspace MCP."""

        normalized = request if isinstance(request, PackReleaseAuditArgs) else PackReleaseAuditArgs.from_wire(request or {})
        return self.tool("pack_release_audit", normalized.to_mcp_arguments())

    def pack_release_audit_report(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackReleaseAuditReport:
        """Return typed release-order and unsequenced-pack evidence."""

        return pack_release_audit_report(self.pack_release_audit(request))

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

    def oncoworlds_clonal_evidence_check(
        self,
        request: OncoClonalEvidenceCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoClonalEvidenceCheckArgs) else OncoClonalEvidenceCheckArgs.from_wire(request)
        return self.tool("oncoworlds_clonal_evidence_check", normalized.to_mcp_arguments())

    def oncoworlds_clonal_evidence_check_report(
        self,
        request: OncoClonalEvidenceCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsClonalEvidenceCheckReport:
        return oncoworlds_clonal_evidence_check_report(self.oncoworlds_clonal_evidence_check(request))

    def oncoworlds_era_shift_check(
        self,
        request: OncoWorldsEraShiftCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEraShiftCheckArgs) else OncoWorldsEraShiftCheckArgs.from_wire(request)
        return self.tool("oncoworlds_era_shift_check", normalized.to_mcp_arguments())

    def oncoworlds_era_shift_check_report(
        self,
        request: OncoWorldsEraShiftCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEraShiftCheckReport:
        return oncoworlds_era_shift_check_report(self.oncoworlds_era_shift_check(request))

    def oncoworlds_equity_check(
        self,
        request: OncoWorldsEquityCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEquityCheckArgs) else OncoWorldsEquityCheckArgs.from_wire(request)
        return self.tool("oncoworlds_equity_check", normalized.to_mcp_arguments())

    def oncoworlds_equity_check_report(
        self,
        request: OncoWorldsEquityCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEquityCheckReport:
        return oncoworlds_equity_check_report(self.oncoworlds_equity_check(request))

    def oncoworlds_entity_world_check(
        self,
        request: OncoWorldsEntityWorldCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEntityWorldCheckArgs) else OncoWorldsEntityWorldCheckArgs.from_wire(request)
        return self.tool("oncoworlds_entity_world_check", normalized.to_mcp_arguments())

    def oncoworlds_entity_world_check_report(
        self,
        request: OncoWorldsEntityWorldCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEntityWorldCheckReport:
        return oncoworlds_entity_world_check_report(self.oncoworlds_entity_world_check(request))

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

    def routing_decision_report(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> RoutingDecisionReport:
        return routing_decision_report(self.routing_decide(fingerprint, evidence, policy, task_id=task_id))

    def routing_lab_run(
        self,
        request: RoutingLabRunArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the offline routing lab through workspace MCP."""

        normalized = request if isinstance(request, RoutingLabRunArgs) else RoutingLabRunArgs.from_wire(request)
        return self.tool("routing_lab_run", normalized.to_mcp_arguments())

    def routing_lab_run_report(
        self,
        request: RoutingLabRunArgs | Mapping[str, Any],
    ) -> RoutingLabRunReport:
        """Return typed holdout, regret, and calibration evidence."""

        return routing_lab_run_report(self.routing_lab_run(request))

    def lab_pareto_audit(
        self,
        request: LabParetoAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Build the offline inference-lab Pareto archive through workspace MCP."""

        normalized = request if isinstance(request, LabParetoAuditArgs) else LabParetoAuditArgs.from_wire(request)
        return self.tool("lab_pareto_audit", normalized.to_mcp_arguments())

    def lab_pareto_audit_report(
        self,
        request: LabParetoAuditArgs | Mapping[str, Any],
    ) -> LabParetoAuditReport:
        """Return typed front, archive, hole, and selection evidence."""

        return lab_pareto_audit_report(self.lab_pareto_audit(request))

    def lab_branch_audit(
        self,
        request: LabBranchAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit risk-triggered branch accounting through workspace MCP."""

        normalized = request if isinstance(request, LabBranchAuditArgs) else LabBranchAuditArgs.from_wire(request)
        return self.tool("lab_branch_audit", normalized.to_mcp_arguments())

    def lab_branch_audit_report(
        self,
        request: LabBranchAuditArgs | Mapping[str, Any],
    ) -> LabBranchAuditReport:
        """Return typed branch cost, catch, escape, and undetermined-risk evidence."""

        return lab_branch_audit_report(self.lab_branch_audit(request))

    def lab_holdout_audit(
        self,
        request: LabHoldoutAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the offline holdout and rollback audit through workspace MCP."""

        normalized = request if isinstance(request, LabHoldoutAuditArgs) else LabHoldoutAuditArgs.from_wire(request)
        return self.tool("lab_holdout_audit", normalized.to_mcp_arguments())

    def lab_holdout_audit_report(
        self,
        request: LabHoldoutAuditArgs | Mapping[str, Any],
    ) -> LabHoldoutAuditReport:
        """Return typed clean-measurement and contamination evidence."""

        return lab_holdout_audit_report(self.lab_holdout_audit(request))

    def lab_evolution_audit(
        self,
        request: LabEvolutionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Assemble and grade a benchmark-gated evolution card through workspace MCP."""

        normalized = request if isinstance(request, LabEvolutionAuditArgs) else LabEvolutionAuditArgs.from_wire(request)
        return self.tool("lab_evolution_audit", normalized.to_mcp_arguments())

    def lab_evolution_audit_report(
        self,
        request: LabEvolutionAuditArgs | Mapping[str, Any],
    ) -> LabEvolutionAuditReport:
        """Return typed clean-claim, contamination, and defeater evidence."""

        return lab_evolution_audit_report(self.lab_evolution_audit(request))

    def lab_space_audit(
        self,
        request: LabSpaceAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and inspect an immutable architecture space through workspace MCP."""

        normalized = request if isinstance(request, LabSpaceAuditArgs) else LabSpaceAuditArgs.from_wire(request)
        return self.tool("lab_space_audit", normalized.to_mcp_arguments())

    def lab_space_audit_report(
        self,
        request: LabSpaceAuditArgs | Mapping[str, Any],
    ) -> LabSpaceAuditReport:
        """Return typed candidate, lineage, and component-diff evidence."""

        return lab_space_audit_report(self.lab_space_audit(request))

    def provider_capability_gate(
        self,
        request: ProviderCapabilityGateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ProviderCapabilityGateArgs) else ProviderCapabilityGateArgs.from_wire(request)
        result = self.client.call_tool("provider_capability_gate", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def provider_capability_gate_report(
        self,
        request: ProviderCapabilityGateArgs | Mapping[str, Any],
    ) -> ProviderCapabilityGateReport:
        return provider_capability_gate_report(self.provider_capability_gate(request))

    def sdk_registry_check(
        self,
        request: SdkRegistryCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, SdkRegistryCheckArgs) else SdkRegistryCheckArgs.from_wire(request)
        result = self.client.call_tool("sdk_registry_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def sdk_registry_check_report(
        self,
        request: SdkRegistryCheckArgs | Mapping[str, Any],
    ) -> SdkRegistryCheckReport:
        return sdk_registry_check_report(self.sdk_registry_check(request))

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

    def obligation_gate_check(
        self,
        request: ObligationGateCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ObligationGateCheckArgs) else ObligationGateCheckArgs.from_wire(request)
        return self.tool("obligation_gate_check", normalized.to_mcp_arguments())

    def obligation_gate_check_report(
        self,
        request: ObligationGateCheckArgs | Mapping[str, Any],
    ) -> ObligationGateCheckReport:
        return obligation_gate_check_report(self.obligation_gate_check(request))

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

    def mission_evaluator_discover(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Discover explicit non-executing evaluator candidates through MCP."""

        normalized = request if isinstance(request, MissionEvaluatorQuery) else MissionEvaluatorQuery(**dict(request or {}))
        return self.tool("mission_evaluator_discover", normalized.to_mcp_arguments())

    def mission_evaluator_discover_report(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> MissionEvaluatorSearchReport:
        """Return typed, digest-bound evaluator candidate evidence through MCP."""

        return mission_evaluator_discover_report(self.mission_evaluator_discover(request))

    def mission_evaluator_review(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review explicit evaluator-to-claim bindings without executing domain tools."""

        normalized = request if isinstance(request, MissionEvaluatorReviewRequest) else MissionEvaluatorReviewRequest(**dict(request))
        return self.tool("mission_evaluator_review", normalized.to_mcp_arguments())

    def mission_evaluator_review_report(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReviewReport:
        """Return typed evaluator binding review evidence through MCP."""

        return mission_evaluator_review_report(self.mission_evaluator_review(request))

    def mission_evaluator_replay(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay retained mission evaluator lineage without executing domain tools."""

        normalized = request if isinstance(request, MissionEvaluatorReplayRequest) else MissionEvaluatorReplayRequest(**dict(request))
        return self.tool("mission_evaluator_replay", normalized.to_mcp_arguments())

    def mission_evaluator_replay_report(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayReport:
        """Return typed evaluator replay, fixture, and coverage evidence through MCP."""

        return mission_evaluator_replay_report(self.mission_evaluator_replay(request))

    def mission_evaluator_replay_compare(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compare retained replay evidence with the current evaluator catalogue through MCP."""

        normalized = request if isinstance(request, MissionEvaluatorReplayCompareRequest) else MissionEvaluatorReplayCompareRequest(**dict(request))
        return self.tool("mission_evaluator_replay_compare", normalized.to_mcp_arguments())

    def mission_evaluator_replay_compare_report(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed digest-drift and binding-compatibility evidence through MCP."""

        return mission_evaluator_replay_comparison_report(self.mission_evaluator_replay_compare(request))

    def mission_evidence_bundle_verify(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a portable mission evidence bundle through workspace MCP."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleVerifyRequest)
            else MissionEvidenceBundleVerifyRequest(**dict(request))
        )
        return self.tool("mission_evidence_bundle_verify", normalized.to_mcp_arguments())

    def mission_evidence_bundle_verification_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed workspace MCP mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(self.mission_evidence_bundle_verify(request))

    def mission_evidence_bundle_import(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Import one verified evidence bundle into the workspace MCP registry."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleImportRequest)
            else MissionEvidenceBundleImportRequest(**dict(request))
        )
        return self.tool("mission_evidence_bundle_import", normalized.to_mcp_arguments())

    def mission_evidence_bundle_import_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        return MissionEvidenceBundleImportReport.from_wire(self.mission_evidence_bundle_import(request))

    def mission_evidence_bundle_query(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Query deterministic mission/domain rows in the workspace MCP registry."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleQueryRequest)
            else MissionEvidenceBundleQueryRequest(**dict(request or {}))
        )
        return self.tool("mission_evidence_bundle_query", normalized.to_mcp_arguments())

    def mission_evidence_bundle_query_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        return MissionEvidenceBundleQueryReport.from_wire(self.mission_evidence_bundle_query(request))

    def mission_evidence_bundle_get(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        if isinstance(request, MissionEvidenceBundleGetRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvidenceBundleGetRequest(request)
        else:
            normalized = MissionEvidenceBundleGetRequest(**dict(request))
        return self.tool("mission_evidence_bundle_get", normalized.to_mcp_arguments())

    def mission_evidence_bundle_get_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        return MissionEvidenceBundleGetReport.from_wire(self.mission_evidence_bundle_get(request))

    def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        """Verify catalogue membership against the authoritative MCP schema set."""

        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return self.tool("capability_audit", {"include_groups": include_groups})

    def capability_audit_report(self, *, include_groups: bool = True) -> CapabilityAuditReport:
        """Return validated parity and schema-quality diagnostics for the capability catalogue."""

        return capability_audit_report(self.capability_audit(include_groups=include_groups))

    def capability_dashboard(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Return a bounded cross-domain surface dashboard through MCP."""

        normalized = request if isinstance(request, CapabilityDashboardQueryArgs) else CapabilityDashboardQueryArgs(**dict(request or {}))
        return self.tool("capability_dashboard", normalized.to_mcp_arguments())

    def capability_dashboard_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return typed callable, partial, declared-only, and surface-gap evidence."""

        return capability_dashboard_report(self.capability_dashboard(request))

    def ci_execution_evidence_audit(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Reconcile caller-supplied CI evidence against a regenerated workbench plan."""

        normalized = request if isinstance(request, CiExecutionEvidenceRequest) else CiExecutionEvidenceRequest(**dict(request))
        return self.tool("ci_execution_evidence_audit", normalized.to_mcp_arguments())

    def ci_execution_evidence_report(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> CiExecutionEvidenceReport:
        """Return typed structural CI evidence and release-candidate findings."""

        return ci_execution_evidence_report(self.ci_execution_evidence_audit(request))

    def ci_provider_normalize(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Normalize a caller-supplied provider payload into canonical CI evidence."""

        normalized = request if isinstance(request, CiProviderNormalizationRequest) else CiProviderNormalizationRequest(**dict(request))
        return self.tool("ci_provider_normalize", normalized.to_mcp_arguments())

    def ci_provider_normalization_report(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> CiProviderNormalizationReport:
        """Return typed provider-normalization evidence and derived-digest warnings."""

        return ci_provider_normalization_report(self.ci_provider_normalize(request))

    def ci_provider_evidence_audit(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit provider-bound artifact, log, and attestation rows through MCP."""

        normalized = request if isinstance(request, CiProviderEvidenceRequest) else CiProviderEvidenceRequest(**dict(request))
        return self.tool("ci_provider_evidence_audit", normalized.to_mcp_arguments())

    def ci_provider_evidence_report(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> CiProviderEvidenceReport:
        """Return typed structural provider-evidence conformance evidence."""

        return ci_provider_evidence_report(self.ci_provider_evidence_audit(request))

    def execution_provenance_audit(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Reconcile a mission report and delegated checks without replaying them."""

        normalized = request if isinstance(request, ExecutionProvenanceRequest) else ExecutionProvenanceRequest(**dict(request))
        return self.tool("execution_provenance_audit", normalized.to_mcp_arguments())

    def execution_provenance_report(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> ExecutionProvenanceReport:
        """Return typed mission/delegated-check provenance evidence."""

        return execution_provenance_report(self.execution_provenance_audit(request))

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

    def domain_workflow_catalogue(self) -> dict[str, Any]:
        """Return the complete deterministic workflow-template catalogue."""

        return self.tool("domain_workflow_catalogue", {})

    def domain_workflow_catalogue_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(self.domain_workflow_catalogue())

    def domain_workflow_scaffold(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowScaffoldRequest)
            else DomainWorkflowScaffoldRequest(**dict(request))
        )
        return self.tool("domain_workflow_scaffold", normalized.to_arguments())

    def domain_workflow_scaffold_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(self.domain_workflow_scaffold(request))

    def domain_workflow_instantiate(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowInstantiateRequest)
            else DomainWorkflowInstantiateRequest(**dict(request))
        )
        return self.tool("domain_workflow_instantiate", normalized.to_arguments())

    def domain_workflow_instantiation_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(self.domain_workflow_instantiate(request))

    def domain_workflow_reconcile(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconcileRequest)
            else DomainWorkflowReconcileRequest(**dict(request))
        )
        return self.tool("domain_workflow_reconcile", normalized.to_arguments())

    def domain_workflow_reconciliation_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(self.domain_workflow_reconcile(request))

    def domain_workflow_reconciliation_import(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationImportRequest)
            else DomainWorkflowReconciliationImportRequest(**dict(request))
        )
        return self.tool("domain_workflow_reconciliation_import", normalized.to_arguments())

    def domain_workflow_reconciliation_import_report(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationImportReport:
        return DomainWorkflowReconciliationImportReport.from_wire(
            self.domain_workflow_reconciliation_import(request)
        )

    def domain_workflow_reconciliation_query(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationQueryRequest)
            else DomainWorkflowReconciliationQueryRequest(**dict(request or {}))
        )
        return self.tool("domain_workflow_reconciliation_query", normalized.to_arguments())

    def domain_workflow_reconciliation_query_report(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainWorkflowReconciliationQueryReport:
        return DomainWorkflowReconciliationQueryReport.from_wire(
            self.domain_workflow_reconciliation_query(request)
        )

    def domain_workflow_reconciliation_get(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationGetRequest)
            else DomainWorkflowReconciliationGetRequest(request)
            if isinstance(request, str)
            else DomainWorkflowReconciliationGetRequest(**dict(request))
        )
        return self.tool("domain_workflow_reconciliation_get", normalized.to_arguments())

    def domain_workflow_reconciliation_get_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            self.domain_workflow_reconciliation_get(request)
        )

    def artifact_registry_audit(
        self,
        arguments: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Register, query, fetch, or traverse bounded cross-domain artifacts through MCP."""

        return self.tool("artifact_registry_audit", arguments)

    def domain_report_project(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainReportProjectRequest)
            else DomainReportProjectRequest(**dict(request))
        )
        return self.tool("domain_report_project", normalized.to_arguments())

    def domain_report_project_report(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        return DomainReportProjectReport.from_wire(self.domain_report_project(request))

    def domain_report_coverage(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainReportCoverageRequest)
            else DomainReportCoverageRequest(**dict(request or {}))
        )
        return self.tool("domain_report_project", normalized.to_arguments())

    def domain_report_coverage_report(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        return DomainReportCoverageReport.from_wire(self.domain_report_coverage(request))

    def domain_evidence_harmonize(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizeRequest)
            else DomainEvidenceHarmonizeRequest(**dict(request))
        )
        return self.tool("domain_evidence_harmonize", normalized.to_arguments())

    def domain_evidence_harmonize_report(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        return DomainEvidenceHarmonizationReport.from_wire(self.domain_evidence_harmonize(request))

    def domain_evidence_intake(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeRequest)
            else DomainEvidenceIntakeRequest(**dict(request))
        )
        return self.tool("domain_evidence_intake", normalized.to_arguments())

    def domain_evidence_intake_report(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        return DomainEvidenceIntakeReport.from_wire(self.domain_evidence_intake(request))

    def domain_evidence_coverage(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeCoverageRequest)
            else DomainEvidenceIntakeCoverageRequest(**dict(request or {}))
        )
        return self.tool("domain_evidence_coverage", normalized.to_arguments())

    def domain_evidence_coverage_report(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        return DomainEvidenceIntakeCoverageReport.from_wire(self.domain_evidence_coverage(request))

    def domain_evidence_source_plan(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourcePlanRequest)
            else DomainEvidenceSourcePlanRequest(**dict(request))
        )
        return self.tool("domain_evidence_source_plan", normalized.to_arguments())

    def domain_evidence_source_plan_report(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        return DomainEvidenceSourcePlanReport.from_wire(self.domain_evidence_source_plan(request))

    def domain_evidence_source_execute(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourceExecutionRequest)
            else DomainEvidenceSourceExecutionRequest(**dict(request))
        )
        return self.tool("domain_evidence_source_execute", normalized.to_arguments())

    def domain_evidence_source_execute_report(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        return DomainEvidenceSourceExecutionReport.from_wire(self.domain_evidence_source_execute(request))

    def domain_evidence_source_project(
        self,
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: SourceAdapterProjectionRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> SourceAdapterProjectionResult:
        """Project a returned bounded source envelope through one local Python adapter."""

        normalized_execution = (
            execution.to_dict()
            if isinstance(execution, DomainEvidenceSourceExecutionReport)
            else dict(execution)
        )
        normalized_request = (
            request
            if isinstance(request, SourceAdapterProjectionRequest)
            else SourceAdapterProjectionRequest(**dict(request))
        )
        return project_source_execution(normalized_execution, normalized_request, runtime=runtime)

    def domain_evidence_source_project_for_domain(
        self,
        catalogue: DomainAcquisitionReport | Mapping[str, Any],
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: DomainEvidencePipelineRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> DomainEvidencePipelineResult:
        """Project a source envelope only when its adapter is declared for the selected domain."""

        normalized_request = (
            request
            if isinstance(request, DomainEvidencePipelineRequest)
            else DomainEvidencePipelineRequest(**dict(request))
        )
        normalized_execution = (
            execution.to_dict()
            if isinstance(execution, DomainEvidenceSourceExecutionReport)
            else dict(execution)
        )
        return project_domain_source_execution(catalogue, normalized_execution, normalized_request, runtime=runtime)

    def domain_evidence_provider_normalize(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderNormalizationRequest)
            else DomainEvidenceProviderNormalizationRequest(**dict(request))
        )
        return self.tool("domain_evidence_provider_normalize", normalized.to_mcp_arguments())

    def domain_evidence_provider_normalization_report(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderNormalizationReport:
        return domain_evidence_provider_normalization_report(self.domain_evidence_provider_normalize(request))

    def domain_evidence_provider_replay_verify(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderReplayRequest)
            else DomainEvidenceProviderReplayRequest(**dict(request))
        )
        return self.tool("domain_evidence_provider_replay_verify", normalized.to_mcp_arguments())

    def domain_evidence_provider_replay_verification_report(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        return domain_evidence_provider_replay_verification_report(
            self.domain_evidence_provider_replay_verify(request)
        )

    def artifact_cross_store_audit(self) -> ArtifactCrossStoreAuditReport:
        """Audit exact identity agreement across the bounded artifact stores."""

        return ArtifactCrossStoreAuditReport.from_wire(
            self.artifact_registry_audit({"operation": "cross_store"})
        )

    def artifact_register(
        self,
        request: ArtifactRegistrationRequest | Mapping[str, Any],
    ) -> ArtifactRegistrationReport:
        normalized = (
            request
            if isinstance(request, ArtifactRegistrationRequest)
            else ArtifactRegistrationRequest(**dict(request))
        )
        return ArtifactRegistrationReport.from_wire(
            self.artifact_registry_audit(
                {"operation": "register", "registration": normalized.to_arguments()}
            )
        )

    def artifact_query(
        self,
        request: ArtifactQueryRequest | Mapping[str, Any] | None = None,
    ) -> ArtifactQueryReport:
        normalized = (
            request
            if isinstance(request, ArtifactQueryRequest)
            else ArtifactQueryRequest(**dict(request or {}))
        )
        return ArtifactQueryReport.from_wire(
            self.artifact_registry_audit({"operation": "query", **normalized.to_arguments()})
        )

    def artifact_get(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactGetReport:
        normalized = (
            request
            if isinstance(request, ArtifactGetRequest)
            else ArtifactGetRequest(request)
            if isinstance(request, str)
            else ArtifactGetRequest(**dict(request))
        )
        return ArtifactGetReport.from_wire(
            self.artifact_registry_audit(
                {"operation": "get", "content_digest": normalized.content_digest}
            )
        )

    def artifact_lineage(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactLineageReport:
        normalized = (
            request
            if isinstance(request, ArtifactGetRequest)
            else ArtifactGetRequest(request)
            if isinstance(request, str)
            else ArtifactGetRequest(**dict(request))
        )
        return ArtifactLineageReport.from_wire(
            self.artifact_registry_audit(
                {"operation": "lineage", "content_digest": normalized.content_digest}
            )
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

    def domain_acquisition_catalogue(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> dict[str, Any]:
        """Build the digest-bound acquisition and adapter route catalogue."""

        normalized = query or DomainAcquisitionQuery()
        if not isinstance(normalized, DomainAcquisitionQuery):
            raise TypeError("query must be a DomainAcquisitionQuery")
        return self.tool(DOMAIN_ACQUISITION_WORKFLOW, normalized.to_mcp_arguments())

    def domain_acquisition_catalogue_report(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> DomainAcquisitionReport:
        """Return typed acquisition routes with separate transport and interpretation planes."""

        return domain_acquisition_report(self.domain_acquisition_catalogue(query))

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

    def literature_bind_check(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, LiteratureBindCheckArgs) else LiteratureBindCheckArgs.from_wire(request)
        return self.tool("literature_bind_check", normalized.to_mcp_arguments())

    def literature_bind_check_report(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> LiteratureBindCheckReport:
        return literature_bind_check_report(self.literature_bind_check(request))

    def modality_support_check(
        self,
        request: ModalitySupportCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalitySupportCheckArgs) else ModalitySupportCheckArgs.from_wire(request)
        return self.tool("modality_support_check", normalized.to_mcp_arguments())

    def modality_support_check_report(
        self,
        request: ModalitySupportCheckArgs | Mapping[str, Any],
    ) -> ModalitySupportCheckReport:
        return modality_support_check_report(self.modality_support_check(request))

    def modality_transport_check(
        self,
        request: ModalityTransportCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalityTransportCheckArgs) else ModalityTransportCheckArgs.from_wire(request)
        return self.tool("modality_transport_check", normalized.to_mcp_arguments())

    def modality_transport_check_report(
        self,
        request: ModalityTransportCheckArgs | Mapping[str, Any],
    ) -> ModalityTransportCheckReport:
        return modality_transport_check_report(self.modality_transport_check(request))

    def modality_comparability_check(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalityComparabilityCheckArgs) else ModalityComparabilityCheckArgs.from_wire(request)
        return self.tool("modality_comparability_check", normalized.to_mcp_arguments())

    def modality_comparability_check_report(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> ModalityComparabilityCheckReport:
        return modality_comparability_check_report(self.modality_comparability_check(request))

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

    def bioeval_acquisition_audit(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit a declared acquisition trace through workspace MCP."""

        normalized = request if isinstance(request, BioevalAcquisitionAuditArgs) else BioevalAcquisitionAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_acquisition_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_acquisition_audit_report(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> BioevalAcquisitionAuditReport:
        """Return typed obligation, stopping, redundancy, and named-regret evidence."""

        return bioeval_acquisition_audit_report(self.bioeval_acquisition_audit(request))

    def bioeval_grounding_audit(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit a claim-evidence graph through workspace MCP."""

        normalized = request if isinstance(request, BioevalGroundingAuditArgs) else BioevalGroundingAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_grounding_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_grounding_audit_report(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> BioevalGroundingAuditReport:
        """Return typed claim-state, locator, staleness, and lineage evidence."""

        return bioeval_grounding_audit_report(self.bioeval_grounding_audit(request))

    def bioeval_estimand_audit(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit an estimand, claim language, identification posture, and transport scope."""

        normalized = request if isinstance(request, BioevalEstimandAuditArgs) else BioevalEstimandAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_estimand_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_estimand_audit_report(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> BioevalEstimandAuditReport:
        """Return typed estimand and identification evidence."""

        return bioeval_estimand_audit_report(self.bioeval_estimand_audit(request))

    def bioeval_evaluator_audit(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit evaluator health separately from task outcomes through workspace MCP."""

        normalized = request if isinstance(request, BioevalEvaluatorAuditArgs) else BioevalEvaluatorAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_evaluator_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_evaluator_audit_report(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> BioevalEvaluatorAuditReport:
        """Return typed evaluator-health, task-evidence, and hidden-data findings."""

        return bioeval_evaluator_audit_report(self.bioeval_evaluator_audit(request))

    def bioeval_plane_audit(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit scored, unscored, and inapplicable cells through workspace MCP."""

        normalized = request if isinstance(request, BioevalPlaneAuditArgs) else BioevalPlaneAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_plane_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_plane_audit_report(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> BioevalPlaneAuditReport:
        """Return typed fold posture and scoring-plane findings."""

        return bioeval_plane_audit_report(self.bioeval_plane_audit(request))

    def bioeval_metamorphic_audit(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit mutation-response families through workspace MCP."""

        normalized = request if isinstance(request, BioevalMetamorphicAuditArgs) else BioevalMetamorphicAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_metamorphic_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_metamorphic_audit_report(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> BioevalMetamorphicAuditReport:
        """Return typed metamorphic failure directions and oracle-quality findings."""

        return bioeval_metamorphic_audit_report(self.bioeval_metamorphic_audit(request))

    def bioeval_waiver_audit(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit release-gate waivers through workspace MCP."""

        normalized = request if isinstance(request, BioevalWaiverAuditArgs) else BioevalWaiverAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_waiver_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_waiver_audit_report(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> BioevalWaiverAuditReport:
        """Return typed release-gate verdicts, waiver evidence, and blockers."""

        return bioeval_waiver_audit_report(self.bioeval_waiver_audit(request))

    def bioeval_design_audit(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit factorial arms and component contrasts through workspace MCP."""

        normalized = request if isinstance(request, BioevalDesignAuditArgs) else BioevalDesignAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_design_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_design_audit_report(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> BioevalDesignAuditReport:
        """Return typed factorial coverage, contrasts, and attribution evidence."""

        return bioeval_design_audit_report(self.bioeval_design_audit(request))

    def bioeval_mesh_audit(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit evaluator independence and disagreement classes through workspace MCP."""

        normalized = request if isinstance(request, BioevalMeshAuditArgs) else BioevalMeshAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_mesh_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_mesh_audit_report(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> BioevalMeshAuditReport:
        """Return typed independence classes, disagreement witnesses, and abstentions."""

        return bioeval_mesh_audit_report(self.bioeval_mesh_audit(request))

    def bioeval_burden_audit(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit nonrenewable resources, inherited residuals, and branch feasibility."""

        normalized = request if isinstance(request, BioevalBurdenAuditArgs) else BioevalBurdenAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_burden_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_burden_audit_report(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> BioevalBurdenAuditReport:
        """Return typed resource, draw, residual, and fork evidence."""

        return bioeval_burden_audit_report(self.bioeval_burden_audit(request))

    def bioeval_reveal_audit(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit prospective commitments, seal locks, and rubric integrity."""

        normalized = request if isinstance(request, BioevalRevealAuditArgs) else BioevalRevealAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_reveal_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_reveal_audit_report(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> BioevalRevealAuditReport:
        """Return typed prospective evaluation evidence."""

        return bioeval_reveal_audit_report(self.bioeval_reveal_audit(request))

    def bioeval_boundary_audit(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit contextual-integrity authorization, denial, violation, and veto states."""

        normalized = request if isinstance(request, BioevalBoundaryAuditArgs) else BioevalBoundaryAuditArgs.from_wire(request)
        result = self.client.call_tool("bioeval_boundary_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def bioeval_boundary_audit_report(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> BioevalBoundaryAuditReport:
        """Return typed flow verdicts, channel exposure, and Pareto posture."""

        return bioeval_boundary_audit_report(self.bioeval_boundary_audit(request))

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
        ci_evidence: CiExecutionEvidenceRequest | Mapping[str, Any] | None = None,
        ci_provider: CiProviderNormalizationRequest | Mapping[str, Any] | None = None,
        ci_provider_evidence: CiProviderEvidenceRequest | Mapping[str, Any] | None = None,
        execution_provenance: ExecutionProvenanceRequest | Mapping[str, Any] | None = None,
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
        if ci_evidence is not None:
            normalized_ci = (
                ci_evidence
                if isinstance(ci_evidence, CiExecutionEvidenceRequest)
                else CiExecutionEvidenceRequest(**dict(ci_evidence))
            )
            arguments["ci_evidence"] = normalized_ci.to_mcp_arguments()
        if ci_provider is not None:
            normalized_provider = (
                ci_provider
                if isinstance(ci_provider, CiProviderNormalizationRequest)
                else CiProviderNormalizationRequest(**dict(ci_provider))
            )
            arguments["ci_provider"] = normalized_provider.to_mcp_arguments()
        if ci_provider_evidence is not None:
            normalized_provider_evidence = (
                ci_provider_evidence
                if isinstance(ci_provider_evidence, CiProviderEvidenceRequest)
                else CiProviderEvidenceRequest(**dict(ci_provider_evidence))
            )
            arguments["ci_provider_evidence"] = normalized_provider_evidence.to_mcp_arguments()
        if execution_provenance is not None:
            normalized_provenance = (
                execution_provenance
                if isinstance(execution_provenance, ExecutionProvenanceRequest)
                else ExecutionProvenanceRequest(**dict(execution_provenance))
            )
            arguments["execution_provenance"] = normalized_provenance.to_mcp_arguments()
        release_request = _targets(request_id, targets)
        if release_request is not None:
            arguments["release_request"] = release_request
        return self.tool("developer_delivery_audit", arguments)

    def developer_delivery_receipt(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Create a content-addressed receipt from a recomputed delivery audit."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptRequest) else DeveloperDeliveryReceiptRequest.from_wire(request)
        return self.tool("developer_delivery_receipt", normalized.to_mcp_arguments())

    def developer_delivery_receipt_verify(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a stored delivery receipt against its completed audit."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptVerificationRequest) else DeveloperDeliveryReceiptVerificationRequest.from_wire(request)
        return self.tool("developer_delivery_receipt_verify", normalized.to_mcp_arguments())

    def developer_platform_status(
        self,
        request: DeveloperPlatformStatusArgs | Mapping[str, Any] | None = None,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Run the bounded developer-platform contract against the local MCP server."""

        if request is not None:
            if include_details is not False or max_items != 100:
                raise ArgumentError("request cannot be combined with include_details or max_items")
            normalized = request if isinstance(request, DeveloperPlatformStatusArgs) else DeveloperPlatformStatusArgs.from_wire(request)
        else:
            normalized = DeveloperPlatformStatusArgs(include_details, max_items)
        return self.tool("developer_platform_status", normalized.to_mcp_arguments())

    def developer_platform_status_report(
        self,
        request: DeveloperPlatformStatusArgs | Mapping[str, Any] | None = None,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> DeveloperPlatformStatusReport:
        """Return typed walkthrough, cookbook, diagnostic, and change-impact evidence."""

        return developer_platform_status_report(
            self.developer_platform_status(
                request, include_details=include_details, max_items=max_items
            )
        )

    def token_context_plan(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a bounded token context without resolving payloads or executing tools."""

        normalized = request if isinstance(request, TokenContextPlanArgs) else TokenContextPlanArgs.from_wire(request)
        return self.tool("token_context_plan", normalized.to_mcp_arguments())

    def token_context_plan_report(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> TokenContextPlanningReport:
        """Return typed estimates, mandatory closure, handles, and policy comparison evidence."""

        return token_context_plan_report(self.token_context_plan(request))

    def weavelang_compile(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Compile WeaveLang and optionally run only its local semantic machine."""

        if isinstance(request, str):
            normalized = WeaveLangCompileArgs(request)
        elif isinstance(request, WeaveLangCompileArgs):
            normalized = request
        else:
            normalized = WeaveLangCompileArgs.from_wire(request)
        return self.tool("weavelang_compile", normalized.to_mcp_arguments())

    def weavelang_compile_report(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> WeaveLangCompileReport:
        """Return typed IR identity, replay posture, liveness, and invariant evidence."""

        return weavelang_compile_report(self.weavelang_compile(request))

    def epistemic_voi(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Price one explicit acquisition or a bounded non-adaptive bundle.

        A structured ``ok=false`` value-of-information refusal is returned as data so callers can
        inspect the fail-closed reason; transport-level MCP errors still raise normally.
        """

        normalized = request if isinstance(request, EpistemicVoiArgs) else EpistemicVoiArgs.from_wire(request)
        result = self.client.call_tool("epistemic_voi", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def epistemic_voi_report(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> EpistemicVoiReport:
        """Return typed gross/cost/net, action-change, bundle, and refusal evidence."""

        return epistemic_voi_report(self.epistemic_voi(request))

    def epistemic_context_audit(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit decision-relative context compression through workspace MCP."""

        normalized = request if isinstance(request, EpistemicContextAuditArgs) else EpistemicContextAuditArgs.from_wire(request)
        result = self.client.call_tool("epistemic_context_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def epistemic_context_audit_report(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> EpistemicContextAuditReport:
        """Return typed frontier, sufficiency, identification, and subset evidence."""

        return epistemic_context_audit_report(self.epistemic_context_audit(request))

    def epistemic_selection_audit(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run bounded observed-evidence selection through workspace MCP."""

        normalized = request if isinstance(request, EpistemicSelectionAuditArgs) else EpistemicSelectionAuditArgs.from_wire(request)
        result = self.client.call_tool("epistemic_selection_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def epistemic_selection_audit_report(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> EpistemicSelectionAuditReport:
        """Return typed selections, guarantee applicability, submodularity, and exactness."""

        return epistemic_selection_audit_report(self.epistemic_selection_audit(request))

    def benchmark_trace_analyze(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Analyze a failing trace without replaying tools or assigning fabricated blame."""

        normalized = request if isinstance(request, BenchmarkTraceAnalyzeArgs) else BenchmarkTraceAnalyzeArgs.from_wire(request)
        result = self.client.call_tool("benchmark_trace_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_trace_analysis_report(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> BenchmarkTraceAnalysisReport:
        """Return typed causal, boundary, episode, repetition, and refusal evidence."""

        return benchmark_trace_analysis_report(self.benchmark_trace_analyze(request))

    def benchmark_decision_audit(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit one decision through the workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkDecisionAuditArgs) else BenchmarkDecisionAuditArgs.from_wire(request)
        result = self.client.call_tool("benchmark_decision_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_decision_audit_report(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> BenchmarkDecisionAuditReport:
        """Return typed decision-cell, firewall, and failure-card evidence."""

        return benchmark_decision_audit_report(self.benchmark_decision_audit(request))

    def benchmark_integrity_audit(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit benchmark portfolio integrity through the workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkIntegrityAuditArgs) else BenchmarkIntegrityAuditArgs.from_wire(request)
        result = self.client.call_tool("benchmark_integrity_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_integrity_audit_report(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> BenchmarkIntegrityAuditReport:
        """Return typed portfolio integrity evidence."""

        return benchmark_integrity_audit_report(self.benchmark_integrity_audit(request))

    def benchmark_counterfactual_check(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and contrast matched DecisionCells through the workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkCounterfactualCheckArgs) else BenchmarkCounterfactualCheckArgs.from_wire(request)
        result = self.client.call_tool("benchmark_counterfactual_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_counterfactual_check_report(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> BenchmarkCounterfactualCheckReport:
        """Return typed matched-pair and contrast evidence."""

        return benchmark_counterfactual_check_report(self.benchmark_counterfactual_check(request))

    def benchmark_oracle_review(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review, grade, and optionally package a benchmark oracle through the workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkOracleReviewArgs) else BenchmarkOracleReviewArgs.from_wire(request)
        result = self.client.call_tool("benchmark_oracle_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_oracle_review_report(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> BenchmarkOracleReviewReport:
        """Return typed oracle review-gate, grade, and packaging evidence."""

        return benchmark_oracle_review_report(self.benchmark_oracle_review(request))

    def benchmark_compile(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the non-executing assembled benchmark compiler through workspace MCP."""

        normalized = request if isinstance(request, BenchmarkCompileArgs) else BenchmarkCompileArgs.from_wire(request)
        result = self.client.call_tool("benchmark_compile", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_compile_report(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReport:
        """Return typed benchmark compiler pipeline evidence."""

        return benchmark_compile_report(self.benchmark_compile(request))

    def benchmark_compile_review(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the complete reviewed benchmark-cell workflow through workspace MCP."""

        normalized = request if isinstance(request, BenchmarkCompileReviewArgs) else BenchmarkCompileReviewArgs.from_wire(request)
        result = self.client.call_tool("benchmark_compile_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def benchmark_compile_review_report(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReviewReport:
        """Return typed reviewed benchmark-cell evidence."""

        return benchmark_compile_review_report(self.benchmark_compile_review(request))

    def foundation_contract_check(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate independent foundation contract gates without executing a world."""

        normalized = request if isinstance(request, FoundationContractCheckArgs) else FoundationContractCheckArgs.from_wire(request)
        result = self.client.call_tool("foundation_contract_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    def foundation_contract_check_report(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> FoundationContractCheckReport:
        """Return typed admissibility, refinement, applicability, world, and plane gates."""

        return foundation_contract_check_report(self.foundation_contract_check(request))

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
        ci_evidence: CiExecutionEvidenceRequest | Mapping[str, Any] | None = None,
        ci_provider: CiProviderNormalizationRequest | Mapping[str, Any] | None = None,
        ci_provider_evidence: CiProviderEvidenceRequest | Mapping[str, Any] | None = None,
        execution_provenance: ExecutionProvenanceRequest | Mapping[str, Any] | None = None,
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
                ci_evidence=ci_evidence,
                ci_provider=ci_provider,
                ci_provider_evidence=ci_provider_evidence,
                execution_provenance=execution_provenance,
            )
        )

    def developer_delivery_receipt_report(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptReport:
        """Return typed target/evidence digests and structural receipt readiness."""

        return developer_delivery_receipt_report(self.developer_delivery_receipt(request))

    def developer_delivery_receipt_verification_report(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptVerificationReport:
        """Return typed receipt digest and projection mismatch evidence."""

        return developer_delivery_receipt_verification_report(self.developer_delivery_receipt_verify(request))

    def bioatlas_publication_audit(
        self,
        atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs,
        *,
        weighting: Mapping[str, Any] | None = None,
        evidence_audit: Mapping[str, Any] | None = None,
        card: Mapping[str, Any] | None = None,
        leaderboard: Mapping[str, Any] | None = None,
        request_id: str | None = None,
        targets: Sequence[str] | None = None,
        max_items: int | None = None,
    ) -> dict[str, Any]:
        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if any(value is not None for value in (weighting, evidence_audit, card, leaderboard, request_id, targets, max_items)):
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return self.tool("bioatlas_publication_audit", atlas.to_mcp_arguments())
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
        atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs,
        **kwargs: Any,
    ) -> BioAtlasPublicationAuditReport:
        """Return typed atlas, evidence, card, leaderboard, and publication gates."""

        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return bioatlas_publication_audit(self.bioatlas_publication_audit(atlas))
        return bioatlas_publication_audit_report(self.bioatlas_publication_audit(atlas, **kwargs))

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

    def telemetry_project_report(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> TelemetryProjectionReport:
        """Project telemetry and return validated loss/metric evidence."""

        return telemetry_project_report(self.telemetry_project(event, policy, trace, metric=metric, observations=observations))

    def ledger_ingest(self, request: LedgerIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Append a bounded event stream through the MCP ledger tool."""

        normalized = request if isinstance(request, LedgerIngestArgs) else LedgerIngestArgs.from_wire(request)
        return self.tool("ledger_ingest", normalized.to_mcp_arguments())

    def ledger_ingest_report(self, request: LedgerIngestArgs | Mapping[str, Any]) -> LedgerIngestReport:
        """Return typed admission, chain, cut, quarantine, and projection evidence."""

        return ledger_ingest_report(self.ledger_ingest(request))

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

    def trace_otel_ingest_report(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> TraceOtelIngestReport:
        """Import OTLP JSON and return typed mapping, loss, and compilation evidence."""

        normalized = request if isinstance(request, TraceOtelIngestArgs) else TraceOtelIngestArgs.from_wire(request)
        return trace_otel_ingest_report(self.tool("trace_otel_ingest", normalized.to_mcp_arguments()))

    def quality_gate_run(self, request: QualityGateRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Run a serialized bounded data-quality gate through MCP."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return self.tool("quality_gate_run", normalized.to_mcp_arguments())

    def quality_gate_run_report(self, request: QualityGateRunArgs | Mapping[str, Any]) -> QualityGateRunReport:
        """Return typed pass, witness, and not-runnable quality evidence."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return quality_gate_run_report(self.tool("quality_gate_run", normalized.to_mcp_arguments()))

    def atlas_report(self, request: AtlasReportArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Run bounded capability-atlas coverage reporting through MCP."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return self.tool("atlas_report", normalized.to_mcp_arguments())

    def atlas_report_typed(self, request: AtlasReportArgs | Mapping[str, Any]) -> AtlasReport:
        """Return typed coverage debt, holes, histograms, and composite evidence."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return atlas_report_parser(self.tool("atlas_report", normalized.to_mcp_arguments()))

    def atlas_surface_audit(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit atlasx coverage debt, failure browsing, and denominator-safe rates through MCP."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return self.tool("atlas_surface_audit", normalized.to_mcp_arguments())

    def atlas_surface_audit_report(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> AtlasSurfaceAuditReport:
        """Return typed atlasx debt discharge, visibility, rate, and surface-soundness evidence."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return atlas_surface_audit_report(self.tool("atlas_surface_audit", normalized.to_mcp_arguments()))

    def engineering_manifest_audit(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit package topology, tickets, ADR history, and ownership rows through MCP."""

        normalized = request if isinstance(request, EngineeringManifestArgs) else EngineeringManifestArgs.from_wire(request)
        return self.tool("engineering_manifest_audit", normalized.to_mcp_arguments())

    def engineering_manifest_audit_report(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> EngineeringAuditReport:
        """Return typed engineering-manifest coherence and readiness evidence."""

        return engineering_manifest_audit_report(self.engineering_manifest_audit(request))

    def engineering_execution_plan(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Derive bounded dependency waves and a critical path through MCP."""

        normalized = request if isinstance(request, EngineeringPlanRequestArgs) else EngineeringPlanRequestArgs.from_wire(request)
        return self.tool("engineering_execution_plan", normalized.to_mcp_arguments())

    def engineering_execution_plan_report(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> EngineeringPlanReport:
        """Return typed engineering execution waves, gates, and blockers."""

        return engineering_execution_plan_report(self.engineering_execution_plan(request))

    def release_pipeline_audit(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit release stages, provenance, promotion policy, and rollback boundaries through MCP."""

        normalized = request if isinstance(request, ReleasePipelineManifestArgs) else ReleasePipelineManifestArgs.from_wire(request)
        return self.tool("release_pipeline_audit", normalized.to_mcp_arguments())

    def release_pipeline_audit_report(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> ReleasePipelineAuditReport:
        """Return typed release readiness, artifact evidence, and production promotion blockers."""

        return release_pipeline_audit_report(self.release_pipeline_audit(request))

    def operational_readiness_audit(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit objectives, observations, fallbacks, runbooks, incidents, and controls through MCP."""

        normalized = request if isinstance(request, OperationalReadinessManifestArgs) else OperationalReadinessManifestArgs.from_wire(request)
        return self.tool("operational_readiness_audit", normalized.to_mcp_arguments())

    def operational_readiness_audit_report(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> OperationalReadinessAuditReport:
        """Return typed operational-readiness evidence and production-operability blockers."""

        return operational_readiness_audit_report(self.operational_readiness_audit(request))

    def security_privacy_audit(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit data governance, authorized flows, identity hardening, threats, reviews, and controls through MCP."""

        normalized = request if isinstance(request, SecurityPrivacyManifestArgs) else SecurityPrivacyManifestArgs.from_wire(request)
        return self.tool("security_privacy_audit", normalized.to_mcp_arguments())

    def security_privacy_audit_report(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> SecurityPrivacyAuditReport:
        """Return typed security/privacy governance evidence and blockers."""

        return security_privacy_audit_report(self.security_privacy_audit(request))

    def sandbox_admission_audit(self, request: SandboxManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit artifact identity, sandbox isolation, capabilities, resources, and output release."""

        normalized = request if isinstance(request, SandboxManifestArgs) else SandboxManifestArgs.from_wire(request)
        return self.tool("sandbox_admission_audit", normalized.to_mcp_arguments())

    def sandbox_admission_audit_report(self, request: SandboxManifestArgs | Mapping[str, Any]) -> SandboxAuditReport:
        """Return typed sandbox admission evidence and fail-closed blockers."""

        return sandbox_admission_audit_report(self.sandbox_admission_audit(request))

    def sandbox_runtime_simulate(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Simulate bounded sandbox requests and preserve capability/resource refusals."""

        normalized = request if isinstance(request, SandboxRuntimeManifestArgs) else SandboxRuntimeManifestArgs.from_wire(request)
        return self.tool("sandbox_runtime_simulate", normalized.to_mcp_arguments())

    def sandbox_runtime_simulate_report(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> SandboxRuntimeAuditReport:
        """Return typed sandbox runtime decisions, usage, and limitations."""

        return sandbox_runtime_simulate_report(self.sandbox_runtime_simulate(request))

    def security_program_audit(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit authorized scope, red-team evidence, remediation, incidents, disclosure, and controls."""

        normalized = request if isinstance(request, SecurityProgramManifestArgs) else SecurityProgramManifestArgs.from_wire(request)
        return self.tool("security_program_audit", normalized.to_mcp_arguments())

    def security_program_audit_report(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> SecurityProgramAuditReport:
        """Return typed security-program evidence and fail-closed blockers."""

        return security_program_audit_report(self.security_program_audit(request))

    def adaptive_panel(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit and query a serialized adaptive evaluation panel through MCP."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return self.tool("adaptive_panel", normalized.to_mcp_arguments())

    def adaptive_panel_report(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> AdaptivePanelReport:
        """Return typed clustered audit, selection, stopping, and estimate evidence."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return adaptive_panel_report(self.tool("adaptive_panel", normalized.to_mcp_arguments()))

    def posterior_gate(self, request: PosteriorGateArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Build a capability posterior and optional release/comparison projections through MCP."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return self.tool("posterior_gate", normalized.to_mcp_arguments())

    def posterior_gate_report(self, request: PosteriorGateArgs | Mapping[str, Any]) -> PosteriorGateReport:
        """Return typed clustered capabilities, fail-closed gate state, and dominance evidence."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return posterior_gate_report(self.tool("posterior_gate", normalized.to_mcp_arguments()))


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

    async def pack_health_assess_report(
        self,
        pack: PackArtifact | Mapping[str, Any],
        observations: Mapping[str, Any],
        *,
        policy: Mapping[str, Any] | None = None,
    ) -> PackHealthAssessmentReport:
        """Async typed counterpart to :meth:`Workspace.pack_health_assess_report`."""

        artifact = pack if isinstance(pack, PackArtifact) else PackArtifact.from_document(pack)
        request = PackHealthAssessArgs(artifact.document, observations, policy)
        result = await self.client.call_tool("pack_health_assess", request.to_mcp_arguments())
        return pack_health_assessment_report(result.require_object())

    async def pack_catalogue(self, *, section: str | None = None, max_items: int | None = None) -> dict[str, Any]:
        arguments: dict[str, Any] = {}
        if section is not None:
            arguments["section"] = section
        if max_items is not None:
            arguments["max_items"] = max_items
        return await self.tool("pack_catalogue", arguments)

    async def security_redteam_simulate(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.security_redteam_simulate`."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        return await self.tool("security_redteam_simulate", normalized.to_mcp_arguments())

    async def security_redteam_simulate_report(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> SecurityRedteamReport:
        """Async typed section-13 safety evidence."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        result = await self.client.call_tool("security_redteam_simulate", normalized.to_mcp_arguments())
        return security_redteam_simulate_report(result.require_object())

    async def world_generate(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.world_generate`."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        return await self.tool("world_generate", normalized.to_mcp_arguments())

    async def world_generate_report(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> WorldGenerateReport:
        """Async typed world-generation evidence."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        result = await self.client.call_tool("world_generate", normalized.to_mcp_arguments())
        return world_generate_report(result.require_object())

    async def factory_lifecycle_simulate(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.factory_lifecycle_simulate`."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        return await self.tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())

    async def factory_lifecycle_simulate_report(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> FactoryLifecycleReport:
        """Async typed factory lifecycle evidence."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        result = await self.client.call_tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())
        return factory_lifecycle_report(result.require_object())

    async def storage_lifecycle_simulate(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.storage_lifecycle_simulate`."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        return await self.tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())

    async def storage_lifecycle_simulate_report(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> StorageLifecycleReport:
        """Async typed storage lifecycle evidence."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        result = await self.client.call_tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())
        return storage_lifecycle_report(result.require_object())

    async def registry_lifecycle_simulate(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.registry_lifecycle_simulate`."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        return await self.tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())

    async def registry_lifecycle_simulate_report(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> RegistryLifecycleReport:
        """Async typed registry lifecycle evidence."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        result = await self.client.call_tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())
        return registry_lifecycle_report(result.require_object())

    async def cache_invalidation_simulate(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.cache_invalidation_simulate`."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        return await self.tool("cache_invalidation_simulate", normalized.to_mcp_arguments())

    async def cache_invalidation_simulate_report(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> CacheInvalidationReport:
        """Async typed cache invalidation evidence."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        result = await self.client.call_tool("cache_invalidation_simulate", normalized.to_mcp_arguments())
        return cache_invalidation_report(result.require_object())

    async def hub_disclosure_review(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async disclosure review replay."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        return await self.tool("hub_disclosure_review", normalized.to_mcp_arguments())

    async def hub_disclosure_review_report(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> HubDisclosureReviewReport:
        """Return async typed disclosure and headline evidence."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        result = await self.client.call_tool("hub_disclosure_review", normalized.to_mcp_arguments())
        return hub_disclosure_review(result.require_object())

    async def hub_card_render(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async typed public-hub card rendering."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        return await self.tool("hub_card_render", normalized.to_mcp_arguments())

    async def hub_card_render_report(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> HubCardRenderReport:
        """Return async typed public-hub card evidence."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        result = await self.client.call_tool("hub_card_render", normalized.to_mcp_arguments())
        return hub_card_render(result.require_object())

    async def hub_leaderboard_render(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async public-hub leaderboard rendering."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        return await self.tool("hub_leaderboard_render", normalized.to_mcp_arguments())

    async def hub_leaderboard_render_report(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> HubLeaderboardRenderReport:
        """Return async typed leaderboard evidence."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        result = await self.client.call_tool("hub_leaderboard_render", normalized.to_mcp_arguments())
        return hub_leaderboard_render(result.require_object())

    async def hub_submission_review(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async public-hub submission and moderation replay."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        return await self.tool("hub_submission_review", normalized.to_mcp_arguments())

    async def hub_submission_review_report(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> HubSubmissionReviewReport:
        """Return async typed submission and moderation evidence."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        result = await self.client.call_tool("hub_submission_review", normalized.to_mcp_arguments())
        return hub_submission_review(result.require_object())

    async def bioatlas_publication_audit(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async composed BioAtlas publication audit."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        return await self.tool("bioatlas_publication_audit", normalized.to_mcp_arguments())

    async def bioatlas_publication_audit_report(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> BioAtlasPublicationAuditReport:
        """Return async typed publication-audit evidence."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioatlas_publication_audit", normalized.to_mcp_arguments())
        return bioatlas_publication_audit(result.require_object())

    async def pack_catalogue_report(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> PackCatalogueReport:
        """Async typed pack portfolio declaration report."""

        if request is not None:
            if section != "all" or max_items != 100:
                raise ArgumentError("request cannot be combined with section or max_items")
            normalized = request if isinstance(request, PackCatalogueArgs) else PackCatalogueArgs.from_wire(request)
        else:
            normalized = PackCatalogueArgs(section, max_items)
        return pack_catalogue_report(await self.tool("pack_catalogue", normalized.to_mcp_arguments()))

    async def pack_coverage_audit(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit benchmark-pack coverage through async workspace MCP."""

        normalized = request if isinstance(request, PackCoverageAuditArgs) else PackCoverageAuditArgs.from_wire(request or {})
        return await self.tool("pack_coverage_audit", normalized.to_mcp_arguments())

    async def pack_coverage_audit_report(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackCoverageAuditReport:
        """Return typed async selected-portfolio coverage evidence."""

        return pack_coverage_audit_report(await self.pack_coverage_audit(request))

    async def pack_release_audit(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit pack release sequencing through async workspace MCP."""

        normalized = request if isinstance(request, PackReleaseAuditArgs) else PackReleaseAuditArgs.from_wire(request or {})
        return await self.tool("pack_release_audit", normalized.to_mcp_arguments())

    async def pack_release_audit_report(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackReleaseAuditReport:
        """Return typed async release-order evidence."""

        return pack_release_audit_report(await self.pack_release_audit(request))

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

    async def oncoworlds_clonal_evidence_check(
        self,
        request: OncoClonalEvidenceCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoClonalEvidenceCheckArgs) else OncoClonalEvidenceCheckArgs.from_wire(request)
        return await self.tool("oncoworlds_clonal_evidence_check", normalized.to_mcp_arguments())

    async def oncoworlds_clonal_evidence_check_report(
        self,
        request: OncoClonalEvidenceCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsClonalEvidenceCheckReport:
        return oncoworlds_clonal_evidence_check_report(await self.oncoworlds_clonal_evidence_check(request))

    async def oncoworlds_era_shift_check(
        self,
        request: OncoWorldsEraShiftCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEraShiftCheckArgs) else OncoWorldsEraShiftCheckArgs.from_wire(request)
        return await self.tool("oncoworlds_era_shift_check", normalized.to_mcp_arguments())

    async def oncoworlds_era_shift_check_report(
        self,
        request: OncoWorldsEraShiftCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEraShiftCheckReport:
        return oncoworlds_era_shift_check_report(await self.oncoworlds_era_shift_check(request))

    async def oncoworlds_equity_check(
        self,
        request: OncoWorldsEquityCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEquityCheckArgs) else OncoWorldsEquityCheckArgs.from_wire(request)
        return await self.tool("oncoworlds_equity_check", normalized.to_mcp_arguments())

    async def oncoworlds_equity_check_report(
        self,
        request: OncoWorldsEquityCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEquityCheckReport:
        return oncoworlds_equity_check_report(await self.oncoworlds_equity_check(request))

    async def oncoworlds_entity_world_check(
        self,
        request: OncoWorldsEntityWorldCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, OncoWorldsEntityWorldCheckArgs) else OncoWorldsEntityWorldCheckArgs.from_wire(request)
        return await self.tool("oncoworlds_entity_world_check", normalized.to_mcp_arguments())

    async def oncoworlds_entity_world_check_report(
        self,
        request: OncoWorldsEntityWorldCheckArgs | Mapping[str, Any],
    ) -> OncoWorldsEntityWorldCheckReport:
        return oncoworlds_entity_world_check_report(await self.oncoworlds_entity_world_check(request))

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

    async def routing_decision_report(
        self,
        fingerprint: Mapping[str, Any] | RoutingDecisionRequest,
        evidence: Sequence[Mapping[str, Any]] | None = None,
        policy: Mapping[str, Any] | None = None,
        *,
        task_id: str | None = None,
    ) -> RoutingDecisionReport:
        return routing_decision_report(await self.routing_decide(fingerprint, evidence, policy, task_id=task_id))

    async def routing_lab_run(
        self,
        request: RoutingLabRunArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the offline routing lab through async workspace MCP."""

        normalized = request if isinstance(request, RoutingLabRunArgs) else RoutingLabRunArgs.from_wire(request)
        return await self.tool("routing_lab_run", normalized.to_mcp_arguments())

    async def routing_lab_run_report(
        self,
        request: RoutingLabRunArgs | Mapping[str, Any],
    ) -> RoutingLabRunReport:
        """Return typed async routing-lab evidence."""

        return routing_lab_run_report(await self.routing_lab_run(request))

    async def lab_pareto_audit(
        self,
        request: LabParetoAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Build the offline inference-lab Pareto archive through async workspace MCP."""

        normalized = request if isinstance(request, LabParetoAuditArgs) else LabParetoAuditArgs.from_wire(request)
        return await self.tool("lab_pareto_audit", normalized.to_mcp_arguments())

    async def lab_pareto_audit_report(
        self,
        request: LabParetoAuditArgs | Mapping[str, Any],
    ) -> LabParetoAuditReport:
        """Return typed front, archive, hole, and selection evidence."""

        return lab_pareto_audit_report(await self.lab_pareto_audit(request))

    async def lab_branch_audit(
        self,
        request: LabBranchAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit risk-triggered branch accounting through async workspace MCP."""

        normalized = request if isinstance(request, LabBranchAuditArgs) else LabBranchAuditArgs.from_wire(request)
        return await self.tool("lab_branch_audit", normalized.to_mcp_arguments())

    async def lab_branch_audit_report(
        self,
        request: LabBranchAuditArgs | Mapping[str, Any],
    ) -> LabBranchAuditReport:
        """Return typed branch cost, catch, escape, and undetermined-risk evidence."""

        return lab_branch_audit_report(await self.lab_branch_audit(request))

    async def lab_holdout_audit(
        self,
        request: LabHoldoutAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the offline holdout and rollback audit through async workspace MCP."""

        normalized = request if isinstance(request, LabHoldoutAuditArgs) else LabHoldoutAuditArgs.from_wire(request)
        return await self.tool("lab_holdout_audit", normalized.to_mcp_arguments())

    async def lab_holdout_audit_report(
        self,
        request: LabHoldoutAuditArgs | Mapping[str, Any],
    ) -> LabHoldoutAuditReport:
        """Return typed clean-measurement and contamination evidence."""

        return lab_holdout_audit_report(await self.lab_holdout_audit(request))

    async def lab_evolution_audit(
        self,
        request: LabEvolutionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Assemble and grade a benchmark-gated evolution card through async workspace MCP."""

        normalized = request if isinstance(request, LabEvolutionAuditArgs) else LabEvolutionAuditArgs.from_wire(request)
        return await self.tool("lab_evolution_audit", normalized.to_mcp_arguments())

    async def lab_evolution_audit_report(
        self,
        request: LabEvolutionAuditArgs | Mapping[str, Any],
    ) -> LabEvolutionAuditReport:
        """Return typed clean-claim, contamination, and defeater evidence."""

        return lab_evolution_audit_report(await self.lab_evolution_audit(request))

    async def lab_space_audit(
        self,
        request: LabSpaceAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and inspect an immutable architecture space through async workspace MCP."""

        normalized = request if isinstance(request, LabSpaceAuditArgs) else LabSpaceAuditArgs.from_wire(request)
        return await self.tool("lab_space_audit", normalized.to_mcp_arguments())

    async def lab_space_audit_report(
        self,
        request: LabSpaceAuditArgs | Mapping[str, Any],
    ) -> LabSpaceAuditReport:
        """Return typed candidate, lineage, and component-diff evidence."""

        return lab_space_audit_report(await self.lab_space_audit(request))

    async def provider_capability_gate(
        self,
        request: ProviderCapabilityGateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ProviderCapabilityGateArgs) else ProviderCapabilityGateArgs.from_wire(request)
        result = await self.client.call_tool("provider_capability_gate", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def provider_capability_gate_report(
        self,
        request: ProviderCapabilityGateArgs | Mapping[str, Any],
    ) -> ProviderCapabilityGateReport:
        return provider_capability_gate_report(await self.provider_capability_gate(request))

    async def sdk_registry_check(
        self,
        request: SdkRegistryCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, SdkRegistryCheckArgs) else SdkRegistryCheckArgs.from_wire(request)
        result = await self.client.call_tool("sdk_registry_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def sdk_registry_check_report(
        self,
        request: SdkRegistryCheckArgs | Mapping[str, Any],
    ) -> SdkRegistryCheckReport:
        return sdk_registry_check_report(await self.sdk_registry_check(request))

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

    async def obligation_gate_check(
        self,
        request: ObligationGateCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ObligationGateCheckArgs) else ObligationGateCheckArgs.from_wire(request)
        return await self.tool("obligation_gate_check", normalized.to_mcp_arguments())

    async def obligation_gate_check_report(
        self,
        request: ObligationGateCheckArgs | Mapping[str, Any],
    ) -> ObligationGateCheckReport:
        return obligation_gate_check_report(await self.obligation_gate_check(request))

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

    async def mission_evaluator_discover(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async discovery of explicit non-executing evaluator candidates through MCP."""

        normalized = request if isinstance(request, MissionEvaluatorQuery) else MissionEvaluatorQuery(**dict(request or {}))
        return await self.tool("mission_evaluator_discover", normalized.to_mcp_arguments())

    async def mission_evaluator_discover_report(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> MissionEvaluatorSearchReport:
        """Return typed async evaluator candidate evidence through MCP."""

        return mission_evaluator_discover_report(await self.mission_evaluator_discover(request))

    async def mission_evaluator_review(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async review of explicit evaluator-to-claim bindings through MCP."""

        normalized = request if isinstance(request, MissionEvaluatorReviewRequest) else MissionEvaluatorReviewRequest(**dict(request))
        return await self.tool("mission_evaluator_review", normalized.to_mcp_arguments())

    async def mission_evaluator_review_report(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReviewReport:
        """Return typed async evaluator binding review evidence through MCP."""

        return mission_evaluator_review_report(await self.mission_evaluator_review(request))

    async def mission_evaluator_replay(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async replay of retained mission evaluator lineage without dispatch."""

        normalized = request if isinstance(request, MissionEvaluatorReplayRequest) else MissionEvaluatorReplayRequest(**dict(request))
        return await self.tool("mission_evaluator_replay", normalized.to_mcp_arguments())

    async def mission_evaluator_replay_report(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayReport:
        """Return typed async evaluator replay and fixture evidence through MCP."""

        return mission_evaluator_replay_report(await self.mission_evaluator_replay(request))

    async def mission_evaluator_replay_compare(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async comparison of retained replay evidence with the current catalogue."""

        normalized = request if isinstance(request, MissionEvaluatorReplayCompareRequest) else MissionEvaluatorReplayCompareRequest(**dict(request))
        return await self.tool("mission_evaluator_replay_compare", normalized.to_mcp_arguments())

    async def mission_evaluator_replay_compare_report(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed async digest-drift and binding-compatibility evidence."""

        return mission_evaluator_replay_comparison_report(
            await self.mission_evaluator_replay_compare(request)
        )

    async def mission_evidence_bundle_verify(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async workspace MCP mission evidence bundle verification."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleVerifyRequest)
            else MissionEvidenceBundleVerifyRequest(**dict(request))
        )
        return await self.tool("mission_evidence_bundle_verify", normalized.to_mcp_arguments())

    async def mission_evidence_bundle_verification_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed async workspace MCP mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(
            await self.mission_evidence_bundle_verify(request)
        )

    async def mission_evidence_bundle_import(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleImportRequest)
            else MissionEvidenceBundleImportRequest(**dict(request))
        )
        return await self.tool("mission_evidence_bundle_import", normalized.to_mcp_arguments())

    async def mission_evidence_bundle_import_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        return MissionEvidenceBundleImportReport.from_wire(await self.mission_evidence_bundle_import(request))

    async def mission_evidence_bundle_query(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleQueryRequest)
            else MissionEvidenceBundleQueryRequest(**dict(request or {}))
        )
        return await self.tool("mission_evidence_bundle_query", normalized.to_mcp_arguments())

    async def mission_evidence_bundle_query_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        return MissionEvidenceBundleQueryReport.from_wire(await self.mission_evidence_bundle_query(request))

    async def mission_evidence_bundle_get(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        if isinstance(request, MissionEvidenceBundleGetRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvidenceBundleGetRequest(request)
        else:
            normalized = MissionEvidenceBundleGetRequest(**dict(request))
        return await self.tool("mission_evidence_bundle_get", normalized.to_mcp_arguments())

    async def mission_evidence_bundle_get_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        return MissionEvidenceBundleGetReport.from_wire(await self.mission_evidence_bundle_get(request))

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

    async def capability_dashboard(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.capability_dashboard`."""

        normalized = request if isinstance(request, CapabilityDashboardQueryArgs) else CapabilityDashboardQueryArgs(**dict(request or {}))
        return await self.tool("capability_dashboard", normalized.to_mcp_arguments())

    async def capability_dashboard_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return typed dashboard evidence through async MCP."""

        return capability_dashboard_report(await self.capability_dashboard(request))

    async def ci_execution_evidence_audit(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.ci_execution_evidence_audit`."""

        normalized = request if isinstance(request, CiExecutionEvidenceRequest) else CiExecutionEvidenceRequest(**dict(request))
        return await self.tool("ci_execution_evidence_audit", normalized.to_mcp_arguments())

    async def ci_execution_evidence_report(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> CiExecutionEvidenceReport:
        """Return typed CI evidence through async MCP."""

        return ci_execution_evidence_report(await self.ci_execution_evidence_audit(request))

    async def ci_provider_normalize(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.ci_provider_normalize`."""

        normalized = request if isinstance(request, CiProviderNormalizationRequest) else CiProviderNormalizationRequest(**dict(request))
        return await self.tool("ci_provider_normalize", normalized.to_mcp_arguments())

    async def ci_provider_normalization_report(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> CiProviderNormalizationReport:
        """Return typed provider-normalization evidence through async MCP."""

        return ci_provider_normalization_report(await self.ci_provider_normalize(request))

    async def ci_provider_evidence_audit(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async provider-bound artifact, log, and attestation conformance audit."""

        normalized = request if isinstance(request, CiProviderEvidenceRequest) else CiProviderEvidenceRequest(**dict(request))
        return await self.tool("ci_provider_evidence_audit", normalized.to_mcp_arguments())

    async def ci_provider_evidence_report(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> CiProviderEvidenceReport:
        """Return typed async provider-evidence conformance evidence."""

        return ci_provider_evidence_report(await self.ci_provider_evidence_audit(request))

    async def execution_provenance_audit(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.execution_provenance_audit`."""

        normalized = request if isinstance(request, ExecutionProvenanceRequest) else ExecutionProvenanceRequest(**dict(request))
        return await self.tool("execution_provenance_audit", normalized.to_mcp_arguments())

    async def execution_provenance_report(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> ExecutionProvenanceReport:
        """Return typed async mission/delegated-check provenance evidence."""

        return execution_provenance_report(await self.execution_provenance_audit(request))

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

    async def domain_workflow_catalogue(self) -> dict[str, Any]:
        """Async complete deterministic workflow-template catalogue."""

        return await self.tool("domain_workflow_catalogue", {})

    async def domain_workflow_catalogue_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(await self.domain_workflow_catalogue())

    async def domain_workflow_scaffold(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowScaffoldRequest)
            else DomainWorkflowScaffoldRequest(**dict(request))
        )
        return await self.tool("domain_workflow_scaffold", normalized.to_arguments())

    async def domain_workflow_scaffold_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(await self.domain_workflow_scaffold(request))

    async def domain_workflow_instantiate(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowInstantiateRequest)
            else DomainWorkflowInstantiateRequest(**dict(request))
        )
        return await self.tool("domain_workflow_instantiate", normalized.to_arguments())

    async def domain_workflow_instantiation_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(
            await self.domain_workflow_instantiate(request)
        )

    async def domain_workflow_reconcile(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconcileRequest)
            else DomainWorkflowReconcileRequest(**dict(request))
        )
        return await self.tool("domain_workflow_reconcile", normalized.to_arguments())

    async def domain_workflow_reconciliation_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(
            await self.domain_workflow_reconcile(request)
        )

    async def domain_workflow_reconciliation_import(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await self.tool(
            "domain_workflow_reconciliation_import",
            (
                request
                if isinstance(request, DomainWorkflowReconciliationImportRequest)
                else DomainWorkflowReconciliationImportRequest(**dict(request))
            ).to_arguments(),
        )

    async def domain_workflow_reconciliation_import_report(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationImportReport:
        return DomainWorkflowReconciliationImportReport.from_wire(
            await self.domain_workflow_reconciliation_import(request)
        )

    async def domain_workflow_reconciliation_query(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationQueryRequest)
            else DomainWorkflowReconciliationQueryRequest(**dict(request or {}))
        )
        return await self.tool("domain_workflow_reconciliation_query", normalized.to_arguments())

    async def domain_workflow_reconciliation_query_report(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainWorkflowReconciliationQueryReport:
        return DomainWorkflowReconciliationQueryReport.from_wire(
            await self.domain_workflow_reconciliation_query(request)
        )

    async def domain_workflow_reconciliation_get(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationGetRequest)
            else DomainWorkflowReconciliationGetRequest(request)
            if isinstance(request, str)
            else DomainWorkflowReconciliationGetRequest(**dict(request))
        )
        return await self.tool("domain_workflow_reconciliation_get", normalized.to_arguments())

    async def domain_workflow_reconciliation_get_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            await self.domain_workflow_reconciliation_get(request)
        )

    async def artifact_registry_audit(
        self,
        arguments: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return await self.tool("artifact_registry_audit", arguments)

    async def domain_report_project(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainReportProjectRequest)
            else DomainReportProjectRequest(**dict(request))
        )
        return await self.tool("domain_report_project", normalized.to_arguments())

    async def domain_report_project_report(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        return DomainReportProjectReport.from_wire(await self.domain_report_project(request))

    async def domain_report_coverage(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainReportCoverageRequest)
            else DomainReportCoverageRequest(**dict(request or {}))
        )
        return await self.tool("domain_report_project", normalized.to_arguments())

    async def domain_report_coverage_report(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        return DomainReportCoverageReport.from_wire(await self.domain_report_coverage(request))

    async def domain_evidence_harmonize(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizeRequest)
            else DomainEvidenceHarmonizeRequest(**dict(request))
        )
        return await self.tool("domain_evidence_harmonize", normalized.to_arguments())

    async def domain_evidence_harmonize_report(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        return DomainEvidenceHarmonizationReport.from_wire(
            await self.domain_evidence_harmonize(request)
        )

    async def domain_evidence_intake(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeRequest)
            else DomainEvidenceIntakeRequest(**dict(request))
        )
        return await self.tool("domain_evidence_intake", normalized.to_arguments())

    async def domain_evidence_intake_report(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        return DomainEvidenceIntakeReport.from_wire(await self.domain_evidence_intake(request))

    async def domain_evidence_coverage(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeCoverageRequest)
            else DomainEvidenceIntakeCoverageRequest(**dict(request or {}))
        )
        return await self.tool("domain_evidence_coverage", normalized.to_arguments())

    async def domain_evidence_coverage_report(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        return DomainEvidenceIntakeCoverageReport.from_wire(await self.domain_evidence_coverage(request))

    async def domain_evidence_source_plan(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourcePlanRequest)
            else DomainEvidenceSourcePlanRequest(**dict(request))
        )
        return await self.tool("domain_evidence_source_plan", normalized.to_arguments())

    async def domain_evidence_source_plan_report(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        return DomainEvidenceSourcePlanReport.from_wire(await self.domain_evidence_source_plan(request))

    async def domain_evidence_source_execute(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourceExecutionRequest)
            else DomainEvidenceSourceExecutionRequest(**dict(request))
        )
        return await self.tool("domain_evidence_source_execute", normalized.to_arguments())

    async def domain_evidence_source_execute_report(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        return DomainEvidenceSourceExecutionReport.from_wire(
            await self.domain_evidence_source_execute(request)
        )

    async def domain_evidence_source_project(
        self,
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: SourceAdapterProjectionRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> SourceAdapterProjectionResult:
        """Project a returned source envelope locally without another MCP call."""

        normalized_execution = (
            execution.to_dict()
            if isinstance(execution, DomainEvidenceSourceExecutionReport)
            else dict(execution)
        )
        normalized_request = (
            request
            if isinstance(request, SourceAdapterProjectionRequest)
            else SourceAdapterProjectionRequest(**dict(request))
        )
        return await asyncio.to_thread(
            project_source_execution,
            normalized_execution,
            normalized_request,
            runtime=runtime,
        )

    async def domain_evidence_source_project_for_domain(
        self,
        catalogue: DomainAcquisitionReport | Mapping[str, Any],
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: DomainEvidencePipelineRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> DomainEvidencePipelineResult:
        """Project a source envelope through a catalogue-bound adapter route locally."""

        normalized_request = (
            request
            if isinstance(request, DomainEvidencePipelineRequest)
            else DomainEvidencePipelineRequest(**dict(request))
        )
        normalized_execution = (
            execution.to_dict()
            if isinstance(execution, DomainEvidenceSourceExecutionReport)
            else dict(execution)
        )
        return await asyncio.to_thread(
            project_domain_source_execution,
            catalogue,
            normalized_execution,
            normalized_request,
            runtime=runtime,
        )

    async def domain_evidence_provider_normalize(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderNormalizationRequest)
            else DomainEvidenceProviderNormalizationRequest(**dict(request))
        )
        return await self.tool("domain_evidence_provider_normalize", normalized.to_mcp_arguments())

    async def domain_evidence_provider_normalization_report(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderNormalizationReport:
        return domain_evidence_provider_normalization_report(
            await self.domain_evidence_provider_normalize(request)
        )

    async def domain_evidence_provider_replay_verify(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderReplayRequest)
            else DomainEvidenceProviderReplayRequest(**dict(request))
        )
        return await self.tool("domain_evidence_provider_replay_verify", normalized.to_mcp_arguments())

    async def domain_evidence_provider_replay_verification_report(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        return domain_evidence_provider_replay_verification_report(
            await self.domain_evidence_provider_replay_verify(request)
        )

    async def artifact_cross_store_audit(self) -> ArtifactCrossStoreAuditReport:
        return ArtifactCrossStoreAuditReport.from_wire(
            await self.artifact_registry_audit({"operation": "cross_store"})
        )

    async def artifact_register(
        self,
        request: ArtifactRegistrationRequest | Mapping[str, Any],
    ) -> ArtifactRegistrationReport:
        normalized = (
            request
            if isinstance(request, ArtifactRegistrationRequest)
            else ArtifactRegistrationRequest(**dict(request))
        )
        return ArtifactRegistrationReport.from_wire(
            await self.artifact_registry_audit(
                {"operation": "register", "registration": normalized.to_arguments()}
            )
        )

    async def artifact_query(
        self,
        request: ArtifactQueryRequest | Mapping[str, Any] | None = None,
    ) -> ArtifactQueryReport:
        normalized = (
            request
            if isinstance(request, ArtifactQueryRequest)
            else ArtifactQueryRequest(**dict(request or {}))
        )
        return ArtifactQueryReport.from_wire(
            await self.artifact_registry_audit(
                {"operation": "query", **normalized.to_arguments()}
            )
        )

    async def artifact_get(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactGetReport:
        normalized = (
            request
            if isinstance(request, ArtifactGetRequest)
            else ArtifactGetRequest(request)
            if isinstance(request, str)
            else ArtifactGetRequest(**dict(request))
        )
        return ArtifactGetReport.from_wire(
            await self.artifact_registry_audit(
                {"operation": "get", "content_digest": normalized.content_digest}
            )
        )

    async def artifact_lineage(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactLineageReport:
        normalized = (
            request
            if isinstance(request, ArtifactGetRequest)
            else ArtifactGetRequest(request)
            if isinstance(request, str)
            else ArtifactGetRequest(**dict(request))
        )
        return ArtifactLineageReport.from_wire(
            await self.artifact_registry_audit(
                {"operation": "lineage", "content_digest": normalized.content_digest}
            )
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

    async def domain_acquisition_catalogue(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`AsyncWorkspace.domain_acquisition_catalogue`."""

        normalized = query or DomainAcquisitionQuery()
        if not isinstance(normalized, DomainAcquisitionQuery):
            raise TypeError("query must be a DomainAcquisitionQuery")
        return await self.tool(DOMAIN_ACQUISITION_WORKFLOW, normalized.to_mcp_arguments())

    async def domain_acquisition_catalogue_report(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> DomainAcquisitionReport:
        """Return typed async acquisition routes."""

        return domain_acquisition_report(await self.domain_acquisition_catalogue(query))

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

    async def literature_bind_check(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, LiteratureBindCheckArgs) else LiteratureBindCheckArgs.from_wire(request)
        return await self.tool("literature_bind_check", normalized.to_mcp_arguments())

    async def literature_bind_check_report(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> LiteratureBindCheckReport:
        return literature_bind_check_report(await self.literature_bind_check(request))

    async def modality_support_check(
        self,
        request: ModalitySupportCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalitySupportCheckArgs) else ModalitySupportCheckArgs.from_wire(request)
        return await self.tool("modality_support_check", normalized.to_mcp_arguments())

    async def modality_support_check_report(
        self,
        request: ModalitySupportCheckArgs | Mapping[str, Any],
    ) -> ModalitySupportCheckReport:
        return modality_support_check_report(await self.modality_support_check(request))

    async def modality_transport_check(
        self,
        request: ModalityTransportCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalityTransportCheckArgs) else ModalityTransportCheckArgs.from_wire(request)
        return await self.tool("modality_transport_check", normalized.to_mcp_arguments())

    async def modality_transport_check_report(
        self,
        request: ModalityTransportCheckArgs | Mapping[str, Any],
    ) -> ModalityTransportCheckReport:
        return modality_transport_check_report(await self.modality_transport_check(request))

    async def modality_comparability_check(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, ModalityComparabilityCheckArgs) else ModalityComparabilityCheckArgs.from_wire(request)
        return await self.tool("modality_comparability_check", normalized.to_mcp_arguments())

    async def modality_comparability_check_report(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> ModalityComparabilityCheckReport:
        return modality_comparability_check_report(await self.modality_comparability_check(request))

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

    async def bioeval_acquisition_audit(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async acquisition-trace audit with structured domain refusals."""

        normalized = request if isinstance(request, BioevalAcquisitionAuditArgs) else BioevalAcquisitionAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_acquisition_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_acquisition_audit_report(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> BioevalAcquisitionAuditReport:
        """Return async typed acquisition-trace evidence."""

        return bioeval_acquisition_audit_report(await self.bioeval_acquisition_audit(request))

    async def bioeval_grounding_audit(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async claim-evidence grounding audit with structured refusals."""

        normalized = request if isinstance(request, BioevalGroundingAuditArgs) else BioevalGroundingAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_grounding_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_grounding_audit_report(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> BioevalGroundingAuditReport:
        """Return async typed claim-evidence grounding evidence."""

        return bioeval_grounding_audit_report(await self.bioeval_grounding_audit(request))

    async def bioeval_estimand_audit(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async estimand and identification audit with structured refusals."""

        normalized = request if isinstance(request, BioevalEstimandAuditArgs) else BioevalEstimandAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_estimand_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_estimand_audit_report(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> BioevalEstimandAuditReport:
        """Return async typed estimand and identification evidence."""

        return bioeval_estimand_audit_report(await self.bioeval_estimand_audit(request))

    async def bioeval_evaluator_audit(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async evaluator-health audit with structured fail-closed refusals."""

        normalized = request if isinstance(request, BioevalEvaluatorAuditArgs) else BioevalEvaluatorAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_evaluator_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_evaluator_audit_report(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> BioevalEvaluatorAuditReport:
        """Return async typed evaluator-health and task-outcome evidence."""

        return bioeval_evaluator_audit_report(await self.bioeval_evaluator_audit(request))

    async def bioeval_plane_audit(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async scoring-plane audit with explicit fold refusal posture."""

        normalized = request if isinstance(request, BioevalPlaneAuditArgs) else BioevalPlaneAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_plane_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_plane_audit_report(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> BioevalPlaneAuditReport:
        """Return async typed scoring-plane and fold evidence."""

        return bioeval_plane_audit_report(await self.bioeval_plane_audit(request))

    async def bioeval_metamorphic_audit(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async metamorphic-response audit with explicit oracle policies."""

        normalized = request if isinstance(request, BioevalMetamorphicAuditArgs) else BioevalMetamorphicAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_metamorphic_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_metamorphic_audit_report(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> BioevalMetamorphicAuditReport:
        """Return async typed metamorphic-response evidence."""

        return bioeval_metamorphic_audit_report(await self.bioeval_metamorphic_audit(request))

    async def bioeval_waiver_audit(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async release-gate waiver audit with explicit fail-closed policies."""

        normalized = request if isinstance(request, BioevalWaiverAuditArgs) else BioevalWaiverAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_waiver_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_waiver_audit_report(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> BioevalWaiverAuditReport:
        """Return async typed release-gate waiver evidence."""

        return bioeval_waiver_audit_report(await self.bioeval_waiver_audit(request))

    async def bioeval_design_audit(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async factorial-design audit with explicit coverage policies."""

        normalized = request if isinstance(request, BioevalDesignAuditArgs) else BioevalDesignAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_design_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_design_audit_report(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> BioevalDesignAuditReport:
        """Return async typed factorial-design evidence."""

        return bioeval_design_audit_report(await self.bioeval_design_audit(request))

    async def bioeval_mesh_audit(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async evaluator-mesh audit with explicit independence policies."""

        normalized = request if isinstance(request, BioevalMeshAuditArgs) else BioevalMeshAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_mesh_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_mesh_audit_report(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> BioevalMeshAuditReport:
        """Return async typed evaluator-mesh evidence."""

        return bioeval_mesh_audit_report(await self.bioeval_mesh_audit(request))

    async def bioeval_burden_audit(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async burden audit with explicit fail-closed policies."""

        normalized = request if isinstance(request, BioevalBurdenAuditArgs) else BioevalBurdenAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_burden_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_burden_audit_report(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> BioevalBurdenAuditReport:
        """Return async typed nonrenewable-resource evidence."""

        return bioeval_burden_audit_report(await self.bioeval_burden_audit(request))

    async def bioeval_reveal_audit(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async prospective reveal audit with fail-closed policies."""

        normalized = request if isinstance(request, BioevalRevealAuditArgs) else BioevalRevealAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_reveal_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_reveal_audit_report(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> BioevalRevealAuditReport:
        """Return async typed prospective reveal evidence."""

        return bioeval_reveal_audit_report(await self.bioeval_reveal_audit(request))

    async def bioeval_boundary_audit(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async contextual-integrity audit with explicit safety policies."""

        normalized = request if isinstance(request, BioevalBoundaryAuditArgs) else BioevalBoundaryAuditArgs.from_wire(request)
        result = await self.client.call_tool("bioeval_boundary_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def bioeval_boundary_audit_report(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> BioevalBoundaryAuditReport:
        """Return async typed boundary evidence."""

        return bioeval_boundary_audit_report(await self.bioeval_boundary_audit(request))

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
        ci_evidence: CiExecutionEvidenceRequest | Mapping[str, Any] | None = None,
        ci_provider: CiProviderNormalizationRequest | Mapping[str, Any] | None = None,
        ci_provider_evidence: CiProviderEvidenceRequest | Mapping[str, Any] | None = None,
        execution_provenance: ExecutionProvenanceRequest | Mapping[str, Any] | None = None,
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
                "ci_evidence": ci_evidence,
                "ci_provider": ci_provider,
                "ci_provider_evidence": ci_provider_evidence,
                "execution_provenance": execution_provenance,
            }
        )
        return (await self.client.call_tool("developer_delivery_audit", arguments)).require_ok()

    async def developer_delivery_receipt(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async content-addressed receipt from a recomputed delivery audit."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptRequest) else DeveloperDeliveryReceiptRequest.from_wire(request)
        return (await self.client.call_tool("developer_delivery_receipt", normalized.to_mcp_arguments())).require_ok()

    async def developer_delivery_receipt_verify(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async verification of a stored delivery receipt against its completed audit."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptVerificationRequest) else DeveloperDeliveryReceiptVerificationRequest.from_wire(request)
        return (await self.client.call_tool("developer_delivery_receipt_verify", normalized.to_mcp_arguments())).require_ok()

    async def developer_platform_status(
        self,
        request: DeveloperPlatformStatusArgs | Mapping[str, Any] | None = None,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async bounded developer-platform contract projection."""

        if request is not None:
            if include_details is not False or max_items != 100:
                raise ArgumentError("request cannot be combined with include_details or max_items")
            normalized = request if isinstance(request, DeveloperPlatformStatusArgs) else DeveloperPlatformStatusArgs.from_wire(request)
        else:
            normalized = DeveloperPlatformStatusArgs(include_details, max_items)
        return (await self.client.call_tool("developer_platform_status", normalized.to_mcp_arguments())).require_ok()

    async def developer_platform_status_report(
        self,
        request: DeveloperPlatformStatusArgs | Mapping[str, Any] | None = None,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> DeveloperPlatformStatusReport:
        """Async typed walkthrough, cookbook, diagnostics, and impact evidence."""

        return developer_platform_status_report(
            await self.developer_platform_status(
                request, include_details=include_details, max_items=max_items
            )
        )

    async def token_context_plan(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async bounded token-context planning without execution."""

        normalized = request if isinstance(request, TokenContextPlanArgs) else TokenContextPlanArgs.from_wire(request)
        return (await self.client.call_tool("token_context_plan", normalized.to_mcp_arguments())).require_ok()

    async def token_context_plan_report(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> TokenContextPlanningReport:
        """Async typed token estimates and policy-only comparison evidence."""

        return token_context_plan_report(await self.token_context_plan(request))

    async def weavelang_compile(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Async WeaveLang compilation and optional local semantic replay."""

        if isinstance(request, str):
            normalized = WeaveLangCompileArgs(request)
        elif isinstance(request, WeaveLangCompileArgs):
            normalized = request
        else:
            normalized = WeaveLangCompileArgs.from_wire(request)
        return (await self.client.call_tool("weavelang_compile", normalized.to_mcp_arguments())).require_ok()

    async def weavelang_compile_report(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> WeaveLangCompileReport:
        """Async typed WeaveLang compilation and replay evidence."""

        return weavelang_compile_report(await self.weavelang_compile(request))

    async def epistemic_voi(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async value-of-information pricing with structured fail-closed refusals."""

        normalized = request if isinstance(request, EpistemicVoiArgs) else EpistemicVoiArgs.from_wire(request)
        result = await self.client.call_tool("epistemic_voi", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def epistemic_voi_report(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> EpistemicVoiReport:
        """Return async typed value-of-information evidence."""

        return epistemic_voi_report(await self.epistemic_voi(request))

    async def epistemic_context_audit(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit decision-relative context compression through async workspace MCP."""

        normalized = request if isinstance(request, EpistemicContextAuditArgs) else EpistemicContextAuditArgs.from_wire(request)
        result = await self.client.call_tool("epistemic_context_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def epistemic_context_audit_report(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> EpistemicContextAuditReport:
        """Return typed frontier, sufficiency, identification, and subset evidence."""

        return epistemic_context_audit_report(await self.epistemic_context_audit(request))

    async def epistemic_selection_audit(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async bounded observed-evidence selection with fail-closed audit metadata."""

        normalized = request if isinstance(request, EpistemicSelectionAuditArgs) else EpistemicSelectionAuditArgs.from_wire(request)
        result = await self.client.call_tool("epistemic_selection_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def epistemic_selection_audit_report(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> EpistemicSelectionAuditReport:
        """Return async typed selection, guarantee, and exactness evidence."""

        return epistemic_selection_audit_report(await self.epistemic_selection_audit(request))

    async def benchmark_trace_analyze(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async benchmark trace analysis with structured fail-closed refusals."""

        normalized = request if isinstance(request, BenchmarkTraceAnalyzeArgs) else BenchmarkTraceAnalyzeArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_trace_analyze", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_trace_analysis_report(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> BenchmarkTraceAnalysisReport:
        """Return async typed benchmark compiler evidence."""

        return benchmark_trace_analysis_report(await self.benchmark_trace_analyze(request))

    async def benchmark_decision_audit(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit one decision through the async workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkDecisionAuditArgs) else BenchmarkDecisionAuditArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_decision_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_decision_audit_report(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> BenchmarkDecisionAuditReport:
        """Return typed async decision-cell evidence."""

        return benchmark_decision_audit_report(await self.benchmark_decision_audit(request))

    async def benchmark_integrity_audit(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit benchmark portfolio integrity through the async workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkIntegrityAuditArgs) else BenchmarkIntegrityAuditArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_integrity_audit", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_integrity_audit_report(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> BenchmarkIntegrityAuditReport:
        """Return typed async portfolio integrity evidence."""

        return benchmark_integrity_audit_report(await self.benchmark_integrity_audit(request))

    async def benchmark_counterfactual_check(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and contrast matched DecisionCells through the async workspace MCP client."""

        normalized = request if isinstance(request, BenchmarkCounterfactualCheckArgs) else BenchmarkCounterfactualCheckArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_counterfactual_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_counterfactual_check_report(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> BenchmarkCounterfactualCheckReport:
        """Return typed async counterfactual evidence."""

        return benchmark_counterfactual_check_report(await self.benchmark_counterfactual_check(request))

    async def benchmark_oracle_review(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review, grade, and optionally package a benchmark oracle through async workspace MCP."""

        normalized = request if isinstance(request, BenchmarkOracleReviewArgs) else BenchmarkOracleReviewArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_oracle_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_oracle_review_report(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> BenchmarkOracleReviewReport:
        """Return typed async oracle review-gate evidence."""

        return benchmark_oracle_review_report(await self.benchmark_oracle_review(request))

    async def benchmark_compile(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the non-executing assembled benchmark compiler through async workspace MCP."""

        normalized = request if isinstance(request, BenchmarkCompileArgs) else BenchmarkCompileArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_compile", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_compile_report(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReport:
        """Return typed async benchmark compiler evidence."""

        return benchmark_compile_report(await self.benchmark_compile(request))

    async def benchmark_compile_review(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the complete reviewed benchmark-cell workflow through async workspace MCP."""

        normalized = request if isinstance(request, BenchmarkCompileReviewArgs) else BenchmarkCompileReviewArgs.from_wire(request)
        result = await self.client.call_tool("benchmark_compile_review", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def benchmark_compile_review_report(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReviewReport:
        """Return typed async reviewed benchmark-cell evidence."""

        return benchmark_compile_review_report(await self.benchmark_compile_review(request))

    async def foundation_contract_check(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async foundation contract validation with structured gate outcomes."""

        normalized = request if isinstance(request, FoundationContractCheckArgs) else FoundationContractCheckArgs.from_wire(request)
        result = await self.client.call_tool("foundation_contract_check", normalized.to_mcp_arguments())
        if result.is_error:
            return result.require_ok()
        return result.require_object()

    async def foundation_contract_check_report(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> FoundationContractCheckReport:
        """Return async typed foundation gate evidence."""

        return foundation_contract_check_report(await self.foundation_contract_check(request))

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
        ci_evidence: CiExecutionEvidenceRequest | Mapping[str, Any] | None = None,
        ci_provider: CiProviderNormalizationRequest | Mapping[str, Any] | None = None,
        ci_provider_evidence: CiProviderEvidenceRequest | Mapping[str, Any] | None = None,
        execution_provenance: ExecutionProvenanceRequest | Mapping[str, Any] | None = None,
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
                ci_evidence=ci_evidence,
                ci_provider=ci_provider,
                ci_provider_evidence=ci_provider_evidence,
                execution_provenance=execution_provenance,
            )
        )

    async def developer_delivery_receipt_report(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptReport:
        """Return async typed target/evidence digests and structural receipt readiness."""

        return developer_delivery_receipt_report(await self.developer_delivery_receipt(request))

    async def developer_delivery_receipt_verification_report(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptVerificationReport:
        """Return async typed receipt digest and projection mismatch evidence."""

        return developer_delivery_receipt_verification_report(
            await self.developer_delivery_receipt_verify(request)
        )

    async def bioatlas_publication_audit(self, atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs, **kwargs: Any) -> dict[str, Any]:
        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return (await self.client.call_tool("bioatlas_publication_audit", atlas.to_mcp_arguments())).require_ok()
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
        atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs,
        **kwargs: Any,
    ) -> BioAtlasPublicationAuditReport:
        """Async typed atlas, evidence, card, leaderboard, and publication gates."""

        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return bioatlas_publication_audit(await self.bioatlas_publication_audit(atlas))
        return bioatlas_publication_audit_report(await self.bioatlas_publication_audit(atlas, **kwargs))

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

    async def telemetry_project_report(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> TelemetryProjectionReport:
        """Async counterpart to :meth:`Workspace.telemetry_project_report`."""

        return telemetry_project_report(await self.telemetry_project(event, policy, trace, metric=metric, observations=observations))

    async def ledger_ingest(self, request: LedgerIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.ledger_ingest`."""

        normalized = request if isinstance(request, LedgerIngestArgs) else LedgerIngestArgs.from_wire(request)
        return await self.tool("ledger_ingest", normalized.to_mcp_arguments())

    async def ledger_ingest_report(self, request: LedgerIngestArgs | Mapping[str, Any]) -> LedgerIngestReport:
        """Return async typed ledger evidence."""

        return ledger_ingest_report(await self.ledger_ingest(request))

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

    async def trace_otel_ingest_report(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> TraceOtelIngestReport:
        """Async counterpart to :meth:`Workspace.trace_otel_ingest_report`."""

        normalized = request if isinstance(request, TraceOtelIngestArgs) else TraceOtelIngestArgs.from_wire(request)
        return trace_otel_ingest_report((await self.client.call_tool("trace_otel_ingest", normalized.to_mcp_arguments())).require_ok())

    async def quality_gate_run(self, request: QualityGateRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.quality_gate_run`."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return (await self.client.call_tool("quality_gate_run", normalized.to_mcp_arguments())).require_ok()

    async def quality_gate_run_report(self, request: QualityGateRunArgs | Mapping[str, Any]) -> QualityGateRunReport:
        """Async counterpart to :meth:`Workspace.quality_gate_run_report`."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return quality_gate_run_report((await self.client.call_tool("quality_gate_run", normalized.to_mcp_arguments())).require_ok())

    async def atlas_report(self, request: AtlasReportArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.atlas_report`."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return (await self.client.call_tool("atlas_report", normalized.to_mcp_arguments())).require_ok()

    async def atlas_report_typed(self, request: AtlasReportArgs | Mapping[str, Any]) -> AtlasReport:
        """Async counterpart to :meth:`Workspace.atlas_report_typed`."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return atlas_report_parser((await self.client.call_tool("atlas_report", normalized.to_mcp_arguments())).require_ok())

    async def atlas_surface_audit(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.atlas_surface_audit."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return (await self.client.call_tool("atlas_surface_audit", normalized.to_mcp_arguments())).require_ok()

    async def atlas_surface_audit_report(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> AtlasSurfaceAuditReport:
        """Async counterpart to Workspace.atlas_surface_audit_report."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return atlas_surface_audit_report((await self.client.call_tool("atlas_surface_audit", normalized.to_mcp_arguments())).require_ok())

    async def engineering_manifest_audit(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.engineering_manifest_audit."""

        normalized = request if isinstance(request, EngineeringManifestArgs) else EngineeringManifestArgs.from_wire(request)
        return (await self.client.call_tool("engineering_manifest_audit", normalized.to_mcp_arguments())).require_ok()

    async def engineering_manifest_audit_report(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> EngineeringAuditReport:
        """Async counterpart to Workspace.engineering_manifest_audit_report."""

        return engineering_manifest_audit_report(await self.engineering_manifest_audit(request))

    async def engineering_execution_plan(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.engineering_execution_plan."""

        normalized = request if isinstance(request, EngineeringPlanRequestArgs) else EngineeringPlanRequestArgs.from_wire(request)
        return (await self.client.call_tool("engineering_execution_plan", normalized.to_mcp_arguments())).require_ok()

    async def engineering_execution_plan_report(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> EngineeringPlanReport:
        """Async counterpart to Workspace.engineering_execution_plan_report."""

        return engineering_execution_plan_report(await self.engineering_execution_plan(request))

    async def release_pipeline_audit(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.release_pipeline_audit."""

        normalized = request if isinstance(request, ReleasePipelineManifestArgs) else ReleasePipelineManifestArgs.from_wire(request)
        return (await self.client.call_tool("release_pipeline_audit", normalized.to_mcp_arguments())).require_ok()

    async def release_pipeline_audit_report(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> ReleasePipelineAuditReport:
        """Async counterpart to Workspace.release_pipeline_audit_report."""

        return release_pipeline_audit_report(await self.release_pipeline_audit(request))

    async def operational_readiness_audit(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.operational_readiness_audit."""

        normalized = request if isinstance(request, OperationalReadinessManifestArgs) else OperationalReadinessManifestArgs.from_wire(request)
        return (await self.client.call_tool("operational_readiness_audit", normalized.to_mcp_arguments())).require_ok()

    async def operational_readiness_audit_report(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> OperationalReadinessAuditReport:
        """Async counterpart to Workspace.operational_readiness_audit_report."""

        return operational_readiness_audit_report(await self.operational_readiness_audit(request))

    async def security_privacy_audit(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.security_privacy_audit."""

        normalized = request if isinstance(request, SecurityPrivacyManifestArgs) else SecurityPrivacyManifestArgs.from_wire(request)
        return (await self.client.call_tool("security_privacy_audit", normalized.to_mcp_arguments())).require_ok()

    async def security_privacy_audit_report(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> SecurityPrivacyAuditReport:
        """Async counterpart to Workspace.security_privacy_audit_report."""

        return security_privacy_audit_report(await self.security_privacy_audit(request))

    async def sandbox_admission_audit(self, request: SandboxManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.sandbox_admission_audit."""

        normalized = request if isinstance(request, SandboxManifestArgs) else SandboxManifestArgs.from_wire(request)
        return (await self.client.call_tool("sandbox_admission_audit", normalized.to_mcp_arguments())).require_ok()

    async def sandbox_admission_audit_report(self, request: SandboxManifestArgs | Mapping[str, Any]) -> SandboxAuditReport:
        """Async counterpart to Workspace.sandbox_admission_audit_report."""

        return sandbox_admission_audit_report(await self.sandbox_admission_audit(request))

    async def sandbox_runtime_simulate(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.sandbox_runtime_simulate."""

        normalized = request if isinstance(request, SandboxRuntimeManifestArgs) else SandboxRuntimeManifestArgs.from_wire(request)
        return (await self.client.call_tool("sandbox_runtime_simulate", normalized.to_mcp_arguments())).require_ok()

    async def sandbox_runtime_simulate_report(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> SandboxRuntimeAuditReport:
        """Return typed sandbox runtime decisions through the async workspace."""

        return sandbox_runtime_simulate_report(await self.sandbox_runtime_simulate(request))

    async def security_program_audit(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to Workspace.security_program_audit."""

        normalized = request if isinstance(request, SecurityProgramManifestArgs) else SecurityProgramManifestArgs.from_wire(request)
        return (await self.client.call_tool("security_program_audit", normalized.to_mcp_arguments())).require_ok()

    async def security_program_audit_report(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> SecurityProgramAuditReport:
        """Async counterpart to Workspace.security_program_audit_report."""

        return security_program_audit_report(await self.security_program_audit(request))

    async def adaptive_panel(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.adaptive_panel`."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return (await self.client.call_tool("adaptive_panel", normalized.to_mcp_arguments())).require_ok()

    async def adaptive_panel_report(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> AdaptivePanelReport:
        """Async counterpart to :meth:`Workspace.adaptive_panel_report`."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return adaptive_panel_report((await self.client.call_tool("adaptive_panel", normalized.to_mcp_arguments())).require_ok())

    async def posterior_gate(self, request: PosteriorGateArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`Workspace.posterior_gate`."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return (await self.client.call_tool("posterior_gate", normalized.to_mcp_arguments())).require_ok()

    async def posterior_gate_report(self, request: PosteriorGateArgs | Mapping[str, Any]) -> PosteriorGateReport:
        """Async typed posterior, release-gate, and comparison evidence."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return posterior_gate_report((await self.client.call_tool("posterior_gate", normalized.to_mcp_arguments())).require_ok())


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
        "ci_evidence",
        "execution_provenance",
    ):
        if kwargs.get(key) is not None:
            value = kwargs[key]
            arguments[key] = (
                value.to_mcp_arguments()
                if isinstance(value, (CiExecutionEvidenceRequest, ExecutionProvenanceRequest))
                else dict(value)
            )
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
