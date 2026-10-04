//! Question-to-decision context program ownership.

pub mod access_governor;
pub mod action_bridge;
pub mod action_graph;
pub mod adaptive_branch_campaign;
pub mod adaptive_context_scheduler;
pub mod adaptive_controller;
pub mod admission_gate;
pub mod branch_campaign;
pub mod branch_evidence;
pub mod branch_planner;
pub mod campaign;
pub mod context_compiler;
pub mod context_replay;
pub mod cross_study_context_diff;
pub mod cross_study_context_invariance_test;
pub mod decision_budget_dashboard;
pub mod decision_context_artifact;
pub mod decision_context_query_api;
pub mod decision_cycle;
pub mod decision_event_update_api;
pub mod decision_loop_governor;
pub mod federated_continual_promotion;
pub mod federated_decision_context;
pub mod mission_bridge;
pub mod multi_study_context_artifact;
pub mod multi_study_epoch_replay;
pub mod multi_study_execution_receipt;
pub mod multi_study_workflow;
pub mod omission_certificate;
pub mod partition_checkpoint;
pub mod snapshot_store;
pub mod uncertainty_branch_explorer;
pub mod value_calibration;
pub mod value_optimizer;
pub use access_governor::{
    govern_federated_context_access,
    ContextAccessDecision,
    ContextAccessFieldDecision,
    ContextAccessGovernorError,
    ContextAccessPurpose,
    FederatedContextAccessDecision,
    FederatedContextAccessRequest,
};
pub use action_bridge::{
    DecisionActionPlan,
    DecisionActionPlanDisposition,
    DecisionActionPlanError,
    DecisionActionPlanRequest,
    plan_decision_actions,
};
pub use action_graph::{
    DecisionActionGraph,
    DecisionActionGraphDisposition,
    DecisionActionGraphError,
    DecisionActionGraphRequest,
    DecisionGraphNode,
    compile_decision_action_graph,
};
pub use adaptive_branch_campaign::{
    AdaptiveDecisionBranchCampaign,
    AdaptiveDecisionBranchCampaignDisposition,
    AdaptiveDecisionBranchCampaignError,
    AdaptiveDecisionBranchCampaignRequest,
    AdaptiveDecisionBranchCampaignRound,
    AdaptiveDecisionBranchCampaignStopReason,
    execute_glioma_adaptive_decision_branch_campaign,
    execute_glioma_adaptive_decision_branch_campaign_dry_run,
};
pub use adaptive_context_scheduler::{
    schedule_glioma_context_refresh,
    ContextRefreshAction,
    ContextRefreshCandidate,
    ContextRefreshDeferral,
    ContextRefreshDeferralReason,
    ContextRefreshReason,
    ContextRefreshSchedule,
    ContextRefreshScheduleDisposition,
    ContextRefreshScheduleError,
    ContextRefreshScheduleRequest,
};
pub use adaptive_controller::{
    AdaptiveDecisionCampaignDisposition,
    AdaptiveDecisionCandidateScore,
    AdaptiveDecisionControllerError,
    AdaptiveDecisionControllerRequest,
    AdaptiveDecisionControllerResult,
    execute_glioma_adaptive_decision_controller,
};
pub use admission_gate::{
    DecisionAdmissionAction,
    DecisionAdmissionCampaignDisposition,
    DecisionAdmissionDisposition,
    DecisionAdmissionError,
    DecisionAdmissionRecord,
    DecisionAdmissionRequest,
    DecisionAdmissionResult,
    admit_glioma_decision_actions,
};
pub use branch_campaign::{
    BranchExecutionDisposition,
    DecisionBranchCampaign,
    DecisionBranchCampaignDisposition,
    DecisionBranchCampaignError,
    DecisionBranchCampaignRequest,
    DecisionBranchCampaignStopReason,
    DecisionBranchExecution,
    execute_glioma_decision_branch_campaign,
};
pub use branch_evidence::{
    DecisionBranchEvidence,
    DecisionBranchEvidenceDisposition,
    DecisionBranchEvidenceError,
    DecisionBranchEvidenceOutcome,
    DecisionBranchEvidenceOutcomeStatus,
    DecisionBranchEvidenceRecord,
    DecisionBranchEvidenceRequest,
    DecisionBranchEvidenceStatus,
    assimilate_glioma_decision_branch_evidence,
};
pub use branch_planner::{
    DecisionBranchPlan,
    DecisionBranchPlanDisposition,
    DecisionBranchPlannerError,
    DecisionBranchPlannerRequest,
    DecisionBranchPortfolio,
    DecisionScenario,
    DecisionScenarioOutcome,
    DecisionScenarioScore,
    plan_glioma_decision_branches,
};
pub use campaign::{
    DecisionContextCampaign,
    DecisionContextCampaignDisposition,
    DecisionContextCampaignError,
    DecisionContextCampaignExecutionFailure,
    DecisionContextCampaignExecutor,
    DecisionContextCampaignRequest,
    DecisionContextCampaignRound,
    DecisionContextCampaignStopReason,
    DryRunDecisionContextCampaignExecutor,
    execute_glioma_decision_context_campaign,
};
pub use context_compiler::{
    DecisionAction,
    DecisionActionKind,
    DecisionContext,
    DecisionContextDisposition,
    DecisionContextError,
    DecisionContextRequest,
    compile_decision_context,
};
pub use context_replay::{
    DecisionContextActionOutcome,
    DecisionContextActionOutcomeStatus,
    DecisionContextEpoch,
    DecisionContextReplay,
    DecisionContextReplayDisposition,
    DecisionContextReplayError,
    DecisionContextReplayRequest,
    DecisionContextReplayTransition,
    replay_glioma_decision_context,
};
pub use cross_study_context_diff::{
    analyze_glioma_cross_study_context_difference,
    ContextDifferenceKind,
    ContextFieldDifference,
    ContextFieldDomain,
    ContextHarmonizationRule,
    ContextModalityCoverage,
    ContextStudyPairDifference,
    ContextUnitAlias,
    ContextValueAlias,
    CrossStudyContextDifferenceDisposition,
    CrossStudyContextDifferenceError,
    CrossStudyContextDifferenceReport,
    CrossStudyContextDifferenceRequest,
    StudyContextField,
    StudyContextSpec,
};
pub use cross_study_context_invariance_test::{
    test_glioma_cross_study_context_invariance,
    ContextDecisionRule,
    ContextInvarianceDisposition,
    ContextInvarianceFieldResult,
    ContextNuisancePerturbation,
    CrossStudyContextInvarianceError,
    CrossStudyContextInvarianceReport,
    CrossStudyContextInvarianceRequest,
    InvarianceEvaluation,
    InvariancePredicate,
    InvariancePredicateOperator,
    InvarianceStudyInput,
};
pub use decision_budget_dashboard::{
    compile_glioma_decision_budget_snapshot,
    BudgetAlert,
    BudgetAlertSeverity,
    BudgetBranchPlan,
    BudgetBucket,
    BudgetEvent,
    BudgetEventState,
    BudgetForecast,
    BudgetReallocationProposal,
    BudgetResource,
    DecisionBudgetBucketSnapshot,
    DecisionBudgetDisposition,
    DecisionBudgetError,
    DecisionBudgetRequest,
    DecisionBudgetSnapshot,
};
pub use decision_context_artifact::{
    DecisionContextArtifact,
    DecisionContextArtifactAction,
    DecisionContextArtifactCompatibility,
    DecisionContextArtifactConsumer,
    DecisionContextArtifactError,
    DecisionContextArtifactRequest,
    materialize_glioma_decision_context_artifact,
};
pub use decision_context_query_api::{
    query_glioma_decision_context,
    DecisionContextQueryCursor,
    DecisionContextQueryError,
    DecisionContextQueryRecord,
    DecisionContextQueryRequest,
    DecisionContextQueryResult,
    DecisionContextQueryRow,
    QueryCapability,
    QueryCompleteness,
    QueryField,
    QueryValueState,
};
pub use decision_cycle::{
    DecisionOperatingCycle,
    DecisionOperatingCycleDisposition,
    DecisionOperatingCycleError,
    DecisionOperatingCycleRequest,
    execute_glioma_decision_operating_cycle,
};
pub use decision_event_update_api::{
    update_glioma_decision_context,
    DecisionContextEvent,
    DecisionContextUpdateDisposition,
    DecisionContextUpdateEpoch,
    DecisionContextUpdateError,
    DecisionContextUpdateRequest,
    DecisionContextUpdateResult,
    DecisionEventKind,
};
pub use decision_loop_governor::{
    DecisionLoopGovernorDisposition,
    DecisionLoopGovernorError,
    DecisionLoopGovernorRequest,
    DecisionLoopGovernorResult,
    DecisionLoopRound,
    DecisionLoopRoundAssessment,
    DecisionLoopRoundDisposition,
    DecisionLoopStopReason,
    govern_glioma_decision_loop,
};
pub use federated_continual_promotion::{
    FederatedContinualPromotionDisposition, FederatedContinualPromotionError,
    FederatedContinualPromotionReport, FederatedContinualPromotionRequest,
    promote_glioma_federated_continual_context,
};
pub use federated_decision_capsule::{
    package_glioma_federated_decision_capsule,
    DecisionCapsulePolicy,
    FederatedDecisionCapsuleDisposition,
    FederatedDecisionCapsuleError,
    FederatedDecisionCapsuleRequest,
    FederatedDecisionContext,
    LocalDecisionContextCapsule,
};
pub use federated_decision_context::{
    FederatedBranchDisposition,
    FederatedBranchOutcome,
    FederatedDecisionBranchObservation,
    FederatedDecisionBranchSummary,
    FederatedDecisionContextError,
    FederatedDecisionContextReport,
    FederatedDecisionContextRequest,
    FederatedDecisionDisposition,
    FederatedDecisionSiteSummary,
    aggregate_glioma_federated_decision_context,
};
pub use mission_bridge::{
    DecisionMissionBridgeDisposition,
    DecisionMissionBridgeError,
    DecisionMissionBridgeRequest,
    DecisionMissionBridgeRun,
    execute_glioma_decision_mission,
};
pub use multi_study_context_artifact::{
    MultiStudyActionDisposition,
    MultiStudyActionOutcome,
    MultiStudyContextDisposition,
    MultiStudyContextError,
    MultiStudyContextInput,
    MultiStudyContextRequest,
    MultiStudyDecisionAction,
    MultiStudyDecisionContextArtifact,
    align_glioma_multi_study_context_artifacts,
};
pub use multi_study_epoch_replay::{
    MultiStudyActionEpochAssessment, MultiStudyActionEpochDisposition,
    MultiStudyActionTemporalAssessment, MultiStudyContextEpochReplay,
    MultiStudyContextEpochReplayDisposition, MultiStudyContextEpochReplayError,
    MultiStudyContextEpochReplayRequest, MultiStudyContextEpochTransition,
    MultiStudyTemporalActionDisposition, replay_glioma_multi_study_context_epochs,
};
pub use multi_study_execution_receipt::{
    MultiStudyExecutionActionOutcome, MultiStudyExecutionReceiptBinding,
    MultiStudyExecutionReceiptDisposition, MultiStudyExecutionReceiptError,
    MultiStudyExecutionReceiptReport, MultiStudyExecutionReceiptRequest,
    MultiStudyExecutionSourceSnapshot, MultiStudyExecutionStageSnapshot,
    MultiStudyExecutionTaskSnapshot, reconcile_glioma_multi_study_execution_receipts,
};
pub use multi_study_workflow::{
    MultiStudyStudyBudget,
    MultiStudyTaskDisposition,
    MultiStudyWorkflowDisposition,
    MultiStudyWorkflowError,
    MultiStudyWorkflowPlan,
    MultiStudyWorkflowRequest,
    MultiStudyWorkflowTask,
    plan_glioma_multi_study_workflow,
};
pub use omission_certificate::{
    DecisionCoverageState,
    DecisionOmissionCertificate,
    DecisionOmissionCertificateError,
    DecisionOmissionCertificateRequest,
    DecisionOmissionDisposition,
    DecisionOmissionEntry,
    certify_decision_omissions,
};
pub use partition_checkpoint::{
    reconcile_partition_resilient_context_checkpoint,
    CheckpointConflictPolicy,
    CheckpointFieldConflict,
    CheckpointNetworkState,
    CheckpointSiteDisposition,
    CheckpointSiteStatus,
    ContextCheckpointDelta,
    ContextCheckpointDisposition,
    ContextCheckpointField,
    ContextFieldDelta,
    ContextFieldValue,
    PartitionCheckpointError,
    PartitionResilientContextCheckpoint,
    PartitionResilientContextCheckpointRequest,
};
pub use snapshot_store::{
    store_glioma_decision_context_snapshots,
    DecisionContextSnapshotIndex,
    DecisionContextSnapshotInput,
    DecisionContextSnapshotRecord,
    DecisionContextSnapshotStoreError,
    DecisionContextSnapshotStoreRequest,
    SnapshotStoreDisposition,
};
pub use uncertainty_branch_explorer::{
    explore_glioma_uncertain_branches,
    DecisionBranchAnnotation,
    DecisionBranchComparison,
    DecisionBranchExplorerDisposition,
    DecisionBranchExplorerError,
    DecisionBranchExplorerStatus,
    UncertaintyBranchExplorer,
    UncertaintyBranchExplorerRequest,
};
pub use value_calibration::{
    DecisionValueCalibrationCampaignDisposition,
    DecisionValueCalibrationDisposition,
    DecisionValueCalibrationError,
    DecisionValueCalibrationRecord,
    DecisionValueCalibrationRequest,
    DecisionValueCalibrationResult,
    DecisionValueObservation,
    calibrate_glioma_decision_value,
};
pub use value_optimizer::{
    DecisionValueCampaignDisposition,
    DecisionValueCandidate,
    DecisionValueCandidateScore,
    DecisionValueDisposition,
    DecisionValueError,
    DecisionValuePortfolio,
    DecisionValueRequest,
    DecisionValueResult,
    DecisionValueWeights,
    optimize_glioma_decision_value,
};

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::DecisionContext;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P04")
}

// Merged vertical feature modules.
pub mod federated_decision_capsule;
