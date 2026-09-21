//! Evidence surveillance program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod acquisition;
pub mod acquisition_campaign;
pub mod calibration;
pub mod campaign;
pub mod contradiction_cut;
pub mod federated_shift;
pub mod novelty_radar;
pub mod operating_cycle;
pub mod priority;
pub mod surveillance;
pub mod temporal_shift;
pub mod triangulation;

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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceSurveillance;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P01")
}
