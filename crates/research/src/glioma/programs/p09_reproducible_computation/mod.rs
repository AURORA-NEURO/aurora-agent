//! Reproducible computation program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod artifact_lineage_index;
pub mod artifact_registry_connector;
pub mod autonomous_stage_bridge;
pub mod campaign;
pub mod computation_event_stream;
pub mod computation_run_inspector;
pub mod cross_study_comparator;
pub mod environment_resolution_agent;
pub mod execution;
pub mod execution_environment_lock;
pub mod federated_compute_cost_exchange;
pub mod federated_replay_conformance;
pub mod federated_replay_discrepancy_scan;
pub mod federated_workflow_template_exchange;
pub mod high_throughput_compute_capacity;
pub mod high_throughput_compute_timeline;
pub mod interpretation_frontier;
pub mod lineage;
pub mod local_compute_cache_governor;
pub mod multistudy_cache_partition;
pub mod operating_cycle;
pub mod partial_result_semantics;
pub mod phase_resolved_imaging_handoff;
pub mod placement;
pub mod placement_stress_evaluation;
pub mod planning;
pub mod portfolio_execution;
pub mod recovery_campaign;
pub mod reproducibility;
pub mod reproducible_task_api;
pub mod robustness;
pub mod robustness_guided;
pub mod workflow;
pub mod workflow_execution_manifest;

pub use robustness::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};

pub use robustness_guided::{
    dry_run_robustness_guided_computation_executor, execute_glioma_robustness_guided_computation,
    RobustnessGuidedCandidate, RobustnessGuidedCandidateScore, RobustnessGuidedComputation,
    RobustnessGuidedComputationDisposition, RobustnessGuidedComputationError,
    RobustnessGuidedComputationRequest,
};

pub use recovery_campaign::{
    execute_glioma_computation_recovery, ComputationRecoveryCampaign,
    ComputationRecoveryDisposition, ComputationRecoveryError, ComputationRecoveryRequest,
    ComputationRecoveryStopReason,
};

pub use cross_study_comparator::{
    compare_glioma_cross_study_computation, ComputationFeatureObservation, ComputationFeatureSpec,
    CrossStudyComputationComparatorRequest, CrossStudyComputationComparison,
    CrossStudyComputationComparisonDisposition, CrossStudyComputationComparisonError,
    CrossStudyComputationStudy, CrossStudyFeatureComparison, FeatureComparisonDisposition,
};

pub use federated_replay_discrepancy_scan::{
    scan_glioma_federated_replay_discrepancy, FederatedReplayAttestation,
    FederatedReplayDiscrepancy, FederatedReplayDiscrepancyDisposition,
    FederatedReplayDiscrepancyError, FederatedReplayDiscrepancyReport,
    FederatedReplayDiscrepancyScanRequest, ReplayDiagnosticTask, ReplayDivergenceKind,
};

pub use execution_environment_lock::{
    lock_glioma_compute_environment, ComputeEnvironmentDependency,
    ComputeEnvironmentDependencyRecord, ComputeEnvironmentLock, ComputeEnvironmentLockDisposition,
    ComputeEnvironmentLockError, ComputeEnvironmentLockRequest, EnvironmentArchitectureProfile,
    EnvironmentDependencyKind, EnvironmentDependencyStatus,
};

pub use environment_resolution_agent::{
    resolve_glioma_compute_environment, EnvironmentResolutionCandidate,
    EnvironmentResolutionChange, EnvironmentResolutionChangeKind, EnvironmentResolutionDisposition,
    EnvironmentResolutionError, EnvironmentResolutionProposal, EnvironmentResolutionRequest,
};

pub use reproducible_task_api::{
    submit_glioma_reproducible_task, ReproducibleExecutionHandle, ReproducibleTaskApiError,
    ReproducibleTaskExchange, ReproducibleTaskExchangeRecord, ReproducibleTaskExchangeStatus,
    ReproducibleTaskInput, ReproducibleTaskPolicyGrant, ReproducibleTaskResourceBudget,
    ReproducibleTaskResultState, ReproducibleTaskSpec, ReproducibleTaskSubmission,
};

pub use federated_workflow_template_exchange::{
    exchange_glioma_federated_workflow_template, FederatedWorkflowTemplateDisposition,
    FederatedWorkflowTemplateExchangeError, FederatedWorkflowTemplateExchangeRequest,
    FederatedWorkflowTemplateManifest, FederatedWorkflowTemplatePackage, WorkflowSharingPolicy,
    WorkflowSiteAttestation, WorkflowSiteConformance, WorkflowSiteCoverage,
    WorkflowTemplateAdaptation, WorkflowValidationCard,
};

pub use artifact_registry_connector::{
    resolve_glioma_registry_artifact, ArtifactRef, ArtifactRegistryAccessGrant,
    ArtifactRegistryConnectorError, ArtifactRegistryResolutionRequest, ArtifactRegistryTrustConfig,
    ArtifactResolutionDisposition, RegistryArtifactCandidate, VerifiedArtifactHandle,
    VerifiedArtifactResolution,
};

pub use federated_replay_conformance::{
    verify_glioma_federated_replay_conformance, FederatedReplayConformanceDisposition,
    FederatedReplayConformanceError, FederatedReplayConformanceReport,
    FederatedReplayConformanceRequest, ReplayConformancePolicy, ReplayConformanceReference,
    ReplayEvidenceStrength, ReplayMetricDeviation, ReplayMetricTolerance, ReplaySiteAttestation,
    ReplaySiteConformance, ReplaySiteConformanceResult,
};

pub use local_compute_cache_governor::{
    govern_glioma_compute_cache, CacheEntry, CacheEvictionPolicy, CacheKey, CacheRetentionPolicy,
    ComputeCacheDecision, ComputeCacheDecisionDisposition, ComputeCacheGovernorError,
    ComputeCacheGovernorRequest,
};

pub use multistudy_cache_partition::{
    partition_glioma_multistudy_cache, CacheSensitivity, MultiStudyCacheEntry,
    MultiStudyCachePartitionDecision, MultiStudyCachePartitionDisposition,
    MultiStudyCachePartitionError, MultiStudyCachePartitionRequest, MultiStudyCachePolicy,
};

pub use high_throughput_compute_capacity::{
    plan_glioma_compute_capacity, CapacityAssignment, CapacityAssignmentDisposition, CapacityJob,
    CapacityPolicy, CapacityRuntimeObservation, CapacityTelemetry, CapacityThroughputForecast,
    ComputeCapacityError, ComputeCapacityPlan, ComputeCapacityPlanDisposition,
    ComputeCapacityRequest,
};

pub use federated_compute_cost_exchange::{
    exchange_glioma_compute_capacity, FederatedCapacitySiteDisposition,
    FederatedCapacitySiteOption, FederatedCapacitySiteSummary, FederatedComputeCapacityEnvelope,
    FederatedComputeCapacityError, FederatedComputeCostExchangeRequest,
    FederatedComputeEnvelopeDisposition, FederatedComputeExchangePolicy,
};

pub use computation_event_stream::{
    stream_glioma_computation_events, ComputationEventAccessScope, ComputationEventBatch,
    ComputationEventBatchDisposition, ComputationEventCursor, ComputationEventFilter,
    ComputationEventGap, ComputationEventKind, ComputationEventStreamError,
    ComputationEventStreamRequest, ComputationStreamEvent,
};

pub use reproducibility::{
    analyze_glioma_computation_reproducibility, ComputationReproducibility,
    ComputationReproducibilityDisposition, ComputationReproducibilityError,
    ComputationReproducibilityRequest, ComputationReproducibilityRun,
    ComputationReproducibilityTaskObservation, ComputationRunOutcome,
    ComputationTaskReproducibilityDisposition, ComputationTaskReproducibilitySummary,
};

pub use execution::{
    execute_glioma_computation, ComputationCacheEntry, ComputationExecution,
    ComputationExecutionDisposition, ComputationExecutionError, ComputationExecutionFailure,
    ComputationExecutionRequest, ComputationExecutionStopReason, ComputationOperation,
    ComputationTask, ComputationTaskDisposition, ComputationTaskResult,
    DryRunGliomaComputationExecutor, GliomaComputationExecutor,
};

pub use interpretation_frontier::{
    compile_glioma_computation_interpretation_frontier,
    execute_glioma_computation_interpretation_frontier, ComputationInterpretationFrontier,
    ComputationInterpretationFrontierDisposition, ComputationInterpretationFrontierError,
    ComputationInterpretationFrontierRequest, ComputationInterpretationFrontierRun,
};

pub use lineage::{
    join_glioma_computation_lineage, ComputationLineage, ComputationLineageDisposition,
    ComputationLineageError, ComputationLineageNode, ComputationLineageNodeStatus,
    ComputationLineageRequest,
};

pub use campaign::{
    execute_glioma_computation_campaign, ComputationOutcomeSummary, GliomaComputationCampaign,
    GliomaComputationCampaignDisposition, GliomaComputationCampaignError,
    GliomaComputationCampaignRequest, GliomaComputationCampaignRound,
    GliomaComputationCampaignStopReason, GliomaComputationPlanner, GliomaComputationPlannerContext,
    GliomaComputationPlannerFailure, StaticGliomaComputationPlanner,
};

pub use planning::{
    plan_glioma_computation_portfolio, ComputationCandidate, ComputationCandidateDisposition,
    ComputationCandidateScore, ComputationPortfolioDisposition, ComputationPortfolioError,
    ComputationPortfolioPlan, ComputationPortfolioRequest,
};

pub use portfolio_execution::{
    execute_glioma_computation_portfolio, ComputationPortfolioExecution,
    ComputationPortfolioExecutionDisposition, ComputationPortfolioExecutionError,
    ComputationPortfolioExecutionRequest,
};

pub use placement::{
    schedule_glioma_computation_placement, ComputationPlacementAssignment,
    ComputationPlacementBlockedTask, ComputationPlacementDisposition, ComputationPlacementError,
    ComputationPlacementRequest, ComputationPlacementSchedule, ComputationWorkerProfile,
    ComputationWorkerUtilization,
};

pub use placement_stress_evaluation::{
    evaluate_glioma_computation_placement_stress, ComputationPlacementStressDisposition,
    ComputationPlacementStressEvaluation, ComputationPlacementStressEvaluationError,
    ComputationPlacementStressEvaluationRequest, ComputationPlacementStressMetric,
    ComputationPlacementStressScenario,
};

pub use workflow::{
    compile_glioma_computation_workflow, GliomaComputationWorkflow, GliomaComputationWorkflowError,
    GliomaComputationWorkflowRequest,
};

pub use workflow_execution_manifest::{
    compile_glioma_workflow_execution_manifest, ExpectedWorkflowOutput, UnsupportedWorkflowStep,
    WorkflowEffect, WorkflowExecutionManifest, WorkflowExecutionManifestDisposition,
    WorkflowExecutionManifestError, WorkflowExecutionManifestRequest, WorkflowResourceBinding,
    WorkflowResourceClass, WorkflowRetryPolicy, WorkflowTaskContract, WorkflowToolPin,
};

pub use operating_cycle::{
    execute_glioma_computation_operating_cycle, execute_glioma_computation_operating_cycle_dry_run,
    ComputationExecutionMode, GliomaComputationOperatingCycle,
    GliomaComputationOperatingCycleDisposition, GliomaComputationOperatingCycleError,
    GliomaComputationOperatingCycleRequest,
};

pub use autonomous_stage_bridge::{
    dry_run_glioma_computation_stage_worker, GliomaComputationStageBridgeError,
    GliomaComputationStageBridgeReceipt, GliomaComputationStageWorker,
};

pub use artifact_lineage_index::{
    index_glioma_artifact_lineage, ArtifactLineageDisposition, ArtifactLineageEdge,
    ArtifactLineageEdgeIntegrity, ArtifactLineageEdgeRecord, ArtifactLineageIndex,
    ArtifactLineageIndexError, ArtifactLineageNodeRecord, ArtifactLineageNodeSpec,
    ArtifactLineageNodeStatus, ArtifactLineagePathProof, ArtifactLineageRelation,
    ArtifactLineageRequest,
};

pub use computation_run_inspector::{
    inspect_glioma_computation_run, ComputationInspectionDisposition, ComputationInspectionEvent,
    ComputationInspectionEventKind, ComputationInspectionTask, ComputationInspectionTaskStatus,
    ComputationRunInspection, ComputationRunInspectionRequest, ComputationRunInspectorError,
};

pub use high_throughput_compute_timeline::{
    compile_glioma_high_throughput_compute_timeline, CampaignBottleneck, CampaignRunDisposition,
    CampaignThroughputForecast, ComputationCampaignRunInput, ComputationCampaignRunSummary,
    ComputationCampaignTimeline, ComputationCampaignTimelineDisposition,
    ComputationCampaignTimelineError, ComputationCampaignTimelineRequest,
};

pub use partial_result_semantics::{
    compile_glioma_partial_result_bundle, PartialResultBundle, PartialResultBundleRequest,
    PartialResultDisposition, PartialResultFieldObservation, PartialResultFieldRecord,
    PartialResultFieldSpec, PartialResultFieldState, PartialResultOperation,
    PartialResultOperationDecision, PartialResultPolicy, PartialResultSemanticsError,
    PartialResultTermination,
};

pub use phase_resolved_imaging_handoff::{
    compile_glioma_phase_resolved_imaging_computation_handoff, PhaseResolvedHandoffExecutionMode,
    PhaseResolvedImagingComputationHandoff, PhaseResolvedImagingHandoffDisposition,
    PhaseResolvedImagingHandoffError, PhaseResolvedImagingHandoffRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ReproducibleComputation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P09")
}
