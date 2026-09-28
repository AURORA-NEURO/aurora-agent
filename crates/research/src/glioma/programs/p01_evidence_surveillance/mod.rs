//! Evidence surveillance program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod acquisition;
pub mod acquisition_campaign;
pub mod acquisition_feedback;
pub mod calibration;
pub mod campaign;
pub mod continual_promotion;
pub mod contradiction_cut;
pub mod evidence_cluster;
pub mod evidence_frontier_join;
pub mod evidence_knowledge_bridge;
pub mod evidence_stream;
pub mod federated_acquisition_policy;
pub mod federated_batch_scheduler;
pub mod federated_execution_handoff;
pub mod federated_operating_cycle;
pub mod federated_outcome_transport;
pub mod federated_shift;
pub mod long_horizon_calibration;
pub mod multimodal_gap_router;
pub mod multimodal_workbench;
pub mod novelty_adjudication;
pub mod novelty_radar;
pub mod operating_cycle;
pub mod outcome_reconciliation;
pub mod priority;
pub mod prospective_triage;
pub mod researcher_workbench;
pub mod surveillance;
pub mod temporal_shift;
pub mod triangulation;
pub mod verification_gate;

pub use novelty_radar::{
    EvidenceNoveltyAction, EvidenceNoveltyActionDisposition, EvidenceNoveltyRadar,
    EvidenceNoveltyRadarDisposition, EvidenceNoveltyRadarError, EvidenceNoveltyRadarRequest,
    EvidenceNoveltyRecord, rank_glioma_evidence_novelty,
};

pub use triangulation::{
    EvidenceTriangulation, EvidenceTriangulationDisposition, EvidenceTriangulationError,
    EvidenceTriangulationRequest, TriangulatedClaim, TriangulatedClaimVerdict,
    triangulate_glioma_evidence,
};

pub use acquisition_feedback::{
    AcquisitionFeedbackDecision, AcquisitionFeedbackError, AcquisitionFeedbackReport,
    AcquisitionFeedbackRequest, AcquisitionFeedbackRow, AcquisitionOutcome,
    AcquisitionOutcomeStatus, assimilate_glioma_acquisition_feedback,
};
pub use calibration::{
    CalibrationBin, CalibrationBinDisposition, EvidenceCalibrationAnalysis,
    EvidenceCalibrationDisposition, EvidenceCalibrationError, EvidenceCalibrationObservation,
    EvidenceCalibrationRequest, SourceCalibration, SourceCalibrationDisposition,
    calibrate_glioma_evidence,
};
pub use campaign::{
    DryRunEvidenceRefreshCampaignExecutor, EvidenceRefreshCampaign,
    EvidenceRefreshCampaignDisposition, EvidenceRefreshCampaignError,
    EvidenceRefreshCampaignExecutor, EvidenceRefreshCampaignRequest, EvidenceRefreshCampaignRound,
    EvidenceRefreshCampaignStopReason, EvidenceRefreshExecutionFailure,
    execute_glioma_evidence_refresh_campaign,
};
pub use continual_promotion::{
    ContinualOutcomeState, ContinualPromotionDecision, ContinualPromotionError,
    ContinualPromotionObservation, ContinualPromotionReport, ContinualPromotionRequest,
    ContinualPromotionStatus, ContinualPromotionWindow, PromotionWindowDisposition,
    evaluate_glioma_continual_promotion,
};
pub use contradiction_cut::{
    ContradictionAuditSelection, ContradictionConflict, ContradictionCut,
    ContradictionCutDisposition, ContradictionCutError, ContradictionCutRequest,
    ContradictionEvidence, EvidencePolarity, plan_glioma_evidence_contradiction_cut,
};
pub use evidence_cluster::{
    EvidenceCluster, EvidenceClusterDisposition, EvidenceClusterError, EvidenceClusterIndex,
    EvidenceClusterMember, EvidenceClusterRequest, EvidenceClusterVerdict, cluster_glioma_evidence,
};
pub use evidence_frontier_join::{
    EvidenceFrontierAction, EvidenceFrontierClaim, EvidenceFrontierJoin, EvidenceFrontierJoinError,
    EvidenceFrontierJoinRequest, EvidenceFrontierVerdict, join_glioma_evidence_frontier,
};
pub use evidence_knowledge_bridge::{
    EvidenceKnowledgeBridge, EvidenceKnowledgeBridgeDecision, EvidenceKnowledgeBridgeDisposition,
    EvidenceKnowledgeBridgeError, EvidenceKnowledgeBridgeOmission, EvidenceKnowledgeBridgeRequest,
    EvidenceKnowledgeClaimLink, bridge_glioma_evidence_to_knowledge,
};
pub use evidence_stream::{
    EvidenceStreamClaim, EvidenceStreamClaimTrend, EvidenceStreamDisposition, EvidenceStreamError,
    EvidenceStreamEvent, EvidenceStreamRequest, EvidenceStreamSnapshot,
    snapshot_glioma_evidence_stream,
};
pub use federated_batch_scheduler::{
    FederatedBatchActionDecision, FederatedBatchDecision, FederatedBatchDisposition,
    FederatedBatchSchedule, FederatedBatchSchedulerError, FederatedBatchSchedulerRequest,
    schedule_glioma_federated_evidence_batch,
};
pub use federated_execution_handoff::{
    FederatedExecutionHandoff, FederatedExecutionHandoffError, FederatedExecutionHandoffReport,
    FederatedExecutionHandoffRequest, FederatedHandoffApproval, HandoffDisposition, HandoffEffect,
    compile_federated_glioma_execution_handoff,
};
pub use federated_operating_cycle::{
    FederatedCycleAction, FederatedCycleActionKind, FederatedCycleDisposition,
    FederatedEvidenceOperatingCycle, FederatedEvidenceOperatingCycleError,
    FederatedEvidenceOperatingCycleRequest, compile_glioma_federated_evidence_operating_cycle,
};
pub use federated_outcome_transport::{
    FederatedOutcomeBundle, FederatedOutcomeBundleDecision, FederatedOutcomeBundleDecisionRecord,
    FederatedOutcomeTransportDisposition, FederatedOutcomeTransportError,
    FederatedOutcomeTransportReport, FederatedOutcomeTransportRequest,
    compile_glioma_federated_outcome_transport,
};
pub use long_horizon_calibration::{
    LongHorizonCalibrationAction, LongHorizonCalibrationAnalysis,
    LongHorizonCalibrationDisposition, LongHorizonCalibrationDrift, LongHorizonCalibrationError,
    LongHorizonCalibrationFamily, LongHorizonCalibrationObservation,
    LongHorizonCalibrationOmission, LongHorizonCalibrationRequest, LongHorizonCalibrationWindow,
    LongHorizonWindowDisposition, calibrate_glioma_evidence_long_horizon,
};
pub use multimodal_gap_router::{
    MultimodalGapAction, MultimodalGapActionKind, MultimodalGapClaim, MultimodalGapDisposition,
    MultimodalGapRouterError, MultimodalGapRouterPlan, MultimodalGapRouterRequest,
    route_glioma_multimodal_evidence_gaps,
};
pub use multimodal_workbench::{
    MultimodalWorkbenchDisposition, MultimodalWorkbenchError, MultimodalWorkbenchOmission,
    MultimodalWorkbenchPanel, MultimodalWorkbenchPlan, MultimodalWorkbenchRecord,
    MultimodalWorkbenchRequest, MultimodalWorkbenchSort, MultimodalWorkbenchStudySummary,
    query_glioma_multimodal_researcher_workbench,
};
pub use novelty_adjudication::{
    NoveltyAdjudication, NoveltyAdjudicationDisposition, NoveltyAdjudicationError,
    NoveltyAdjudicationItem, NoveltyAdjudicationRecord, NoveltyAdjudicationRequest,
    NoveltyAdjudicationVerdict, adjudicate_glioma_evidence_novelty,
};
pub use outcome_reconciliation::{
    MultiSiteOutcomeAction, MultiSiteOutcomeClaim, MultiSiteOutcomeDisposition,
    MultiSiteOutcomeObservation, MultiSiteOutcomeOmission, MultiSiteOutcomeReconciliation,
    MultiSiteOutcomeReconciliationError, MultiSiteOutcomeReconciliationRequest,
    MultiSiteOutcomeSiteSummary, MultiSiteOutcomeVerdict, reconcile_glioma_multisite_outcomes,
};

pub use acquisition::{
    EvidenceAcquisitionCandidate, EvidenceAcquisitionDisposition, EvidenceAcquisitionError,
    EvidenceAcquisitionPlan, EvidenceAcquisitionRequest, EvidenceAcquisitionSelection,
    EvidenceAcquisitionSourceKind, EvidenceAcquisitionWeights, plan_glioma_evidence_acquisition,
};
pub use acquisition_campaign::{
    DryRunEvidenceAcquisitionExecutor, EvidenceAcquisitionCampaign,
    EvidenceAcquisitionCampaignDisposition, EvidenceAcquisitionCampaignError,
    EvidenceAcquisitionCampaignRequest, EvidenceAcquisitionCampaignStopReason,
    EvidenceAcquisitionExecutionFailure, EvidenceAcquisitionExecutor, EvidenceAcquisitionResult,
    EvidenceAcquisitionResultDisposition, execute_glioma_evidence_acquisition_campaign,
};
pub use federated_acquisition_policy::{
    FederatedAcquisitionAction, FederatedAcquisitionDecision, FederatedAcquisitionKind,
    FederatedAcquisitionPolicyError, FederatedAcquisitionPolicyRequest, FederatedAcquisitionSite,
    FederatedEvidenceAcquisitionPolicy, FederatedEvidenceNeed, FederatedNeedDecision,
    plan_federated_glioma_evidence_acquisition,
};
pub use federated_shift::{
    FederatedEvidenceShift, FederatedEvidenceShiftAction, FederatedEvidenceShiftDisposition,
    FederatedEvidenceShiftError, FederatedEvidenceShiftKind, FederatedEvidenceShiftRequest,
    FederatedEvidenceShiftSite, analyze_federated_evidence_shifts,
};
pub use operating_cycle::{
    EvidenceExecutionMode, GliomaEvidenceOperatingCycle, GliomaEvidenceOperatingCycleDisposition,
    GliomaEvidenceOperatingCycleError, GliomaEvidenceOperatingCycleRequest,
    execute_glioma_evidence_operating_cycle, execute_glioma_evidence_operating_cycle_dry_run,
};
pub use priority::{
    EvidencePriorityAction, EvidencePriorityActionKind, EvidencePriorityDisposition,
    EvidencePriorityError, EvidencePriorityPlan, EvidencePriorityRequest, EvidencePriorityWeights,
    prioritize_glioma_evidence,
};
pub use prospective_triage::{
    EvidenceProspectiveTriageError, EvidenceProspectiveTriagePlan, EvidenceProspectiveTriagePolicy,
    EvidenceProspectiveTriageRequest, EvidenceTriageActionKind, EvidenceTriageDecision,
    EvidenceTriageDisposition, EvidenceTriageReviewObservation, EvidenceTriageReviewState,
    EvidenceTriageReviewerCapacity, EvidenceTriageReviewerLoad, EvidenceTriageStatus,
    plan_glioma_prospective_evidence_triage,
};
pub use researcher_workbench::{
    EvidenceWorkbenchDisposition, EvidenceWorkbenchError, EvidenceWorkbenchHit,
    EvidenceWorkbenchOmission, EvidenceWorkbenchPlan, EvidenceWorkbenchRequest,
    EvidenceWorkbenchSort, query_glioma_evidence_workbench,
};
pub use surveillance::{
    EvidenceChange, EvidenceChangeKind, EvidenceSurveillance, EvidenceSurveillanceAction,
    EvidenceSurveillanceActionKind, EvidenceSurveillanceDisposition, EvidenceSurveillanceError,
    EvidenceSurveillanceRequest, surveil_glioma_evidence,
};
pub use temporal_shift::{
    EvidenceTemporalObservation, EvidenceTemporalShift, EvidenceTemporalShiftAction,
    EvidenceTemporalShiftDisposition, EvidenceTemporalShiftError, EvidenceTemporalShiftKind,
    EvidenceTemporalShiftRequest, detect_glioma_evidence_temporal_shifts,
};
pub use verification_gate::{
    EvidenceVerificationDisposition, EvidenceVerificationError, EvidenceVerificationFinding,
    EvidenceVerificationOmission, EvidenceVerificationReport, EvidenceVerificationRequest,
    EvidenceVerificationSeverity, verify_glioma_evidence,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceSurveillance;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P01")
}
