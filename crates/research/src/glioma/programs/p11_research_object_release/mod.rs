//! Research-object release program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod replay;

pub use replay::{
    execute_glioma_replay_campaign, DryRunReplayCampaignExecutor, ReplayCampaign,
    ReplayCampaignDisposition, ReplayCampaignError, ReplayCampaignExecutor, ReplayCampaignRequest,
    ReplayCampaignRound, ReplayCampaignStopReason, ReplayExecutionFailure, ReplayObservation,
    ReplayObservationStatus, ReplayTask,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ResearchObjectRelease;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P11")
}
