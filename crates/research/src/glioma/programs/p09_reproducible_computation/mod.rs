//! Reproducible computation program ownership.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub mod campaign;
pub mod execution;
pub mod interpretation_frontier;
pub mod lineage;
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
    RobustnessCase, RobustnessCaseKind, RobustnessDisposition, RobustnessError, RobustnessRequest,
    RobustnessSuite, assess_glioma_robustness,
};

pub use robustness_guided::{
    RobustnessGuidedCandidate, RobustnessGuidedCandidateScore, RobustnessGuidedComputation,
    RobustnessGuidedComputationDisposition, RobustnessGuidedComputationError,
    RobustnessGuidedComputationRequest, dry_run_robustness_guided_computation_executor,
    execute_glioma_robustness_guided_computation,
};

pub use recovery_campaign::{
    ComputationRecoveryCampaign, ComputationRecoveryDisposition, ComputationRecoveryError,
    ComputationRecoveryRequest, ComputationRecoveryStopReason, execute_glioma_computation_recovery,
};

pub use reproducibility::{
    ComputationReproducibility, ComputationReproducibilityDisposition,
    ComputationReproducibilityError, ComputationReproducibilityRequest,
    ComputationReproducibilityRun, ComputationReproducibilityTaskObservation,
    ComputationRunOutcome, ComputationTaskReproducibilityDisposition,
    ComputationTaskReproducibilitySummary, analyze_glioma_computation_reproducibility,
};

pub use execution::{
    ComputationCacheEntry, ComputationExecution, ComputationExecutionDisposition,
    ComputationExecutionError, ComputationExecutionFailure, ComputationExecutionRequest,
    ComputationExecutionStopReason, ComputationOperation, ComputationTask,
    ComputationTaskDisposition, ComputationTaskResult, DryRunGliomaComputationExecutor,
    GliomaComputationExecutor, execute_glioma_computation,
};

pub use interpretation_frontier::{
    ComputationInterpretationFrontier, ComputationInterpretationFrontierDisposition,
    ComputationInterpretationFrontierError, ComputationInterpretationFrontierRequest,
    ComputationInterpretationFrontierRun, compile_glioma_computation_interpretation_frontier,
    execute_glioma_computation_interpretation_frontier,
};

pub use lineage::{
    ComputationLineage, ComputationLineageDisposition, ComputationLineageError,
    ComputationLineageNode, ComputationLineageNodeStatus, ComputationLineageRequest,
    join_glioma_computation_lineage,
};

pub use campaign::{
    GliomaComputationCampaign, GliomaComputationCampaignDisposition,
    GliomaComputationCampaignError, GliomaComputationCampaignRequest,
    GliomaComputationCampaignRound, GliomaComputationCampaignStopReason, GliomaComputationPlanner,
    GliomaComputationPlannerContext, GliomaComputationPlannerFailure,
    StaticGliomaComputationPlanner, execute_glioma_computation_campaign,
};

pub use planning::{
    ComputationCandidate, ComputationCandidateDisposition, ComputationCandidateScore,
    ComputationPortfolioDisposition, ComputationPortfolioError, ComputationPortfolioPlan,
    ComputationPortfolioRequest, plan_glioma_computation_portfolio,
};

pub use portfolio_execution::{
    ComputationPortfolioExecution, ComputationPortfolioExecutionDisposition,
    ComputationPortfolioExecutionError, ComputationPortfolioExecutionRequest,
    execute_glioma_computation_portfolio,
};

pub use placement::{
    ComputationPlacementAssignment, ComputationPlacementBlockedTask,
    ComputationPlacementDisposition, ComputationPlacementError, ComputationPlacementRequest,
    ComputationPlacementSchedule, ComputationWorkerProfile, ComputationWorkerUtilization,
    schedule_glioma_computation_placement,
};

pub use workflow::{
    GliomaComputationWorkflow, GliomaComputationWorkflowError, GliomaComputationWorkflowRequest,
    compile_glioma_computation_workflow,
};

pub use operating_cycle::{
    ComputationExecutionMode, GliomaComputationOperatingCycle,
    GliomaComputationOperatingCycleDisposition, GliomaComputationOperatingCycleError,
    GliomaComputationOperatingCycleRequest, execute_glioma_computation_operating_cycle,
    execute_glioma_computation_operating_cycle_dry_run,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ReproducibleComputation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P09")
}
