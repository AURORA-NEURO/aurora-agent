//! Causal interpretation and replication program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod adaptive_frontier;
pub mod campaign;
pub mod causal_adjustment;
pub mod causal_contrast;
pub mod clone_outcomes;
pub mod mediation;
pub mod meta_analysis;
pub mod sensitivity;
pub mod state_transition;
pub mod synthesis;
pub mod trajectory;
pub mod transportability;

pub use adaptive_frontier::{
    plan_glioma_adaptive_research_frontier, AdaptiveFrontierCandidate, AdaptiveFrontierDisposition,
    AdaptiveFrontierError, AdaptiveFrontierRequest, AdaptiveResearchFrontier, AdaptiveTarget,
};
pub use campaign::{
    execute_glioma_replication_campaign, DryRunGliomaReplicationCampaignExecutor,
    GliomaReplicationAction, GliomaReplicationActionKind, GliomaReplicationCampaign,
    GliomaReplicationCampaignDisposition, GliomaReplicationCampaignError,
    GliomaReplicationCampaignExecutor, GliomaReplicationCampaignObservation,
    GliomaReplicationCampaignRequest, GliomaReplicationCampaignRound,
    GliomaReplicationCampaignStopReason, GliomaReplicationExecutionFailure,
};
pub use causal_adjustment::{
    analyze_stratified_causal_adjustment, CausalStratumSummary, StratifiedCausalActionKind,
    StratifiedCausalAdjustment, StratifiedCausalDisposition, StratifiedCausalError,
    StratifiedCausalRequest, StratifiedObservation,
};
pub use causal_contrast::{
    analyze_glioma_causal_contrast, CausalContrastAnalysis, CausalContrastDisposition,
    CausalContrastError, CausalContrastRequest, UnitContrast,
};
pub use clone_outcomes::{
    analyze_glioma_clone_panel_outcomes, ClonePanelBranchAnalysis, ClonePanelCandidateAnalysis,
    ClonePanelCellAnalysis, ClonePanelCellDisposition, ClonePanelMeasurementState,
    ClonePanelObservation, ClonePanelOutcomeAnalysis, ClonePanelOutcomeDisposition,
    ClonePanelOutcomeError, ClonePanelOutcomeRequest,
};
pub use mediation::{
    analyze_glioma_mediation, MediationAnalysis, MediationDisposition, MediationError,
    MediationObservation, MediationRequest,
};

pub use meta_analysis::{
    analyze_replication_meta_analysis, MetaAnalysisDisposition, MetaAnalysisError,
    MetaAnalysisRequest, MetaStudyContribution, ReplicationMetaAnalysis,
};
pub use sensitivity::{
    analyze_causal_sensitivity, CausalSensitivityAnalysis, SensitivityDirection,
    SensitivityDisposition, SensitivityError, SensitivityObservation, SensitivityPoint,
    SensitivityRequest,
};
pub use state_transition::{
    analyze_glioma_state_transitions, StateTransitionAnalysis, StateTransitionCell,
    StateTransitionContrast, StateTransitionDisposition, StateTransitionError,
    StateTransitionObservation, StateTransitionRequest, TransitionCellDisposition,
    TransitionContrastDisposition, TransitionDirection,
};

pub use synthesis::{
    synthesize_glioma_interpretation, InterpretationEvidence, InterpretationEvidenceDirection,
    InterpretationEvidenceFamily, InterpretationFamilySummary, InterpretationSynthesis,
    InterpretationSynthesisDisposition, InterpretationSynthesisError,
    InterpretationSynthesisRequest,
};

pub use trajectory::{
    analyze_glioma_trajectories, TrajectoryAnalysis, TrajectoryArmSummary, TrajectoryDisposition,
    TrajectoryError, TrajectoryObservation, TrajectoryRequest, UnitTrajectory,
    UnitTrajectoryDisposition,
};
pub use transportability::{
    analyze_glioma_transportability, TransportStudy, TransportStudyContribution,
    TransportabilityAnalysis, TransportabilityDisposition, TransportabilityError,
    TransportabilityRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InterpretationReplication;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P10")
}
