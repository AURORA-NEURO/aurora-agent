//! Reproducible computation program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod execution;
pub mod campaign;
pub mod planning;
pub mod portfolio_execution;
pub mod robustness;

pub use robustness::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};

pub use execution::{
    execute_glioma_computation, ComputationCacheEntry, ComputationExecution,
    ComputationExecutionDisposition, ComputationExecutionError, ComputationExecutionFailure,
    ComputationExecutionRequest, ComputationExecutionStopReason, ComputationOperation,
    ComputationTask, ComputationTaskDisposition, ComputationTaskResult,
    DryRunGliomaComputationExecutor, GliomaComputationExecutor,
};

pub use campaign::{
    execute_glioma_computation_campaign, GliomaComputationCampaign,
    GliomaComputationCampaignDisposition, GliomaComputationCampaignError,
    GliomaComputationCampaignRequest, GliomaComputationCampaignRound,
    GliomaComputationCampaignStopReason, GliomaComputationPlanner,
    GliomaComputationPlannerContext, GliomaComputationPlannerFailure,
    StaticGliomaComputationPlanner,
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

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ReproducibleComputation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P09")
}
