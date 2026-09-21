//! Research-object release program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod migration;
pub mod multimodal_bundle;
pub mod operating_cycle;
pub mod release_gate;
pub mod replay;

pub use replay::{
    execute_glioma_replay_campaign, DryRunReplayCampaignExecutor, ReplayCampaign,
    ReplayCampaignDisposition, ReplayCampaignError, ReplayCampaignExecutor, ReplayCampaignRequest,
    ReplayCampaignRound, ReplayCampaignStopReason, ReplayExecutionFailure, ReplayObservation,
    ReplayObservationStatus, ReplayTask,
};

pub use release_gate::{
    evaluate_glioma_release_gate, ReleaseGateError, ReleaseGateEvaluation, ReleaseGateRequest,
    ReleaseGateStatus, ReleaseReviewAttestation, ReleaseReviewDecision,
};

pub use migration::{
    plan_glioma_research_object_migration, ResearchObjectMigrationAction,
    ResearchObjectMigrationDecision, ResearchObjectMigrationDisposition,
    ResearchObjectMigrationError, ResearchObjectMigrationPlan, ResearchObjectMigrationRequest,
};
pub use multimodal_bundle::{
    compile_glioma_multimodal_research_object, MultimodalResearchObjectBundle,
    MultimodalResearchObjectDisposition, MultimodalResearchObjectEntry,
    MultimodalResearchObjectError, MultimodalResearchObjectInput, MultimodalResearchObjectRequest,
};

pub use operating_cycle::{
    execute_glioma_release_operating_cycle, execute_glioma_release_operating_cycle_dry_run,
    GliomaReleaseOperatingCycle, GliomaReleaseOperatingCycleDisposition,
    GliomaReleaseOperatingCycleError, GliomaReleaseOperatingCycleRequest, ReleaseExecutionMode,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ResearchObjectRelease;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P11")
}
