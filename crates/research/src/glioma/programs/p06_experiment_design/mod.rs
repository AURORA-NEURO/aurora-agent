//! Power-aware experiment-design program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod adaptive_allocation;
pub mod campaign;
pub mod dose_response;
pub mod synergy;

pub use adaptive_allocation::{
    allocate_glioma_assays, AdaptiveAllocation, AdaptiveAllocationActionKind,
    AdaptiveAllocationDisposition, AdaptiveAllocationError, AdaptiveAllocationRequest,
    AdaptiveArmObservation, AdaptiveArmPosterior,
};
pub use campaign::{
    plan_glioma_closed_loop_campaign, CampaignAction, CampaignActionScore, CampaignMechanism,
    CampaignMechanismPosterior, CampaignObservation, CampaignRound, CampaignStopReason,
    ClosedLoopCampaign, ClosedLoopCampaignDisposition, ClosedLoopCampaignError,
    ClosedLoopCampaignRequest,
};
pub use dose_response::{
    analyze_glioma_dose_response, DoseDirection, DoseResponseAnalysis, DoseResponseDisposition,
    DoseResponseError, DoseResponseObservation, DoseResponsePoint, DoseResponseRequest,
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
