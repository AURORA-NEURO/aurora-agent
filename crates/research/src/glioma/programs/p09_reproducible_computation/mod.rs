//! Reproducible computation program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod campaign;
pub mod execution;
pub mod interpretation_frontier;
pub mod operating_cycle;
pub mod placement;
pub mod planning;
pub mod portfolio_execution;
pub mod recovery_campaign;
pub mod reproducibility;
pub mod robustness;
pub mod robustness_guided;
pub mod workflow;

pub use robustness::{
    assess_glioma_robustness, RobustnessCase, RobustnessCaseKind, RobustnessDisposition,
    RobustnessError, RobustnessRequest, RobustnessSuite,
};

pub use robustness_guided::{
    dry_run_robustness_guided_computation_executor, execute_glioma_robustness_guided_computation,
    RobustnessGuidedCandidate, RobustnessGuidedCandidateScore, RobustnessGuidedComputation,
    RobustnessGuidedComputationDisposition, RobustnessGuidedComputationError,
    RobustnessGuidedComputationRequest,
};

pub use recovery_campaign::{
    execute_glioma_computation_recovery, ComputationRecoveryCampaign,
    ComputationRecoveryDisposition, ComputationRecoveryError, ComputationRecoveryRequest,
    ComputationRecoveryStopReason,
};

pub use reproducibility::{
    analyze_glioma_computation_reproducibility, ComputationReproducibility,
    ComputationReproducibilityDisposition, ComputationReproducibilityError,
    ComputationReproducibilityRequest, ComputationReproducibilityRun,
    ComputationReproducibilityTaskObservation, ComputationRunOutcome,
    ComputationTaskReproducibilityDisposition, ComputationTaskReproducibilitySummary,
};

pub use execution::{
    execute_glioma_computation, ComputationCacheEntry, ComputationExecution,
    ComputationExecutionDisposition, ComputationExecutionError, ComputationExecutionFailure,
    ComputationExecutionRequest, ComputationExecutionStopReason, ComputationOperation,
    ComputationTask, ComputationTaskDisposition, ComputationTaskResult,
    DryRunGliomaComputationExecutor, GliomaComputationExecutor,
};

pub use interpretation_frontier::{
    compile_glioma_computation_interpretation_frontier,
    execute_glioma_computation_interpretation_frontier, ComputationInterpretationFrontier,
    ComputationInterpretationFrontierDisposition, ComputationInterpretationFrontierError,
    ComputationInterpretationFrontierRequest, ComputationInterpretationFrontierRun,
};

pub use campaign::{
    execute_glioma_computation_campaign, GliomaComputationCampaign,
    GliomaComputationCampaignDisposition, GliomaComputationCampaignError,
    GliomaComputationCampaignRequest, GliomaComputationCampaignRound,
    GliomaComputationCampaignStopReason, GliomaComputationPlanner, GliomaComputationPlannerContext,
    GliomaComputationPlannerFailure, StaticGliomaComputationPlanner,
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

pub use placement::{
    schedule_glioma_computation_placement, ComputationPlacementAssignment,
    ComputationPlacementBlockedTask, ComputationPlacementDisposition, ComputationPlacementError,
    ComputationPlacementRequest, ComputationPlacementSchedule, ComputationWorkerProfile,
    ComputationWorkerUtilization,
};

pub use workflow::{
    compile_glioma_computation_workflow, GliomaComputationWorkflow, GliomaComputationWorkflowError,
    GliomaComputationWorkflowRequest,
};

pub use operating_cycle::{
    execute_glioma_computation_operating_cycle, execute_glioma_computation_operating_cycle_dry_run,
    ComputationExecutionMode, GliomaComputationOperatingCycle,
    GliomaComputationOperatingCycleDisposition, GliomaComputationOperatingCycleError,
    GliomaComputationOperatingCycleRequest,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ReproducibleComputation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P09")
}
