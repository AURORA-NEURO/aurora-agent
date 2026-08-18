"""Standard-library HTTP client for the bounded Prism API gateway.

The HTTP client is deliberately separate from the stdio MCP client.  It supports the gateway's
health/capability routes, REST tool calls, cursor-based events, and signed webhook outbox lifecycle;
it never retries a domain refusal automatically and never treats a 2xx transport response as proof
that a scientific claim was accepted.
"""

from __future__ import annotations

import asyncio
import http.client
import json
import math
import ssl
import time
from typing import Any, Mapping, Sequence
from urllib.parse import quote, urlencode, urlsplit

from .biological import AdapterPlanReport, AdapterPlanRequest, adapter_plan_report
from .adapter_execution_evidence import (
    AdapterExecutionEvidenceReport,
    AdapterExecutionEvidenceRequest,
    adapter_execution_evidence_report,
)
from .adapter_execution_evidence_query import (
    AdapterExecutionEvidenceQueryReport,
    AdapterExecutionEvidenceQueryRequest,
    adapter_execution_evidence_query_report,
)
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
from .domain_report_bridges import (
    AdapterDomainReportResult,
    adapter_domain_report_arguments,
    ProviderDomainReportResult,
    provider_domain_report_arguments,
    external_provider_domain_report_arguments,
)
from .domain_evidence import (
    DomainEvidenceHarmonizationCoverageReport,
    DomainEvidenceHarmonizationCoverageRequest,
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
from .domain_evidence_provider_handoff import (
    DomainEvidenceProviderHandoffReport,
    DomainEvidenceProviderHandoffRequest,
    domain_evidence_provider_handoff_report,
)
from .domain_evidence_provider_external import (
    DomainEvidenceProviderExternalPayloadReceiptReport,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest,
    DomainEvidenceProviderExternalPayloadReplayVerificationReport,
    DomainEvidenceProviderExternalPayloadNormalizationRequest,
    DomainEvidenceProviderExternalPayloadNormalizationReport,
    DomainEvidenceProviderExternalPayloadLineageAuditRequest,
    DomainEvidenceProviderExternalPayloadLineageAuditReport,
    DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest,
    DomainEvidenceProviderExternalPayloadExecutionEvidenceReport,
    domain_evidence_provider_external_payload_receipt_report,
    domain_evidence_provider_external_payload_replay_verification_report,
    domain_evidence_provider_external_payload_normalization_report,
    domain_evidence_provider_external_payload_lineage_audit_report,
    domain_evidence_provider_external_payload_execution_evidence_report,
)
from .domain_evidence_provider_external_query import (
    DomainEvidenceProviderExternalPayloadEvidenceQueryReport,
    DomainEvidenceProviderExternalPayloadEvidenceQueryRequest,
    domain_evidence_provider_external_payload_evidence_query_report,
)
from .domain_acquisition import (
    DOMAIN_ACQUISITION_WORKFLOW,
    DomainAcquisitionQuery,
    DomainAcquisitionReport,
    domain_acquisition_report,
)
from .capability import (
    CapabilityAuditReport,
    CapabilitySearchReport,
    CapabilityQuery,
    CapabilityRouteNeed,
    CapabilityRouteReport,
    CapabilityRouteReviewReport,
    CapabilityRouteReviewRequest,
    CapabilityRoutePlanReport,
    CapabilityRoutePlanRequest,
    CapabilityRoutePlanVerifyReport,
    CapabilityRoutePlanVerifyRequest,
    CapabilityRouteRequest,
    DomainWorkflowCatalogueReport,
    DomainWorkflowInstantiateRequest,
    DomainWorkflowInstantiationReport,
    DomainWorkflowPortfolioRequest,
    DomainWorkflowPortfolioReport,
    DomainWorkflowPortfolioVerifyRequest,
    DomainWorkflowPortfolioVerifyReport,
    DomainWorkflowVerifyRequest,
    DomainWorkflowVerifyReport,
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
    MissionEvaluatorReplayQueryReport,
    MissionEvaluatorReplayQueryRequest,
    MissionEvidenceBundleReport,
    MissionEvidenceBundleRequest,
    MissionEvidenceBundleImportReport,
    MissionEvidenceBundleImportRequest,
    MissionEvidenceBundleQueryReport,
    MissionEvidenceBundleQueryRequest,
    MissionEvidenceBundleGetReport,
    MissionEvidenceBundleGetRequest,
    MissionEvidenceBundleVerificationReport,
    MissionEvidenceBundleVerifyRequest,
    MissionEvaluatorReviewReport,
    MissionEvaluatorReviewRequest,
    MissionEvaluatorSearchReport,
    capability_audit_report,
    capability_route_report,
    capability_discover_report,
    capability_route_review_report,
    capability_route_plan_report,
    capability_route_plan_verify_report,
    domain_workflow_catalogue_report,
    domain_workflow_instantiation_report,
    domain_workflow_portfolio_report,
    domain_workflow_portfolio_verify_report,
    domain_workflow_verify_report,
    domain_workflow_scaffold_report,
    domain_workflow_reconciliation_report,
    mission_evaluator_discover_report,
    mission_evaluator_review_report,
    mission_evaluator_replay_report,
    mission_evaluator_replay_comparison_report,
    mission_evaluator_replay_query_report,
    mission_evidence_bundle_report,
    mission_evidence_bundle_verification_report,
)
from .capability_dashboard import CapabilityDashboardQueryArgs, CapabilityDashboardReport, capability_dashboard_report
from .ci_evidence import CiExecutionEvidenceReport, CiExecutionEvidenceRequest, ci_execution_evidence_report
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
from .execution_provenance import ExecutionProvenanceReport, ExecutionProvenanceRequest, execution_provenance_report
from .adaptive_execution import (
    AdaptiveCostedReport,
    AdaptiveCostedRequest,
    AdaptiveExecutionReport,
    AdaptiveExecutionRequest,
    adaptive_costed_report,
    adaptive_execution_report,
)
from .workflow_execution import (
    WorkflowExecutionReport,
    WorkflowExecutionRequest,
    workflow_execution_report,
)
from .workflow_execution_evidence import (
    WorkflowExecutionEvidenceReport,
    WorkflowExecutionEvidenceRequest,
    workflow_execution_evidence_report,
)
from .delivery_receipt import (
    DeveloperDeliveryReceiptReport,
    DeveloperDeliveryReceiptRequest,
    DeveloperDeliveryReceiptVerificationReport,
    DeveloperDeliveryReceiptVerificationRequest,
    developer_delivery_receipt_report,
    developer_delivery_receipt_verification_report,
)
from .conformance import ConformanceRunReport, conformance_run_report
from .context_requests import (
    ContextLayer,
    FiberCompileRequest,
    FiberExplainRequest,
    FiberRefineRequest,
    FiberVerifyRequest,
    ProjectionBundleRequest,
)
from .fiber_contract import (
    FiberAdaptiveAcquisitionSummary,
    FiberDecisionQuotientSummary,
    FiberRateDistortionSummary,
    fiber_adaptive_acquisition_summary,
    fiber_decision_quotient_summary,
    fiber_rate_distortion_summary,
)
from .delivery import DeveloperDeliveryAuditReport, developer_delivery_audit_report
from .developer_platform import (
    DeveloperPlatformStatusArgs,
    DeveloperPlatformStatusReport,
    developer_platform_status_report,
)
from .errors import ApiError, ArgumentError, MissionWaitTimeout, TransportError
from .events import (
    MAX_EVENT_PAGE,
    MAX_OPERATIONS_SNAPSHOT_LIMIT,
    MAX_OPERATIONS_DOMAIN_GROUPS,
    DeliveryAttemptPage,
    DeliveryPage,
    DeliveryReceiptAttempts,
    DeliveryReceiptEvents,
    EventPage,
    EventPersistenceStatus,
    OperationsSnapshot,
    OperationsHandoff,
    OperationsDomainActivity,
    OperationsDomainGates,
    OperationsGateReview,
    OperationsGateReviews,
    RecoveryMatrix,
    RouteReviewEvidence,
    SseSnapshot,
    parse_sse,
    validate_receipt_id,
    validate_review_id,
)
from .bioql import BioQlCompileRequest
from .evidence import (
    BioCapabilityEvidenceAuditReport,
    BioCapabilityEvidenceAuditRequest,
    biocapability_evidence_audit_report,
)
from .domain_requests import LabPlanRequest, RoutingDecisionRequest, WorldClaimCheckRequest
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
from .mission import (
    MissionAssembly,
    MissionClaimLineage,
    MAX_MISSION_LIST_LIMIT,
    MAX_MISSION_POLL_INTERVAL_SECONDS,
    MAX_MISSION_TRACE_PAGE,
    MAX_MISSION_WAIT_SECONDS,
    MissionJob,
    MissionExecutionProvenance,
    MissionInventoryPage,
    MissionPersistenceStatus,
    MissionQueueFlushResult,
    MissionQueueInventory,
    MissionQueueLockReleaseResult,
    MissionQueueStatus,
    MissionPolicy,
    MissionPreflight,
    MissionRequest,
    OperationsGateReviewRequest,
    MissionRouteSelection,
    MissionTracePage,
    mission_from_route as assemble_mission_from_route,
    preflight_mission,
)
from .publication import BioAtlasPublicationAuditReport, bioatlas_publication_audit_report
from .release import ReleaseAuditArgs, ReleaseAuditReport, release_audit_report
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
from .token_context import TokenContextPlanArgs, TokenContextPlanningReport, token_context_plan_report
from .weavelang import WeaveLangCompileArgs, WeaveLangCompileReport, weavelang_compile_report
from .epistemic import EpistemicVoiArgs, EpistemicVoiReport, epistemic_voi_report
from .epistemic_adaptive import EpistemicAdaptiveArgs, EpistemicAdaptiveReport, epistemic_adaptive_report
from .epistemic_quotient import EpistemicDecisionQuotientArgs, EpistemicDecisionQuotientReport, epistemic_decision_quotient_report
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
from .standards import MeasurementCompareArgs, MeasurementCompareReport, measurement_compare_report
from .world import (
    ObservedWorldDeclareArgs,
    ObservedWorldDeclareReport,
    WorldClaimCheckReport,
    observed_world_declare_report,
    world_claim_check_report,
)
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
from .atlas_surface import AtlasSurfaceAuditArgs, AtlasSurfaceAuditReport, atlas_surface_audit_report
from .engineering_manifest import EngineeringAuditReport, EngineeringManifestArgs, engineering_manifest_audit_report
from .engineering_plan import EngineeringPlanReport, EngineeringPlanRequestArgs, engineering_execution_plan_report
from .release_pipeline import ReleasePipelineAuditReport, ReleasePipelineManifestArgs, release_pipeline_audit_report
from .operational_readiness import OperationalReadinessAuditReport, OperationalReadinessManifestArgs, operational_readiness_audit_report
from .security_privacy import SecurityPrivacyAuditReport, SecurityPrivacyManifestArgs, security_privacy_audit_report
from .sandbox_admission import SandboxAuditReport, SandboxManifestArgs, sandbox_admission_audit_report
from .sandbox_runtime import SandboxRuntimeAuditReport, SandboxRuntimeManifestArgs, sandbox_runtime_simulate_report
from .security_program import SecurityProgramAuditReport, SecurityProgramManifestArgs, security_program_audit_report
from .adaptive_panel import AdaptivePanelReport, AdaptivePanelRunArgs, adaptive_panel_report
from .posterior_gate import PosteriorGateArgs, PosteriorGateReport, posterior_gate_report
from .tooling import ToolCallPlan, ToolCatalogue


def _capability_query_arguments(
    query: CapabilityQuery | str | None,
    *,
    text: str | None,
    domain: str | None,
    tool: str | None,
    group_id: str | None,
    max_items: int,
    include_tools: bool,
) -> dict[str, Any]:
    if isinstance(query, CapabilityQuery):
        if (
            any(value is not None for value in (text, domain, tool, group_id))
            or max_items != 50
            or include_tools
        ):
            raise ArgumentError("query cannot be combined with individual capability filters")
        return query.to_mcp_arguments()
    if isinstance(query, str):
        if text is not None:
            raise ArgumentError("query cannot be combined with text")
        return CapabilityQuery(query, group_id, domain, tool, max_items, include_tools).to_mcp_arguments()
    if query is not None:
        raise ArgumentError("query must be a CapabilityQuery or string")
    return CapabilityQuery(text, group_id, domain, tool, max_items, include_tools).to_mcp_arguments()


def _developer_delivery_arguments(
    *,
    request_id: str | None,
    targets: Sequence[str] | None,
    checks: Mapping[
        str,
        Mapping[str, Any]
        | CiExecutionEvidenceRequest
        | CiProviderNormalizationRequest
        | CiProviderEvidenceRequest
        | ExecutionProvenanceRequest
        | None,
    ],
) -> dict[str, Any]:
    arguments: dict[str, Any] = {}
    for name, value in checks.items():
        if value is not None:
            arguments[name] = (
                value.to_mcp_arguments()
                if isinstance(
                    value,
                    (
                        CiExecutionEvidenceRequest,
                        CiProviderNormalizationRequest,
                        CiProviderEvidenceRequest,
                        ExecutionProvenanceRequest,
                    ),
                )
                else dict(value)
            )
    if request_id is None and targets is None:
        return arguments
    if not isinstance(request_id, str) or not request_id:
        raise ArgumentError("request_id is required when targets are supplied")
    if not isinstance(targets, Sequence) or isinstance(targets, (str, bytes)) or not targets:
        raise ArgumentError("targets must contain at least one target")
    if any(not isinstance(target, str) or not target for target in targets):
        raise ArgumentError("targets must contain non-empty strings")
    if len(set(targets)) != len(targets):
        raise ArgumentError("targets must be unique")
    arguments["release_request"] = {"id": request_id, "targets": list(targets)}
    return arguments


class ApiClient:
    """Synchronous, bounded HTTP client for ``bioprism-api``."""

    def __init__(
        self,
        base_url: str,
        *,
        bearer_token: str | None = None,
        timeout: float = 30.0,
        max_response_bytes: int = 20_000_000,
        ssl_context: ssl.SSLContext | None = None,
    ) -> None:
        parsed = urlsplit(base_url.rstrip("/"))
        if parsed.scheme not in {"http", "https"} or not parsed.hostname:
            raise ArgumentError("base_url must be an http(s) URL with a host")
        if parsed.path not in {"", "/"} or parsed.query or parsed.fragment:
            raise ArgumentError("base_url must not include a path, query, or fragment")
        if timeout <= 0 or max_response_bytes <= 0:
            raise ArgumentError("timeout and max_response_bytes must be positive")
        if bearer_token is not None and (len(bearer_token) < 16 or any(ord(c) <= 0x20 for c in bearer_token)):
            raise ArgumentError("bearer_token must contain at least 16 visible characters")
        self.base_url = parsed
        self.bearer_token = bearer_token
        self.timeout = timeout
        self.max_response_bytes = max_response_bytes
        self.ssl_context = ssl_context

    def request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        *,
        headers: Mapping[str, str] | None = None,
    ) -> dict[str, Any]:
        if method not in {"GET", "POST", "DELETE", "OPTIONS"}:
            raise ArgumentError("method must be GET, POST, DELETE, or OPTIONS")
        if not path.startswith("/") or "\r" in path or "\n" in path:
            raise ArgumentError("path must be an origin-form path")
        body = b""
        request_headers = {"Accept": "application/json"}
        if payload is not None:
            try:
                body = json.dumps(payload, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise ArgumentError(f"payload is not JSON-safe: {error}") from error
            request_headers["Content-Type"] = "application/json"
        if self.bearer_token is not None:
            request_headers["Authorization"] = f"Bearer {self.bearer_token}"
        if headers is not None:
            for name, value in headers.items():
                if not name or "\r" in name or "\n" in name or "\r" in value or "\n" in value:
                    raise ArgumentError("HTTP headers must not contain control-line breaks")
                request_headers[name] = value
        connection: http.client.HTTPConnection | http.client.HTTPSConnection
        try:
            if self.base_url.scheme == "https":
                connection = http.client.HTTPSConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                    context=self.ssl_context,
                )
            else:
                connection = http.client.HTTPConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                )
            connection.request(method, path, body=body, headers=request_headers)
            response = connection.getresponse()
            raw = response.read(self.max_response_bytes + 1)
            status = response.status
        except (OSError, http.client.HTTPException) as error:
            raise TransportError(f"HTTP API request failed: {error}") from error
        finally:
            try:
                connection.close()
            except UnboundLocalError:
                pass
        if len(raw) > self.max_response_bytes:
            raise TransportError("HTTP API response exceeded max_response_bytes")
        if not raw:
            parsed: Any = {}
        else:
            try:
                parsed = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise TransportError(f"HTTP API returned invalid JSON: {error}") from error
        if not isinstance(parsed, dict):
            raise TransportError("HTTP API response must be a JSON object")
        if status >= 400:
            raise ApiError(status, parsed)
        return parsed

    def health(self) -> dict[str, Any]:
        return self.request("GET", "/healthz")

    def request_text(
        self,
        method: str,
        path: str,
        *,
        headers: Mapping[str, str] | None = None,
    ) -> tuple[str, dict[str, str]]:
        """Issue a bounded text request for protocol surfaces such as SSE snapshots."""

        if method not in {"GET", "OPTIONS"}:
            raise ArgumentError("text requests support only GET or OPTIONS")
        if not path.startswith("/") or "\r" in path or "\n" in path:
            raise ArgumentError("path must be an origin-form path")
        request_headers = {"Accept": "text/event-stream"}
        if self.bearer_token is not None:
            request_headers["Authorization"] = f"Bearer {self.bearer_token}"
        if headers is not None:
            for name, value in headers.items():
                if not name or "\r" in name or "\n" in name or "\r" in value or "\n" in value:
                    raise ArgumentError("HTTP headers must not contain control-line breaks")
                request_headers[name] = value
        connection: http.client.HTTPConnection | http.client.HTTPSConnection
        try:
            if self.base_url.scheme == "https":
                connection = http.client.HTTPSConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                    context=self.ssl_context,
                )
            else:
                connection = http.client.HTTPConnection(
                    self.base_url.hostname,
                    self.base_url.port,
                    timeout=self.timeout,
                )
            connection.request(method, path, headers=request_headers)
            response = connection.getresponse()
            raw = response.read(self.max_response_bytes + 1)
            status = response.status
            response_headers = {name.lower(): value for name, value in response.getheaders()}
        except (OSError, http.client.HTTPException) as error:
            raise TransportError(f"HTTP API request failed: {error}") from error
        finally:
            try:
                connection.close()
            except UnboundLocalError:
                pass
        if len(raw) > self.max_response_bytes:
            raise TransportError("HTTP API response exceeded max_response_bytes")
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError as error:
            raise TransportError(f"HTTP API returned invalid UTF-8 text: {error}") from error
        if status >= 400:
            try:
                payload = json.loads(text)
            except json.JSONDecodeError:
                payload = {"error": text}
            raise ApiError(status, payload)
        return text, response_headers

    def capabilities(self) -> dict[str, Any]:
        return self.request("GET", "/v1/capabilities")

    def tools(self) -> list[dict[str, Any]]:
        value = self.request("GET", "/v1/tools")
        tools = value.get("tools")
        if not isinstance(tools, list) or any(not isinstance(tool, dict) for tool in tools):
            raise TransportError("HTTP API tools response has no object array")
        return tools

    def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        if not isinstance(name, str) or not name or "/" in name:
            raise ArgumentError("tool name must be a non-empty path-safe string")
        return self.request("POST", f"/v1/tools/{name}", dict(arguments or {}))

    def register_artifact(
        self,
        request: ArtifactRegistrationRequest | Mapping[str, Any],
    ) -> ArtifactRegistrationReport:
        normalized = (
            request
            if isinstance(request, ArtifactRegistrationRequest)
            else ArtifactRegistrationRequest(**dict(request))
        )
        return ArtifactRegistrationReport.from_wire(
            self.request("POST", "/v1/artifacts", normalized.to_arguments())
        )

    def query_artifacts(
        self,
        request: ArtifactQueryRequest | Mapping[str, Any] | None = None,
    ) -> ArtifactQueryReport:
        normalized = (
            request
            if isinstance(request, ArtifactQueryRequest)
            else ArtifactQueryRequest(**dict(request or {}))
        )
        return ArtifactQueryReport.from_wire(
            self.request("GET", f"/v1/artifacts?{urlencode(normalized.to_query_params())}")
        )

    def get_artifact(
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
            self.request(
                "GET",
                f"/v1/artifacts/{quote(normalized.content_digest, safe='')}",
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
            self.request(
                "GET",
                f"/v1/artifacts/{quote(normalized.content_digest, safe='')}/lineage",
            )
        )

    def artifact_registry_persistence(self) -> dict[str, Any]:
        return self.request("GET", "/v1/artifacts/persistence")

    def artifact_cross_store_audit(self) -> ArtifactCrossStoreAuditReport:
        """Compare retained evidence/reconciliation identities with artifact projections."""

        return ArtifactCrossStoreAuditReport.from_wire(
            self.request("GET", "/v1/artifacts/cross-store")
        )

    def flush_artifact_registry_persistence(self) -> dict[str, Any]:
        return self.request("POST", "/v1/artifacts/persistence/flush", {})

    def artifact_registry_audit(self, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Call the MCP artifact registry tool while preserving its bounded evidence envelope."""

        return self.call_tool("artifact_registry_audit", arguments)

    def domain_report_project(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        """Project and index one explicit caller-supplied report over REST."""

        normalized = (
            request
            if isinstance(request, DomainReportProjectRequest)
            else DomainReportProjectRequest(**dict(request))
        )
        return DomainReportProjectReport.from_wire(
            self.request("POST", "/v1/domain-reports", normalized.to_arguments())
        )

    def domain_report_coverage(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        """Report retained structured projections by capability group over REST."""

        normalized = (
            request
            if isinstance(request, DomainReportCoverageRequest)
            else DomainReportCoverageRequest(**dict(request or {}))
        )
        return DomainReportCoverageReport.from_wire(
            self.request(
                "GET",
                f"/v1/domain-reports/coverage?{urlencode(normalized.to_query_params())}",
            )
        )

    def domain_report_project_tool(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        normalized = (
            request
            if isinstance(request, DomainReportProjectRequest)
            else DomainReportProjectRequest(**dict(request))
        )
        return DomainReportProjectReport.from_wire(
            self.call_tool("domain_report_project", normalized.to_arguments())
        )

    def domain_report_from_adapter_execution(
        self,
        evidence: AdapterExecutionEvidenceRequest | Mapping[str, Any],
        conformance: Mapping[str, Any] | None = None,
    ) -> AdapterDomainReportResult:
        """Validate/index adapter evidence and compose its canonical domain report over MCP."""

        return AdapterDomainReportResult.from_wire(
            self.call_tool(
                "domain_report_project",
                adapter_domain_report_arguments(evidence, conformance),
            )
        )

    def domain_report_from_provider_normalization(
        self,
        normalization: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> ProviderDomainReportResult:
        """Normalize provider evidence and compose its canonical domain report over MCP."""

        return ProviderDomainReportResult.from_wire(
            self.call_tool(
                "domain_report_project",
                provider_domain_report_arguments(normalization),
            )
        )

    def domain_report_from_external_provider_normalization(
        self,
        normalization: DomainEvidenceProviderExternalPayloadNormalizationRequest
        | Mapping[str, Any],
    ) -> ProviderDomainReportResult:
        """Compose a domain report from receipt-verified external provider normalization."""

        return ProviderDomainReportResult.from_wire(
            self.call_tool(
                "domain_report_project",
                external_provider_domain_report_arguments(normalization),
            )
        )

    def domain_report_coverage_tool(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        normalized = (
            request
            if isinstance(request, DomainReportCoverageRequest)
            else DomainReportCoverageRequest(**dict(request or {}))
        )
        return DomainReportCoverageReport.from_wire(
            self.call_tool("domain_report_project", normalized.to_arguments())
        )

    def domain_evidence_harmonize(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        """Join exact domain reports into a traceability-only artifact over REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizeRequest)
            else DomainEvidenceHarmonizeRequest(**dict(request))
        )
        return DomainEvidenceHarmonizationReport.from_wire(
            self.request("POST", "/v1/domain-evidence/harmonize", normalized.to_arguments())
        )

    def domain_evidence_harmonize_tool(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizeRequest)
            else DomainEvidenceHarmonizeRequest(**dict(request))
        )
        return DomainEvidenceHarmonizationReport.from_wire(
            self.call_tool("domain_evidence_harmonize", normalized.to_arguments())
        )

    def domain_evidence_harmonization_coverage(
        self,
        request: DomainEvidenceHarmonizationCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceHarmonizationCoverageReport:
        """Query retained harmonization rows through the cursor-based REST route."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizationCoverageRequest)
            else DomainEvidenceHarmonizationCoverageRequest(**dict(request or {}))
        )
        return DomainEvidenceHarmonizationCoverageReport.from_wire(
            self.request(
                "GET",
                f"/v1/domain-evidence/harmonization/coverage?{urlencode(normalized.to_query_params())}",
            )
        )

    def domain_evidence_harmonization_coverage_tool(
        self,
        request: DomainEvidenceHarmonizationCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceHarmonizationCoverageReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceHarmonizationCoverageRequest)
            else DomainEvidenceHarmonizationCoverageRequest(**dict(request or {}))
        )
        return DomainEvidenceHarmonizationCoverageReport.from_wire(
            self.call_tool(
                "domain_evidence_harmonization_coverage", normalized.to_arguments()
            )
        )

    def domain_evidence_intake(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        """Retain one raw source-tool envelope as an exact-digest intake artifact over REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeRequest)
            else DomainEvidenceIntakeRequest(**dict(request))
        )
        return DomainEvidenceIntakeReport.from_wire(
            self.request("POST", "/v1/domain-evidence/intake", normalized.to_arguments())
        )

    def domain_evidence_intake_tool(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeRequest)
            else DomainEvidenceIntakeRequest(**dict(request))
        )
        return DomainEvidenceIntakeReport.from_wire(
            self.call_tool("domain_evidence_intake", normalized.to_arguments())
        )

    def domain_evidence_coverage(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        """Report retained raw-intake coverage by authoritative group over REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeCoverageRequest)
            else DomainEvidenceIntakeCoverageRequest(**dict(request or {}))
        )
        return DomainEvidenceIntakeCoverageReport.from_wire(
            self.request(
                "GET",
                f"/v1/domain-evidence/coverage?{urlencode(normalized.to_query_params())}",
            )
        )

    def domain_evidence_coverage_tool(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceIntakeCoverageRequest)
            else DomainEvidenceIntakeCoverageRequest(**dict(request or {}))
        )
        return DomainEvidenceIntakeCoverageReport.from_wire(
            self.call_tool("domain_evidence_coverage", normalized.to_arguments())
        )

    def domain_evidence_source_plan(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        """Plan and index a non-fetching external evidence source over REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceSourcePlanRequest)
            else DomainEvidenceSourcePlanRequest(**dict(request))
        )
        return DomainEvidenceSourcePlanReport.from_wire(
            self.request("POST", "/v1/domain-evidence/sources", normalized.to_arguments())
        )

    def domain_evidence_source_plan_tool(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourcePlanRequest)
            else DomainEvidenceSourcePlanRequest(**dict(request))
        )
        return DomainEvidenceSourcePlanReport.from_wire(
            self.call_tool("domain_evidence_source_plan", normalized.to_arguments())
        )

    def domain_evidence_source_execute(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        """Execute a retained source plan through the bounded connector kernel over REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceSourceExecutionRequest)
            else DomainEvidenceSourceExecutionRequest(**dict(request))
        )
        return DomainEvidenceSourceExecutionReport.from_wire(
            self.request("POST", "/v1/domain-evidence/sources/execute", normalized.to_arguments())
        )

    def domain_evidence_source_execute_tool(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceSourceExecutionRequest)
            else DomainEvidenceSourceExecutionRequest(**dict(request))
        )
        return DomainEvidenceSourceExecutionReport.from_wire(
            self.call_tool("domain_evidence_source_execute", normalized.to_arguments())
        )

    def domain_evidence_source_project(
        self,
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: SourceAdapterProjectionRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> SourceAdapterProjectionResult:
        """Project a returned bounded source envelope through one local Python adapter.

        This is deliberately a local operation: it never performs another HTTP request or reads
        the original locator. The source response remains the sole parser input.
        """

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
        """Project a source envelope only when the selected adapter is declared for its domain."""

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
    ) -> DomainEvidenceProviderNormalizationReport:
        """Normalize and retain a caller-managed provider payload through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderNormalizationRequest)
            else DomainEvidenceProviderNormalizationRequest(**dict(request))
        )
        return domain_evidence_provider_normalization_report(
            self.request("POST", "/v1/tools/domain_evidence_provider_normalize", normalized.to_mcp_arguments())
        )

    def domain_evidence_provider_normalize_tool(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderNormalizationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderNormalizationRequest)
            else DomainEvidenceProviderNormalizationRequest(**dict(request))
        )
        return domain_evidence_provider_normalization_report(
            self.call_tool("domain_evidence_provider_normalize", normalized.to_mcp_arguments())
        )

    def domain_evidence_provider_replay_verify(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        """Verify a caller-managed provider payload against retained identities through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderReplayRequest)
            else DomainEvidenceProviderReplayRequest(**dict(request))
        )
        return domain_evidence_provider_replay_verification_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_replay_verify",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_replay_verify_tool(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderReplayRequest)
            else DomainEvidenceProviderReplayRequest(**dict(request))
        )
        return domain_evidence_provider_replay_verification_report(
            self.call_tool(
                "domain_evidence_provider_replay_verify", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_connector_handoff(
        self,
        request: DomainEvidenceProviderHandoffRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderHandoffReport:
        """Declare and retain a caller-managed connector boundary through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderHandoffRequest)
            else DomainEvidenceProviderHandoffRequest.from_wire(request)
        )
        return domain_evidence_provider_handoff_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_connector_handoff",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_connector_handoff_tool(
        self,
        request: DomainEvidenceProviderHandoffRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderHandoffReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderHandoffRequest)
            else DomainEvidenceProviderHandoffRequest.from_wire(request)
        )
        return domain_evidence_provider_handoff_report(
            self.call_tool(
                "domain_evidence_provider_connector_handoff", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_receipt(
        self,
        request: DomainEvidenceProviderExternalPayloadReceiptRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReceiptReport:
        """Retain metadata for an out-of-line provider payload through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadReceiptRequest)
            else DomainEvidenceProviderExternalPayloadReceiptRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_receipt_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_receipt",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_receipt_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadReceiptRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReceiptReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadReceiptRequest)
            else DomainEvidenceProviderExternalPayloadReceiptRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_receipt_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_receipt", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_replay_verify(
        self,
        request: DomainEvidenceProviderExternalPayloadReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReplayVerificationReport:
        """Verify retained external receipt metadata through the REST tool route."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadReplayRequest)
            else DomainEvidenceProviderExternalPayloadReplayRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_replay_verification_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_replay_verify",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_replay_verify_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReplayVerificationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadReplayRequest)
            else DomainEvidenceProviderExternalPayloadReplayRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_replay_verification_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_replay_verify", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_normalize(
        self,
        request: DomainEvidenceProviderExternalPayloadNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadNormalizationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadNormalizationRequest)
            else DomainEvidenceProviderExternalPayloadNormalizationRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_normalization_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_normalize",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_normalize_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadNormalizationReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadNormalizationRequest)
            else DomainEvidenceProviderExternalPayloadNormalizationRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_normalization_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_normalize", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_lineage_audit(
        self,
        request: DomainEvidenceProviderExternalPayloadLineageAuditRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadLineageAuditReport:
        """Audit external receipt lineage against a retained handoff through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadLineageAuditRequest)
            else DomainEvidenceProviderExternalPayloadLineageAuditRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_lineage_audit_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_lineage_audit",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_lineage_audit_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadLineageAuditRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadLineageAuditReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadLineageAuditRequest)
            else DomainEvidenceProviderExternalPayloadLineageAuditRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_lineage_audit_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_lineage_audit", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_execution_evidence(
        self,
        request: DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadExecutionEvidenceReport:
        """Retain caller-reported external transfer evidence through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest)
            else DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_execution_evidence_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_execution_evidence",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_execution_evidence_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadExecutionEvidenceReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest)
            else DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest.from_wire(request)
        )
        return domain_evidence_provider_external_payload_execution_evidence_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_execution_evidence", normalized.to_mcp_arguments()
            )
        )

    def domain_evidence_provider_external_payload_evidence_query(
        self,
        request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
        """Return a joined receipt/lineage/execution projection through REST."""

        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadEvidenceQueryRequest)
            else DomainEvidenceProviderExternalPayloadEvidenceQueryRequest.from_wire(request or {})
        )
        return domain_evidence_provider_external_payload_evidence_query_report(
            self.request(
                "POST",
                "/v1/tools/domain_evidence_provider_external_payload_evidence_query",
                normalized.to_mcp_arguments(),
            )
        )

    def domain_evidence_provider_external_payload_evidence_query_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
        normalized = (
            request
            if isinstance(request, DomainEvidenceProviderExternalPayloadEvidenceQueryRequest)
            else DomainEvidenceProviderExternalPayloadEvidenceQueryRequest.from_wire(request or {})
        )
        return domain_evidence_provider_external_payload_evidence_query_report(
            self.call_tool(
                "domain_evidence_provider_external_payload_evidence_query", normalized.to_mcp_arguments()
            )
        )

    def domain_workflow_catalogue(self) -> dict[str, Any]:
        """Read the deterministic workflow template for every capability group."""

        return self.request("GET", "/v1/domain-workflows")

    def domain_workflow_catalogue_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(self.domain_workflow_catalogue())

    def domain_workflow_scaffold(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Build and preflight an execution-disabled scaffold for one capability group."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowScaffoldRequest)
            else DomainWorkflowScaffoldRequest(**dict(request))
        )
        return self.request("POST", "/v1/domain-workflows/scaffold", normalized.to_arguments())

    def domain_workflow_scaffold_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(self.domain_workflow_scaffold(request))

    def domain_workflow_instantiate(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Instantiate and authoritative-preflight one group-scoped workflow without dispatch."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowInstantiateRequest)
            else DomainWorkflowInstantiateRequest(**dict(request))
        )
        return self.request(
            "POST", "/v1/domain-workflows/instantiate", normalized.to_arguments()
        )

    def domain_workflow_instantiation_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(self.domain_workflow_instantiate(request))

    def domain_workflow_portfolio(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a bounded multi-domain workflow portfolio through REST."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowPortfolioRequest)
            else DomainWorkflowPortfolioRequest(**dict(request))
        )
        return self.request("POST", "/v1/domain-workflows/portfolio", normalized.to_arguments())

    def domain_workflow_portfolio_report(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioReport:
        return domain_workflow_portfolio_report(self.domain_workflow_portfolio(request))

    def domain_workflow_portfolio_verify(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a retained portfolio through the dedicated REST route."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowPortfolioVerifyRequest)
            else DomainWorkflowPortfolioVerifyRequest(**dict(request))
        )
        return self.request(
            "POST", "/v1/domain-workflows/portfolio/verify", normalized.to_arguments()
        )

    def domain_workflow_portfolio_verify_report(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioVerifyReport:
        return domain_workflow_portfolio_verify_report(self.domain_workflow_portfolio_verify(request))

    def domain_workflow_verify(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a retained workflow contract through the dedicated REST route."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowVerifyRequest)
            else DomainWorkflowVerifyRequest(**dict(request))
        )
        return self.request("POST", "/v1/domain-workflows/verify", normalized.to_arguments())

    def domain_workflow_verify_report(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowVerifyReport:
        return domain_workflow_verify_report(self.domain_workflow_verify(request))

    def domain_workflow_reconcile(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Reconcile retained mission evidence against a workflow contract without dispatch."""

        normalized = (
            request
            if isinstance(request, DomainWorkflowReconcileRequest)
            else DomainWorkflowReconcileRequest(**dict(request))
        )
        return self.request("POST", "/v1/domain-workflows/reconcile", normalized.to_arguments())

    def domain_workflow_reconciliation_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(self.domain_workflow_reconcile(request))

    def domain_workflow_catalogue_tool(self) -> dict[str, Any]:
        return self.call_tool("domain_workflow_catalogue", {})

    def domain_workflow_catalogue_tool_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(self.domain_workflow_catalogue_tool())

    def domain_workflow_scaffold_tool(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowScaffoldRequest)
            else DomainWorkflowScaffoldRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_scaffold", normalized.to_arguments())

    def domain_workflow_scaffold_tool_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(self.domain_workflow_scaffold_tool(request))

    def domain_workflow_instantiate_tool(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowInstantiateRequest)
            else DomainWorkflowInstantiateRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_instantiate", normalized.to_arguments())

    def domain_workflow_instantiation_tool_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(self.domain_workflow_instantiate_tool(request))

    def domain_workflow_portfolio_tool(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowPortfolioRequest)
            else DomainWorkflowPortfolioRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_portfolio", normalized.to_arguments())

    def domain_workflow_portfolio_tool_report(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioReport:
        return domain_workflow_portfolio_report(self.domain_workflow_portfolio_tool(request))

    def domain_workflow_portfolio_verify_tool(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowPortfolioVerifyRequest)
            else DomainWorkflowPortfolioVerifyRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_portfolio_verify", normalized.to_arguments())

    def domain_workflow_portfolio_verify_tool_report(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioVerifyReport:
        return domain_workflow_portfolio_verify_report(
            self.domain_workflow_portfolio_verify_tool(request)
        )

    def domain_workflow_verify_tool(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowVerifyRequest)
            else DomainWorkflowVerifyRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_verify", normalized.to_arguments())

    def domain_workflow_verify_tool_report(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowVerifyReport:
        return domain_workflow_verify_report(self.domain_workflow_verify_tool(request))

    def domain_workflow_reconcile_tool(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconcileRequest)
            else DomainWorkflowReconcileRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_reconcile", normalized.to_arguments())

    def domain_workflow_reconciliation_tool_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(self.domain_workflow_reconcile_tool(request))

    def domain_workflow_reconciliation_import(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationImportRequest)
            else DomainWorkflowReconciliationImportRequest(**dict(request))
        )
        return self.request("POST", "/v1/domain-workflows/reconciliations", normalized.to_http_body())

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
        query = urlencode(normalized.to_query_params())
        return self.request("GET", f"/v1/domain-workflows/reconciliations?{query}")

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
        return self.request(
            "GET",
            f"/v1/domain-workflows/reconciliations/{quote(normalized.reconciliation_digest, safe='')}",
        )

    def domain_workflow_reconciliation_get_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            self.domain_workflow_reconciliation_get(request)
        )

    def domain_workflow_reconciliation_import_tool(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationImportRequest)
            else DomainWorkflowReconciliationImportRequest(**dict(request))
        )
        return self.call_tool("domain_workflow_reconciliation_import", normalized.to_arguments())

    def domain_workflow_reconciliation_import_tool_report(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationImportReport:
        return DomainWorkflowReconciliationImportReport.from_wire(
            self.domain_workflow_reconciliation_import_tool(request)
        )

    def domain_workflow_reconciliation_query_tool(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        normalized = (
            request
            if isinstance(request, DomainWorkflowReconciliationQueryRequest)
            else DomainWorkflowReconciliationQueryRequest(**dict(request or {}))
        )
        return self.call_tool("domain_workflow_reconciliation_query", normalized.to_arguments())

    def domain_workflow_reconciliation_query_tool_report(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainWorkflowReconciliationQueryReport:
        return DomainWorkflowReconciliationQueryReport.from_wire(
            self.domain_workflow_reconciliation_query_tool(request)
        )

    def domain_workflow_reconciliation_get_tool(
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
        return self.call_tool("domain_workflow_reconciliation_get", normalized.to_arguments())

    def domain_workflow_reconciliation_get_tool_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            self.domain_workflow_reconciliation_get_tool(request)
        )

    def submit_mission(self, request: MissionRequest | Mapping[str, Any]) -> MissionJob:
        """Submit a validated mission to the cooperative asynchronous HTTP executor."""

        if not isinstance(request, (MissionRequest, Mapping)):
            raise ArgumentError("mission request must be a MissionRequest or mapping")
        arguments = request.to_mcp_arguments() if isinstance(request, MissionRequest) else dict(request)
        return MissionJob.from_wire(self.request("POST", "/v1/missions", arguments))

    def mission_status(self, mission_id: str) -> MissionJob:
        self._mission_id(mission_id)
        return MissionJob.from_wire(self.request("GET", f"/v1/missions/{mission_id}"))

    def mission_provenance(self, mission_id: str) -> MissionExecutionProvenance:
        """Read the retained gate/review/evaluator evidence for an accepted execution."""

        self._mission_id(mission_id)
        return MissionExecutionProvenance.from_wire(
            self.request("GET", f"/v1/missions/{mission_id}/provenance")
        )

    def mission_claim_lineage(self, mission_id: str) -> MissionClaimLineage:
        """Read the bounded claim-to-step evidence projection for a terminal mission."""

        self._mission_id(mission_id)
        return MissionClaimLineage.from_wire(
            self.request("GET", f"/v1/missions/{mission_id}/claims")
        )

    def mission_evaluator_replay_query(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Read durable full or summary-only evaluator replay evidence for one mission."""

        if isinstance(request, MissionEvaluatorReplayQueryRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvaluatorReplayQueryRequest(request)
        elif isinstance(request, Mapping):
            normalized = MissionEvaluatorReplayQueryRequest(**dict(request))
        else:
            raise ArgumentError("mission evaluator replay query must be a request, mapping, or mission id")
        self._mission_id(normalized.mission_id)
        query = urlencode(normalized.to_query_params())
        return self.request(
            "GET",
            f"/v1/missions/{quote(normalized.mission_id, safe='')}/evaluator-replay?{query}",
        )

    def mission_evaluator_replay_query_report(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> MissionEvaluatorReplayQueryReport:
        """Return the typed durable evaluator replay query projection."""

        return mission_evaluator_replay_query_report(self.mission_evaluator_replay_query(request))

    def mission_evaluator_replay_compare_query(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Compare durable replay evidence with the current evaluator catalogue."""

        if isinstance(request, MissionEvaluatorReplayQueryRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvaluatorReplayQueryRequest(request)
        elif isinstance(request, Mapping):
            normalized = MissionEvaluatorReplayQueryRequest(**dict(request))
        else:
            raise ArgumentError("mission evaluator replay comparison query must be a request, mapping, or mission id")
        self._mission_id(normalized.mission_id)
        query = urlencode(normalized.to_query_params())
        return self.request(
            "GET",
            f"/v1/missions/{quote(normalized.mission_id, safe='')}/evaluator-replay/compare?{query}",
        )

    def mission_evaluator_replay_compare_query_report(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed catalog-drift comparison evidence from the durable REST route."""

        return mission_evaluator_replay_comparison_report(
            self.mission_evaluator_replay_compare_query(request)
        )

    def mission_evidence_bundle(
        self,
        request: MissionEvidenceBundleRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Export a bounded content-addressed mission evidence bundle."""

        if isinstance(request, MissionEvidenceBundleRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvidenceBundleRequest(request)
        elif isinstance(request, Mapping):
            normalized = MissionEvidenceBundleRequest(**dict(request))
        else:
            raise ArgumentError("mission evidence bundle must be a request, mapping, or mission id")
        self._mission_id(normalized.mission_id)
        query = urlencode(normalized.to_query_params())
        return self.request(
            "GET",
            f"/v1/missions/{quote(normalized.mission_id, safe='')}/evidence-bundle?{query}",
        )

    def mission_evidence_bundle_report(
        self,
        request: MissionEvidenceBundleRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleReport:
        """Return the typed durable mission evidence bundle."""

        return mission_evidence_bundle_report(self.mission_evidence_bundle(request))

    def mission_evidence_bundle_verify(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify one portable mission evidence bundle through the REST gateway."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleVerifyRequest)
            else MissionEvidenceBundleVerifyRequest(**dict(request))
        )
        return self.request("POST", "/v1/evidence-bundles/verify", normalized.to_http_body())

    def mission_evidence_bundle_verification_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed portable mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(self.mission_evidence_bundle_verify(request))

    def mission_evidence_bundle_import(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Import one verified evidence bundle into the durable REST registry."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleImportRequest)
            else MissionEvidenceBundleImportRequest(**dict(request))
        )
        return self.request("POST", "/v1/evidence-bundles", normalized.to_http_body())

    def mission_evidence_bundle_import_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        """Return typed idempotent evidence registry import evidence."""

        return MissionEvidenceBundleImportReport.from_wire(self.mission_evidence_bundle_import(request))

    def mission_evidence_bundle_query(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Query deterministic mission/domain rows in the durable REST registry."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleQueryRequest)
            else MissionEvidenceBundleQueryRequest(**dict(request or {}))
        )
        query = urlencode(normalized.to_query_params())
        return self.request("GET", f"/v1/evidence-bundles?{query}")

    def mission_evidence_bundle_query_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        """Return typed deterministic evidence registry index evidence."""

        return MissionEvidenceBundleQueryReport.from_wire(self.mission_evidence_bundle_query(request))

    def mission_evidence_bundle_get(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Fetch one verified evidence bundle from the durable REST registry."""

        if isinstance(request, MissionEvidenceBundleGetRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvidenceBundleGetRequest(request)
        else:
            normalized = MissionEvidenceBundleGetRequest(**dict(request))
        return self.request(
            "GET",
            f"/v1/evidence-bundles/{quote(normalized.bundle_digest, safe='')}",
        )

    def mission_evidence_bundle_get_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        """Return typed lookup evidence for one registry bundle."""

        return MissionEvidenceBundleGetReport.from_wire(self.mission_evidence_bundle_get(request))

    def mission_trace(self, mission_id: str, *, after: int = 0, limit: int = 100) -> MissionTracePage:
        """Read a bounded cursor page from the authoritative clock-free mission trace."""

        self._mission_id(mission_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_MISSION_TRACE_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_MISSION_TRACE_PAGE}")
        query = urlencode({"after": str(after), "limit": str(limit)})
        return MissionTracePage.from_wire(self.request("GET", f"/v1/missions/{mission_id}/trace?{query}"))

    def wait_mission(
        self,
        mission_id: str,
        *,
        timeout: float = 30.0,
        poll_interval: float = 0.25,
    ) -> MissionJob:
        """Poll until a mission is terminal, with explicit bounds and the last live snapshot on timeout."""

        self._mission_id(mission_id)
        timeout_value, poll_value = self._mission_wait_options(timeout, poll_interval)
        deadline = time.monotonic() + timeout_value
        job = self.mission_status(mission_id)
        while not job.terminal:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise MissionWaitTimeout(mission_id, timeout_value, job)
            time.sleep(min(poll_value, remaining))
            job = self.mission_status(mission_id)
        return job

    def cancel_mission(self, mission_id: str, reason: str | None = None) -> MissionJob:
        self._mission_id(mission_id)
        if reason is not None and (not isinstance(reason, str) or not reason.strip() or len(reason) > 2_048):
            raise ArgumentError("reason must be a non-empty string of at most 2048 bytes")
        payload = {} if reason is None else {"reason": reason}
        return MissionJob.from_wire(self.request("POST", f"/v1/missions/{mission_id}/cancel", payload))

    def delete_mission(self, mission_id: str) -> dict[str, Any]:
        """Remove a terminal mission from the bounded in-process registry."""

        self._mission_id(mission_id)
        return self.request("DELETE", f"/v1/missions/{mission_id}")

    def tool_catalogue(self) -> ToolCatalogue:
        """Snapshot the authoritative live HTTP ``/v1/tools`` catalogue."""

        return ToolCatalogue.from_definitions(self.tools())

    def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate any advertised tool's JSON shape without issuing a POST."""

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
        """Run any advertised tool after conservative schema preflight."""

        plan = self.plan_tool(name, arguments, catalogue=catalogue)
        return self.call_tool(plan.tool, plan.to_mcp_arguments())

    def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Review a mission against the live HTTP tool catalogue without issuing a POST."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else self.tool_catalogue()
        return preflight_mission(request, snapshot)

    def preflight_mission(self, request: MissionRequest | Mapping[str, Any]) -> dict[str, Any]:
        """Ask the Rust gateway for a no-dispatch, authoritative mission plan."""

        if not isinstance(request, (MissionRequest, Mapping)):
            raise ArgumentError("mission request must be a MissionRequest or mapping")
        arguments = request.to_mcp_arguments() if isinstance(request, MissionRequest) else dict(request)
        return self.request("POST", "/v1/missions/preflight", arguments)

    def mission_inventory(self, *, status: str | None = None, limit: int = 100) -> MissionInventoryPage:
        """Return a typed bounded page from the authoritative process-local registry."""

        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_MISSION_LIST_LIMIT:
            raise ArgumentError(f"limit must be between 1 and {MAX_MISSION_LIST_LIMIT}")
        if status is not None and (not isinstance(status, str) or not status.strip()):
            raise ArgumentError("status must be a non-empty string when supplied")
        query: dict[str, str] = {"limit": str(limit)}
        if status is not None:
            query["status"] = status
        return MissionInventoryPage.from_wire(self.request("GET", f"/v1/missions?{urlencode(query)}"))

    def mission_persistence(self) -> MissionPersistenceStatus:
        """Inspect the optional restart-aware mission checkpoint and its bounded file state."""

        return MissionPersistenceStatus.from_wire(self.request("GET", "/v1/missions/persistence"))

    def flush_mission_persistence(self) -> MissionPersistenceStatus:
        """Force a checkpoint and return the gateway's resulting persistence status."""

        return MissionPersistenceStatus.from_wire(
            self.request("POST", "/v1/missions/persistence/flush", {})
        )

    def mission_queue(self) -> MissionQueueInventory:
        """Return the bounded queue projection without exposing checkpointed job specifications."""

        return MissionQueueInventory.from_wire(self.request("GET", "/v1/missions/queue"))

    def mission_queue_persistence(self) -> MissionQueueStatus:
        """Inspect the queue checkpoint digest, recovery rows, and no-resume boundary."""

        return MissionQueueStatus.from_wire(
            self.request("GET", "/v1/missions/queue/persistence")
        )

    def flush_mission_queue_persistence(self) -> MissionQueueFlushResult:
        """Atomically checkpoint the queue and return the resulting integrity projection."""

        return MissionQueueFlushResult.from_wire(
            self.request("POST", "/v1/missions/queue/persistence/flush", {})
        )

    def release_mission_queue_lock(self, operator: str, reason: str) -> MissionQueueLockReleaseResult:
        """Explicitly release an orphaned shared-authority lock with an auditable operator reason."""

        if not isinstance(operator, str) or not operator.strip():
            raise ArgumentError("operator must be a non-empty string")
        if not isinstance(reason, str) or not reason.strip():
            raise ArgumentError("reason must be a non-empty string")
        return MissionQueueLockReleaseResult.from_wire(
            self.request(
                "POST",
                "/v1/missions/queue/authority/release-lock",
                {"operator": operator, "reason": reason},
            )
        )

    def missions(self, *, status: str | None = None, limit: int = 100) -> dict[str, Any]:
        """Backward-compatible raw mission inventory response."""

        return self.mission_inventory(status=status, limit=limit).to_dict()

    def mission_from_route(
        self,
        route: Mapping[str, Any],
        mission_id: str,
        selections: Sequence[MissionRouteSelection | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> MissionAssembly:
        """Assemble a route-bound mission locally; callers still preflight before any POST."""

        return assemble_mission_from_route(route, mission_id, selections, policy=policy)

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
        """Run the cross-domain delivery audit through the HTTP gateway."""

        arguments = _developer_delivery_arguments(
            request_id=request_id,
            targets=targets,
            checks={
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
            },
        )
        return self.call_tool("developer_delivery_audit", arguments)

    def developer_delivery_receipt(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Create a content-addressed delivery receipt through the HTTP gateway."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptRequest) else DeveloperDeliveryReceiptRequest.from_wire(request)
        return self.call_tool("developer_delivery_receipt", normalized.to_mcp_arguments())

    def developer_delivery_receipt_verify(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a stored delivery receipt through the HTTP gateway."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptVerificationRequest) else DeveloperDeliveryReceiptVerificationRequest.from_wire(request)
        return self.call_tool("developer_delivery_receipt_verify", normalized.to_mcp_arguments())

    def developer_platform_status(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Run the bounded developer-platform contract through the HTTP gateway."""

        request = DeveloperPlatformStatusArgs(include_details, max_items)
        return self.call_tool("developer_platform_status", request.to_mcp_arguments())

    def developer_platform_status_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> DeveloperPlatformStatusReport:
        """Return typed HTTP walkthrough, cookbook, diagnostic, and impact evidence."""

        return developer_platform_status_report(
            self.developer_platform_status(include_details=include_details, max_items=max_items)
        )

    def token_context_plan(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a token context through the HTTP gateway without execution."""

        normalized = request if isinstance(request, TokenContextPlanArgs) else TokenContextPlanArgs.from_wire(request)
        return self.call_tool("token_context_plan", normalized.to_mcp_arguments())

    def token_context_plan_report(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> TokenContextPlanningReport:
        """Return typed HTTP token estimates and policy-only comparison evidence."""

        return token_context_plan_report(self.token_context_plan(request))

    def weavelang_compile(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Compile WeaveLang through the HTTP gateway."""

        if isinstance(request, str):
            normalized = WeaveLangCompileArgs(request)
        elif isinstance(request, WeaveLangCompileArgs):
            normalized = request
        else:
            normalized = WeaveLangCompileArgs.from_wire(request)
        return self.call_tool("weavelang_compile", normalized.to_mcp_arguments())

    def weavelang_compile_report(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> WeaveLangCompileReport:
        """Return typed HTTP WeaveLang compilation and replay evidence."""

        return weavelang_compile_report(self.weavelang_compile(request))

    def epistemic_voi(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Price explicit evidence through the HTTP gateway, preserving domain refusals."""

        normalized = request if isinstance(request, EpistemicVoiArgs) else EpistemicVoiArgs.from_wire(request)
        return self.call_tool("epistemic_voi", normalized.to_mcp_arguments())

    def epistemic_voi_report(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> EpistemicVoiReport:
        """Return typed HTTP value-of-information evidence."""

        return epistemic_voi_report(self.epistemic_voi(request))

    def epistemic_adaptive_acquisition(
        self,
        request: EpistemicAdaptiveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a bounded exact adaptive acquisition policy without executing it."""

        normalized = request if isinstance(request, EpistemicAdaptiveArgs) else EpistemicAdaptiveArgs.from_wire(request)
        return self.call_tool("epistemic_adaptive_acquisition", normalized.to_mcp_arguments())

    def epistemic_adaptive_acquisition_report(
        self,
        request: EpistemicAdaptiveArgs | Mapping[str, Any],
    ) -> EpistemicAdaptiveReport:
        """Return a validated adaptive policy tree or fail-closed refusal."""

        return epistemic_adaptive_report(self.epistemic_adaptive_acquisition(request))

    def epistemic_adaptive_execute(
        self,
        request: AdaptiveExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Execute or replay an adaptive policy through the HTTP gateway."""

        normalized = request if isinstance(request, AdaptiveExecutionRequest) else AdaptiveExecutionRequest.from_wire(request)
        return self.call_tool("epistemic_adaptive_execute", normalized.to_mcp_arguments())

    def epistemic_adaptive_execute_report(
        self,
        request: AdaptiveExecutionRequest | Mapping[str, Any],
    ) -> AdaptiveExecutionReport:
        """Return typed HTTP adaptive execution and provenance evidence."""

        return adaptive_execution_report(self.epistemic_adaptive_execute(request))

    def epistemic_adaptive_costed(
        self,
        request: AdaptiveCostedRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a component-wise feasible adaptive policy through HTTP."""

        normalized = request if isinstance(request, AdaptiveCostedRequest) else AdaptiveCostedRequest.from_wire(request)
        return self.call_tool("epistemic_adaptive_costed", normalized.to_mcp_arguments())

    def epistemic_adaptive_costed_report(
        self,
        request: AdaptiveCostedRequest | Mapping[str, Any],
    ) -> AdaptiveCostedReport:
        """Return typed HTTP vector-cost planning evidence."""

        return adaptive_costed_report(self.epistemic_adaptive_costed(request))

    def interweave_workflow_execute(
        self,
        request: WorkflowExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Simulate or replay a workflow-bound adaptive execution through HTTP."""

        normalized = request if isinstance(request, WorkflowExecutionRequest) else WorkflowExecutionRequest.from_wire(request)
        return self.call_tool("interweave_workflow_execute", normalized.to_mcp_arguments())

    def interweave_workflow_execute_report(
        self,
        request: WorkflowExecutionRequest | Mapping[str, Any],
    ) -> WorkflowExecutionReport:
        """Return typed HTTP workflow execution and provenance evidence."""

        return workflow_execution_report(self.interweave_workflow_execute(request))

    def interweave_workflow_execution_evidence(
        self,
        request: WorkflowExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Index a workflow receipt as portable evidence through the HTTP tool dispatcher."""

        normalized = request if isinstance(request, WorkflowExecutionEvidenceRequest) else WorkflowExecutionEvidenceRequest.from_wire(request)
        return self.call_tool(
            "interweave_workflow_execution_evidence", normalized.to_mcp_arguments()
        )

    def interweave_workflow_execution_evidence_report(
        self,
        request: WorkflowExecutionEvidenceRequest | Mapping[str, Any],
    ) -> WorkflowExecutionEvidenceReport:
        """Return typed HTTP workflow execution evidence."""

        return workflow_execution_evidence_report(self.interweave_workflow_execution_evidence(request))

    def interweave_workflow_execution_evidence_import(
        self,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Import one portable workflow execution evidence record through HTTP."""

        return self.call_tool(
            "interweave_workflow_execution_evidence_import", {"evidence": dict(evidence)}
        )

    def interweave_workflow_execution_evidence_query(
        self,
        filters: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Query bounded workflow execution evidence through HTTP."""

        return self.call_tool(
            "interweave_workflow_execution_evidence_query", dict(filters or {})
        )

    def interweave_workflow_execution_evidence_get(self, evidence_digest: str) -> dict[str, Any]:
        """Fetch one retained workflow execution evidence record through HTTP."""

        return self.call_tool(
            "interweave_workflow_execution_evidence_get", {"evidence_digest": evidence_digest}
        )

    def epistemic_decision_quotient(
        self,
        request: EpistemicDecisionQuotientArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compute a decision-relative model quotient through HTTP."""

        normalized = request if isinstance(request, EpistemicDecisionQuotientArgs) else EpistemicDecisionQuotientArgs.from_wire(request)
        return self.call_tool("epistemic_decision_quotient", normalized.to_mcp_arguments())

    def epistemic_decision_quotient_report(
        self,
        request: EpistemicDecisionQuotientArgs | Mapping[str, Any],
    ) -> EpistemicDecisionQuotientReport:
        """Return typed class membership, compression, and refusal evidence."""

        return epistemic_decision_quotient_report(self.epistemic_decision_quotient(request))

    def epistemic_context_audit(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit decision-relative context compression through HTTP."""

        normalized = request if isinstance(request, EpistemicContextAuditArgs) else EpistemicContextAuditArgs.from_wire(request)
        return self.call_tool("epistemic_context_audit", normalized.to_mcp_arguments())

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
        """Run bounded observed-evidence selection through the HTTP gateway."""

        normalized = request if isinstance(request, EpistemicSelectionAuditArgs) else EpistemicSelectionAuditArgs.from_wire(request)
        return self.call_tool("epistemic_selection_audit", normalized.to_mcp_arguments())

    def epistemic_selection_audit_report(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> EpistemicSelectionAuditReport:
        """Return typed HTTP selection, guarantee, and exactness evidence."""

        return epistemic_selection_audit_report(self.epistemic_selection_audit(request))

    def benchmark_trace_analyze(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Analyze serialized failing/reference traces through the HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkTraceAnalyzeArgs) else BenchmarkTraceAnalyzeArgs.from_wire(request)
        return self.call_tool("benchmark_trace_analyze", normalized.to_mcp_arguments())

    def benchmark_trace_analysis_report(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> BenchmarkTraceAnalysisReport:
        """Return typed HTTP benchmark compiler evidence."""

        return benchmark_trace_analysis_report(self.benchmark_trace_analyze(request))

    def benchmark_decision_audit(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit one reconstructed decision through the HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkDecisionAuditArgs) else BenchmarkDecisionAuditArgs.from_wire(request)
        return self.call_tool("benchmark_decision_audit", normalized.to_mcp_arguments())

    def benchmark_decision_audit_report(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> BenchmarkDecisionAuditReport:
        """Return typed HTTP decision-cell, firewall, and failure-card evidence."""

        return benchmark_decision_audit_report(self.benchmark_decision_audit(request))

    def benchmark_integrity_audit(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit benchmark portfolio integrity through the HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkIntegrityAuditArgs) else BenchmarkIntegrityAuditArgs.from_wire(request)
        return self.call_tool("benchmark_integrity_audit", normalized.to_mcp_arguments())

    def benchmark_integrity_audit_report(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> BenchmarkIntegrityAuditReport:
        """Return typed HTTP dedup, contamination, holdout, calibration, and diversity evidence."""

        return benchmark_integrity_audit_report(self.benchmark_integrity_audit(request))

    def benchmark_counterfactual_check(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and contrast matched DecisionCells through the HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkCounterfactualCheckArgs) else BenchmarkCounterfactualCheckArgs.from_wire(request)
        return self.call_tool("benchmark_counterfactual_check", normalized.to_mcp_arguments())

    def benchmark_counterfactual_check_report(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> BenchmarkCounterfactualCheckReport:
        """Return typed HTTP matched-pair and response-contrast evidence."""

        return benchmark_counterfactual_check_report(self.benchmark_counterfactual_check(request))

    def benchmark_oracle_review(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review, grade, and optionally package a benchmark oracle through HTTP."""

        normalized = request if isinstance(request, BenchmarkOracleReviewArgs) else BenchmarkOracleReviewArgs.from_wire(request)
        return self.call_tool("benchmark_oracle_review", normalized.to_mcp_arguments())

    def benchmark_oracle_review_report(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> BenchmarkOracleReviewReport:
        """Return typed oracle review-gate, acceptance, and cell-packaging evidence."""

        return benchmark_oracle_review_report(self.benchmark_oracle_review(request))

    def benchmark_compile(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the non-executing assembled benchmark compiler through HTTP."""

        normalized = request if isinstance(request, BenchmarkCompileArgs) else BenchmarkCompileArgs.from_wire(request)
        return self.call_tool("benchmark_compile", normalized.to_mcp_arguments())

    def benchmark_compile_report(
        self,
        request: BenchmarkCompileArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReport:
        """Return typed causal, minimization, oracle, and confidence pipeline evidence."""

        return benchmark_compile_report(self.benchmark_compile(request))

    def benchmark_compile_review(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run compilation, review, optional grading, and cell packaging through HTTP."""

        normalized = request if isinstance(request, BenchmarkCompileReviewArgs) else BenchmarkCompileReviewArgs.from_wire(request)
        return self.call_tool("benchmark_compile_review", normalized.to_mcp_arguments())

    def benchmark_compile_review_report(
        self,
        request: BenchmarkCompileReviewArgs | Mapping[str, Any],
    ) -> BenchmarkCompileReviewReport:
        """Return typed end-to-end reviewed benchmark-cell evidence."""

        return benchmark_compile_review_report(self.benchmark_compile_review(request))

    def foundation_contract_check(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate foundation contract gates through the HTTP gateway."""

        normalized = request if isinstance(request, FoundationContractCheckArgs) else FoundationContractCheckArgs.from_wire(request)
        return self.call_tool("foundation_contract_check", normalized.to_mcp_arguments())

    def foundation_contract_check_report(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> FoundationContractCheckReport:
        """Return typed HTTP foundation gate evidence."""

        return foundation_contract_check_report(self.foundation_contract_check(request))

    def pack_catalogue(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Read bounded pack portfolio declarations through the HTTP gateway."""

        if request is not None:
            if section != "all" or max_items != 100:
                raise ArgumentError("request cannot be combined with section or max_items")
            normalized = request if isinstance(request, PackCatalogueArgs) else PackCatalogueArgs.from_wire(request)
        else:
            normalized = PackCatalogueArgs(section, max_items)
        return self.call_tool("pack_catalogue", normalized.to_mcp_arguments())

    def pack_catalogue_report(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> PackCatalogueReport:
        """Return typed HTTP pack portfolio declarations."""

        return pack_catalogue_report(self.pack_catalogue(request, section=section, max_items=max_items))

    def pack_coverage_audit(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit capability-family and domain coverage across the benchmark portfolio."""

        normalized = request if isinstance(request, PackCoverageAuditArgs) else PackCoverageAuditArgs.from_wire(request or {})
        return self.call_tool("pack_coverage_audit", normalized.to_mcp_arguments())

    def pack_coverage_audit_report(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackCoverageAuditReport:
        """Return typed selected-portfolio gap and coverage evidence."""

        return pack_coverage_audit_report(self.pack_coverage_audit(request))

    def pack_release_audit(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit stable release sequencing and the explicit unsequenced remainder."""

        normalized = request if isinstance(request, PackReleaseAuditArgs) else PackReleaseAuditArgs.from_wire(request or {})
        return self.call_tool("pack_release_audit", normalized.to_mcp_arguments())

    def pack_release_audit_report(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackReleaseAuditReport:
        """Return typed release-order and unsequenced-pack evidence."""

        return pack_release_audit_report(self.pack_release_audit(request))

    def pack_health_assess(
        self,
        request: PackHealthAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run the observed pack-health gate through the HTTP gateway."""

        normalized = request if isinstance(request, PackHealthAssessArgs) else PackHealthAssessArgs.from_wire(request)
        return self.call_tool("pack_health_assess", normalized.to_mcp_arguments())

    def pack_health_assess_report(
        self,
        request: PackHealthAssessArgs | Mapping[str, Any],
    ) -> PackHealthAssessmentReport:
        """Return typed health findings and a score only when the server reportability gate clears."""

        return pack_health_assessment_report(self.pack_health_assess(request))

    def security_redteam_simulate(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Run the bounded section-13 safety workflow through the HTTP gateway."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        return self.call_tool("security_redteam_simulate", normalized.to_mcp_arguments())

    def security_redteam_simulate_report(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> SecurityRedteamReport:
        """Return typed section-13 safety evidence without collapsing partial refusals."""

        return security_redteam_simulate_report(self.security_redteam_simulate(request))

    def world_generate(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Generate a deterministic synthetic world/query pair through the HTTP gateway."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        return self.call_tool("world_generate", normalized.to_mcp_arguments())

    def world_generate_report(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> WorldGenerateReport:
        """Return typed world/query identity and validation evidence."""

        return world_generate_report(self.world_generate(request))

    def factory_lifecycle_simulate(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay the typed factory lifecycle through the HTTP gateway."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        return self.call_tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())

    def factory_lifecycle_simulate_report(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> FactoryLifecycleReport:
        """Return typed factory traces and final visibility through the HTTP gateway."""

        return factory_lifecycle_report(self.factory_lifecycle_simulate(request))

    def storage_lifecycle_simulate(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan and optionally apply storage lifecycle accounting through the HTTP gateway."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        return self.call_tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())

    def storage_lifecycle_simulate_report(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> StorageLifecycleReport:
        """Return typed storage tiering and quota evidence through the HTTP gateway."""

        return storage_lifecycle_report(self.storage_lifecycle_simulate(request))

    def registry_lifecycle_simulate(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Replay the registry lifecycle through the HTTP gateway."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        return self.call_tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())

    def registry_lifecycle_simulate_report(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> RegistryLifecycleReport:
        """Return typed registry lifecycle evidence through the HTTP gateway."""

        return registry_lifecycle_report(self.registry_lifecycle_simulate(request))

    def cache_invalidation_simulate(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan and optionally apply cache invalidation through the HTTP gateway."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        return self.call_tool("cache_invalidation_simulate", normalized.to_mcp_arguments())

    def cache_invalidation_simulate_report(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> CacheInvalidationReport:
        """Return typed cache invalidation evidence through the HTTP gateway."""

        return cache_invalidation_report(self.cache_invalidation_simulate(request))

    def hub_disclosure_review(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay public-hub disclosure actions through the HTTP gateway."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        return self.call_tool("hub_disclosure_review", normalized.to_mcp_arguments())

    def hub_disclosure_review_report(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> HubDisclosureReviewReport:
        """Return typed disclosure evidence through the HTTP gateway."""

        return hub_disclosure_review(self.hub_disclosure_review(request))

    def hub_card_render(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render a public-hub card through the HTTP gateway."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        return self.call_tool("hub_card_render", normalized.to_mcp_arguments())

    def hub_card_render_report(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> HubCardRenderReport:
        """Return typed card evidence through the HTTP gateway."""

        return hub_card_render(self.hub_card_render(request))

    def hub_leaderboard_render(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render the public-hub leaderboard through the HTTP gateway."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        return self.call_tool("hub_leaderboard_render", normalized.to_mcp_arguments())

    def hub_leaderboard_render_report(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> HubLeaderboardRenderReport:
        """Return typed leaderboard evidence through the HTTP gateway."""

        return hub_leaderboard_render(self.hub_leaderboard_render(request))

    def hub_submission_review(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay public-hub submission and moderation review through HTTP."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        return self.call_tool("hub_submission_review", normalized.to_mcp_arguments())

    def hub_submission_review_report(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> HubSubmissionReviewReport:
        """Return typed submission and moderation evidence through HTTP."""

        return hub_submission_review(self.hub_submission_review(request))

    def bioatlas_publication_audit(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compose the BioAtlas publication audit through the HTTP gateway."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        return self.call_tool("bioatlas_publication_audit", normalized.to_mcp_arguments())

    def bioatlas_publication_audit_report(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> BioAtlasPublicationAuditReport:
        """Return typed composed publication evidence through the HTTP gateway."""

        return bioatlas_publication_audit(self.bioatlas_publication_audit(request))

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
        """Return typed delivery-readiness evidence from the HTTP gateway."""

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
        """Return typed HTTP delivery receipt digests and readiness."""

        return developer_delivery_receipt_report(self.developer_delivery_receipt(request))

    def developer_delivery_receipt_verification_report(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptVerificationReport:
        """Return typed HTTP receipt verification evidence."""

        return developer_delivery_receipt_verification_report(self.developer_delivery_receipt_verify(request))

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
        """Search the complete cross-domain catalogue through the HTTP gateway."""

        arguments = _capability_query_arguments(
            query,
            text=text,
            domain=domain,
            tool=tool,
            group_id=group_id,
            max_items=max_items,
            include_tools=include_tools,
        )
        return self.call_tool("capability_discover", arguments)

    def mission_evaluator_discover(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Discover explicit non-executing evaluator candidates through HTTP."""

        normalized = request if isinstance(request, MissionEvaluatorQuery) else MissionEvaluatorQuery(**dict(request or {}))
        return self.call_tool("mission_evaluator_discover", normalized.to_mcp_arguments())

    def mission_evaluator_discover_report(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> MissionEvaluatorSearchReport:
        """Return typed, digest-bound evaluator candidate evidence through HTTP."""

        return mission_evaluator_discover_report(self.mission_evaluator_discover(request))

    def mission_evaluator_review(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review explicit evaluator-to-claim bindings without executing domain tools."""

        normalized = request if isinstance(request, MissionEvaluatorReviewRequest) else MissionEvaluatorReviewRequest(**dict(request))
        return self.call_tool("mission_evaluator_review", normalized.to_mcp_arguments())

    def mission_evaluator_review_report(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReviewReport:
        """Return typed evaluator binding review evidence through HTTP."""

        return mission_evaluator_review_report(self.mission_evaluator_review(request))

    def mission_evaluator_replay(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Replay retained mission evaluator lineage without executing domain tools."""

        normalized = request if isinstance(request, MissionEvaluatorReplayRequest) else MissionEvaluatorReplayRequest(**dict(request))
        return self.call_tool("mission_evaluator_replay", normalized.to_mcp_arguments())

    def mission_evaluator_replay_report(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayReport:
        """Return typed evaluator replay, fixture, and coverage evidence through HTTP."""

        return mission_evaluator_replay_report(self.mission_evaluator_replay(request))

    def mission_evaluator_replay_compare(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compare a retained mission replay with the current catalogue over MCP."""

        normalized = request if isinstance(request, MissionEvaluatorReplayCompareRequest) else MissionEvaluatorReplayCompareRequest(**dict(request))
        return self.call_tool("mission_evaluator_replay_compare", normalized.to_mcp_arguments())

    def mission_evaluator_replay_compare_report(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed MCP catalog-drift comparison evidence."""

        return mission_evaluator_replay_comparison_report(self.mission_evaluator_replay_compare(request))

    def mission_evidence_bundle_verify_tool(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify one portable mission evidence bundle through MCP."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleVerifyRequest)
            else MissionEvidenceBundleVerifyRequest(**dict(request))
        )
        return self.call_tool("mission_evidence_bundle_verify", normalized.to_mcp_arguments())

    def mission_evidence_bundle_verification_tool_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed MCP portable mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(self.mission_evidence_bundle_verify_tool(request))

    def mission_evidence_bundle_import_tool(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Import one verified evidence bundle through the MCP bridge."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleImportRequest)
            else MissionEvidenceBundleImportRequest(**dict(request))
        )
        return self.call_tool("mission_evidence_bundle_import", normalized.to_mcp_arguments())

    def mission_evidence_bundle_import_tool_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        return MissionEvidenceBundleImportReport.from_wire(self.mission_evidence_bundle_import_tool(request))

    def mission_evidence_bundle_query_tool(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Query the process-local MCP evidence registry."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleQueryRequest)
            else MissionEvidenceBundleQueryRequest(**dict(request or {}))
        )
        return self.call_tool("mission_evidence_bundle_query", normalized.to_mcp_arguments())

    def mission_evidence_bundle_query_tool_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        return MissionEvidenceBundleQueryReport.from_wire(self.mission_evidence_bundle_query_tool(request))

    def mission_evidence_bundle_get_tool(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Fetch one verified bundle through the MCP bridge."""

        if isinstance(request, MissionEvidenceBundleGetRequest):
            normalized = request
        elif isinstance(request, str):
            normalized = MissionEvidenceBundleGetRequest(request)
        else:
            normalized = MissionEvidenceBundleGetRequest(**dict(request))
        return self.call_tool("mission_evidence_bundle_get", normalized.to_mcp_arguments())

    def mission_evidence_bundle_get_tool_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        return MissionEvidenceBundleGetReport.from_wire(self.mission_evidence_bundle_get_tool(request))

    def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return self.call_tool("capability_audit", {"include_groups": include_groups})

    def capability_audit_report(self, *, include_groups: bool = True) -> CapabilityAuditReport:
        """Return validated capability parity and schema-quality diagnostics over HTTP."""

        return capability_audit_report(self.capability_audit(include_groups=include_groups))

    def capability_dashboard(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Return a bounded cross-domain surface dashboard through HTTP."""

        normalized = request if isinstance(request, CapabilityDashboardQueryArgs) else CapabilityDashboardQueryArgs(**dict(request or {}))
        return self.call_tool("capability_dashboard", normalized.to_mcp_arguments())

    def capability_dashboard_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return typed capability dashboard evidence through HTTP."""

        return capability_dashboard_report(self.capability_dashboard(request))

    def capability_dashboard_rest(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Query the capability dashboard through the direct cache-friendly REST route."""

        normalized = request if isinstance(request, CapabilityDashboardQueryArgs) else CapabilityDashboardQueryArgs(**dict(request or {}))
        return self.request("GET", f"/v1/capabilities/dashboard?{urlencode(normalized.to_query_params())}")

    def capability_dashboard_rest_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return a typed report from the direct capability dashboard REST route."""

        return capability_dashboard_report(self.capability_dashboard_rest(request))

    def ci_execution_evidence_audit(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Reconcile caller-supplied CI evidence through the HTTP gateway."""

        normalized = request if isinstance(request, CiExecutionEvidenceRequest) else CiExecutionEvidenceRequest(**dict(request))
        return self.call_tool("ci_execution_evidence_audit", normalized.to_mcp_arguments())

    def ci_execution_evidence_report(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> CiExecutionEvidenceReport:
        """Return typed structural CI evidence through HTTP."""

        return ci_execution_evidence_report(self.ci_execution_evidence_audit(request))

    def ci_provider_normalize(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Normalize a caller-supplied provider payload through the HTTP gateway."""

        normalized = request if isinstance(request, CiProviderNormalizationRequest) else CiProviderNormalizationRequest(**dict(request))
        return self.call_tool("ci_provider_normalize", normalized.to_mcp_arguments())

    def ci_provider_normalization_report(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> CiProviderNormalizationReport:
        """Return typed provider-normalization evidence through HTTP."""

        return ci_provider_normalization_report(self.ci_provider_normalize(request))

    def ci_provider_evidence_audit(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit provider-bound artifact, log, and attestation rows through HTTP."""

        normalized = request if isinstance(request, CiProviderEvidenceRequest) else CiProviderEvidenceRequest(**dict(request))
        return self.call_tool("ci_provider_evidence_audit", normalized.to_mcp_arguments())

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
        """Reconcile mission and delegated-check provenance through HTTP."""

        normalized = request if isinstance(request, ExecutionProvenanceRequest) else ExecutionProvenanceRequest(**dict(request))
        return self.call_tool("execution_provenance_audit", normalized.to_mcp_arguments())

    def execution_provenance_report(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> ExecutionProvenanceReport:
        """Return typed mission/delegated-check provenance through HTTP."""

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
        """Return a validated ranked projection from the HTTP capability catalogue."""

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
        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return self.call_tool("capability_route", request.to_mcp_arguments())

    def capability_route_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Return a validated typed view over an HTTP route proposal."""

        return capability_route_report(
            self.capability_route(
                goal,
                needs,
                max_candidates_per_need=max_candidates_per_need,
                max_tools=max_tools,
                include_tools=include_tools,
            )
        )

    def capability_route_rest(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Submit a non-executing route proposal through the dedicated REST endpoint."""

        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return self.request("POST", "/v1/capabilities/route", request.to_mcp_arguments())

    def capability_route_rest_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Return a typed report from the dedicated route REST endpoint."""

        return capability_route_report(
            self.capability_route_rest(
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
        """Review explicit route selections through the HTTP gateway."""

        request = CapabilityRouteReviewRequest(route, selections, validate_schemas)
        return self.call_tool("capability_route_review", request.to_mcp_arguments())

    def capability_route_review_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Return typed HTTP diagnostics for a route-to-mission handoff review."""

        return capability_route_review_report(
            self.capability_route_review(route, selections, validate_schemas=validate_schemas)
        )

    def capability_route_review_rest(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> dict[str, Any]:
        """Review route selections through the dedicated raw REST endpoint."""

        request = CapabilityRouteReviewRequest(route, selections, validate_schemas)
        return self.request("POST", "/v1/capabilities/route/review", request.to_mcp_arguments())

    def capability_route_review_rest_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Return typed route-review diagnostics from the dedicated REST endpoint."""

        return capability_route_review_report(
            self.capability_route_review_rest(
                route,
                selections,
                validate_schemas=validate_schemas,
            )
        )

    def capability_route_plan(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Compose a reviewed route into a non-executing mission preflight."""

        request = CapabilityRoutePlanRequest(
            mission_id,
            route,
            selections,
            validate_schemas,
            policy,
            claim_requests,
            evaluator_review,
            workflow_binding,
        )
        return self.call_tool("capability_route_plan", request.to_mcp_arguments())

    def capability_route_plan_report(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> CapabilityRoutePlanReport:
        """Return the typed route-review and mission-preflight composition."""

        return capability_route_plan_report(
            self.capability_route_plan(
                mission_id,
                route,
                selections,
                validate_schemas=validate_schemas,
                policy=policy,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
            )
        )

    def capability_route_plan_rest(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Submit the route-to-plan composition through its dedicated REST endpoint."""

        request = CapabilityRoutePlanRequest(
            mission_id,
            route,
            selections,
            validate_schemas,
            policy,
            claim_requests,
            evaluator_review,
            workflow_binding,
        )
        return self.request("POST", "/v1/capabilities/route/plan", request.to_mcp_arguments())

    def capability_route_plan_rest_report(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> CapabilityRoutePlanReport:
        """Return typed route-plan diagnostics from direct REST."""

        return capability_route_plan_report(
            self.capability_route_plan_rest(
                mission_id,
                route,
                selections,
                validate_schemas=validate_schemas,
                policy=policy,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
            )
        )

    def capability_route_plan_verify(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a retained route plan through MCP without dispatch or execution."""

        normalized = (
            request
            if isinstance(request, CapabilityRoutePlanVerifyRequest)
            else CapabilityRoutePlanVerifyRequest(**dict(request))
        )
        return self.call_tool("capability_route_plan_verify", normalized.to_mcp_arguments())

    def capability_route_plan_verify_report(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> CapabilityRoutePlanVerifyReport:
        return capability_route_plan_verify_report(self.capability_route_plan_verify(request))

    def capability_route_plan_verify_rest(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Verify a retained route plan through the dedicated REST endpoint."""

        normalized = (
            request
            if isinstance(request, CapabilityRoutePlanVerifyRequest)
            else CapabilityRoutePlanVerifyRequest(**dict(request))
        )
        return self.request(
            "POST",
            "/v1/capabilities/route/plan/verify",
            normalized.to_mcp_arguments(),
        )

    def capability_route_plan_verify_rest_report(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> CapabilityRoutePlanVerifyReport:
        return capability_route_plan_verify_report(self.capability_route_plan_verify_rest(request))

    def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Plan native and Python-delegated adapters through the HTTP gateway."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return self.call_tool("adapter_plan", request.to_mcp_arguments())

    def adapter_plan_report(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> AdapterPlanReport:
        """Return typed adapter planning evidence from the HTTP gateway."""

        return adapter_plan_report(
            self.adapter_plan(
                source_id,
                source_kind,
                declared_format=declared_format,
                required_conformance=required_conformance,
                available_dependencies=available_dependencies,
            )
        )

    def adapter_execution_evidence(
        self,
        request: AdapterExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Retain caller-supplied adapter execution evidence through the HTTP gateway."""

        normalized = request if isinstance(request, AdapterExecutionEvidenceRequest) else AdapterExecutionEvidenceRequest.from_wire(request)
        return self.call_tool("adapter_execution_evidence", normalized.to_mcp_arguments())

    def adapter_execution_evidence_report(
        self,
        request: AdapterExecutionEvidenceRequest | Mapping[str, Any],
    ) -> AdapterExecutionEvidenceReport:
        return adapter_execution_evidence_report(self.adapter_execution_evidence(request))

    def adapter_execution_evidence_query(
        self,
        request: AdapterExecutionEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Query retained adapter observations and explicit source/workflow joins."""

        normalized = request or AdapterExecutionEvidenceQueryRequest()
        if not isinstance(normalized, AdapterExecutionEvidenceQueryRequest):
            normalized = AdapterExecutionEvidenceQueryRequest.from_wire(request or {})
        return self.call_tool("adapter_execution_evidence_query", normalized.to_mcp_arguments())

    def adapter_execution_evidence_query_report(
        self,
        request: AdapterExecutionEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> AdapterExecutionEvidenceQueryReport:
        return adapter_execution_evidence_query_report(self.adapter_execution_evidence_query(request))

    def domain_acquisition_catalogue(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> dict[str, Any]:
        """Build the digest-bound two-plane route catalogue over all declared domains."""

        normalized = query or DomainAcquisitionQuery()
        if not isinstance(normalized, DomainAcquisitionQuery):
            raise TypeError("query must be a DomainAcquisitionQuery")
        return self.call_tool(DOMAIN_ACQUISITION_WORKFLOW, normalized.to_mcp_arguments())

    def domain_acquisition_catalogue_report(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> DomainAcquisitionReport:
        """Return typed acquisition routes and explicit adapter interpretation boundaries."""

        return domain_acquisition_report(self.domain_acquisition_catalogue(query))

    def tabular_ingest(self, request: TabularIngestRequest) -> dict[str, Any]:
        """Execute the Rust CSV/TSV adapter through the HTTP gateway."""

        if not isinstance(request, TabularIngestRequest):
            raise ArgumentError("request must be a TabularIngestRequest")
        return self.call_tool("tabular_ingest", request.to_mcp_arguments())

    def tabular_ingest_report(self, request: TabularIngestRequest) -> TabularIngestReport:
        """Return typed HTTP manifest, conformance, loss, and fact evidence."""

        return tabular_ingest_report(self.tabular_ingest(request))

    def conformance_run(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Run the shipped fixture-verified suite through the HTTP gateway."""

        if not isinstance(include_details, bool):
            raise ArgumentError("include_details must be a boolean")
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= 1_000:
            raise ArgumentError("max_items must be between 1 and 1000")
        return self.call_tool("conformance_run", {"include_details": include_details, "max_items": max_items})

    def conformance_run_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> ConformanceRunReport:
        """Return typed HTTP suite, case, pyramid, and release evidence."""

        return conformance_run_report(
            self.conformance_run(include_details=include_details, max_items=max_items)
        )

    def release_audit(self, request: ReleaseAuditArgs) -> dict[str, Any]:
        """Compose release gates through the HTTP gateway."""

        if not isinstance(request, ReleaseAuditArgs):
            raise ArgumentError("request must be a ReleaseAuditArgs")
        return self.call_tool("release_audit", request.to_mcp_arguments())

    def release_audit_report(self, request: ReleaseAuditArgs) -> ReleaseAuditReport:
        """Return typed HTTP release readiness and delegated check evidence."""

        return release_audit_report(self.release_audit(request))

    def operations_catalog(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Inspect operations contracts through the HTTP gateway."""

        request = OperationsCatalogArgs(include_details, max_items)
        return self.call_tool("operations_catalog", request.to_mcp_arguments())

    def operations_catalog_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> OperationsCatalogReport:
        """Return typed HTTP operations topology, service, and metric evidence."""

        return operations_catalog_report(
            self.operations_catalog(include_details=include_details, max_items=max_items)
        )

    def ops_acceptance(self, *, max_items: int = 100) -> dict[str, Any]:
        """Run operational acceptance through the HTTP gateway."""

        request = OpsAcceptanceArgs(max_items)
        return self.call_tool("ops_acceptance", request.to_mcp_arguments())

    def ops_acceptance_report(self, *, max_items: int = 100) -> OpsAcceptanceReport:
        """Return typed HTTP met/refuted/unverifiable acceptance evidence."""

        return ops_acceptance_report(self.ops_acceptance(max_items=max_items))

    def safety_release_gate(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Evaluate a reviewer-labelled safety release assessment over HTTP."""

        if isinstance(assessment, SafetyReleaseGateArgs):
            request = assessment
        else:
            request = SafetyReleaseGateArgs(
                assessment if isinstance(assessment, RiskAssessmentRequest) else RiskAssessmentRequest.from_wire(assessment)
            )
        return self.call_tool("safety_release_gate", request.to_mcp_arguments())

    def safety_release_gate_report(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> SafetyReleaseGateReport:
        """Return typed HTTP fail-closed safety-gate evidence."""

        return safety_release_gate_report(self.safety_release_gate(assessment))

    def medical_boundary_check(self, request: MedicalBoundaryRequest) -> dict[str, Any]:
        """Check the research-only medical boundary over HTTP."""

        if not isinstance(request, MedicalBoundaryRequest):
            raise ArgumentError("request must be a MedicalBoundaryRequest")
        return self.call_tool("medical_boundary_check", request.to_mcp_arguments())

    def medical_boundary_report(self, request: MedicalBoundaryRequest) -> MedicalBoundaryReport:
        """Return typed HTTP research admission or clinical refusal evidence."""

        return medical_boundary_report(self.medical_boundary_check(request))

    def safety_posture(self, *, include_threats: bool = False) -> dict[str, Any]:
        """Summarize section-13 threat populations through the HTTP gateway."""

        request = SafetyPostureArgs(include_threats)
        return self.call_tool("safety_posture", request.to_mcp_arguments())

    def safety_posture_report(self, *, include_threats: bool = False) -> SafetyPostureReport:
        """Return typed HTTP section-13 posture evidence."""

        return safety_posture_report(self.safety_posture(include_threats=include_threats))

    def measurement_compare(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compare standards declarations through the HTTP gateway."""

        normalized = request if isinstance(request, MeasurementCompareArgs) else MeasurementCompareArgs.from_wire(request)
        return self.call_tool("measurement_compare", normalized.to_mcp_arguments())

    def measurement_compare_report(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> MeasurementCompareReport:
        """Return typed HTTP measurement-comparability evidence."""

        return measurement_compare_report(self.measurement_compare(request))

    def literature_bind_check(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, LiteratureBindCheckArgs) else LiteratureBindCheckArgs.from_wire(request)
        return self.call_tool("literature_bind_check", normalized.to_mcp_arguments())

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
        return self.call_tool("modality_support_check", normalized.to_mcp_arguments())

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
        return self.call_tool("modality_transport_check", normalized.to_mcp_arguments())

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
        return self.call_tool("modality_comparability_check", normalized.to_mcp_arguments())

    def modality_comparability_check_report(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> ModalityComparabilityCheckReport:
        return modality_comparability_check_report(self.modality_comparability_check(request))

    def hub_search(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Search bounded federated catalogs through the HTTP gateway."""

        normalized = request if isinstance(request, HubSearchArgs) else HubSearchArgs.from_wire(request)
        return self.call_tool("hub_search", normalized.to_mcp_arguments())

    def hub_search_report(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> HubSearchReport:
        """Return typed HTTP federated hub-search evidence."""

        return hub_search_report(self.hub_search(request))

    def hub_resolve(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Resolve one federated pack through the HTTP gateway."""

        normalized = request if isinstance(request, HubResolveArgs) else HubResolveArgs.from_wire(request)
        return self.call_tool("hub_resolve", normalized.to_mcp_arguments())

    def hub_resolve_report(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> HubResolveReport:
        """Return typed HTTP federated resolution evidence."""

        return hub_resolve_report(self.hub_resolve(request))

    def hub_lock(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Resolve a bounded dependency closure through the HTTP gateway."""

        normalized = request if isinstance(request, HubLockArgs) else HubLockArgs.from_wire(request)
        return self.call_tool("hub_lock", normalized.to_mcp_arguments())

    def hub_lock_report(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> HubLockReport:
        """Return typed HTTP dependency-lock evidence."""

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
        tier = minimum_deciding_tier if isinstance(minimum_deciding_tier, EvidenceTier) else EvidenceTier(minimum_deciding_tier)
        request = OracleCombineRequest(subject, at, tuple(judgements), tier, max_items)
        return self.call_tool("oracle_combine", request.to_mcp_arguments())

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

    def oracle_reference_panel(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        request = ReferencePanelRequest(panel, rule, model_call, max_items)
        return self.call_tool("oracle_reference_panel", request.to_mcp_arguments())

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
        return self.call_tool("oracle_missingness", request.to_mcp_arguments())

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
        return self.call_tool("bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments())

    def bioeval_reference_audit_report(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> BioevalReferenceAuditReport:
        return bioeval_reference_audit_report(self.bioeval_reference_audit(reference, state=state))

    def bioeval_acquisition_audit(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit a declared acquisition trace through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalAcquisitionAuditArgs) else BioevalAcquisitionAuditArgs.from_wire(request)
        return self.call_tool("bioeval_acquisition_audit", normalized.to_mcp_arguments())

    def bioeval_acquisition_audit_report(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> BioevalAcquisitionAuditReport:
        """Return typed HTTP acquisition-trace evidence."""

        return bioeval_acquisition_audit_report(self.bioeval_acquisition_audit(request))

    def bioeval_grounding_audit(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit a claim-evidence graph through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalGroundingAuditArgs) else BioevalGroundingAuditArgs.from_wire(request)
        return self.call_tool("bioeval_grounding_audit", normalized.to_mcp_arguments())

    def bioeval_grounding_audit_report(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> BioevalGroundingAuditReport:
        """Return typed HTTP claim-evidence grounding evidence."""

        return bioeval_grounding_audit_report(self.bioeval_grounding_audit(request))

    def bioeval_estimand_audit(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit an estimand through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalEstimandAuditArgs) else BioevalEstimandAuditArgs.from_wire(request)
        return self.call_tool("bioeval_estimand_audit", normalized.to_mcp_arguments())

    def bioeval_estimand_audit_report(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> BioevalEstimandAuditReport:
        """Return typed HTTP estimand and identification evidence."""

        return bioeval_estimand_audit_report(self.bioeval_estimand_audit(request))

    def bioeval_evaluator_audit(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit evaluator health separately from task outcomes through HTTP."""

        normalized = request if isinstance(request, BioevalEvaluatorAuditArgs) else BioevalEvaluatorAuditArgs.from_wire(request)
        return self.call_tool("bioeval_evaluator_audit", normalized.to_mcp_arguments())

    def bioeval_evaluator_audit_report(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> BioevalEvaluatorAuditReport:
        """Return typed HTTP evaluator-health and task-outcome evidence."""

        return bioeval_evaluator_audit_report(self.bioeval_evaluator_audit(request))

    def bioeval_plane_audit(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit a serialized scoring plane through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalPlaneAuditArgs) else BioevalPlaneAuditArgs.from_wire(request)
        return self.call_tool("bioeval_plane_audit", normalized.to_mcp_arguments())

    def bioeval_plane_audit_report(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> BioevalPlaneAuditReport:
        """Return typed HTTP scoring-plane and fold evidence."""

        return bioeval_plane_audit_report(self.bioeval_plane_audit(request))

    def bioeval_metamorphic_audit(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit metamorphic-response families through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalMetamorphicAuditArgs) else BioevalMetamorphicAuditArgs.from_wire(request)
        return self.call_tool("bioeval_metamorphic_audit", normalized.to_mcp_arguments())

    def bioeval_metamorphic_audit_report(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> BioevalMetamorphicAuditReport:
        """Return typed HTTP metamorphic-response evidence."""

        return bioeval_metamorphic_audit_report(self.bioeval_metamorphic_audit(request))

    def bioeval_waiver_audit(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit release-gate waivers through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalWaiverAuditArgs) else BioevalWaiverAuditArgs.from_wire(request)
        return self.call_tool("bioeval_waiver_audit", normalized.to_mcp_arguments())

    def bioeval_waiver_audit_report(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> BioevalWaiverAuditReport:
        """Return typed HTTP release-gate waiver evidence."""

        return bioeval_waiver_audit_report(self.bioeval_waiver_audit(request))

    def bioeval_design_audit(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit factorial-design contrasts through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalDesignAuditArgs) else BioevalDesignAuditArgs.from_wire(request)
        return self.call_tool("bioeval_design_audit", normalized.to_mcp_arguments())

    def bioeval_design_audit_report(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> BioevalDesignAuditReport:
        """Return typed HTTP factorial-design evidence."""

        return bioeval_design_audit_report(self.bioeval_design_audit(request))

    def bioeval_mesh_audit(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit evaluator-mesh independence through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalMeshAuditArgs) else BioevalMeshAuditArgs.from_wire(request)
        return self.call_tool("bioeval_mesh_audit", normalized.to_mcp_arguments())

    def bioeval_mesh_audit_report(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> BioevalMeshAuditReport:
        """Return typed HTTP evaluator-mesh evidence."""

        return bioeval_mesh_audit_report(self.bioeval_mesh_audit(request))

    def bioeval_burden_audit(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit nonrenewable resources and branch feasibility through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalBurdenAuditArgs) else BioevalBurdenAuditArgs.from_wire(request)
        return self.call_tool("bioeval_burden_audit", normalized.to_mcp_arguments())

    def bioeval_burden_audit_report(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> BioevalBurdenAuditReport:
        """Return typed HTTP burden and fork-feasibility evidence."""

        return bioeval_burden_audit_report(self.bioeval_burden_audit(request))

    def bioeval_reveal_audit(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit prospective commitments and rubric integrity through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalRevealAuditArgs) else BioevalRevealAuditArgs.from_wire(request)
        return self.call_tool("bioeval_reveal_audit", normalized.to_mcp_arguments())

    def bioeval_reveal_audit_report(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> BioevalRevealAuditReport:
        """Return typed HTTP seal/reveal evidence."""

        return bioeval_reveal_audit_report(self.bioeval_reveal_audit(request))

    def bioeval_boundary_audit(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit contextual-integrity flows through the HTTP gateway."""

        normalized = request if isinstance(request, BioevalBoundaryAuditArgs) else BioevalBoundaryAuditArgs.from_wire(request)
        return self.call_tool("bioeval_boundary_audit", normalized.to_mcp_arguments())

    def bioeval_boundary_audit_report(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> BioevalBoundaryAuditReport:
        """Return typed HTTP boundary, veto, and Pareto evidence."""

        return bioeval_boundary_audit_report(self.bioeval_boundary_audit(request))

    def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return self.call_tool("evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments())

    def evaluation_worldline_audit_report(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> EvaluationWorldlineReport:
        return evaluation_worldline_audit_report(self.evaluation_worldline_audit(worldline, at=at))

    def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        return self.call_tool("evaluation_reproduction_check", EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments())

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
        return self.call_tool("evaluation_trajectory_check", EvaluationTrajectoryRequest(trajectory, step, horizon).to_mcp_arguments())

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
        normalized = request if isinstance(request, RuntimeEffectCheckArgs) else RuntimeEffectCheckArgs.from_wire(request)
        return self.call_tool("runtime_effect_check", normalized.to_mcp_arguments())

    def runtime_effect_check_report(
        self,
        request: RuntimeEffectCheckArgs | Mapping[str, Any],
    ) -> RuntimeEffectReport:
        return runtime_effect_check_report(self.runtime_effect_check(request))

    def runtime_tape_verify(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, RuntimeTapeVerifyArgs) else RuntimeTapeVerifyArgs.from_wire(request)
        return self.call_tool("runtime_tape_verify", normalized.to_mcp_arguments())

    def runtime_tape_verify_report(
        self,
        request: RuntimeTapeVerifyArgs | Mapping[str, Any],
    ) -> RuntimeTapeVerifyReport:
        return runtime_tape_verify_report(self.runtime_tape_verify(request))

    def runtime_execution_simulate(
        self,
        request: RuntimeExecutionSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, RuntimeExecutionSimulateArgs) else RuntimeExecutionSimulateArgs.from_wire(request)
        return self.call_tool("runtime_execution_simulate", normalized.to_mcp_arguments())

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
        return self.call_tool("bioethics_action_review", normalized.to_mcp_arguments())

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
        return self.call_tool("bioethics_human_subject_screen", normalized.to_mcp_arguments())

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
        return self.call_tool("bioethics_dual_use_review", normalized.to_mcp_arguments())

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
        return self.call_tool("bioethics_validation_check", normalized.to_mcp_arguments())

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
        return self.call_tool("bioethics_representation_audit", normalized.to_mcp_arguments())

    def bioethics_representation_audit_report(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> BioethicsRepresentationAuditReport:
        return bioethics_representation_audit_report(self.bioethics_representation_audit(request))

    def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Run the evidence-conditioned capability audit through the HTTP gateway."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return self.call_tool("biocapability_evidence_audit", request.to_mcp_arguments())

    def biocapability_evidence_audit_report(
        self, request: BioCapabilityEvidenceAuditRequest
    ) -> BioCapabilityEvidenceAuditReport:
        """Return typed evidence states, claim blockers, and release posture over HTTP."""

        return biocapability_evidence_audit_report(self.biocapability_evidence_audit(request))

    def bioatlas_publication_audit(
        self, atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs, **kwargs: Any
    ) -> dict[str, Any]:
        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return self.call_tool("bioatlas_publication_audit", atlas.to_mcp_arguments())
        arguments: dict[str, Any] = {"atlas": dict(atlas)}
        for key in ("weighting", "evidence_audit", "card", "leaderboard"):
            if kwargs.get(key) is not None:
                arguments[key] = dict(kwargs[key])
        if kwargs.get("release_request") is not None:
            arguments["release_request"] = dict(kwargs["release_request"])
        if kwargs.get("max_items") is not None:
            arguments["max_items"] = kwargs["max_items"]
        return self.call_tool("bioatlas_publication_audit", arguments)

    def bioatlas_publication_audit_report(
        self, atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs, **kwargs: Any
    ) -> BioAtlasPublicationAuditReport:
        """Return typed publication-readiness evidence from the HTTP gateway."""

        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return bioatlas_publication_audit(self.bioatlas_publication_audit(atlas))
        return bioatlas_publication_audit_report(self.bioatlas_publication_audit(atlas, **kwargs))

    def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Compile BioQL through the HTTP gateway without executing it."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return self.call_tool("bioql_compile", request.to_mcp_arguments())

    def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Check a serialized claim against world provenance through HTTP."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return self.call_tool("world_claim_check", request.to_mcp_arguments())

    def observed_world_declare(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate an observed-world declaration through the HTTP gateway."""

        normalized = request if isinstance(request, ObservedWorldDeclareArgs) else ObservedWorldDeclareArgs.from_wire(request)
        return self.call_tool("observed_world_declare", normalized.to_mcp_arguments())

    def observed_world_declare_report(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> ObservedWorldDeclareReport:
        """Return typed HTTP observed-world declaration evidence."""

        return observed_world_declare_report(self.observed_world_declare(request))

    def world_claim_check_report(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> WorldClaimCheckReport:
        """Return typed HTTP grounded evidence or structured refusal."""

        return world_claim_check_report(self.world_claim_check(provenance, claim))

    def lineage_audit(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit bounded specimen lineage through the HTTP gateway."""

        normalized = request if isinstance(request, LineageAuditArgs) else LineageAuditArgs.from_wire(request)
        return self.call_tool("lineage_audit", normalized.to_mcp_arguments())

    def lineage_audit_report(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> LineageAuditReport:
        return lineage_audit_report(self.lineage_audit(request))

    def preanalytic_apply(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Apply a declared pre-analytic mutation through the HTTP gateway."""

        normalized = request if isinstance(request, PreanalyticApplyArgs) else PreanalyticApplyArgs.from_wire(request)
        return self.call_tool("preanalytic_apply", normalized.to_mcp_arguments())

    def preanalytic_apply_report(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> PreanalyticApplyReport:
        return preanalytic_apply_report(self.preanalytic_apply(request))

    def contradiction_review(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review a bounded contradiction program through the HTTP gateway."""

        normalized = request if isinstance(request, ContradictionReviewArgs) else ContradictionReviewArgs.from_wire(request)
        return self.call_tool("contradiction_review", normalized.to_mcp_arguments())

    def contradiction_review_report(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> ContradictionReviewReport:
        return contradiction_review_report(self.contradiction_review(request))

    def onco_boundary_check(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Apply the oncology research-only boundary through the HTTP gateway."""

        normalized = request if isinstance(request, OncoBoundaryArgs) else OncoBoundaryArgs.from_wire(request)
        return self.call_tool("onco_boundary_check", normalized.to_mcp_arguments())

    def onco_boundary_report(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> OncoBoundaryReport:
        return onco_boundary_report(self.onco_boundary_check(request))

    def onco_response_assess(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Assess criteria-aware response through the HTTP gateway."""

        normalized = request if isinstance(request, OncoResponseAssessArgs) else OncoResponseAssessArgs.from_wire(request)
        return self.call_tool("onco_response_assess", normalized.to_mcp_arguments())

    def onco_response_report(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> OncoResponseReport:
        return onco_response_report(self.onco_response_assess(request))

    def onco_worldline_view(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Render an oncology worldline with explicit visibility semantics over HTTP."""

        normalized = request if isinstance(request, OncoWorldlineViewArgs) else OncoWorldlineViewArgs.from_wire(request)
        return self.call_tool("onco_worldline_view", normalized.to_mcp_arguments())

    def onco_worldline_report(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> OncoWorldlineReport:
        return onco_worldline_report(self.onco_worldline_view(request))

    def onco_classification_check(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Run integrated oncology classification through the HTTP gateway."""

        normalized = request if isinstance(request, OncoClassificationArgs) else OncoClassificationArgs.from_wire(request)
        return self.call_tool("onco_classification_check", normalized.to_mcp_arguments())

    def onco_classification_report(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> OncoClassificationReport:
        return onco_classification_report(self.onco_classification_check(request))

    def oncoworlds_identity_join(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Check an onco-worlds identity join through HTTP without hiding declined joins."""

        normalized = request if isinstance(request, OncoIdentityJoinArgs) else OncoIdentityJoinArgs.from_wire(request)
        return self.call_tool("oncoworlds_identity_join", normalized.to_mcp_arguments())

    def oncoworlds_identity_join_report(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> OncoIdentityJoinReport:
        return onco_identity_join_report(self.oncoworlds_identity_join(request))

    def onco_outcome_analyze(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Analyze a single oncology follow-up record through HTTP."""

        normalized = request if isinstance(request, OncoOutcomeAnalyzeArgs) else OncoOutcomeAnalyzeArgs.from_wire(request)
        return self.call_tool("onco_outcome_analyze", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_model_transport", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_methylation_classify", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_methylation_compare", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_radiogenomic_check", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_clonal_history_check", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_clonal_evidence_check", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_era_shift_check", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_equity_check", normalized.to_mcp_arguments())

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
        return self.call_tool("oncoworlds_entity_world_check", normalized.to_mcp_arguments())

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
        return self.call_tool("stress_profile", normalized.to_mcp_arguments())

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
        return self.call_tool("stress_report", normalized.to_mcp_arguments())

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
        return self.call_tool("influence_analyze", normalized.to_mcp_arguments())

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
        """Run the offline routing lab through HTTP."""

        normalized = request if isinstance(request, RoutingLabRunArgs) else RoutingLabRunArgs.from_wire(request)
        return self.call_tool("routing_lab_run", normalized.to_mcp_arguments())

    def routing_lab_run_report(
        self,
        request: RoutingLabRunArgs | Mapping[str, Any],
    ) -> RoutingLabRunReport:
        """Return typed holdout, regret, comparator, and calibration evidence."""

        return routing_lab_run_report(self.routing_lab_run(request))

    def lab_pareto_audit(
        self,
        request: LabParetoAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Build the offline inference-lab Pareto archive through HTTP."""

        normalized = request if isinstance(request, LabParetoAuditArgs) else LabParetoAuditArgs.from_wire(request)
        return self.call_tool("lab_pareto_audit", normalized.to_mcp_arguments())

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
        """Audit risk-triggered branch accounting through HTTP."""

        normalized = request if isinstance(request, LabBranchAuditArgs) else LabBranchAuditArgs.from_wire(request)
        return self.call_tool("lab_branch_audit", normalized.to_mcp_arguments())

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
        """Run the offline holdout and rollback audit through HTTP."""

        normalized = request if isinstance(request, LabHoldoutAuditArgs) else LabHoldoutAuditArgs.from_wire(request)
        return self.call_tool("lab_holdout_audit", normalized.to_mcp_arguments())

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
        """Assemble and grade a benchmark-gated evolution card through HTTP."""

        normalized = request if isinstance(request, LabEvolutionAuditArgs) else LabEvolutionAuditArgs.from_wire(request)
        return self.call_tool("lab_evolution_audit", normalized.to_mcp_arguments())

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
        """Validate and inspect an immutable architecture space through HTTP."""

        normalized = request if isinstance(request, LabSpaceAuditArgs) else LabSpaceAuditArgs.from_wire(request)
        return self.call_tool("lab_space_audit", normalized.to_mcp_arguments())

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
        return self.call_tool("provider_capability_gate", normalized.to_mcp_arguments())

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
        return self.call_tool("sdk_registry_check", normalized.to_mcp_arguments())

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
        """Plan bounded evidence acquisition through HTTP without executing actions."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return self.call_tool("lab_plan", request.to_mcp_arguments())

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
        return self.call_tool("obligation_gate_check", normalized.to_mcp_arguments())

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
        """Route an unseen task through HTTP without exposing a hidden oracle answer."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return self.call_tool("routing_decide", request.to_mcp_arguments())

    def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Discover repository modules through the HTTP gateway."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return self.call_tool("repository_catalog", request.to_mcp_arguments())

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
        """Compile a route-specific repository context through HTTP."""

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
        return self.call_tool("repository_bundle", request.to_mcp_arguments())

    def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compute conservative repository impact through HTTP."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return self.call_tool("repository_impact", request.to_mcp_arguments())

    def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Project a redacted telemetry event through HTTP."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return self.call_tool("telemetry_project", request.to_mcp_arguments())

    def telemetry_project_report(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> TelemetryProjectionReport:
        """Return typed telemetry projection evidence through HTTP."""

        return telemetry_project_report(self.telemetry_project(event, policy, trace, metric=metric, observations=observations))

    def ledger_ingest(self, request: LedgerIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Append a bounded event stream through the HTTP gateway."""

        normalized = request if isinstance(request, LedgerIngestArgs) else LedgerIngestArgs.from_wire(request)
        return self.call_tool("ledger_ingest", normalized.to_mcp_arguments())

    def ledger_ingest_report(self, request: LedgerIngestArgs | Mapping[str, Any]) -> LedgerIngestReport:
        """Return typed ledger evidence through HTTP."""

        return ledger_ingest_report(self.ledger_ingest(request))

    def trace_otel_ingest(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Import bounded OTLP JSON through the HTTP gateway."""

        normalized = request if isinstance(request, TraceOtelIngestArgs) else TraceOtelIngestArgs.from_wire(request)
        return self.call_tool("trace_otel_ingest", normalized.to_mcp_arguments())

    def trace_otel_ingest_report(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> TraceOtelIngestReport:
        """Return typed OTLP mapping, semantic-loss, and readiness evidence through HTTP."""

        return trace_otel_ingest_report(self.trace_otel_ingest(request))

    def quality_gate_run(self, request: QualityGateRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Run a serialized bounded quality gate through the HTTP gateway."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return self.call_tool("quality_gate_run", normalized.to_mcp_arguments())

    def quality_gate_run_report(self, request: QualityGateRunArgs | Mapping[str, Any]) -> QualityGateRunReport:
        """Return typed quality verdicts, witnesses, and run obstructions through HTTP."""

        return quality_gate_run_report(self.quality_gate_run(request))

    def atlas_report(self, request: AtlasReportArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Run bounded capability-atlas reporting through the HTTP gateway."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return self.call_tool("atlas_report", normalized.to_mcp_arguments())

    def atlas_report_typed(self, request: AtlasReportArgs | Mapping[str, Any]) -> AtlasReport:
        """Return typed atlas coverage, debt, and composite evidence through HTTP."""

        return atlas_report_parser(self.atlas_report(request))

    def atlas_surface_audit(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit atlasx coverage debt and failure surfaces through the HTTP gateway."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return self.call_tool("atlas_surface_audit", normalized.to_mcp_arguments())

    def atlas_surface_audit_report(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> AtlasSurfaceAuditReport:
        """Return typed atlasx debt, visibility, rate, and surface evidence through HTTP."""

        return atlas_surface_audit_report(self.atlas_surface_audit(request))

    def engineering_manifest_audit(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit an engineering manifest through the HTTP gateway."""

        normalized = request if isinstance(request, EngineeringManifestArgs) else EngineeringManifestArgs.from_wire(request)
        return self.call_tool("engineering_manifest_audit", normalized.to_mcp_arguments())

    def engineering_manifest_audit_report(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> EngineeringAuditReport:
        """Return typed engineering-manifest evidence through HTTP."""

        return engineering_manifest_audit_report(self.engineering_manifest_audit(request))

    def engineering_execution_plan(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Derive bounded engineering execution waves through the HTTP gateway."""

        normalized = request if isinstance(request, EngineeringPlanRequestArgs) else EngineeringPlanRequestArgs.from_wire(request)
        return self.call_tool("engineering_execution_plan", normalized.to_mcp_arguments())

    def engineering_execution_plan_report(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> EngineeringPlanReport:
        """Return typed engineering execution-plan evidence through HTTP."""

        return engineering_execution_plan_report(self.engineering_execution_plan(request))

    def release_pipeline_audit(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit release provenance and promotion boundaries through the HTTP gateway."""

        normalized = request if isinstance(request, ReleasePipelineManifestArgs) else ReleasePipelineManifestArgs.from_wire(request)
        return self.call_tool("release_pipeline_audit", normalized.to_mcp_arguments())

    def release_pipeline_audit_report(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> ReleasePipelineAuditReport:
        """Return typed release-pipeline evidence through HTTP."""

        return release_pipeline_audit_report(self.release_pipeline_audit(request))

    def operational_readiness_audit(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit operational objectives and controls through the HTTP gateway."""

        normalized = request if isinstance(request, OperationalReadinessManifestArgs) else OperationalReadinessManifestArgs.from_wire(request)
        return self.call_tool("operational_readiness_audit", normalized.to_mcp_arguments())

    def operational_readiness_audit_report(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> OperationalReadinessAuditReport:
        """Return typed operational-readiness evidence through HTTP."""

        return operational_readiness_audit_report(self.operational_readiness_audit(request))

    def security_privacy_audit(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit security/privacy governance through the HTTP gateway."""

        normalized = request if isinstance(request, SecurityPrivacyManifestArgs) else SecurityPrivacyManifestArgs.from_wire(request)
        return self.call_tool("security_privacy_audit", normalized.to_mcp_arguments())

    def security_privacy_audit_report(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> SecurityPrivacyAuditReport:
        """Return typed security/privacy governance evidence through HTTP."""

        return security_privacy_audit_report(self.security_privacy_audit(request))

    def sandbox_admission_audit(self, request: SandboxManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit sandbox admission and research-artifact release boundaries through HTTP."""

        normalized = request if isinstance(request, SandboxManifestArgs) else SandboxManifestArgs.from_wire(request)
        return self.call_tool("sandbox_admission_audit", normalized.to_mcp_arguments())

    def sandbox_admission_audit_report(self, request: SandboxManifestArgs | Mapping[str, Any]) -> SandboxAuditReport:
        """Return typed sandbox admission evidence through HTTP."""

        return sandbox_admission_audit_report(self.sandbox_admission_audit(request))

    def sandbox_runtime_simulate(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Simulate bounded sandbox requests through the HTTP gateway."""

        normalized = request if isinstance(request, SandboxRuntimeManifestArgs) else SandboxRuntimeManifestArgs.from_wire(request)
        return self.call_tool("sandbox_runtime_simulate", normalized.to_mcp_arguments())

    def sandbox_runtime_simulate_report(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> SandboxRuntimeAuditReport:
        """Return typed sandbox runtime decisions through HTTP."""

        return sandbox_runtime_simulate_report(self.sandbox_runtime_simulate(request))

    def security_program_audit(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit the security, safety, and red-team program through HTTP."""

        normalized = request if isinstance(request, SecurityProgramManifestArgs) else SecurityProgramManifestArgs.from_wire(request)
        return self.call_tool("security_program_audit", normalized.to_mcp_arguments())

    def security_program_audit_report(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> SecurityProgramAuditReport:
        """Return typed security-program evidence through HTTP."""

        return security_program_audit_report(self.security_program_audit(request))

    def adaptive_panel(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Audit and query a serialized adaptive panel through the HTTP gateway."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return self.call_tool("adaptive_panel", normalized.to_mcp_arguments())

    def adaptive_panel_report(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> AdaptivePanelReport:
        """Return typed adaptive audit and selection evidence through HTTP."""

        return adaptive_panel_report(self.adaptive_panel(request))

    def posterior_gate(self, request: PosteriorGateArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Build a capability posterior and optional release/comparison projections through HTTP."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return self.call_tool("posterior_gate", normalized.to_mcp_arguments())

    def posterior_gate_report(self, request: PosteriorGateArgs | Mapping[str, Any]) -> PosteriorGateReport:
        """Return typed clustered capabilities and fail-closed gate/comparison evidence."""

        return posterior_gate_report(self.posterior_gate(request))

    def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Compile a typed world/query pair through the HTTP gateway."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return self.call_tool("fiber_compile", request.to_mcp_arguments())

    def fiber_compile_decision_quotient(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberDecisionQuotientSummary:
        """Compile and validate the executable 0.3 decision-quotient projection."""

        return fiber_decision_quotient_summary(self.fiber_compile(world, query, layer=layer))

    def fiber_compile_rate_distortion(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberRateDistortionSummary:
        """Compile and validate the executable 0.4 rate-distortion projection."""

        return fiber_rate_distortion_summary(self.fiber_compile(world, query, layer=layer))

    def fiber_compile_adaptive_acquisition(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberAdaptiveAcquisitionSummary:
        """Compile and validate the certificate-bound 0.5 adaptive policy projection."""

        return fiber_adaptive_acquisition_summary(self.fiber_compile(world, query, layer=layer))

    def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Refine a compiled context through the HTTP gateway."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return self.call_tool("fiber_refine", request.to_mcp_arguments())

    def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Request a compile-plan explanation through the HTTP gateway."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return self.call_tool("fiber_explain", request.to_mcp_arguments())

    def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Verify a context certificate through the HTTP gateway."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return self.call_tool("fiber_verify", request.to_mcp_arguments())

    def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Request bounded context projections through the HTTP gateway."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return self.call_tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    def events(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> dict[str, Any]:
        if after < 0 or not 1 <= limit <= 1000:
            raise ArgumentError("after must be non-negative and limit must be 1..=1000")
        if review_id is not None and receipt_id is not None:
            raise ArgumentError("review_id and receipt_id are mutually exclusive")
        suffix = ""
        if review_id is not None:
            suffix = f"&review_id={quote(validate_review_id(review_id), safe='')}"
        elif receipt_id is not None:
            suffix = f"&receipt_id={quote(validate_receipt_id(receipt_id), safe='')}"
        return self.request("GET", f"/v1/events?after={after}&limit={limit}{suffix}")

    def event_page(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> EventPage:
        """Read a typed cursor page over all retained tool and mission events."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        if review_id is not None and receipt_id is not None:
            raise ArgumentError("review_id and receipt_id are mutually exclusive")
        suffix = ""
        if review_id is not None:
            suffix = f"&review_id={quote(validate_review_id(review_id), safe='')}"
        elif receipt_id is not None:
            suffix = f"&receipt_id={quote(validate_receipt_id(receipt_id), safe='')}"
        return EventPage.from_wire(
            self.request("GET", f"/v1/events?after={after}&limit={limit}{suffix}")
        )

    def event_stream(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> SseSnapshot:
        """Fetch and parse the bounded SSE snapshot without requiring an EventSource runtime."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        if review_id is not None and receipt_id is not None:
            raise ArgumentError("review_id and receipt_id are mutually exclusive")
        suffix = ""
        if review_id is not None:
            suffix = f"&review_id={quote(validate_review_id(review_id), safe='')}"
        elif receipt_id is not None:
            suffix = f"&receipt_id={quote(validate_receipt_id(receipt_id), safe='')}"
        raw, headers = self.request_text(
            "GET", f"/v1/events/stream?after={after}&limit={limit}{suffix}"
        )
        next_after_value = headers.get("x-next-after")
        if next_after_value is None:
            next_after = None
        elif next_after_value.isdigit():
            next_after = int(next_after_value)
        else:
            raise TransportError("HTTP API x-next-after header is not an unsigned integer")
        return SseSnapshot(headers.get("content-type", ""), next_after, parse_sse(raw), raw)

    def route_review_evidence(
        self, review_id: str, *, after: int = 0, limit: int = 100
    ) -> RouteReviewEvidence:
        """Retrieve retained event evidence for one content-addressed route review."""

        review_id = validate_review_id(review_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        return RouteReviewEvidence.from_wire(
            self.request(
                "GET",
                f"/v1/route-reviews/{quote(review_id, safe='')}/evidence?after={after}&limit={limit}",
            )
        )

    def delivery_receipt_events(
        self, receipt_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryReceiptEvents:
        """Retrieve retained event evidence for one delivery receipt identifier."""

        receipt_id = validate_receipt_id(receipt_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        return DeliveryReceiptEvents.from_wire(
            self.request(
                "GET",
                f"/v1/delivery-receipts/{quote(receipt_id, safe='')}/events?after={after}&limit={limit}",
            )
        )

    def delivery_receipt_attempts(
        self, receipt_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryReceiptAttempts:
        """Retrieve durable webhook-attempt provenance correlated to one receipt."""

        receipt_id = validate_receipt_id(receipt_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        return DeliveryReceiptAttempts.from_wire(
            self.request(
                "GET",
                f"/v1/delivery-receipts/{quote(receipt_id, safe='')}/attempts?after={after}&limit={limit}",
            )
        )

    def event_persistence(self) -> EventPersistenceStatus:
        """Inspect the optional restart-aware event cursor checkpoint."""

        return EventPersistenceStatus.from_wire(self.request("GET", "/v1/events/persistence"))

    def recovery_matrix(self) -> RecoveryMatrix:
        """Return one typed matrix of mission, event, secret, and external-effect boundaries."""

        return RecoveryMatrix.from_wire(self.request("GET", "/v1/recovery"))

    def operations_snapshot(self, *, after: int = 0, limit: int = 100) -> OperationsSnapshot:
        """Return one bounded operator snapshot across events, missions, and recovery."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if (
            isinstance(limit, bool)
            or not isinstance(limit, int)
            or not 1 <= limit <= MAX_OPERATIONS_SNAPSHOT_LIMIT
        ):
            raise ArgumentError(
                f"limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"
            )
        return OperationsSnapshot.from_wire(
            self.request("GET", f"/v1/operations/snapshot?after={after}&limit={limit}")
        )

    def operations_handoff(
        self,
        *,
        goal: str | None = None,
        domains: Sequence[str] | None = None,
        group_ids: Sequence[str] | None = None,
        include_complete: bool = True,
        max_groups: int = MAX_OPERATIONS_DOMAIN_GROUPS,
    ) -> OperationsHandoff:
        """Build a typed, non-executing routing handoff from domain coverage evidence."""

        if goal is not None and (
            not isinstance(goal, str)
            or not goal.strip()
            or len(goal.encode("utf-8")) > 1024
            or any(ord(character) < 0x20 for character in goal)
        ):
            raise ArgumentError("goal must be a non-empty visible string of at most 1024 bytes")
        if not isinstance(include_complete, bool):
            raise ArgumentError("include_complete must be a boolean")
        if (
            isinstance(max_groups, bool)
            or not isinstance(max_groups, int)
            or not 1 <= max_groups <= MAX_OPERATIONS_DOMAIN_GROUPS
        ):
            raise ArgumentError(
                f"max_groups must be between 1 and {MAX_OPERATIONS_DOMAIN_GROUPS}"
            )

        payload: dict[str, Any] = {"include_complete": include_complete, "max_groups": max_groups}
        for name, values in (("domains", domains), ("group_ids", group_ids)):
            if values is None:
                continue
            if isinstance(values, (str, bytes)) or len(values) > MAX_OPERATIONS_DOMAIN_GROUPS:
                raise ArgumentError(
                    f"{name} must contain at most {MAX_OPERATIONS_DOMAIN_GROUPS} visible strings"
                )
            normalized: list[str] = []
            for value in values:
                if (
                    not isinstance(value, str)
                    or not value.strip()
                    or len(value.encode("utf-8")) > 128
                    or any(ord(character) < 0x20 for character in value)
                ):
                    raise ArgumentError(
                        f"{name} entries must be non-empty visible strings of at most 128 bytes"
                    )
                normalized.append(value)
            payload[name] = sorted(set(normalized))
        if goal is not None:
            payload["goal"] = goal
        return OperationsHandoff.from_wire(self.request("POST", "/v1/operations/handoff", payload))

    def operations_domain_activity(
        self, *, after: int = 0, limit: int = 100
    ) -> OperationsDomainActivity:
        """Return bounded local activity evidence grouped by capability domain."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if (
            isinstance(limit, bool)
            or not isinstance(limit, int)
            or not 1 <= limit <= MAX_OPERATIONS_SNAPSHOT_LIMIT
        ):
            raise ArgumentError(
                f"limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"
            )
        return OperationsDomainActivity.from_wire(
            self.request("GET", f"/v1/operations/domains?after={after}&limit={limit}")
        )

    def operations_domain_gates(
        self, *, after: int = 0, limit: int = 100
    ) -> OperationsDomainGates:
        """Return bounded per-domain evidence gates without claiming readiness."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if (
            isinstance(limit, bool)
            or not isinstance(limit, int)
            or not 1 <= limit <= MAX_OPERATIONS_SNAPSHOT_LIMIT
        ):
            raise ArgumentError(
                f"limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"
            )
        return OperationsDomainGates.from_wire(
            self.request("GET", f"/v1/operations/gates?after={after}&limit={limit}")
        )

    def operations_gate_reviews(
        self, *, after: int = 0, limit: int = 100, review_id: str | None = None
    ) -> OperationsGateReviews:
        """Replay durable operations gate reviews with explicit cursor and retention evidence."""

        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if (
            isinstance(limit, bool)
            or not isinstance(limit, int)
            or not 1 <= limit <= MAX_OPERATIONS_SNAPSHOT_LIMIT
        ):
            raise ArgumentError(
                f"limit must be between 1 and {MAX_OPERATIONS_SNAPSHOT_LIMIT}"
            )
        query = f"/v1/operations/gate-reviews?after={after}&limit={limit}"
        if review_id is not None:
            query += f"&review_id={quote(validate_review_id(review_id), safe='')}"
        return OperationsGateReviews.from_wire(self.request("GET", query))

    def create_operations_gate_review(
        self, review: OperationsGateReviewRequest | Mapping[str, Any]
    ) -> OperationsGateReview:
        """Persist a current, content-addressed operator review before mission execution."""

        if isinstance(review, OperationsGateReviewRequest):
            payload = review.to_dict()
        elif isinstance(review, Mapping):
            payload = dict(review)
        else:
            raise ArgumentError("review must be an OperationsGateReviewRequest or mapping")
        return OperationsGateReview.from_wire(
            self.request("POST", "/v1/operations/gate-reviews", payload)
        )

    def flush_event_persistence(self) -> EventPersistenceStatus:
        """Force an event cursor checkpoint and return typed bounded status."""

        return EventPersistenceStatus.from_wire(
            self.request("POST", "/v1/events/persistence/flush", {})
        )

    def subscribe(
        self,
        endpoint: str,
        secret: str,
        *,
        subscription_id: str | None = None,
        events: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {"endpoint": endpoint, "secret": secret}
        if subscription_id is not None:
            payload["id"] = subscription_id
        if events is not None:
            payload["events"] = list(events)
        return self.request("POST", "/v1/webhooks/subscriptions", payload)

    def rebind_subscription(self, subscription_id: str, secret: str) -> dict[str, Any]:
        """Rebind a restored webhook secret in memory and re-sign pending envelopes."""

        self._subscription_id(subscription_id)
        if not isinstance(secret, str) or len(secret) < 8 or len(secret) > 4096 or any(ord(character) < 0x20 for character in secret):
            raise ArgumentError("secret must contain 8..=4096 printable characters")
        return self.request(
            "POST",
            f"/v1/webhooks/subscriptions/{subscription_id}/rebind",
            {"secret": secret},
        )

    def deliveries(self, subscription_id: str, *, after: int = 0, limit: int = 100) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        if after < 0 or not 1 <= limit <= 1000:
            raise ArgumentError("after must be non-negative and limit must be 1..=1000")
        return self.request("GET", f"/v1/webhooks/subscriptions/{subscription_id}/deliveries?after={after}&limit={limit}")

    def delivery_page(self, subscription_id: str, *, after: int = 0, limit: int = 100) -> DeliveryPage:
        """Read a typed cursor page over a subscription's pending signed deliveries."""

        self._subscription_id(subscription_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        return DeliveryPage.from_wire(
            self.request(
                "GET",
                f"/v1/webhooks/subscriptions/{subscription_id}/deliveries?after={after}&limit={limit}",
            )
        )

    def delivery_attempts(
        self, subscription_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryAttemptPage:
        """Read durable send/retry/replay/acknowledgement provenance for a subscription."""

        self._subscription_id(subscription_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ArgumentError("after must be a non-negative integer")
        if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= MAX_EVENT_PAGE:
            raise ArgumentError(f"limit must be between 1 and {MAX_EVENT_PAGE}")
        return DeliveryAttemptPage.from_wire(
            self.request(
                "GET",
                f"/v1/webhooks/subscriptions/{subscription_id}/attempts?after={after}&limit={limit}",
            )
        )

    def acknowledge(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("POST", f"/v1/webhooks/subscriptions/{subscription_id}/ack", {"delivery_ids": list(delivery_ids)})

    def retry(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("POST", f"/v1/webhooks/subscriptions/{subscription_id}/retry", {"delivery_ids": list(delivery_ids)})

    def replay(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        """Reset selected pending deliveries for an explicit bounded replay."""

        self._subscription_id(subscription_id)
        return self.request("POST", f"/v1/webhooks/subscriptions/{subscription_id}/replay", {"delivery_ids": list(delivery_ids)})

    def delete_subscription(self, subscription_id: str) -> dict[str, Any]:
        self._subscription_id(subscription_id)
        return self.request("DELETE", f"/v1/webhooks/subscriptions/{subscription_id}")

    @staticmethod
    def _subscription_id(value: str) -> None:
        if not isinstance(value, str) or not value or "/" in value or "\r" in value or "\n" in value:
            raise ArgumentError("subscription_id must be a non-empty path-safe string")

    @staticmethod
    def _mission_id(value: str) -> None:
        if not isinstance(value, str) or not value or "/" in value or "\r" in value or "\n" in value:
            raise ArgumentError("mission_id must be a non-empty path-safe string")

    @staticmethod
    def _mission_wait_options(timeout: float, poll_interval: float) -> tuple[float, float]:
        if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not math.isfinite(timeout) or not 0 < timeout <= MAX_MISSION_WAIT_SECONDS:
            raise ArgumentError(f"timeout must be finite and between 0 and {MAX_MISSION_WAIT_SECONDS:g} seconds")
        if isinstance(poll_interval, bool) or not isinstance(poll_interval, (int, float)) or not math.isfinite(poll_interval) or not 0 < poll_interval <= MAX_MISSION_POLL_INTERVAL_SECONDS:
            raise ArgumentError(
                f"poll_interval must be finite and between 0 and {MAX_MISSION_POLL_INTERVAL_SECONDS:g} seconds"
            )
        return float(timeout), float(poll_interval)


class AsyncApiClient:
    """Async facade over :class:`ApiClient`, using bounded worker threads for stdlib portability."""

    def __init__(self, client: ApiClient) -> None:
        self.client = client

    async def request(self, method: str, path: str, payload: Mapping[str, Any] | None = None, *, headers: Mapping[str, str] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.request, method, path, payload, headers=headers)

    async def health(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.health)

    async def capabilities(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.capabilities)

    async def tools(self) -> list[dict[str, Any]]:
        return await asyncio.to_thread(self.client.tools)

    async def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.call_tool, name, arguments)

    async def register_artifact(
        self,
        request: ArtifactRegistrationRequest | Mapping[str, Any],
    ) -> ArtifactRegistrationReport:
        return await asyncio.to_thread(self.client.register_artifact, request)

    async def query_artifacts(
        self,
        request: ArtifactQueryRequest | Mapping[str, Any] | None = None,
    ) -> ArtifactQueryReport:
        return await asyncio.to_thread(self.client.query_artifacts, request)

    async def get_artifact(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactGetReport:
        return await asyncio.to_thread(self.client.get_artifact, request)

    async def artifact_lineage(
        self,
        request: ArtifactGetRequest | Mapping[str, Any] | str,
    ) -> ArtifactLineageReport:
        return await asyncio.to_thread(self.client.artifact_lineage, request)

    async def artifact_registry_persistence(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.artifact_registry_persistence)

    async def artifact_cross_store_audit(self) -> ArtifactCrossStoreAuditReport:
        return await asyncio.to_thread(self.client.artifact_cross_store_audit)

    async def flush_artifact_registry_persistence(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.flush_artifact_registry_persistence)

    async def artifact_registry_audit(self, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.artifact_registry_audit, arguments)

    async def domain_report_project(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        return await asyncio.to_thread(self.client.domain_report_project, request)

    async def domain_report_coverage(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        return await asyncio.to_thread(self.client.domain_report_coverage, request)

    async def domain_report_project_tool(
        self,
        request: DomainReportProjectRequest | Mapping[str, Any],
    ) -> DomainReportProjectReport:
        return await asyncio.to_thread(self.client.domain_report_project_tool, request)

    async def domain_report_from_adapter_execution(
        self,
        evidence: AdapterExecutionEvidenceRequest | Mapping[str, Any],
        conformance: Mapping[str, Any] | None = None,
    ) -> AdapterDomainReportResult:
        return await asyncio.to_thread(
            self.client.domain_report_from_adapter_execution,
            evidence,
            conformance,
        )

    async def domain_report_from_provider_normalization(
        self,
        normalization: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> ProviderDomainReportResult:
        return await asyncio.to_thread(
            self.client.domain_report_from_provider_normalization,
            normalization,
        )

    async def domain_report_from_external_provider_normalization(
        self,
        normalization: DomainEvidenceProviderExternalPayloadNormalizationRequest
        | Mapping[str, Any],
    ) -> ProviderDomainReportResult:
        return await asyncio.to_thread(
            self.client.domain_report_from_external_provider_normalization,
            normalization,
        )

    async def domain_report_coverage_tool(
        self,
        request: DomainReportCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainReportCoverageReport:
        return await asyncio.to_thread(self.client.domain_report_coverage_tool, request)

    async def domain_evidence_harmonize(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        return await asyncio.to_thread(self.client.domain_evidence_harmonize, request)

    async def domain_evidence_harmonize_tool(
        self,
        request: DomainEvidenceHarmonizeRequest | Mapping[str, Any],
    ) -> DomainEvidenceHarmonizationReport:
        return await asyncio.to_thread(self.client.domain_evidence_harmonize_tool, request)

    async def domain_evidence_harmonization_coverage(
        self,
        request: DomainEvidenceHarmonizationCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceHarmonizationCoverageReport:
        return await asyncio.to_thread(self.client.domain_evidence_harmonization_coverage, request)

    async def domain_evidence_harmonization_coverage_tool(
        self,
        request: DomainEvidenceHarmonizationCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceHarmonizationCoverageReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_harmonization_coverage_tool, request
        )

    async def domain_evidence_intake(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        return await asyncio.to_thread(self.client.domain_evidence_intake, request)

    async def domain_evidence_intake_tool(
        self,
        request: DomainEvidenceIntakeRequest | Mapping[str, Any],
    ) -> DomainEvidenceIntakeReport:
        return await asyncio.to_thread(self.client.domain_evidence_intake_tool, request)

    async def domain_evidence_coverage(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        return await asyncio.to_thread(self.client.domain_evidence_coverage, request)

    async def domain_evidence_coverage_tool(
        self,
        request: DomainEvidenceIntakeCoverageRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceIntakeCoverageReport:
        return await asyncio.to_thread(self.client.domain_evidence_coverage_tool, request)

    async def domain_evidence_source_plan(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        return await asyncio.to_thread(self.client.domain_evidence_source_plan, request)

    async def domain_evidence_source_plan_tool(
        self,
        request: DomainEvidenceSourcePlanRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourcePlanReport:
        return await asyncio.to_thread(self.client.domain_evidence_source_plan_tool, request)

    async def domain_evidence_source_execute(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        return await asyncio.to_thread(self.client.domain_evidence_source_execute, request)

    async def domain_evidence_source_execute_tool(
        self,
        request: DomainEvidenceSourceExecutionRequest | Mapping[str, Any],
    ) -> DomainEvidenceSourceExecutionReport:
        return await asyncio.to_thread(self.client.domain_evidence_source_execute_tool, request)

    async def domain_evidence_source_project(
        self,
        execution: DomainEvidenceSourceExecutionReport | Mapping[str, Any],
        request: SourceAdapterProjectionRequest | Mapping[str, Any],
        *,
        runtime: AdapterRuntime | None = None,
    ) -> SourceAdapterProjectionResult:
        """Project a returned source envelope locally without another network call."""

        return await asyncio.to_thread(
            self.client.domain_evidence_source_project,
            execution,
            request,
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

        return await asyncio.to_thread(
            self.client.domain_evidence_source_project_for_domain,
            catalogue,
            execution,
            request,
            runtime=runtime,
        )

    async def domain_evidence_provider_normalize(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderNormalizationReport:
        return await asyncio.to_thread(self.client.domain_evidence_provider_normalize, request)

    async def domain_evidence_provider_normalize_tool(
        self,
        request: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderNormalizationReport:
        return await asyncio.to_thread(self.client.domain_evidence_provider_normalize_tool, request)

    async def domain_evidence_provider_replay_verify(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        return await asyncio.to_thread(self.client.domain_evidence_provider_replay_verify, request)

    async def domain_evidence_provider_replay_verify_tool(
        self,
        request: DomainEvidenceProviderReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderReplayVerificationReport:
        return await asyncio.to_thread(self.client.domain_evidence_provider_replay_verify_tool, request)

    async def domain_evidence_provider_connector_handoff(
        self,
        request: DomainEvidenceProviderHandoffRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderHandoffReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_connector_handoff, request
        )

    async def domain_evidence_provider_connector_handoff_tool(
        self,
        request: DomainEvidenceProviderHandoffRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderHandoffReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_connector_handoff_tool, request
        )

    async def domain_evidence_provider_external_payload_receipt(
        self,
        request: DomainEvidenceProviderExternalPayloadReceiptRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReceiptReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_receipt, request
        )

    async def domain_evidence_provider_external_payload_receipt_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadReceiptRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReceiptReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_receipt_tool, request
        )

    async def domain_evidence_provider_external_payload_replay_verify(
        self,
        request: DomainEvidenceProviderExternalPayloadReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReplayVerificationReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_replay_verify, request
        )

    async def domain_evidence_provider_external_payload_replay_verify_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadReplayRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadReplayVerificationReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_replay_verify_tool, request
        )

    async def domain_evidence_provider_external_payload_normalize(
        self,
        request: DomainEvidenceProviderExternalPayloadNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadNormalizationReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_normalize, request
        )

    async def domain_evidence_provider_external_payload_normalize_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadNormalizationRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadNormalizationReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_normalize_tool, request
        )

    async def domain_evidence_provider_external_payload_lineage_audit(
        self,
        request: DomainEvidenceProviderExternalPayloadLineageAuditRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadLineageAuditReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_lineage_audit, request
        )

    async def domain_evidence_provider_external_payload_lineage_audit_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadLineageAuditRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadLineageAuditReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_lineage_audit_tool, request
        )

    async def domain_evidence_provider_external_payload_execution_evidence(
        self,
        request: DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadExecutionEvidenceReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_execution_evidence, request
        )

    async def domain_evidence_provider_external_payload_execution_evidence_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest | Mapping[str, Any],
    ) -> DomainEvidenceProviderExternalPayloadExecutionEvidenceReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_execution_evidence_tool, request
        )

    async def domain_evidence_provider_external_payload_evidence_query(
        self,
        request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_evidence_query, request
        )

    async def domain_evidence_provider_external_payload_evidence_query_tool(
        self,
        request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
        return await asyncio.to_thread(
            self.client.domain_evidence_provider_external_payload_evidence_query_tool, request
        )

    async def domain_workflow_catalogue(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_catalogue)

    async def domain_workflow_catalogue_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(await self.domain_workflow_catalogue())

    async def domain_workflow_scaffold(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_scaffold, request)

    async def domain_workflow_scaffold_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(await self.domain_workflow_scaffold(request))

    async def domain_workflow_instantiate(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_instantiate, request)

    async def domain_workflow_instantiation_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(await self.domain_workflow_instantiate(request))

    async def domain_workflow_portfolio(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_portfolio, request)

    async def domain_workflow_portfolio_report(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioReport:
        return domain_workflow_portfolio_report(await self.domain_workflow_portfolio(request))

    async def domain_workflow_portfolio_verify(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_portfolio_verify, request)

    async def domain_workflow_portfolio_verify_report(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioVerifyReport:
        return domain_workflow_portfolio_verify_report(
            await self.domain_workflow_portfolio_verify(request)
        )

    async def domain_workflow_verify(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_verify, request)

    async def domain_workflow_verify_report(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowVerifyReport:
        return domain_workflow_verify_report(await self.domain_workflow_verify(request))

    async def domain_workflow_reconcile(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconcile, request)

    async def domain_workflow_reconciliation_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(await self.domain_workflow_reconcile(request))

    async def domain_workflow_catalogue_tool(self) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_catalogue_tool)

    async def domain_workflow_catalogue_tool_report(self) -> DomainWorkflowCatalogueReport:
        return domain_workflow_catalogue_report(await self.domain_workflow_catalogue_tool())

    async def domain_workflow_scaffold_tool(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_scaffold_tool, request)

    async def domain_workflow_scaffold_tool_report(
        self,
        request: DomainWorkflowScaffoldRequest | Mapping[str, Any],
    ) -> DomainWorkflowScaffoldReport:
        return domain_workflow_scaffold_report(await self.domain_workflow_scaffold_tool(request))

    async def domain_workflow_instantiate_tool(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_instantiate_tool, request)

    async def domain_workflow_instantiation_tool_report(
        self,
        request: DomainWorkflowInstantiateRequest | Mapping[str, Any],
    ) -> DomainWorkflowInstantiationReport:
        return domain_workflow_instantiation_report(
            await self.domain_workflow_instantiate_tool(request)
        )

    async def domain_workflow_portfolio_tool(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_portfolio_tool, request)

    async def domain_workflow_portfolio_tool_report(
        self,
        request: DomainWorkflowPortfolioRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioReport:
        return domain_workflow_portfolio_report(await self.domain_workflow_portfolio_tool(request))

    async def domain_workflow_portfolio_verify_tool(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_portfolio_verify_tool, request)

    async def domain_workflow_portfolio_verify_tool_report(
        self,
        request: DomainWorkflowPortfolioVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowPortfolioVerifyReport:
        return domain_workflow_portfolio_verify_report(
            await self.domain_workflow_portfolio_verify_tool(request)
        )

    async def domain_workflow_verify_tool(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_verify_tool, request)

    async def domain_workflow_verify_tool_report(
        self,
        request: DomainWorkflowVerifyRequest | Mapping[str, Any],
    ) -> DomainWorkflowVerifyReport:
        return domain_workflow_verify_report(await self.domain_workflow_verify_tool(request))

    async def domain_workflow_reconcile_tool(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconcile_tool, request)

    async def domain_workflow_reconciliation_tool_report(
        self,
        request: DomainWorkflowReconcileRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationReport:
        return domain_workflow_reconciliation_report(
            await self.domain_workflow_reconcile_tool(request)
        )

    async def domain_workflow_reconciliation_import(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_import, request)

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
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_query, request)

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
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_get, request)

    async def domain_workflow_reconciliation_get_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            await self.domain_workflow_reconciliation_get(request)
        )

    async def domain_workflow_reconciliation_import_tool(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_import_tool, request)

    async def domain_workflow_reconciliation_import_tool_report(
        self,
        request: DomainWorkflowReconciliationImportRequest | Mapping[str, Any],
    ) -> DomainWorkflowReconciliationImportReport:
        return DomainWorkflowReconciliationImportReport.from_wire(
            await self.domain_workflow_reconciliation_import_tool(request)
        )

    async def domain_workflow_reconciliation_query_tool(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_query_tool, request)

    async def domain_workflow_reconciliation_query_tool_report(
        self,
        request: DomainWorkflowReconciliationQueryRequest | Mapping[str, Any] | None = None,
    ) -> DomainWorkflowReconciliationQueryReport:
        return DomainWorkflowReconciliationQueryReport.from_wire(
            await self.domain_workflow_reconciliation_query_tool(request)
        )

    async def domain_workflow_reconciliation_get_tool(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.domain_workflow_reconciliation_get_tool, request)

    async def domain_workflow_reconciliation_get_tool_report(
        self,
        request: DomainWorkflowReconciliationGetRequest | Mapping[str, Any] | str,
    ) -> DomainWorkflowReconciliationGetReport:
        return DomainWorkflowReconciliationGetReport.from_wire(
            await self.domain_workflow_reconciliation_get_tool(request)
        )

    async def submit_mission(self, request: MissionRequest | Mapping[str, Any]) -> MissionJob:
        return await asyncio.to_thread(self.client.submit_mission, request)

    async def mission_status(self, mission_id: str) -> MissionJob:
        return await asyncio.to_thread(self.client.mission_status, mission_id)

    async def mission_provenance(self, mission_id: str) -> MissionExecutionProvenance:
        """Async read of the retained gate/review/evaluator execution evidence."""

        return await asyncio.to_thread(self.client.mission_provenance, mission_id)

    async def mission_claim_lineage(self, mission_id: str) -> MissionClaimLineage:
        """Async read of the bounded claim-to-step evidence projection."""

        return await asyncio.to_thread(self.client.mission_claim_lineage, mission_id)

    async def mission_evaluator_replay_query(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Async read of durable full or summary-only evaluator replay evidence."""

        return await asyncio.to_thread(self.client.mission_evaluator_replay_query, request)

    async def mission_evaluator_replay_query_report(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> MissionEvaluatorReplayQueryReport:
        """Return the typed async durable evaluator replay query projection."""

        return mission_evaluator_replay_query_report(await self.mission_evaluator_replay_query(request))

    async def mission_evaluator_replay_compare_query(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Async durable REST replay comparison."""

        return await asyncio.to_thread(self.client.mission_evaluator_replay_compare_query, request)

    async def mission_evaluator_replay_compare_query_report(
        self,
        request: MissionEvaluatorReplayQueryRequest | Mapping[str, Any] | str,
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed async durable REST replay comparison evidence."""

        return mission_evaluator_replay_comparison_report(
            await self.mission_evaluator_replay_compare_query(request)
        )

    async def mission_evidence_bundle(
        self,
        request: MissionEvidenceBundleRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Async durable mission evidence bundle export."""

        return await asyncio.to_thread(self.client.mission_evidence_bundle, request)

    async def mission_evidence_bundle_report(
        self,
        request: MissionEvidenceBundleRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleReport:
        """Return typed async durable mission evidence bundle."""

        return mission_evidence_bundle_report(await self.mission_evidence_bundle(request))

    async def mission_evidence_bundle_verify(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async durable REST mission evidence bundle verification."""

        return await asyncio.to_thread(self.client.mission_evidence_bundle_verify, request)

    async def mission_evidence_bundle_verification_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed async durable mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(await self.mission_evidence_bundle_verify(request))

    async def mission_evidence_bundle_import(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_import, request)

    async def mission_evidence_bundle_import_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        return MissionEvidenceBundleImportReport.from_wire(await self.mission_evidence_bundle_import(request))

    async def mission_evidence_bundle_query(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_query, request)

    async def mission_evidence_bundle_query_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        return MissionEvidenceBundleQueryReport.from_wire(await self.mission_evidence_bundle_query(request))

    async def mission_evidence_bundle_get(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_get, request)

    async def mission_evidence_bundle_get_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        return MissionEvidenceBundleGetReport.from_wire(await self.mission_evidence_bundle_get(request))

    async def mission_trace(self, mission_id: str, *, after: int = 0, limit: int = 100) -> MissionTracePage:
        """Async bounded cursor page over the authoritative mission trace."""

        return await asyncio.to_thread(self.client.mission_trace, mission_id, after=after, limit=limit)

    async def wait_mission(
        self,
        mission_id: str,
        *,
        timeout: float = 30.0,
        poll_interval: float = 0.25,
    ) -> MissionJob:
        """Async bounded mission wait that does not block the event loop between polls."""

        self.client._mission_id(mission_id)
        timeout_value, poll_value = self.client._mission_wait_options(timeout, poll_interval)
        deadline = asyncio.get_running_loop().time() + timeout_value
        job = await self.mission_status(mission_id)
        while not job.terminal:
            remaining = deadline - asyncio.get_running_loop().time()
            if remaining <= 0:
                raise MissionWaitTimeout(mission_id, timeout_value, job)
            await asyncio.sleep(min(poll_value, remaining))
            job = await self.mission_status(mission_id)
        return job

    async def cancel_mission(self, mission_id: str, reason: str | None = None) -> MissionJob:
        return await asyncio.to_thread(self.client.cancel_mission, mission_id, reason)

    async def delete_mission(self, mission_id: str) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.delete_mission, mission_id)

    async def tool_catalogue(self) -> ToolCatalogue:
        """Async snapshot of the authoritative live HTTP ``/v1/tools`` catalogue."""

        return ToolCatalogue.from_definitions(await self.tools())

    async def plan_tool(
        self,
        name: str,
        arguments: Mapping[str, Any] | None = None,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> ToolCallPlan:
        """Validate any advertised tool's JSON shape without issuing a POST."""

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
        """Run any advertised tool after conservative schema preflight."""

        plan = await self.plan_tool(name, arguments, catalogue=catalogue)
        return await self.call_tool(plan.tool, plan.to_mcp_arguments())

    async def mission_preflight(
        self,
        request: MissionRequest,
        *,
        catalogue: ToolCatalogue | None = None,
    ) -> MissionPreflight:
        """Async review of a mission against the live HTTP tool catalogue."""

        if not isinstance(request, MissionRequest):
            raise ArgumentError("request must be a MissionRequest")
        snapshot = catalogue if catalogue is not None else await self.tool_catalogue()
        return preflight_mission(request, snapshot)

    async def preflight_mission(self, request: MissionRequest | Mapping[str, Any]) -> dict[str, Any]:
        """Async request for the Rust-owned no-dispatch mission plan."""

        return await asyncio.to_thread(self.client.preflight_mission, request)

    async def missions(self, *, status: str | None = None, limit: int = 100) -> dict[str, Any]:
        """Async bounded mission inventory."""

        return await asyncio.to_thread(self.client.missions, status=status, limit=limit)

    async def event_page(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> EventPage:
        """Async typed cursor page over retained tool and mission events."""

        return await asyncio.to_thread(
            self.client.event_page,
            after=after,
            limit=limit,
            review_id=review_id,
            receipt_id=receipt_id,
        )

    async def event_stream(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> SseSnapshot:
        """Async bounded SSE snapshot with the same cursor contract as the sync client."""

        return await asyncio.to_thread(
            self.client.event_stream,
            after=after,
            limit=limit,
            review_id=review_id,
            receipt_id=receipt_id,
        )

    async def route_review_evidence(
        self, review_id: str, *, after: int = 0, limit: int = 100
    ) -> RouteReviewEvidence:
        """Async retained route-review evidence lookup."""

        return await asyncio.to_thread(
            self.client.route_review_evidence, review_id, after=after, limit=limit
        )

    async def delivery_receipt_events(
        self, receipt_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryReceiptEvents:
        """Async retained delivery-receipt event lookup."""

        return await asyncio.to_thread(
            self.client.delivery_receipt_events, receipt_id, after=after, limit=limit
        )

    async def delivery_receipt_attempts(
        self, receipt_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryReceiptAttempts:
        """Async durable webhook-attempt provenance lookup for one receipt."""

        return await asyncio.to_thread(
            self.client.delivery_receipt_attempts, receipt_id, after=after, limit=limit
        )

    async def event_persistence(self) -> EventPersistenceStatus:
        """Async inspection of the optional event cursor checkpoint."""

        return await asyncio.to_thread(self.client.event_persistence)

    async def recovery_matrix(self) -> RecoveryMatrix:
        """Async typed recovery-boundary matrix."""

        return await asyncio.to_thread(self.client.recovery_matrix)

    async def operations_snapshot(
        self, *, after: int = 0, limit: int = 100
    ) -> OperationsSnapshot:
        """Async bounded operator snapshot across events, missions, and recovery."""

        return await asyncio.to_thread(
            self.client.operations_snapshot, after=after, limit=limit
        )

    async def operations_handoff(
        self,
        *,
        goal: str | None = None,
        domains: Sequence[str] | None = None,
        group_ids: Sequence[str] | None = None,
        include_complete: bool = True,
        max_groups: int = MAX_OPERATIONS_DOMAIN_GROUPS,
    ) -> OperationsHandoff:
        """Async typed domain routing handoff without dispatch."""

        return await asyncio.to_thread(
            self.client.operations_handoff,
            goal=goal,
            domains=domains,
            group_ids=group_ids,
            include_complete=include_complete,
            max_groups=max_groups,
        )

    async def operations_domain_activity(
        self, *, after: int = 0, limit: int = 100
    ) -> OperationsDomainActivity:
        """Async typed per-domain activity projection with an explicit event cursor."""

        return await asyncio.to_thread(
            self.client.operations_domain_activity, after=after, limit=limit
        )

    async def operations_domain_gates(
        self, *, after: int = 0, limit: int = 100
    ) -> OperationsDomainGates:
        """Async typed evidence gates with an explicit bounded event cursor."""

        return await asyncio.to_thread(
            self.client.operations_domain_gates, after=after, limit=limit
        )

    async def operations_gate_reviews(
        self, *, after: int = 0, limit: int = 100, review_id: str | None = None
    ) -> OperationsGateReviews:
        """Async replay of durable operations gate reviews."""

        return await asyncio.to_thread(
            self.client.operations_gate_reviews,
            after=after,
            limit=limit,
            review_id=review_id,
        )

    async def create_operations_gate_review(
        self, review: OperationsGateReviewRequest | Mapping[str, Any]
    ) -> OperationsGateReview:
        """Async persistence of one current operations gate review."""

        return await asyncio.to_thread(self.client.create_operations_gate_review, review)

    async def flush_event_persistence(self) -> EventPersistenceStatus:
        """Async forced event cursor checkpoint with typed bounded status."""

        return await asyncio.to_thread(self.client.flush_event_persistence)

    async def mission_inventory(self, *, status: str | None = None, limit: int = 100) -> MissionInventoryPage:
        """Async typed bounded mission inventory."""

        return await asyncio.to_thread(self.client.mission_inventory, status=status, limit=limit)

    async def mission_persistence(self) -> MissionPersistenceStatus:
        """Async inspection of the optional restart-aware mission checkpoint."""

        return await asyncio.to_thread(self.client.mission_persistence)

    async def flush_mission_persistence(self) -> MissionPersistenceStatus:
        """Async forced checkpoint with typed bounded status."""

        return await asyncio.to_thread(self.client.flush_mission_persistence)

    async def mission_queue(self) -> MissionQueueInventory:
        """Async typed projection of the bounded mission queue inventory."""

        return await asyncio.to_thread(self.client.mission_queue)

    async def mission_queue_persistence(self) -> MissionQueueStatus:
        """Async inspection of queue checkpoint integrity and startup recoveries."""

        return await asyncio.to_thread(self.client.mission_queue_persistence)

    async def flush_mission_queue_persistence(self) -> MissionQueueFlushResult:
        """Async atomic queue checkpoint flush."""

        return await asyncio.to_thread(self.client.flush_mission_queue_persistence)

    async def release_mission_queue_lock(self, operator: str, reason: str) -> MissionQueueLockReleaseResult:
        """Async attributed release of an orphaned shared-authority lock."""

        return await asyncio.to_thread(self.client.release_mission_queue_lock, operator, reason)

    async def delivery_page(self, subscription_id: str, *, after: int = 0, limit: int = 100) -> DeliveryPage:
        """Async typed cursor page over pending signed deliveries."""

        return await asyncio.to_thread(self.client.delivery_page, subscription_id, after=after, limit=limit)

    async def delivery_attempts(
        self, subscription_id: str, *, after: int = 0, limit: int = 100
    ) -> DeliveryAttemptPage:
        """Async typed cursor page over durable delivery-attempt provenance."""

        return await asyncio.to_thread(
            self.client.delivery_attempts, subscription_id, after=after, limit=limit
        )

    async def rebind_subscription(self, subscription_id: str, secret: str) -> dict[str, Any]:
        """Async in-memory webhook secret rebind and pending-envelope re-sign."""

        return await asyncio.to_thread(self.client.rebind_subscription, subscription_id, secret)

    async def replay(self, subscription_id: str, delivery_ids: Sequence[int]) -> dict[str, Any]:
        """Async explicit bounded replay that resets selected delivery attempts."""

        return await asyncio.to_thread(self.client.replay, subscription_id, delivery_ids)

    async def mission_from_route(
        self,
        route: Mapping[str, Any],
        mission_id: str,
        selections: Sequence[MissionRouteSelection | Mapping[str, Any]],
        *,
        policy: MissionPolicy | Mapping[str, Any] | None = None,
    ) -> MissionAssembly:
        """Async local route-to-mission assembly; no HTTP request is issued."""

        return assemble_mission_from_route(route, mission_id, selections, policy=policy)

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
        """Async cross-domain delivery audit through the HTTP gateway."""

        arguments = _developer_delivery_arguments(
            request_id=request_id,
            targets=targets,
            checks={
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
            },
        )
        return await self.call_tool("developer_delivery_audit", arguments)

    async def developer_delivery_receipt(
        self,
        request: DeveloperDeliveryReceiptRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async content-addressed delivery receipt through the HTTP gateway."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptRequest) else DeveloperDeliveryReceiptRequest.from_wire(request)
        return await self.call_tool("developer_delivery_receipt", normalized.to_mcp_arguments())

    async def developer_delivery_receipt_verify(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async receipt verification through the HTTP gateway."""

        normalized = request if isinstance(request, DeveloperDeliveryReceiptVerificationRequest) else DeveloperDeliveryReceiptVerificationRequest.from_wire(request)
        return await self.call_tool("developer_delivery_receipt_verify", normalized.to_mcp_arguments())

    async def developer_platform_status(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async developer-platform contract through the HTTP gateway."""

        request = DeveloperPlatformStatusArgs(include_details, max_items)
        return await self.call_tool("developer_platform_status", request.to_mcp_arguments())

    async def developer_platform_status_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> DeveloperPlatformStatusReport:
        """Return async typed HTTP platform evidence."""

        return developer_platform_status_report(
            await self.developer_platform_status(include_details=include_details, max_items=max_items)
        )

    async def token_context_plan(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async token-context planning through the HTTP gateway."""

        normalized = request if isinstance(request, TokenContextPlanArgs) else TokenContextPlanArgs.from_wire(request)
        return await self.call_tool("token_context_plan", normalized.to_mcp_arguments())

    async def token_context_plan_report(
        self,
        request: TokenContextPlanArgs | Mapping[str, Any],
    ) -> TokenContextPlanningReport:
        """Return async typed HTTP token planning evidence."""

        return token_context_plan_report(await self.token_context_plan(request))

    async def weavelang_compile(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        """Async WeaveLang compilation through the HTTP gateway."""

        if isinstance(request, str):
            normalized = WeaveLangCompileArgs(request)
        elif isinstance(request, WeaveLangCompileArgs):
            normalized = request
        else:
            normalized = WeaveLangCompileArgs.from_wire(request)
        return await self.call_tool("weavelang_compile", normalized.to_mcp_arguments())

    async def weavelang_compile_report(
        self,
        request: WeaveLangCompileArgs | Mapping[str, Any] | str,
    ) -> WeaveLangCompileReport:
        """Return async typed HTTP WeaveLang compilation and replay evidence."""

        return weavelang_compile_report(await self.weavelang_compile(request))

    async def epistemic_voi(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP value-of-information pricing."""

        normalized = request if isinstance(request, EpistemicVoiArgs) else EpistemicVoiArgs.from_wire(request)
        return await self.call_tool("epistemic_voi", normalized.to_mcp_arguments())

    async def epistemic_voi_report(
        self,
        request: EpistemicVoiArgs | Mapping[str, Any],
    ) -> EpistemicVoiReport:
        """Return async typed HTTP value-of-information evidence."""

        return epistemic_voi_report(await self.epistemic_voi(request))

    async def epistemic_adaptive_acquisition(
        self,
        request: EpistemicAdaptiveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Plan a bounded exact adaptive acquisition policy without executing it."""

        normalized = request if isinstance(request, EpistemicAdaptiveArgs) else EpistemicAdaptiveArgs.from_wire(request)
        return await self.call_tool("epistemic_adaptive_acquisition", normalized.to_mcp_arguments())

    async def epistemic_adaptive_acquisition_report(
        self,
        request: EpistemicAdaptiveArgs | Mapping[str, Any],
    ) -> EpistemicAdaptiveReport:
        """Return a validated adaptive policy tree or fail-closed refusal."""

        return epistemic_adaptive_report(await self.epistemic_adaptive_acquisition(request))

    async def epistemic_adaptive_execute(
        self,
        request: AdaptiveExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP execution or receipt-only replay through the adaptive boundary."""

        normalized = request if isinstance(request, AdaptiveExecutionRequest) else AdaptiveExecutionRequest.from_wire(request)
        return await self.call_tool("epistemic_adaptive_execute", normalized.to_mcp_arguments())

    async def epistemic_adaptive_execute_report(
        self,
        request: AdaptiveExecutionRequest | Mapping[str, Any],
    ) -> AdaptiveExecutionReport:
        """Return typed async HTTP adaptive execution and provenance evidence."""

        return adaptive_execution_report(await self.epistemic_adaptive_execute(request))

    async def epistemic_adaptive_costed(
        self,
        request: AdaptiveCostedRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP vector-cost adaptive planning."""

        normalized = request if isinstance(request, AdaptiveCostedRequest) else AdaptiveCostedRequest.from_wire(request)
        return await self.call_tool("epistemic_adaptive_costed", normalized.to_mcp_arguments())

    async def epistemic_adaptive_costed_report(
        self,
        request: AdaptiveCostedRequest | Mapping[str, Any],
    ) -> AdaptiveCostedReport:
        """Return typed async HTTP vector-cost planning evidence."""

        return adaptive_costed_report(await self.epistemic_adaptive_costed(request))

    async def interweave_workflow_execute(
        self,
        request: WorkflowExecutionRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Simulate or replay a workflow-bound adaptive execution through async HTTP."""

        normalized = request if isinstance(request, WorkflowExecutionRequest) else WorkflowExecutionRequest.from_wire(request)
        return await self.call_tool("interweave_workflow_execute", normalized.to_mcp_arguments())

    async def interweave_workflow_execute_report(
        self,
        request: WorkflowExecutionRequest | Mapping[str, Any],
    ) -> WorkflowExecutionReport:
        """Return typed async HTTP workflow execution and provenance evidence."""

        return workflow_execution_report(await self.interweave_workflow_execute(request))

    async def interweave_workflow_execution_evidence(
        self,
        request: WorkflowExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP receipt-to-evidence conversion."""

        normalized = request if isinstance(request, WorkflowExecutionEvidenceRequest) else WorkflowExecutionEvidenceRequest.from_wire(request)
        return await self.call_tool(
            "interweave_workflow_execution_evidence", normalized.to_mcp_arguments()
        )

    async def interweave_workflow_execution_evidence_report(
        self,
        request: WorkflowExecutionEvidenceRequest | Mapping[str, Any],
    ) -> WorkflowExecutionEvidenceReport:
        """Return typed async HTTP workflow execution evidence."""

        return workflow_execution_evidence_report(
            await self.interweave_workflow_execution_evidence(request)
        )

    async def interweave_workflow_execution_evidence_import(
        self,
        evidence: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP import of a portable workflow execution evidence record."""

        return await self.call_tool(
            "interweave_workflow_execution_evidence_import", {"evidence": dict(evidence)}
        )

    async def interweave_workflow_execution_evidence_query(
        self,
        filters: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async bounded HTTP query over workflow execution evidence."""

        return await self.call_tool(
            "interweave_workflow_execution_evidence_query", dict(filters or {})
        )

    async def interweave_workflow_execution_evidence_get(
        self, evidence_digest: str
    ) -> dict[str, Any]:
        """Async HTTP fetch of one retained workflow execution evidence record."""

        return await self.call_tool(
            "interweave_workflow_execution_evidence_get", {"evidence_digest": evidence_digest}
        )

    async def epistemic_decision_quotient(
        self,
        request: EpistemicDecisionQuotientArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Compute a decision-relative model quotient through async HTTP."""

        normalized = request if isinstance(request, EpistemicDecisionQuotientArgs) else EpistemicDecisionQuotientArgs.from_wire(request)
        return await self.call_tool("epistemic_decision_quotient", normalized.to_mcp_arguments())

    async def epistemic_decision_quotient_report(
        self,
        request: EpistemicDecisionQuotientArgs | Mapping[str, Any],
    ) -> EpistemicDecisionQuotientReport:
        return epistemic_decision_quotient_report(await self.epistemic_decision_quotient(request))

    async def epistemic_context_audit(
        self,
        request: EpistemicContextAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit decision-relative context compression through async HTTP."""

        normalized = request if isinstance(request, EpistemicContextAuditArgs) else EpistemicContextAuditArgs.from_wire(request)
        return await self.call_tool("epistemic_context_audit", normalized.to_mcp_arguments())

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
        """Async bounded observed-evidence selection through HTTP."""

        normalized = request if isinstance(request, EpistemicSelectionAuditArgs) else EpistemicSelectionAuditArgs.from_wire(request)
        return await self.call_tool("epistemic_selection_audit", normalized.to_mcp_arguments())

    async def epistemic_selection_audit_report(
        self,
        request: EpistemicSelectionAuditArgs | Mapping[str, Any],
    ) -> EpistemicSelectionAuditReport:
        """Return async typed HTTP selection and guarantee evidence."""

        return epistemic_selection_audit_report(await self.epistemic_selection_audit(request))

    async def benchmark_trace_analyze(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP benchmark trace analysis."""

        normalized = request if isinstance(request, BenchmarkTraceAnalyzeArgs) else BenchmarkTraceAnalyzeArgs.from_wire(request)
        return await self.call_tool("benchmark_trace_analyze", normalized.to_mcp_arguments())

    async def benchmark_trace_analysis_report(
        self,
        request: BenchmarkTraceAnalyzeArgs | Mapping[str, Any],
    ) -> BenchmarkTraceAnalysisReport:
        """Return async typed HTTP benchmark compiler evidence."""

        return benchmark_trace_analysis_report(await self.benchmark_trace_analyze(request))

    async def benchmark_decision_audit(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit one reconstructed decision through the async HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkDecisionAuditArgs) else BenchmarkDecisionAuditArgs.from_wire(request)
        return await self.call_tool("benchmark_decision_audit", normalized.to_mcp_arguments())

    async def benchmark_decision_audit_report(
        self,
        request: BenchmarkDecisionAuditArgs | Mapping[str, Any],
    ) -> BenchmarkDecisionAuditReport:
        """Return typed async HTTP decision-cell evidence."""

        return benchmark_decision_audit_report(await self.benchmark_decision_audit(request))

    async def benchmark_integrity_audit(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Audit benchmark portfolio integrity through the async HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkIntegrityAuditArgs) else BenchmarkIntegrityAuditArgs.from_wire(request)
        return await self.call_tool("benchmark_integrity_audit", normalized.to_mcp_arguments())

    async def benchmark_integrity_audit_report(
        self,
        request: BenchmarkIntegrityAuditArgs | Mapping[str, Any],
    ) -> BenchmarkIntegrityAuditReport:
        """Return typed async HTTP benchmark integrity evidence."""

        return benchmark_integrity_audit_report(await self.benchmark_integrity_audit(request))

    async def benchmark_counterfactual_check(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Validate and contrast matched DecisionCells through the async HTTP gateway."""

        normalized = request if isinstance(request, BenchmarkCounterfactualCheckArgs) else BenchmarkCounterfactualCheckArgs.from_wire(request)
        return await self.call_tool("benchmark_counterfactual_check", normalized.to_mcp_arguments())

    async def benchmark_counterfactual_check_report(
        self,
        request: BenchmarkCounterfactualCheckArgs | Mapping[str, Any],
    ) -> BenchmarkCounterfactualCheckReport:
        """Return typed async HTTP counterfactual evidence."""

        return benchmark_counterfactual_check_report(await self.benchmark_counterfactual_check(request))

    async def benchmark_oracle_review(
        self,
        request: BenchmarkOracleReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Review, grade, and optionally package a benchmark oracle through async HTTP."""

        normalized = request if isinstance(request, BenchmarkOracleReviewArgs) else BenchmarkOracleReviewArgs.from_wire(request)
        return await self.call_tool("benchmark_oracle_review", normalized.to_mcp_arguments())

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
        """Run the non-executing assembled benchmark compiler through async HTTP."""

        normalized = request if isinstance(request, BenchmarkCompileArgs) else BenchmarkCompileArgs.from_wire(request)
        return await self.call_tool("benchmark_compile", normalized.to_mcp_arguments())

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
        """Run compilation, review, optional grading, and packaging through async HTTP."""

        normalized = request if isinstance(request, BenchmarkCompileReviewArgs) else BenchmarkCompileReviewArgs.from_wire(request)
        return await self.call_tool("benchmark_compile_review", normalized.to_mcp_arguments())

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
        """Async HTTP foundation contract validation."""

        normalized = request if isinstance(request, FoundationContractCheckArgs) else FoundationContractCheckArgs.from_wire(request)
        return await self.call_tool("foundation_contract_check", normalized.to_mcp_arguments())

    async def foundation_contract_check_report(
        self,
        request: FoundationContractCheckArgs | Mapping[str, Any],
    ) -> FoundationContractCheckReport:
        """Return async typed HTTP foundation gate evidence."""

        return foundation_contract_check_report(await self.foundation_contract_check(request))

    async def pack_catalogue(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async bounded pack portfolio declarations through HTTP."""

        if request is not None:
            if section != "all" or max_items != 100:
                raise ArgumentError("request cannot be combined with section or max_items")
            normalized = request if isinstance(request, PackCatalogueArgs) else PackCatalogueArgs.from_wire(request)
        else:
            normalized = PackCatalogueArgs(section, max_items)
        return await self.call_tool("pack_catalogue", normalized.to_mcp_arguments())

    async def pack_catalogue_report(
        self,
        request: PackCatalogueArgs | Mapping[str, Any] | None = None,
        *,
        section: str = "all",
        max_items: int = 100,
    ) -> PackCatalogueReport:
        """Return async typed HTTP pack portfolio declarations."""

        return pack_catalogue_report(await self.pack_catalogue(request, section=section, max_items=max_items))

    async def pack_coverage_audit(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit benchmark-pack coverage through async HTTP."""

        normalized = request if isinstance(request, PackCoverageAuditArgs) else PackCoverageAuditArgs.from_wire(request or {})
        return await self.call_tool("pack_coverage_audit", normalized.to_mcp_arguments())

    async def pack_coverage_audit_report(
        self,
        request: PackCoverageAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackCoverageAuditReport:
        """Return typed async benchmark-pack coverage evidence."""

        return pack_coverage_audit_report(await self.pack_coverage_audit(request))

    async def pack_release_audit(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Audit pack release sequencing through async HTTP."""

        normalized = request if isinstance(request, PackReleaseAuditArgs) else PackReleaseAuditArgs.from_wire(request or {})
        return await self.call_tool("pack_release_audit", normalized.to_mcp_arguments())

    async def pack_release_audit_report(
        self,
        request: PackReleaseAuditArgs | Mapping[str, Any] | None = None,
    ) -> PackReleaseAuditReport:
        """Return typed async release-order evidence."""

        return pack_release_audit_report(await self.pack_release_audit(request))

    async def pack_health_assess(
        self,
        request: PackHealthAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async observed pack-health gate through the HTTP gateway."""

        normalized = request if isinstance(request, PackHealthAssessArgs) else PackHealthAssessArgs.from_wire(request)
        return await self.call_tool("pack_health_assess", normalized.to_mcp_arguments())

    async def pack_health_assess_report(
        self,
        request: PackHealthAssessArgs | Mapping[str, Any],
    ) -> PackHealthAssessmentReport:
        """Return async typed pack-health evidence."""

        return pack_health_assessment_report(await self.pack_health_assess(request))

    async def security_redteam_simulate(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async section-13 safety workflow through the HTTP gateway."""

        normalized = SecurityRedteamSimulateArgs() if request is None else request if isinstance(request, SecurityRedteamSimulateArgs) else SecurityRedteamSimulateArgs.from_wire(request)
        return await self.call_tool("security_redteam_simulate", normalized.to_mcp_arguments())

    async def security_redteam_simulate_report(
        self,
        request: SecurityRedteamSimulateArgs | Mapping[str, Any] | None = None,
    ) -> SecurityRedteamReport:
        """Return async typed section-13 safety evidence."""

        return security_redteam_simulate_report(await self.security_redteam_simulate(request))

    async def world_generate(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async deterministic world generation through the HTTP gateway."""

        normalized = request if isinstance(request, WorldGenerateArgs) else WorldGenerateArgs.from_wire(request)
        return await self.call_tool("world_generate", normalized.to_mcp_arguments())

    async def world_generate_report(
        self,
        request: WorldGenerateArgs | Mapping[str, Any],
    ) -> WorldGenerateReport:
        """Return async typed world-generation evidence."""

        return world_generate_report(await self.world_generate(request))

    async def factory_lifecycle_simulate(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async factory lifecycle replay through the HTTP gateway."""

        normalized = request if isinstance(request, FactoryLifecycleSimulateArgs) else FactoryLifecycleSimulateArgs.from_wire(request)
        return await self.call_tool("factory_lifecycle_simulate", normalized.to_mcp_arguments())

    async def factory_lifecycle_simulate_report(
        self,
        request: FactoryLifecycleSimulateArgs | Mapping[str, Any],
    ) -> FactoryLifecycleReport:
        """Return async typed factory lifecycle evidence."""

        return factory_lifecycle_report(await self.factory_lifecycle_simulate(request))

    async def storage_lifecycle_simulate(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async storage lifecycle accounting through the HTTP gateway."""

        normalized = request if isinstance(request, StorageLifecycleSimulateArgs) else StorageLifecycleSimulateArgs.from_wire(request)
        return await self.call_tool("storage_lifecycle_simulate", normalized.to_mcp_arguments())

    async def storage_lifecycle_simulate_report(
        self,
        request: StorageLifecycleSimulateArgs | Mapping[str, Any],
    ) -> StorageLifecycleReport:
        """Return async typed storage lifecycle evidence."""

        return storage_lifecycle_report(await self.storage_lifecycle_simulate(request))

    async def registry_lifecycle_simulate(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async registry lifecycle replay through the HTTP gateway."""

        normalized = RegistryLifecycleSimulateArgs() if request is None else request if isinstance(request, RegistryLifecycleSimulateArgs) else RegistryLifecycleSimulateArgs.from_wire(request)
        return await self.call_tool("registry_lifecycle_simulate", normalized.to_mcp_arguments())

    async def registry_lifecycle_simulate_report(
        self,
        request: RegistryLifecycleSimulateArgs | Mapping[str, Any] | None = None,
    ) -> RegistryLifecycleReport:
        """Return async typed registry lifecycle evidence."""

        return registry_lifecycle_report(await self.registry_lifecycle_simulate(request))

    async def cache_invalidation_simulate(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async cache invalidation planning through the HTTP gateway."""

        normalized = request if isinstance(request, CacheInvalidationSimulateArgs) else CacheInvalidationSimulateArgs.from_wire(request)
        return await self.call_tool("cache_invalidation_simulate", normalized.to_mcp_arguments())

    async def cache_invalidation_simulate_report(
        self,
        request: CacheInvalidationSimulateArgs | Mapping[str, Any],
    ) -> CacheInvalidationReport:
        """Return async typed cache invalidation evidence."""

        return cache_invalidation_report(await self.cache_invalidation_simulate(request))

    async def hub_disclosure_review(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async public-hub disclosure replay through the HTTP gateway."""

        normalized = request if isinstance(request, HubDisclosureReviewArgs) else HubDisclosureReviewArgs.from_wire(request)
        return await self.call_tool("hub_disclosure_review", normalized.to_mcp_arguments())

    async def hub_disclosure_review_report(
        self,
        request: HubDisclosureReviewArgs | Mapping[str, Any],
    ) -> HubDisclosureReviewReport:
        """Return async typed disclosure evidence through the HTTP gateway."""

        return hub_disclosure_review(await self.hub_disclosure_review(request))

    async def hub_card_render(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async public-hub card rendering through the HTTP gateway."""

        normalized = request if isinstance(request, HubCardRenderArgs) else HubCardRenderArgs.from_wire(request)
        return await self.call_tool("hub_card_render", normalized.to_mcp_arguments())

    async def hub_card_render_report(
        self,
        request: HubCardRenderArgs | Mapping[str, Any],
    ) -> HubCardRenderReport:
        """Return async typed card evidence through the HTTP gateway."""

        return hub_card_render(await self.hub_card_render(request))

    async def hub_leaderboard_render(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async public-hub leaderboard rendering through the HTTP gateway."""

        normalized = request if isinstance(request, HubLeaderboardRenderArgs) else HubLeaderboardRenderArgs.from_wire(request)
        return await self.call_tool("hub_leaderboard_render", normalized.to_mcp_arguments())

    async def hub_leaderboard_render_report(
        self,
        request: HubLeaderboardRenderArgs | Mapping[str, Any],
    ) -> HubLeaderboardRenderReport:
        """Return async typed leaderboard evidence through the HTTP gateway."""

        return hub_leaderboard_render(await self.hub_leaderboard_render(request))

    async def hub_submission_review(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async submission and moderation replay through HTTP."""

        normalized = request if isinstance(request, HubSubmissionReviewArgs) else HubSubmissionReviewArgs.from_wire(request)
        return await self.call_tool("hub_submission_review", normalized.to_mcp_arguments())

    async def hub_submission_review_report(
        self,
        request: HubSubmissionReviewArgs | Mapping[str, Any],
    ) -> HubSubmissionReviewReport:
        """Return async typed submission evidence through HTTP."""

        return hub_submission_review(await self.hub_submission_review(request))

    async def bioatlas_publication_audit(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async composed BioAtlas publication audit through the HTTP gateway."""

        normalized = request if isinstance(request, BioAtlasPublicationAuditArgs) else BioAtlasPublicationAuditArgs.from_wire(request)
        return await self.call_tool("bioatlas_publication_audit", normalized.to_mcp_arguments())

    async def bioatlas_publication_audit_report(
        self,
        request: BioAtlasPublicationAuditArgs | Mapping[str, Any],
    ) -> BioAtlasPublicationAuditReport:
        """Return async typed composed publication evidence through the HTTP gateway."""

        return bioatlas_publication_audit(await self.bioatlas_publication_audit(request))

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
        """Async typed delivery-readiness evidence from the HTTP gateway."""

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
        """Return async typed HTTP delivery receipt digests and readiness."""

        return developer_delivery_receipt_report(await self.developer_delivery_receipt(request))

    async def developer_delivery_receipt_verification_report(
        self,
        request: DeveloperDeliveryReceiptVerificationRequest | Mapping[str, Any],
    ) -> DeveloperDeliveryReceiptVerificationReport:
        """Return async typed HTTP receipt verification evidence."""

        return developer_delivery_receipt_verification_report(
            await self.developer_delivery_receipt_verify(request)
        )

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
        arguments = _capability_query_arguments(
            query,
            text=text,
            domain=domain,
            tool=tool,
            group_id=group_id,
            max_items=max_items,
            include_tools=include_tools,
        )
        return await self.call_tool("capability_discover", arguments)

    async def mission_evaluator_discover(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async discovery of explicit non-executing evaluator candidates through HTTP."""

        normalized = request if isinstance(request, MissionEvaluatorQuery) else MissionEvaluatorQuery(**dict(request or {}))
        return await self.call_tool("mission_evaluator_discover", normalized.to_mcp_arguments())

    async def mission_evaluator_discover_report(
        self,
        request: MissionEvaluatorQuery | Mapping[str, Any] | None = None,
    ) -> MissionEvaluatorSearchReport:
        """Return typed async evaluator candidate evidence through HTTP."""

        return mission_evaluator_discover_report(await self.mission_evaluator_discover(request))

    async def mission_evaluator_review(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async review of explicit evaluator-to-claim bindings through HTTP."""

        normalized = request if isinstance(request, MissionEvaluatorReviewRequest) else MissionEvaluatorReviewRequest(**dict(request))
        return await self.call_tool("mission_evaluator_review", normalized.to_mcp_arguments())

    async def mission_evaluator_review_report(
        self,
        request: MissionEvaluatorReviewRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReviewReport:
        """Return typed async evaluator binding review evidence through HTTP."""

        return mission_evaluator_review_report(await self.mission_evaluator_review(request))

    async def mission_evaluator_replay(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async replay of retained mission evaluator lineage without dispatch."""

        normalized = request if isinstance(request, MissionEvaluatorReplayRequest) else MissionEvaluatorReplayRequest(**dict(request))
        return await self.call_tool("mission_evaluator_replay", normalized.to_mcp_arguments())

    async def mission_evaluator_replay_report(
        self,
        request: MissionEvaluatorReplayRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayReport:
        """Return typed async evaluator replay and fixture evidence through HTTP."""

        return mission_evaluator_replay_report(await self.mission_evaluator_replay(request))

    async def mission_evaluator_replay_compare(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async MCP replay comparison."""

        return await asyncio.to_thread(self.client.mission_evaluator_replay_compare, request)

    async def mission_evaluator_replay_compare_report(
        self,
        request: MissionEvaluatorReplayCompareRequest | Mapping[str, Any],
    ) -> MissionEvaluatorReplayComparisonReport:
        """Return typed async MCP catalog-drift comparison evidence."""

        return mission_evaluator_replay_comparison_report(
            await self.mission_evaluator_replay_compare(request)
        )

    async def mission_evidence_bundle_verify_tool(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async MCP mission evidence bundle verification."""

        normalized = (
            request
            if isinstance(request, MissionEvidenceBundleVerifyRequest)
            else MissionEvidenceBundleVerifyRequest(**dict(request))
        )
        return await self.call_tool("mission_evidence_bundle_verify", normalized.to_mcp_arguments())

    async def mission_evidence_bundle_verification_tool_report(
        self,
        request: MissionEvidenceBundleVerifyRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleVerificationReport:
        """Return typed async MCP mission evidence verification evidence."""

        return mission_evidence_bundle_verification_report(
            await self.mission_evidence_bundle_verify_tool(request)
        )

    async def mission_evidence_bundle_import_tool(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_import_tool, request)

    async def mission_evidence_bundle_import_tool_report(
        self,
        request: MissionEvidenceBundleImportRequest | Mapping[str, Any],
    ) -> MissionEvidenceBundleImportReport:
        return MissionEvidenceBundleImportReport.from_wire(
            await self.mission_evidence_bundle_import_tool(request)
        )

    async def mission_evidence_bundle_query_tool(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_query_tool, request)

    async def mission_evidence_bundle_query_tool_report(
        self,
        request: MissionEvidenceBundleQueryRequest | Mapping[str, Any] | None = None,
    ) -> MissionEvidenceBundleQueryReport:
        return MissionEvidenceBundleQueryReport.from_wire(
            await self.mission_evidence_bundle_query_tool(request)
        )

    async def mission_evidence_bundle_get_tool(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(self.client.mission_evidence_bundle_get_tool, request)

    async def mission_evidence_bundle_get_tool_report(
        self,
        request: MissionEvidenceBundleGetRequest | Mapping[str, Any] | str,
    ) -> MissionEvidenceBundleGetReport:
        return MissionEvidenceBundleGetReport.from_wire(
            await self.mission_evidence_bundle_get_tool(request)
        )

    async def capability_audit(self, *, include_groups: bool = True) -> dict[str, Any]:
        if not isinstance(include_groups, bool):
            raise ArgumentError("include_groups must be a boolean")
        return await self.call_tool("capability_audit", {"include_groups": include_groups})

    async def capability_audit_report(self, *, include_groups: bool = True) -> CapabilityAuditReport:
        """Async typed capability parity and schema-quality diagnostics over HTTP."""

        return capability_audit_report(
            await self.capability_audit(include_groups=include_groups)
        )

    async def capability_dashboard(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Async bounded cross-domain surface dashboard through HTTP."""

        normalized = request if isinstance(request, CapabilityDashboardQueryArgs) else CapabilityDashboardQueryArgs(**dict(request or {}))
        return await self.call_tool("capability_dashboard", normalized.to_mcp_arguments())

    async def capability_dashboard_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return async typed capability dashboard evidence through HTTP."""

        return capability_dashboard_report(await self.capability_dashboard(request))

    async def capability_dashboard_rest(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> dict[str, Any]:
        """Async direct REST query for the bounded capability dashboard."""

        return await asyncio.to_thread(self.client.capability_dashboard_rest, request)

    async def capability_dashboard_rest_report(self, request: CapabilityDashboardQueryArgs | Mapping[str, Any] | None = None) -> CapabilityDashboardReport:
        """Return a typed report from the async direct capability dashboard route."""

        return capability_dashboard_report(await self.capability_dashboard_rest(request))

    async def ci_execution_evidence_audit(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.ci_execution_evidence_audit`."""

        normalized = request if isinstance(request, CiExecutionEvidenceRequest) else CiExecutionEvidenceRequest(**dict(request))
        return await self.call_tool("ci_execution_evidence_audit", normalized.to_mcp_arguments())

    async def ci_execution_evidence_report(
        self,
        request: CiExecutionEvidenceRequest | Mapping[str, Any],
    ) -> CiExecutionEvidenceReport:
        """Return typed structural CI evidence through async HTTP."""

        return ci_execution_evidence_report(await self.ci_execution_evidence_audit(request))

    async def ci_provider_normalize(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.ci_provider_normalize`."""

        normalized = request if isinstance(request, CiProviderNormalizationRequest) else CiProviderNormalizationRequest(**dict(request))
        return await self.call_tool("ci_provider_normalize", normalized.to_mcp_arguments())

    async def ci_provider_normalization_report(
        self,
        request: CiProviderNormalizationRequest | Mapping[str, Any],
    ) -> CiProviderNormalizationReport:
        """Return typed provider-normalization evidence through async HTTP."""

        return ci_provider_normalization_report(await self.ci_provider_normalize(request))

    async def ci_provider_evidence_audit(
        self,
        request: CiProviderEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async provider-bound artifact, log, and attestation conformance audit."""

        normalized = request if isinstance(request, CiProviderEvidenceRequest) else CiProviderEvidenceRequest(**dict(request))
        return await self.call_tool("ci_provider_evidence_audit", normalized.to_mcp_arguments())

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
        """Async counterpart to :meth:`ApiClient.execution_provenance_audit`."""

        normalized = request if isinstance(request, ExecutionProvenanceRequest) else ExecutionProvenanceRequest(**dict(request))
        return await self.call_tool("execution_provenance_audit", normalized.to_mcp_arguments())

    async def execution_provenance_report(
        self,
        request: ExecutionProvenanceRequest | Mapping[str, Any],
    ) -> ExecutionProvenanceReport:
        """Return typed async mission/delegated-check provenance through HTTP."""

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
        """Async typed ranked projection from the HTTP capability catalogue."""

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
        request = CapabilityRouteRequest(
            goal,
            needs,
            max_candidates_per_need,
            max_tools,
            include_tools,
        )
        return await self.call_tool("capability_route", request.to_mcp_arguments())

    async def capability_route_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Async counterpart to :meth:`ApiClient.capability_route_report`."""

        return capability_route_report(
            await self.capability_route(
                goal,
                needs,
                max_candidates_per_need=max_candidates_per_need,
                max_tools=max_tools,
                include_tools=include_tools,
            )
        )

    async def capability_route_rest(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> dict[str, Any]:
        """Async direct REST route proposal without an MCP envelope."""

        return await asyncio.to_thread(
            self.client.capability_route_rest,
            goal,
            needs,
            max_candidates_per_need=max_candidates_per_need,
            max_tools=max_tools,
            include_tools=include_tools,
        )

    async def capability_route_rest_report(
        self,
        goal: str,
        needs: Sequence[CapabilityRouteNeed | Mapping[str, Any]],
        *,
        max_candidates_per_need: int = 10,
        max_tools: int = 128,
        include_tools: bool = False,
    ) -> CapabilityRouteReport:
        """Return typed async diagnostics from the direct route REST endpoint."""

        return capability_route_report(
            await self.capability_route_rest(
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
        """Async counterpart to :meth:`ApiClient.capability_route_review`."""

        request = CapabilityRouteReviewRequest(route, selections, validate_schemas)
        return await self.call_tool("capability_route_review", request.to_mcp_arguments())

    async def capability_route_review_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Async typed HTTP diagnostics for a route-to-mission handoff review."""

        return capability_route_review_report(
            await self.capability_route_review(route, selections, validate_schemas=validate_schemas)
        )

    async def capability_route_review_rest(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> dict[str, Any]:
        """Async route review through the dedicated raw REST endpoint."""

        return await asyncio.to_thread(
            self.client.capability_route_review_rest,
            route,
            selections,
            validate_schemas=validate_schemas,
        )

    async def capability_route_review_rest_report(
        self,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = False,
    ) -> CapabilityRouteReviewReport:
        """Return typed async route-review diagnostics from direct REST."""

        return capability_route_review_report(
            await self.capability_route_review_rest(
                route,
                selections,
                validate_schemas=validate_schemas,
            )
        )

    async def capability_route_plan(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.capability_route_plan`."""

        request = CapabilityRoutePlanRequest(
            mission_id,
            route,
            selections,
            validate_schemas,
            policy,
            claim_requests,
            evaluator_review,
            workflow_binding,
        )
        return await self.call_tool("capability_route_plan", request.to_mcp_arguments())

    async def capability_route_plan_report(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> CapabilityRoutePlanReport:
        """Return typed async route-plan diagnostics."""

        return capability_route_plan_report(
            await self.capability_route_plan(
                mission_id,
                route,
                selections,
                validate_schemas=validate_schemas,
                policy=policy,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
            )
        )

    async def capability_route_plan_rest(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async direct REST route-to-plan composition."""

        return await asyncio.to_thread(
            self.client.capability_route_plan_rest,
            mission_id,
            route,
            selections,
            validate_schemas=validate_schemas,
            policy=policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
        )

    async def capability_route_plan_rest_report(
        self,
        mission_id: str,
        route: Mapping[str, Any],
        selections: Sequence[Mapping[str, Any]],
        *,
        validate_schemas: bool = True,
        policy: Mapping[str, Any] | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
    ) -> CapabilityRoutePlanReport:
        """Return typed async route-plan diagnostics from direct REST."""

        return capability_route_plan_report(
            await self.capability_route_plan_rest(
                mission_id,
                route,
                selections,
                validate_schemas=validate_schemas,
                policy=policy,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
            )
        )

    async def capability_route_plan_verify(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async MCP verification of a retained route plan."""

        normalized = (
            request
            if isinstance(request, CapabilityRoutePlanVerifyRequest)
            else CapabilityRoutePlanVerifyRequest(**dict(request))
        )
        return await self.call_tool("capability_route_plan_verify", normalized.to_mcp_arguments())

    async def capability_route_plan_verify_report(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> CapabilityRoutePlanVerifyReport:
        return capability_route_plan_verify_report(await self.capability_route_plan_verify(request))

    async def capability_route_plan_verify_rest(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async REST verification of a retained route plan."""

        normalized = (
            request
            if isinstance(request, CapabilityRoutePlanVerifyRequest)
            else CapabilityRoutePlanVerifyRequest(**dict(request))
        )
        return await asyncio.to_thread(
            self.client.capability_route_plan_verify_rest,
            normalized,
        )

    async def capability_route_plan_verify_rest_report(
        self,
        request: CapabilityRoutePlanVerifyRequest | Mapping[str, Any],
    ) -> CapabilityRoutePlanVerifyReport:
        return capability_route_plan_verify_report(await self.capability_route_plan_verify_rest(request))

    async def adapter_plan(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.adapter_plan`."""

        request = AdapterPlanRequest(
            source_id,
            source_kind,
            declared_format,
            required_conformance,
            available_dependencies,
        )
        return await self.call_tool("adapter_plan", request.to_mcp_arguments())

    async def adapter_plan_report(
        self,
        source_id: str,
        source_kind: str,
        *,
        declared_format: str | None = None,
        required_conformance: str | None = None,
        available_dependencies: Sequence[str] | None = None,
    ) -> AdapterPlanReport:
        """Async typed adapter planning evidence from the HTTP gateway."""

        return adapter_plan_report(
            await self.adapter_plan(
                source_id,
                source_kind,
                declared_format=declared_format,
                required_conformance=required_conformance,
                available_dependencies=available_dependencies,
            )
        )

    async def adapter_execution_evidence(
        self,
        request: AdapterExecutionEvidenceRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.adapter_execution_evidence`."""

        normalized = request if isinstance(request, AdapterExecutionEvidenceRequest) else AdapterExecutionEvidenceRequest.from_wire(request)
        return await self.call_tool("adapter_execution_evidence", normalized.to_mcp_arguments())

    async def adapter_execution_evidence_report(
        self,
        request: AdapterExecutionEvidenceRequest | Mapping[str, Any],
    ) -> AdapterExecutionEvidenceReport:
        return adapter_execution_evidence_report(await self.adapter_execution_evidence(request))

    async def adapter_execution_evidence_query(
        self,
        request: AdapterExecutionEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.adapter_execution_evidence_query`."""

        normalized = request or AdapterExecutionEvidenceQueryRequest()
        if not isinstance(normalized, AdapterExecutionEvidenceQueryRequest):
            normalized = AdapterExecutionEvidenceQueryRequest.from_wire(request or {})
        return await self.call_tool("adapter_execution_evidence_query", normalized.to_mcp_arguments())

    async def adapter_execution_evidence_query_report(
        self,
        request: AdapterExecutionEvidenceQueryRequest | Mapping[str, Any] | None = None,
    ) -> AdapterExecutionEvidenceQueryReport:
        return adapter_execution_evidence_query_report(await self.adapter_execution_evidence_query(request))

    async def domain_acquisition_catalogue(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.domain_acquisition_catalogue`."""

        normalized = query or DomainAcquisitionQuery()
        if not isinstance(normalized, DomainAcquisitionQuery):
            raise TypeError("query must be a DomainAcquisitionQuery")
        return await self.call_tool(DOMAIN_ACQUISITION_WORKFLOW, normalized.to_mcp_arguments())

    async def domain_acquisition_catalogue_report(
        self,
        query: DomainAcquisitionQuery | None = None,
    ) -> DomainAcquisitionReport:
        """Return typed async acquisition routes and adapter boundaries."""

        return domain_acquisition_report(await self.domain_acquisition_catalogue(query))

    async def tabular_ingest(self, request: TabularIngestRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.tabular_ingest`."""

        if not isinstance(request, TabularIngestRequest):
            raise ArgumentError("request must be a TabularIngestRequest")
        return await self.call_tool("tabular_ingest", request.to_mcp_arguments())

    async def tabular_ingest_report(self, request: TabularIngestRequest) -> TabularIngestReport:
        """Return typed async HTTP tabular conformance and loss evidence."""

        return tabular_ingest_report(await self.tabular_ingest(request))

    async def conformance_run(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.conformance_run`."""

        if not isinstance(include_details, bool):
            raise ArgumentError("include_details must be a boolean")
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= 1_000:
            raise ArgumentError("max_items must be between 1 and 1000")
        return await self.call_tool("conformance_run", {"include_details": include_details, "max_items": max_items})

    async def conformance_run_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> ConformanceRunReport:
        """Return typed async HTTP conformance and release evidence."""

        return conformance_run_report(
            await self.conformance_run(include_details=include_details, max_items=max_items)
        )

    async def release_audit(self, request: ReleaseAuditArgs) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.release_audit`."""

        if not isinstance(request, ReleaseAuditArgs):
            raise ArgumentError("request must be a ReleaseAuditArgs")
        return await self.call_tool("release_audit", request.to_mcp_arguments())

    async def release_audit_report(self, request: ReleaseAuditArgs) -> ReleaseAuditReport:
        """Return typed async HTTP release gates and delegated evidence."""

        return release_audit_report(await self.release_audit(request))

    async def operations_catalog(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.operations_catalog`."""

        request = OperationsCatalogArgs(include_details, max_items)
        return await self.call_tool("operations_catalog", request.to_mcp_arguments())

    async def operations_catalog_report(
        self,
        *,
        include_details: bool = False,
        max_items: int = 100,
    ) -> OperationsCatalogReport:
        """Return typed async HTTP operations evidence."""

        return operations_catalog_report(
            await self.operations_catalog(include_details=include_details, max_items=max_items)
        )

    async def ops_acceptance(self, *, max_items: int = 100) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.ops_acceptance`."""

        request = OpsAcceptanceArgs(max_items)
        return await self.call_tool("ops_acceptance", request.to_mcp_arguments())

    async def ops_acceptance_report(self, *, max_items: int = 100) -> OpsAcceptanceReport:
        """Return typed async acceptance evidence and decidability state."""

        return ops_acceptance_report(await self.ops_acceptance(max_items=max_items))

    async def safety_release_gate(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.safety_release_gate`."""

        if isinstance(assessment, SafetyReleaseGateArgs):
            request = assessment
        else:
            request = SafetyReleaseGateArgs(
                assessment if isinstance(assessment, RiskAssessmentRequest) else RiskAssessmentRequest.from_wire(assessment)
            )
        return await self.call_tool("safety_release_gate", request.to_mcp_arguments())

    async def safety_release_gate_report(
        self,
        assessment: SafetyReleaseGateArgs | RiskAssessmentRequest | Mapping[str, Any],
    ) -> SafetyReleaseGateReport:
        """Return typed async HTTP fail-closed safety-gate evidence."""

        return safety_release_gate_report(await self.safety_release_gate(assessment))

    async def medical_boundary_check(self, request: MedicalBoundaryRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.medical_boundary_check`."""

        if not isinstance(request, MedicalBoundaryRequest):
            raise ArgumentError("request must be a MedicalBoundaryRequest")
        return await self.call_tool("medical_boundary_check", request.to_mcp_arguments())

    async def medical_boundary_report(self, request: MedicalBoundaryRequest) -> MedicalBoundaryReport:
        """Return typed async HTTP medical boundary evidence."""

        return medical_boundary_report(await self.medical_boundary_check(request))

    async def safety_posture(self, *, include_threats: bool = False) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.safety_posture`."""

        request = SafetyPostureArgs(include_threats)
        return await self.call_tool("safety_posture", request.to_mcp_arguments())

    async def safety_posture_report(self, *, include_threats: bool = False) -> SafetyPostureReport:
        """Return typed async HTTP section-13 posture evidence."""

        return safety_posture_report(await self.safety_posture(include_threats=include_threats))

    async def measurement_compare(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.measurement_compare`."""

        normalized = request if isinstance(request, MeasurementCompareArgs) else MeasurementCompareArgs.from_wire(request)
        return await self.call_tool("measurement_compare", normalized.to_mcp_arguments())

    async def measurement_compare_report(
        self,
        request: MeasurementCompareArgs | Mapping[str, Any],
    ) -> MeasurementCompareReport:
        """Return typed async HTTP measurement-comparability evidence."""

        return measurement_compare_report(await self.measurement_compare(request))

    async def literature_bind_check(
        self,
        request: LiteratureBindCheckArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        normalized = request if isinstance(request, LiteratureBindCheckArgs) else LiteratureBindCheckArgs.from_wire(request)
        return await self.call_tool("literature_bind_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("modality_support_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("modality_transport_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("modality_comparability_check", normalized.to_mcp_arguments())

    async def modality_comparability_check_report(
        self,
        request: ModalityComparabilityCheckArgs | Mapping[str, Any],
    ) -> ModalityComparabilityCheckReport:
        return modality_comparability_check_report(await self.modality_comparability_check(request))

    async def hub_search(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.hub_search`."""

        normalized = request if isinstance(request, HubSearchArgs) else HubSearchArgs.from_wire(request)
        return await self.call_tool("hub_search", normalized.to_mcp_arguments())

    async def hub_search_report(
        self,
        request: HubSearchArgs | Mapping[str, Any],
    ) -> HubSearchReport:
        """Return typed async HTTP federated hub-search evidence."""

        return hub_search_report(await self.hub_search(request))

    async def hub_resolve(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.hub_resolve`."""

        normalized = request if isinstance(request, HubResolveArgs) else HubResolveArgs.from_wire(request)
        return await self.call_tool("hub_resolve", normalized.to_mcp_arguments())

    async def hub_resolve_report(
        self,
        request: HubResolveArgs | Mapping[str, Any],
    ) -> HubResolveReport:
        """Return typed async HTTP federated resolution evidence."""

        return hub_resolve_report(await self.hub_resolve(request))

    async def hub_lock(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.hub_lock`."""

        normalized = request if isinstance(request, HubLockArgs) else HubLockArgs.from_wire(request)
        return await self.call_tool("hub_lock", normalized.to_mcp_arguments())

    async def hub_lock_report(
        self,
        request: HubLockArgs | Mapping[str, Any],
    ) -> HubLockReport:
        """Return typed async HTTP dependency-lock evidence."""

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
        return await self.call_tool("oracle_combine", request.to_mcp_arguments())

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

    async def oracle_reference_panel(
        self,
        panel: Mapping[str, Any],
        *,
        rule: Mapping[str, Any] | None = None,
        model_call: str | None = None,
        max_items: int = 100,
    ) -> dict[str, Any]:
        request = ReferencePanelRequest(panel, rule, model_call, max_items)
        return await self.call_tool("oracle_reference_panel", request.to_mcp_arguments())

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
        return await self.call_tool("oracle_missingness", request.to_mcp_arguments())

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
        return await self.call_tool("bioeval_reference_audit", ReferenceStandardAuditRequest(reference, state).to_mcp_arguments())

    async def bioeval_reference_audit_report(
        self, reference: Mapping[str, Any], *, state: str | None = None
    ) -> BioevalReferenceAuditReport:
        return bioeval_reference_audit_report(await self.bioeval_reference_audit(reference, state=state))

    async def bioeval_acquisition_audit(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async acquisition-trace audit through HTTP."""

        normalized = request if isinstance(request, BioevalAcquisitionAuditArgs) else BioevalAcquisitionAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_acquisition_audit", normalized.to_mcp_arguments())

    async def bioeval_acquisition_audit_report(
        self,
        request: BioevalAcquisitionAuditArgs | Mapping[str, Any],
    ) -> BioevalAcquisitionAuditReport:
        """Return async typed HTTP acquisition-trace evidence."""

        return bioeval_acquisition_audit_report(await self.bioeval_acquisition_audit(request))

    async def bioeval_grounding_audit(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async claim-evidence grounding audit through HTTP."""

        normalized = request if isinstance(request, BioevalGroundingAuditArgs) else BioevalGroundingAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_grounding_audit", normalized.to_mcp_arguments())

    async def bioeval_grounding_audit_report(
        self,
        request: BioevalGroundingAuditArgs | Mapping[str, Any],
    ) -> BioevalGroundingAuditReport:
        """Return async typed HTTP grounding evidence."""

        return bioeval_grounding_audit_report(await self.bioeval_grounding_audit(request))

    async def bioeval_estimand_audit(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async estimand and identification audit through HTTP."""

        normalized = request if isinstance(request, BioevalEstimandAuditArgs) else BioevalEstimandAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_estimand_audit", normalized.to_mcp_arguments())

    async def bioeval_estimand_audit_report(
        self,
        request: BioevalEstimandAuditArgs | Mapping[str, Any],
    ) -> BioevalEstimandAuditReport:
        """Return async typed HTTP estimand evidence."""

        return bioeval_estimand_audit_report(await self.bioeval_estimand_audit(request))

    async def bioeval_evaluator_audit(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP evaluator-health audit."""

        normalized = request if isinstance(request, BioevalEvaluatorAuditArgs) else BioevalEvaluatorAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_evaluator_audit", normalized.to_mcp_arguments())

    async def bioeval_evaluator_audit_report(
        self,
        request: BioevalEvaluatorAuditArgs | Mapping[str, Any],
    ) -> BioevalEvaluatorAuditReport:
        """Return async typed HTTP evaluator-health evidence."""

        return bioeval_evaluator_audit_report(await self.bioeval_evaluator_audit(request))

    async def bioeval_plane_audit(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP scoring-plane audit."""

        normalized = request if isinstance(request, BioevalPlaneAuditArgs) else BioevalPlaneAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_plane_audit", normalized.to_mcp_arguments())

    async def bioeval_plane_audit_report(
        self,
        request: BioevalPlaneAuditArgs | Mapping[str, Any],
    ) -> BioevalPlaneAuditReport:
        """Return async typed HTTP scoring-plane evidence."""

        return bioeval_plane_audit_report(await self.bioeval_plane_audit(request))

    async def bioeval_metamorphic_audit(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP metamorphic-response audit."""

        normalized = request if isinstance(request, BioevalMetamorphicAuditArgs) else BioevalMetamorphicAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_metamorphic_audit", normalized.to_mcp_arguments())

    async def bioeval_metamorphic_audit_report(
        self,
        request: BioevalMetamorphicAuditArgs | Mapping[str, Any],
    ) -> BioevalMetamorphicAuditReport:
        """Return async typed HTTP metamorphic-response evidence."""

        return bioeval_metamorphic_audit_report(await self.bioeval_metamorphic_audit(request))

    async def bioeval_waiver_audit(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP release-gate waiver audit."""

        normalized = request if isinstance(request, BioevalWaiverAuditArgs) else BioevalWaiverAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_waiver_audit", normalized.to_mcp_arguments())

    async def bioeval_waiver_audit_report(
        self,
        request: BioevalWaiverAuditArgs | Mapping[str, Any],
    ) -> BioevalWaiverAuditReport:
        """Return async typed HTTP release-gate waiver evidence."""

        return bioeval_waiver_audit_report(await self.bioeval_waiver_audit(request))

    async def bioeval_design_audit(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP factorial-design audit."""

        normalized = request if isinstance(request, BioevalDesignAuditArgs) else BioevalDesignAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_design_audit", normalized.to_mcp_arguments())

    async def bioeval_design_audit_report(
        self,
        request: BioevalDesignAuditArgs | Mapping[str, Any],
    ) -> BioevalDesignAuditReport:
        """Return async typed HTTP factorial-design evidence."""

        return bioeval_design_audit_report(await self.bioeval_design_audit(request))

    async def bioeval_mesh_audit(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP evaluator-mesh audit."""

        normalized = request if isinstance(request, BioevalMeshAuditArgs) else BioevalMeshAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_mesh_audit", normalized.to_mcp_arguments())

    async def bioeval_mesh_audit_report(
        self,
        request: BioevalMeshAuditArgs | Mapping[str, Any],
    ) -> BioevalMeshAuditReport:
        """Return async typed HTTP evaluator-mesh evidence."""

        return bioeval_mesh_audit_report(await self.bioeval_mesh_audit(request))

    async def bioeval_burden_audit(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP nonrenewable-resource audit."""

        normalized = request if isinstance(request, BioevalBurdenAuditArgs) else BioevalBurdenAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_burden_audit", normalized.to_mcp_arguments())

    async def bioeval_burden_audit_report(
        self,
        request: BioevalBurdenAuditArgs | Mapping[str, Any],
    ) -> BioevalBurdenAuditReport:
        """Return async typed burden evidence."""

        return bioeval_burden_audit_report(await self.bioeval_burden_audit(request))

    async def bioeval_reveal_audit(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP prospective reveal audit."""

        normalized = request if isinstance(request, BioevalRevealAuditArgs) else BioevalRevealAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_reveal_audit", normalized.to_mcp_arguments())

    async def bioeval_reveal_audit_report(
        self,
        request: BioevalRevealAuditArgs | Mapping[str, Any],
    ) -> BioevalRevealAuditReport:
        """Return async typed seal/reveal evidence."""

        return bioeval_reveal_audit_report(await self.bioeval_reveal_audit(request))

    async def bioeval_boundary_audit(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async HTTP contextual-integrity audit."""

        normalized = request if isinstance(request, BioevalBoundaryAuditArgs) else BioevalBoundaryAuditArgs.from_wire(request)
        return await self.call_tool("bioeval_boundary_audit", normalized.to_mcp_arguments())

    async def bioeval_boundary_audit_report(
        self,
        request: BioevalBoundaryAuditArgs | Mapping[str, Any],
    ) -> BioevalBoundaryAuditReport:
        """Return async typed boundary evidence."""

        return bioeval_boundary_audit_report(await self.bioeval_boundary_audit(request))

    async def evaluation_worldline_audit(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> dict[str, Any]:
        return await self.call_tool("evaluation_worldline_audit", EvaluationWorldlineRequest(worldline, at).to_mcp_arguments())

    async def evaluation_worldline_audit_report(
        self, worldline: Mapping[str, Any], *, at: str | None = None
    ) -> EvaluationWorldlineReport:
        return evaluation_worldline_audit_report(await self.evaluation_worldline_audit(worldline, at=at))

    async def evaluation_reproduction_check(
        self, reexecution: Mapping[str, Any], *, biological_claim: str | None = None
    ) -> dict[str, Any]:
        return await self.call_tool("evaluation_reproduction_check", EvaluationReproductionRequest(reexecution, biological_claim).to_mcp_arguments())

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
        return await self.call_tool("evaluation_trajectory_check", EvaluationTrajectoryRequest(trajectory, step, horizon).to_mcp_arguments())

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
        return await self.call_tool("runtime_effect_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("runtime_tape_verify", normalized.to_mcp_arguments())

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
        return await self.call_tool("runtime_execution_simulate", normalized.to_mcp_arguments())

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
        return await self.call_tool("bioethics_action_review", normalized.to_mcp_arguments())

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
        return await self.call_tool("bioethics_human_subject_screen", normalized.to_mcp_arguments())

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
        return await self.call_tool("bioethics_dual_use_review", normalized.to_mcp_arguments())

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
        return await self.call_tool("bioethics_validation_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("bioethics_representation_audit", normalized.to_mcp_arguments())

    async def bioethics_representation_audit_report(
        self,
        request: BioethicsRepresentationAuditArgs | Mapping[str, Any],
    ) -> BioethicsRepresentationAuditReport:
        return bioethics_representation_audit_report(await self.bioethics_representation_audit(request))

    async def biocapability_evidence_audit(
        self,
        request: BioCapabilityEvidenceAuditRequest,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.biocapability_evidence_audit`."""

        if not isinstance(request, BioCapabilityEvidenceAuditRequest):
            raise ArgumentError("request must be a BioCapabilityEvidenceAuditRequest")
        return await self.call_tool("biocapability_evidence_audit", request.to_mcp_arguments())

    async def biocapability_evidence_audit_report(
        self, request: BioCapabilityEvidenceAuditRequest
    ) -> BioCapabilityEvidenceAuditReport:
        """Async typed evidence states, claim blockers, and release posture over HTTP."""

        return biocapability_evidence_audit_report(
            await self.biocapability_evidence_audit(request)
        )

    async def bioatlas_publication_audit(
        self, atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs, **kwargs: Any
    ) -> dict[str, Any]:
        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationArgs cannot be combined with keyword options")
            return await asyncio.to_thread(self.client.bioatlas_publication_audit, atlas)
        return await asyncio.to_thread(self.client.bioatlas_publication_audit, atlas, **kwargs)

    async def bioatlas_publication_audit_report(
        self, atlas: Mapping[str, Any] | BioAtlasPublicationAuditArgs, **kwargs: Any
    ) -> BioAtlasPublicationAuditReport:
        """Async typed publication-readiness evidence from the HTTP gateway."""

        if isinstance(atlas, BioAtlasPublicationAuditArgs):
            if kwargs:
                raise ArgumentError("typed BioAtlasPublicationAuditArgs cannot be combined with keyword options")
            return bioatlas_publication_audit(await self.bioatlas_publication_audit(atlas))
        return bioatlas_publication_audit_report(await self.bioatlas_publication_audit(atlas, **kwargs))

    async def bioql_compile(
        self,
        query: str | BioQlCompileRequest,
        schema: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.bioql_compile`."""

        if isinstance(query, BioQlCompileRequest):
            if schema is not None:
                raise ArgumentError("schema must be omitted when query is a BioQlCompileRequest")
            request = query
        else:
            if schema is None:
                raise ArgumentError("schema is required when query is a string")
            request = BioQlCompileRequest(query, schema)
        return await self.call_tool("bioql_compile", request.to_mcp_arguments())

    async def world_claim_check(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.world_claim_check`."""

        if isinstance(provenance, WorldClaimCheckRequest):
            if claim is not None:
                raise ArgumentError("claim must be omitted when provenance is a WorldClaimCheckRequest")
            request = provenance
        else:
            if claim is None:
                raise ArgumentError("claim is required when provenance is a mapping")
            request = WorldClaimCheckRequest(provenance, claim)
        return await self.call_tool("world_claim_check", request.to_mcp_arguments())

    async def observed_world_declare(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.observed_world_declare`."""

        normalized = request if isinstance(request, ObservedWorldDeclareArgs) else ObservedWorldDeclareArgs.from_wire(request)
        return await self.call_tool("observed_world_declare", normalized.to_mcp_arguments())

    async def observed_world_declare_report(
        self,
        request: ObservedWorldDeclareArgs | Mapping[str, Any],
    ) -> ObservedWorldDeclareReport:
        """Return typed async HTTP observed-world declaration evidence."""

        return observed_world_declare_report(await self.observed_world_declare(request))

    async def world_claim_check_report(
        self,
        provenance: Mapping[str, Any] | WorldClaimCheckRequest,
        claim: Mapping[str, Any] | None = None,
    ) -> WorldClaimCheckReport:
        """Return typed async HTTP grounded evidence or refusal."""

        return world_claim_check_report(await self.world_claim_check(provenance, claim))

    async def lineage_audit(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.lineage_audit`."""

        normalized = request if isinstance(request, LineageAuditArgs) else LineageAuditArgs.from_wire(request)
        return await self.call_tool("lineage_audit", normalized.to_mcp_arguments())

    async def lineage_audit_report(
        self,
        request: LineageAuditArgs | Mapping[str, Any],
    ) -> LineageAuditReport:
        return lineage_audit_report(await self.lineage_audit(request))

    async def preanalytic_apply(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.preanalytic_apply`."""

        normalized = request if isinstance(request, PreanalyticApplyArgs) else PreanalyticApplyArgs.from_wire(request)
        return await self.call_tool("preanalytic_apply", normalized.to_mcp_arguments())

    async def preanalytic_apply_report(
        self,
        request: PreanalyticApplyArgs | Mapping[str, Any],
    ) -> PreanalyticApplyReport:
        return preanalytic_apply_report(await self.preanalytic_apply(request))

    async def contradiction_review(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.contradiction_review`."""

        normalized = request if isinstance(request, ContradictionReviewArgs) else ContradictionReviewArgs.from_wire(request)
        return await self.call_tool("contradiction_review", normalized.to_mcp_arguments())

    async def contradiction_review_report(
        self,
        request: ContradictionReviewArgs | Mapping[str, Any],
    ) -> ContradictionReviewReport:
        return contradiction_review_report(await self.contradiction_review(request))

    async def onco_boundary_check(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.onco_boundary_check`."""

        normalized = request if isinstance(request, OncoBoundaryArgs) else OncoBoundaryArgs.from_wire(request)
        return await self.call_tool("onco_boundary_check", normalized.to_mcp_arguments())

    async def onco_boundary_report(
        self,
        request: OncoBoundaryArgs | Mapping[str, Any],
    ) -> OncoBoundaryReport:
        return onco_boundary_report(await self.onco_boundary_check(request))

    async def onco_response_assess(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.onco_response_assess`."""

        normalized = request if isinstance(request, OncoResponseAssessArgs) else OncoResponseAssessArgs.from_wire(request)
        return await self.call_tool("onco_response_assess", normalized.to_mcp_arguments())

    async def onco_response_report(
        self,
        request: OncoResponseAssessArgs | Mapping[str, Any],
    ) -> OncoResponseReport:
        return onco_response_report(await self.onco_response_assess(request))

    async def onco_worldline_view(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.onco_worldline_view`."""

        normalized = request if isinstance(request, OncoWorldlineViewArgs) else OncoWorldlineViewArgs.from_wire(request)
        return await self.call_tool("onco_worldline_view", normalized.to_mcp_arguments())

    async def onco_worldline_report(
        self,
        request: OncoWorldlineViewArgs | Mapping[str, Any],
    ) -> OncoWorldlineReport:
        return onco_worldline_report(await self.onco_worldline_view(request))

    async def onco_classification_check(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.onco_classification_check`."""

        normalized = request if isinstance(request, OncoClassificationArgs) else OncoClassificationArgs.from_wire(request)
        return await self.call_tool("onco_classification_check", normalized.to_mcp_arguments())

    async def onco_classification_report(
        self,
        request: OncoClassificationArgs | Mapping[str, Any],
    ) -> OncoClassificationReport:
        return onco_classification_report(await self.onco_classification_check(request))

    async def oncoworlds_identity_join(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.oncoworlds_identity_join`."""

        normalized = request if isinstance(request, OncoIdentityJoinArgs) else OncoIdentityJoinArgs.from_wire(request)
        return await self.call_tool("oncoworlds_identity_join", normalized.to_mcp_arguments())

    async def oncoworlds_identity_join_report(
        self,
        request: OncoIdentityJoinArgs | Mapping[str, Any],
    ) -> OncoIdentityJoinReport:
        return onco_identity_join_report(await self.oncoworlds_identity_join(request))

    async def onco_outcome_analyze(
        self,
        request: OncoOutcomeAnalyzeArgs | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.onco_outcome_analyze`."""

        normalized = request if isinstance(request, OncoOutcomeAnalyzeArgs) else OncoOutcomeAnalyzeArgs.from_wire(request)
        return await self.call_tool("onco_outcome_analyze", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_model_transport", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_methylation_classify", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_methylation_compare", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_radiogenomic_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_clonal_history_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_clonal_evidence_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_era_shift_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_equity_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("oncoworlds_entity_world_check", normalized.to_mcp_arguments())

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
        return await self.call_tool("stress_profile", normalized.to_mcp_arguments())

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
        return await self.call_tool("stress_report", normalized.to_mcp_arguments())

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
        return await self.call_tool("influence_analyze", normalized.to_mcp_arguments())

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
        """Run the offline routing lab through async HTTP."""

        normalized = request if isinstance(request, RoutingLabRunArgs) else RoutingLabRunArgs.from_wire(request)
        return await self.call_tool("routing_lab_run", normalized.to_mcp_arguments())

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
        """Build the offline inference-lab Pareto archive through async HTTP."""

        normalized = request if isinstance(request, LabParetoAuditArgs) else LabParetoAuditArgs.from_wire(request)
        return await self.call_tool("lab_pareto_audit", normalized.to_mcp_arguments())

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
        """Audit risk-triggered branch accounting through async HTTP."""

        normalized = request if isinstance(request, LabBranchAuditArgs) else LabBranchAuditArgs.from_wire(request)
        return await self.call_tool("lab_branch_audit", normalized.to_mcp_arguments())

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
        """Run the offline holdout and rollback audit through async HTTP."""

        normalized = request if isinstance(request, LabHoldoutAuditArgs) else LabHoldoutAuditArgs.from_wire(request)
        return await self.call_tool("lab_holdout_audit", normalized.to_mcp_arguments())

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
        """Assemble and grade a benchmark-gated evolution card through async HTTP."""

        normalized = request if isinstance(request, LabEvolutionAuditArgs) else LabEvolutionAuditArgs.from_wire(request)
        return await self.call_tool("lab_evolution_audit", normalized.to_mcp_arguments())

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
        """Validate and inspect an immutable architecture space through async HTTP."""

        normalized = request if isinstance(request, LabSpaceAuditArgs) else LabSpaceAuditArgs.from_wire(request)
        return await self.call_tool("lab_space_audit", normalized.to_mcp_arguments())

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
        return await self.call_tool("provider_capability_gate", normalized.to_mcp_arguments())

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
        return await self.call_tool("sdk_registry_check", normalized.to_mcp_arguments())

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
        """Async counterpart to :meth:`ApiClient.lab_plan`."""

        if isinstance(graph, LabPlanRequest):
            if actions is not None or budget is not None:
                raise ArgumentError("actions and budget must be omitted when graph is a LabPlanRequest")
            request = graph
        else:
            if actions is None or budget is None:
                raise ArgumentError("actions and budget are required when graph is a mapping")
            request = LabPlanRequest(graph, actions, budget, marginal_value_floor, hypotheses, observations, max_items)
        return await self.call_tool("lab_plan", request.to_mcp_arguments())

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
        return await self.call_tool("obligation_gate_check", normalized.to_mcp_arguments())

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
        """Async counterpart to :meth:`ApiClient.routing_decide`."""

        if isinstance(fingerprint, RoutingDecisionRequest):
            if evidence is not None or policy is not None or task_id is not None:
                raise ArgumentError("other routing arguments must be omitted when fingerprint is a RoutingDecisionRequest")
            request = fingerprint
        else:
            if evidence is None or policy is None:
                raise ArgumentError("evidence and policy are required when fingerprint is a mapping")
            request = RoutingDecisionRequest(fingerprint, evidence, policy, task_id)
        return await self.call_tool("routing_decide", request.to_mcp_arguments())

    async def repository_catalog(
        self,
        request: RepositoryCatalogRequest | None = None,
        *,
        prefix: str | None = None,
        limit: int = 200,
        include_briefs: bool = False,
        include_findings: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.repository_catalog`."""

        if request is not None:
            if prefix is not None or limit != 200 or include_briefs or include_findings:
                raise ArgumentError("catalog options must be omitted when passing a RepositoryCatalogRequest")
        else:
            request = RepositoryCatalogRequest(prefix, limit, include_briefs, include_findings)
        return await self.call_tool("repository_catalog", request.to_mcp_arguments())

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
        """Async counterpart to :meth:`ApiClient.repository_bundle`."""

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
        return await self.call_tool("repository_bundle", request.to_mcp_arguments())

    async def repository_impact(
        self,
        changed: str | RepositoryImpactRequest,
        *,
        route: Mapping[str, Any] | None = None,
        routes: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.repository_impact`."""

        if isinstance(changed, RepositoryImpactRequest):
            if route is not None or routes is not None:
                raise ArgumentError("route and routes must be omitted when passing a RepositoryImpactRequest")
            request = changed
        else:
            request = RepositoryImpactRequest(changed, route, routes)
        return await self.call_tool("repository_impact", request.to_mcp_arguments())

    async def telemetry_project(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.telemetry_project`."""

        if isinstance(event, TelemetryProjectRequest):
            if policy is not None or trace is not None or metric is not None or observations is not None:
                raise ArgumentError("telemetry fields must be omitted when passing a TelemetryProjectRequest")
            request = event
        else:
            if policy is None or trace is None:
                raise ArgumentError("policy and trace are required when event is a mapping")
            request = TelemetryProjectRequest(event, policy, trace, metric, observations)
        return await self.call_tool("telemetry_project", request.to_mcp_arguments())

    async def telemetry_project_report(
        self,
        event: Mapping[str, Any] | TelemetryProjectRequest,
        policy: Mapping[str, Any] | None = None,
        trace: str | None = None,
        *,
        metric: Mapping[str, Any] | None = None,
        observations: Mapping[str, Any] | None = None,
    ) -> TelemetryProjectionReport:
        """Return async typed telemetry projection evidence through HTTP."""

        return telemetry_project_report(await self.telemetry_project(event, policy, trace, metric=metric, observations=observations))

    async def ledger_ingest(self, request: LedgerIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.ledger_ingest`."""

        normalized = request if isinstance(request, LedgerIngestArgs) else LedgerIngestArgs.from_wire(request)
        return await self.call_tool("ledger_ingest", normalized.to_mcp_arguments())

    async def ledger_ingest_report(self, request: LedgerIngestArgs | Mapping[str, Any]) -> LedgerIngestReport:
        """Return async typed ledger evidence through HTTP."""

        return ledger_ingest_report(await self.ledger_ingest(request))

    async def trace_otel_ingest(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async bounded OTLP JSON import through the HTTP gateway."""

        normalized = request if isinstance(request, TraceOtelIngestArgs) else TraceOtelIngestArgs.from_wire(request)
        return await self.call_tool("trace_otel_ingest", normalized.to_mcp_arguments())

    async def trace_otel_ingest_report(self, request: TraceOtelIngestArgs | Mapping[str, Any]) -> TraceOtelIngestReport:
        """Return async typed OTLP ingestion evidence through HTTP."""

        return trace_otel_ingest_report(await self.trace_otel_ingest(request))

    async def quality_gate_run(self, request: QualityGateRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async bounded quality gate through the HTTP gateway."""

        normalized = request if isinstance(request, QualityGateRunArgs) else QualityGateRunArgs.from_wire(request)
        return await self.call_tool("quality_gate_run", normalized.to_mcp_arguments())

    async def quality_gate_run_report(self, request: QualityGateRunArgs | Mapping[str, Any]) -> QualityGateRunReport:
        """Return async typed quality evidence through HTTP."""

        return quality_gate_run_report(await self.quality_gate_run(request))

    async def atlas_report(self, request: AtlasReportArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async bounded capability-atlas reporting through the HTTP gateway."""

        normalized = request if isinstance(request, AtlasReportArgs) else AtlasReportArgs.from_wire(request)
        return await self.call_tool("atlas_report", normalized.to_mcp_arguments())

    async def atlas_report_typed(self, request: AtlasReportArgs | Mapping[str, Any]) -> AtlasReport:
        """Return async typed atlas coverage, debt, and composite evidence."""

        return atlas_report_parser(await self.atlas_report(request))

    async def atlas_surface_audit(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async atlasx coverage and failure-surface audit through the HTTP gateway."""

        normalized = request if isinstance(request, AtlasSurfaceAuditArgs) else AtlasSurfaceAuditArgs.from_wire(request)
        return await self.call_tool("atlas_surface_audit", normalized.to_mcp_arguments())

    async def atlas_surface_audit_report(self, request: AtlasSurfaceAuditArgs | Mapping[str, Any]) -> AtlasSurfaceAuditReport:
        """Return async typed atlasx surface evidence."""

        return atlas_surface_audit_report(await self.atlas_surface_audit(request))

    async def engineering_manifest_audit(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async engineering-manifest audit through the HTTP gateway."""

        normalized = request if isinstance(request, EngineeringManifestArgs) else EngineeringManifestArgs.from_wire(request)
        return await self.call_tool("engineering_manifest_audit", normalized.to_mcp_arguments())

    async def engineering_manifest_audit_report(self, request: EngineeringManifestArgs | Mapping[str, Any]) -> EngineeringAuditReport:
        """Return async typed engineering-manifest evidence through HTTP."""

        return engineering_manifest_audit_report(await self.engineering_manifest_audit(request))

    async def engineering_execution_plan(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async engineering execution-plan request through the HTTP gateway."""

        normalized = request if isinstance(request, EngineeringPlanRequestArgs) else EngineeringPlanRequestArgs.from_wire(request)
        return await self.call_tool("engineering_execution_plan", normalized.to_mcp_arguments())

    async def engineering_execution_plan_report(self, request: EngineeringPlanRequestArgs | Mapping[str, Any]) -> EngineeringPlanReport:
        """Return async typed engineering execution-plan evidence through HTTP."""

        return engineering_execution_plan_report(await self.engineering_execution_plan(request))

    async def release_pipeline_audit(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async release-pipeline audit through the HTTP gateway."""

        normalized = request if isinstance(request, ReleasePipelineManifestArgs) else ReleasePipelineManifestArgs.from_wire(request)
        return await self.call_tool("release_pipeline_audit", normalized.to_mcp_arguments())

    async def release_pipeline_audit_report(self, request: ReleasePipelineManifestArgs | Mapping[str, Any]) -> ReleasePipelineAuditReport:
        """Return async typed release-pipeline evidence through HTTP."""

        return release_pipeline_audit_report(await self.release_pipeline_audit(request))

    async def operational_readiness_audit(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async operational-readiness audit through the HTTP gateway."""

        normalized = request if isinstance(request, OperationalReadinessManifestArgs) else OperationalReadinessManifestArgs.from_wire(request)
        return await self.call_tool("operational_readiness_audit", normalized.to_mcp_arguments())

    async def operational_readiness_audit_report(self, request: OperationalReadinessManifestArgs | Mapping[str, Any]) -> OperationalReadinessAuditReport:
        """Return async typed operational-readiness evidence through HTTP."""

        return operational_readiness_audit_report(await self.operational_readiness_audit(request))

    async def security_privacy_audit(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async security/privacy governance audit through the HTTP gateway."""

        normalized = request if isinstance(request, SecurityPrivacyManifestArgs) else SecurityPrivacyManifestArgs.from_wire(request)
        return await self.call_tool("security_privacy_audit", normalized.to_mcp_arguments())

    async def security_privacy_audit_report(self, request: SecurityPrivacyManifestArgs | Mapping[str, Any]) -> SecurityPrivacyAuditReport:
        """Return async typed security/privacy governance evidence through HTTP."""

        return security_privacy_audit_report(await self.security_privacy_audit(request))

    async def sandbox_admission_audit(self, request: SandboxManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async sandbox admission audit through HTTP."""

        normalized = request if isinstance(request, SandboxManifestArgs) else SandboxManifestArgs.from_wire(request)
        return await self.call_tool("sandbox_admission_audit", normalized.to_mcp_arguments())

    async def sandbox_admission_audit_report(self, request: SandboxManifestArgs | Mapping[str, Any]) -> SandboxAuditReport:
        """Return async typed sandbox admission evidence through HTTP."""

        return sandbox_admission_audit_report(await self.sandbox_admission_audit(request))

    async def sandbox_runtime_simulate(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.sandbox_runtime_simulate`."""

        normalized = request if isinstance(request, SandboxRuntimeManifestArgs) else SandboxRuntimeManifestArgs.from_wire(request)
        return await self.call_tool("sandbox_runtime_simulate", normalized.to_mcp_arguments())

    async def sandbox_runtime_simulate_report(self, request: SandboxRuntimeManifestArgs | Mapping[str, Any]) -> SandboxRuntimeAuditReport:
        """Return async typed sandbox runtime decisions through HTTP."""

        return sandbox_runtime_simulate_report(await self.sandbox_runtime_simulate(request))

    async def security_program_audit(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async security-program audit through HTTP."""

        normalized = request if isinstance(request, SecurityProgramManifestArgs) else SecurityProgramManifestArgs.from_wire(request)
        return await self.call_tool("security_program_audit", normalized.to_mcp_arguments())

    async def security_program_audit_report(self, request: SecurityProgramManifestArgs | Mapping[str, Any]) -> SecurityProgramAuditReport:
        """Return async typed security-program evidence through HTTP."""

        return security_program_audit_report(await self.security_program_audit(request))

    async def adaptive_panel(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async adaptive panel audit through the HTTP gateway."""

        normalized = request if isinstance(request, AdaptivePanelRunArgs) else AdaptivePanelRunArgs.from_wire(request)
        return await self.call_tool("adaptive_panel", normalized.to_mcp_arguments())

    async def adaptive_panel_report(self, request: AdaptivePanelRunArgs | Mapping[str, Any]) -> AdaptivePanelReport:
        """Return async typed adaptive audit and selection evidence through HTTP."""

        return adaptive_panel_report(await self.adaptive_panel(request))

    async def posterior_gate(self, request: PosteriorGateArgs | Mapping[str, Any]) -> dict[str, Any]:
        """Async posterior gate projection through HTTP."""

        normalized = request if isinstance(request, PosteriorGateArgs) else PosteriorGateArgs.from_wire(request)
        return await self.call_tool("posterior_gate", normalized.to_mcp_arguments())

    async def posterior_gate_report(self, request: PosteriorGateArgs | Mapping[str, Any]) -> PosteriorGateReport:
        """Return async typed posterior, release-gate, and comparison evidence."""

        return posterior_gate_report(await self.posterior_gate(request))

    async def fiber_compile(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_compile`."""

        if isinstance(world, FiberCompileRequest):
            if query is not None or layer not in (ContextLayer.L0, "l0"):
                raise ArgumentError("query and layer must be omitted when passing a FiberCompileRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberCompileRequest(world, query, layer)
        return await self.call_tool("fiber_compile", request.to_mcp_arguments())

    async def fiber_compile_decision_quotient(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberDecisionQuotientSummary:
        """Async counterpart to :meth:`ApiClient.fiber_compile_decision_quotient`."""

        return fiber_decision_quotient_summary(await self.fiber_compile(world, query, layer=layer))

    async def fiber_compile_rate_distortion(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberRateDistortionSummary:
        """Async counterpart to :meth:`ApiClient.fiber_compile_rate_distortion`."""

        return fiber_rate_distortion_summary(await self.fiber_compile(world, query, layer=layer))

    async def fiber_compile_adaptive_acquisition(
        self,
        world: str | FiberCompileRequest,
        query: str | None = None,
        *,
        layer: ContextLayer | str = ContextLayer.L0,
    ) -> FiberAdaptiveAcquisitionSummary:
        """Async counterpart to :meth:`ApiClient.fiber_compile_adaptive_acquisition`."""

        return fiber_adaptive_acquisition_summary(await self.fiber_compile(world, query, layer=layer))

    async def fiber_refine(
        self,
        layer: ContextLayer | str | FiberRefineRequest,
        *,
        handle: Mapping[str, Any] | None = None,
        world: str | None = None,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_refine`."""

        if isinstance(layer, FiberRefineRequest):
            if handle is not None or world is not None or query is not None:
                raise ArgumentError("source arguments must be omitted when passing a FiberRefineRequest")
            request = layer
        else:
            request = FiberRefineRequest(layer, handle, world, query)
        return await self.call_tool("fiber_refine", request.to_mcp_arguments())

    async def fiber_explain(
        self,
        world: str | FiberExplainRequest,
        query: str | None = None,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_explain`."""

        if isinstance(world, FiberExplainRequest):
            if query is not None:
                raise ArgumentError("query must be omitted when passing a FiberExplainRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = FiberExplainRequest(world, query)
        return await self.call_tool("fiber_explain", request.to_mcp_arguments())

    async def fiber_verify(self, certificate: str | FiberVerifyRequest) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.fiber_verify`."""

        request = certificate if isinstance(certificate, FiberVerifyRequest) else FiberVerifyRequest(certificate)
        return await self.call_tool("fiber_verify", request.to_mcp_arguments())

    async def projection_bundle(
        self,
        world: str | ProjectionBundleRequest,
        query: str | None = None,
        *,
        include_views: bool = False,
    ) -> dict[str, Any]:
        """Async counterpart to :meth:`ApiClient.projection_bundle`."""

        if isinstance(world, ProjectionBundleRequest):
            if query is not None or include_views:
                raise ArgumentError("query and include_views must be omitted when passing a ProjectionBundleRequest")
            request = world
        else:
            if query is None:
                raise ArgumentError("query is required when world is a path string")
            request = ProjectionBundleRequest(world=world, query=query, include_views=include_views)
        return await self.call_tool("projection_bundle", request.to_mcp_arguments())

    context_compile = fiber_compile
    context_refine = fiber_refine
    context_explain = fiber_explain
    context_verify = fiber_verify

    async def events(
        self,
        *,
        after: int = 0,
        limit: int = 100,
        review_id: str | None = None,
        receipt_id: str | None = None,
    ) -> dict[str, Any]:
        return await asyncio.to_thread(
            self.client.events,
            after=after,
            limit=limit,
            review_id=review_id,
            receipt_id=receipt_id,
        )
