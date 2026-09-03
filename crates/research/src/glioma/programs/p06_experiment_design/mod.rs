//! Power-aware experiment-design program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod active_learning;
pub mod adaptive_allocation;
pub mod adaptive_information_campaign;
pub mod campaign;
pub mod dose_response;
pub mod information_design;
pub mod multi_fidelity;
pub mod synergy;

pub use active_learning::{
    plan_glioma_active_learning, ActiveLearningCandidate, ActiveLearningCandidateDisposition,
    ActiveLearningDirection, ActiveLearningDisposition, ActiveLearningError,
    ActiveLearningObservation, ActiveLearningPlan, ActiveLearningRequest, ActiveLearningScore,
};
pub use adaptive_allocation::{
    allocate_glioma_assays, AdaptiveAllocation, AdaptiveAllocationActionKind,
    AdaptiveAllocationDisposition, AdaptiveAllocationError, AdaptiveAllocationRequest,
    AdaptiveArmObservation, AdaptiveArmPosterior,
};
pub use adaptive_information_campaign::{
    execute_glioma_adaptive_information_campaign, plan_glioma_adaptive_information_campaign,
    AdaptiveInformationCampaignDisposition, AdaptiveInformationCampaignError,
    AdaptiveInformationCampaignExecution, AdaptiveInformationCampaignPlan,
    AdaptiveInformationCampaignRequest, AdaptiveInformationCampaignRound,
    AdaptiveInformationCampaignTermination, AdaptiveInformationExecutionFailure,
    AdaptiveInformationObservation, AdaptiveMechanismPosterior, GliomaInformationDesignExecutor,
};
pub use campaign::{
    execute_glioma_closed_loop_campaign, plan_glioma_closed_loop_campaign, CampaignAction,
    CampaignActionScore, CampaignExecutionFailure, CampaignExecutionRound, CampaignMechanism,
    CampaignMechanismPosterior, CampaignObservation, CampaignRound, CampaignStopReason,
    ClosedLoopCampaign, ClosedLoopCampaignDisposition, ClosedLoopCampaignError,
    ClosedLoopCampaignExecution, ClosedLoopCampaignRequest, GliomaCampaignExecutor,
    EXECUTION_OUTPUT_SCHEMA,
};
pub use dose_response::{
    analyze_glioma_dose_response, DoseDirection, DoseResponseAnalysis, DoseResponseDisposition,
    DoseResponseError, DoseResponseObservation, DoseResponsePoint, DoseResponseRequest,
};
pub use information_design::{
    plan_glioma_information_design, DesignAction, DesignMechanism, DesignOutcome,
    InformationDesignActionScore, InformationDesignDisposition, InformationDesignError,
    InformationDesignPlan, InformationDesignRequest,
};
pub use multi_fidelity::{
    plan_glioma_multi_fidelity_optimization, EstimateSource, FidelityCalibration,
    FidelityCandidate, FidelityEstimate, FidelityLevel, FidelityObservation,
    MultiFidelityDisposition, MultiFidelityOptimizationError, MultiFidelityOptimizationPlan,
    MultiFidelityOptimizationRequest, OptimizationDirection,
};
pub use synergy::{
    analyze_glioma_combination_synergy, CombinationCell, CombinationCellDisposition,
    CombinationObservation, CombinationSynergyAnalysis, CombinationSynergyDisposition,
    CombinationSynergyError, CombinationSynergyRequest, DosePair,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ExperimentDesign;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P06")
}
