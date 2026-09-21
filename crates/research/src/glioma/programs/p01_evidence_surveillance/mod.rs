//! Evidence surveillance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod acquisition;
pub mod acquisition_campaign;
pub mod acquisition_feedback;
pub mod calibration;
pub mod campaign;
pub mod contradiction_cut;
pub mod evidence_cluster;
pub mod evidence_frontier_join;
pub mod evidence_stream;
pub mod federated_acquisition_policy;
pub mod federated_execution_handoff;
pub mod federated_shift;
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
    rank_glioma_evidence_novelty, EvidenceNoveltyAction, EvidenceNoveltyActionDisposition,
    EvidenceNoveltyRadar, EvidenceNoveltyRadarDisposition, EvidenceNoveltyRadarError,
    EvidenceNoveltyRadarRequest, EvidenceNoveltyRecord,
};

pub use triangulation::{
    triangulate_glioma_evidence, EvidenceTriangulation, EvidenceTriangulationDisposition,
    EvidenceTriangulationError, EvidenceTriangulationRequest, TriangulatedClaim,
    TriangulatedClaimVerdict,
};

pub use acquisition_feedback::{
    assimilate_glioma_acquisition_feedback, AcquisitionFeedbackDecision, AcquisitionFeedbackError,
    AcquisitionFeedbackReport, AcquisitionFeedbackRequest, AcquisitionFeedbackRow,
    AcquisitionOutcome, AcquisitionOutcomeStatus,
};
pub use calibration::{
    calibrate_glioma_evidence, CalibrationBin, CalibrationBinDisposition,
    EvidenceCalibrationAnalysis, EvidenceCalibrationDisposition, EvidenceCalibrationError,
    EvidenceCalibrationObservation, EvidenceCalibrationRequest, SourceCalibration,
    SourceCalibrationDisposition,
};
pub use campaign::{
    execute_glioma_evidence_refresh_campaign, DryRunEvidenceRefreshCampaignExecutor,
    EvidenceRefreshCampaign, EvidenceRefreshCampaignDisposition, EvidenceRefreshCampaignError,
    EvidenceRefreshCampaignExecutor, EvidenceRefreshCampaignRequest, EvidenceRefreshCampaignRound,
    EvidenceRefreshCampaignStopReason, EvidenceRefreshExecutionFailure,
};
pub use contradiction_cut::{
    plan_glioma_evidence_contradiction_cut, ContradictionAuditSelection, ContradictionConflict,
    ContradictionCut, ContradictionCutDisposition, ContradictionCutError, ContradictionCutRequest,
    ContradictionEvidence, EvidencePolarity,
};
pub use evidence_cluster::{
    cluster_glioma_evidence, EvidenceCluster, EvidenceClusterDisposition, EvidenceClusterError,
    EvidenceClusterIndex, EvidenceClusterMember, EvidenceClusterRequest, EvidenceClusterVerdict,
};
pub use evidence_frontier_join::{
    join_glioma_evidence_frontier, EvidenceFrontierAction, EvidenceFrontierClaim,
    EvidenceFrontierJoin, EvidenceFrontierJoinError, EvidenceFrontierJoinRequest,
    EvidenceFrontierVerdict,
};
pub use evidence_stream::{
    snapshot_glioma_evidence_stream, EvidenceStreamClaim, EvidenceStreamClaimTrend,
    EvidenceStreamDisposition, EvidenceStreamError, EvidenceStreamEvent, EvidenceStreamRequest,
    EvidenceStreamSnapshot,
};
pub use federated_execution_handoff::{
    compile_federated_glioma_execution_handoff, FederatedExecutionHandoff,
    FederatedExecutionHandoffError, FederatedExecutionHandoffReport,
    FederatedExecutionHandoffRequest, FederatedHandoffApproval, HandoffDisposition, HandoffEffect,
};
pub use multimodal_gap_router::{
    route_glioma_multimodal_evidence_gaps, MultimodalGapAction, MultimodalGapActionKind,
    MultimodalGapClaim, MultimodalGapDisposition, MultimodalGapRouterError,
    MultimodalGapRouterPlan, MultimodalGapRouterRequest,
};
pub use multimodal_workbench::{
    query_glioma_multimodal_researcher_workbench, MultimodalWorkbenchDisposition,
    MultimodalWorkbenchError, MultimodalWorkbenchOmission, MultimodalWorkbenchPanel,
    MultimodalWorkbenchPlan, MultimodalWorkbenchRecord, MultimodalWorkbenchRequest,
    MultimodalWorkbenchSort, MultimodalWorkbenchStudySummary,
};
pub use novelty_adjudication::{
    adjudicate_glioma_evidence_novelty, NoveltyAdjudication, NoveltyAdjudicationDisposition,
    NoveltyAdjudicationError, NoveltyAdjudicationItem, NoveltyAdjudicationRecord,
    NoveltyAdjudicationRequest, NoveltyAdjudicationVerdict,
};
pub use outcome_reconciliation::{
    reconcile_glioma_multisite_outcomes, MultiSiteOutcomeAction, MultiSiteOutcomeClaim,
    MultiSiteOutcomeDisposition, MultiSiteOutcomeObservation, MultiSiteOutcomeOmission,
    MultiSiteOutcomeReconciliation, MultiSiteOutcomeReconciliationError,
    MultiSiteOutcomeReconciliationRequest, MultiSiteOutcomeSiteSummary, MultiSiteOutcomeVerdict,
};

pub use acquisition::{
    plan_glioma_evidence_acquisition, EvidenceAcquisitionCandidate, EvidenceAcquisitionDisposition,
    EvidenceAcquisitionError, EvidenceAcquisitionPlan, EvidenceAcquisitionRequest,
    EvidenceAcquisitionSelection, EvidenceAcquisitionSourceKind, EvidenceAcquisitionWeights,
};
pub use acquisition_campaign::{
    execute_glioma_evidence_acquisition_campaign, DryRunEvidenceAcquisitionExecutor,
    EvidenceAcquisitionCampaign, EvidenceAcquisitionCampaignDisposition,
    EvidenceAcquisitionCampaignError, EvidenceAcquisitionCampaignRequest,
    EvidenceAcquisitionCampaignStopReason, EvidenceAcquisitionExecutionFailure,
    EvidenceAcquisitionExecutor, EvidenceAcquisitionResult, EvidenceAcquisitionResultDisposition,
};
pub use federated_acquisition_policy::{
    plan_federated_glioma_evidence_acquisition, FederatedAcquisitionAction,
    FederatedAcquisitionDecision, FederatedAcquisitionKind, FederatedAcquisitionPolicyError,
    FederatedAcquisitionPolicyRequest, FederatedAcquisitionSite,
    FederatedEvidenceAcquisitionPolicy, FederatedEvidenceNeed, FederatedNeedDecision,
};
pub use federated_shift::{
    analyze_federated_evidence_shifts, FederatedEvidenceShift, FederatedEvidenceShiftAction,
    FederatedEvidenceShiftDisposition, FederatedEvidenceShiftError, FederatedEvidenceShiftKind,
    FederatedEvidenceShiftRequest, FederatedEvidenceShiftSite,
};
pub use operating_cycle::{
    execute_glioma_evidence_operating_cycle, execute_glioma_evidence_operating_cycle_dry_run,
    EvidenceExecutionMode, GliomaEvidenceOperatingCycle, GliomaEvidenceOperatingCycleDisposition,
    GliomaEvidenceOperatingCycleError, GliomaEvidenceOperatingCycleRequest,
};
pub use priority::{
    prioritize_glioma_evidence, EvidencePriorityAction, EvidencePriorityActionKind,
    EvidencePriorityDisposition, EvidencePriorityError, EvidencePriorityPlan,
    EvidencePriorityRequest, EvidencePriorityWeights,
};
pub use prospective_triage::{
    plan_glioma_prospective_evidence_triage, EvidenceProspectiveTriageError,
    EvidenceProspectiveTriagePlan, EvidenceProspectiveTriagePolicy,
    EvidenceProspectiveTriageRequest, EvidenceTriageActionKind, EvidenceTriageDecision,
    EvidenceTriageDisposition, EvidenceTriageReviewObservation, EvidenceTriageReviewState,
    EvidenceTriageReviewerCapacity, EvidenceTriageReviewerLoad, EvidenceTriageStatus,
};
pub use researcher_workbench::{
    query_glioma_evidence_workbench, EvidenceWorkbenchDisposition, EvidenceWorkbenchError,
    EvidenceWorkbenchHit, EvidenceWorkbenchOmission, EvidenceWorkbenchPlan,
    EvidenceWorkbenchRequest, EvidenceWorkbenchSort,
};
pub use surveillance::{
    surveil_glioma_evidence, EvidenceChange, EvidenceChangeKind, EvidenceSurveillance,
    EvidenceSurveillanceAction, EvidenceSurveillanceActionKind, EvidenceSurveillanceDisposition,
    EvidenceSurveillanceError, EvidenceSurveillanceRequest,
};
pub use temporal_shift::{
    detect_glioma_evidence_temporal_shifts, EvidenceTemporalObservation, EvidenceTemporalShift,
    EvidenceTemporalShiftAction, EvidenceTemporalShiftDisposition, EvidenceTemporalShiftError,
    EvidenceTemporalShiftKind, EvidenceTemporalShiftRequest,
};
pub use verification_gate::{
    verify_glioma_evidence, EvidenceVerificationDisposition, EvidenceVerificationError,
    EvidenceVerificationFinding, EvidenceVerificationOmission, EvidenceVerificationReport,
    EvidenceVerificationRequest, EvidenceVerificationSeverity,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceSurveillance;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P01")
}
