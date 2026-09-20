//! Protocol simulation and adaptive workflow program ownership.
//!
//! The workflow planner is re-exported here so callers can discover the P07 surface through the
//! folder-owned program module while the shared glioma namespace retains a stable API.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub use crate::glioma::workflow::{
    execute_glioma_workflow, plan_glioma_workflow, GliomaWorkflowBranch, GliomaWorkflowError,
    GliomaWorkflowExecution, GliomaWorkflowMode, GliomaWorkflowNode, GliomaWorkflowPlan,
    GliomaWorkflowRequest, WorkflowNodeDecision,
};
pub mod action_execution;
pub mod active_learning_campaign;
pub mod adaptive_scheduler;
pub mod autonomous_campaign;
pub mod autonomous_engine;
pub mod autonomous_protocol;
pub mod branch_optimizer;
pub mod clone_campaign;
pub mod clone_continuation;
pub mod compensation;
pub mod director;
pub mod evidence_campaign;
pub mod evidence_gate;
pub mod evidence_surface;
pub mod execution;
pub mod frontier_execution;
pub mod intent_mission;
pub mod mechanism_autopilot;
pub mod mechanism_campaign;
pub mod mechanism_discovery_engine;
pub mod mission;
pub mod mission_recovery;
pub mod multimodal_mission;
pub mod multistudy_fusion;
pub mod program_cycle;
pub mod program_scheduler;
pub mod research_autopilot;
pub mod robust_active_learning_campaign;
pub mod scientific_frontier;
pub mod simulator;

pub use action_execution::{
    execute_glioma_action_portfolio, ActionExecutionDisposition, ActionExecutionFailure,
    ActionExecutionResult, ActionPortfolioExecution, ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, ActionPortfolioStopReason,
    DryRunGliomaActionExecutor, GliomaActionExecutor,
};

pub use adaptive_scheduler::{
    plan_glioma_adaptive_workflow, GliomaAdaptiveWorkflowSchedulerDisposition,
    GliomaAdaptiveWorkflowSchedulerError, GliomaAdaptiveWorkflowSchedulerPlan,
    GliomaAdaptiveWorkflowSchedulerRequest, SchedulerDecision, SchedulerObservation,
    SchedulerOutcome,
};

pub use active_learning_campaign::{
    execute_glioma_active_learning_campaign, ActiveLearningCampaign,
    ActiveLearningCampaignDisposition, ActiveLearningCampaignError, ActiveLearningCampaignExecutor,
    ActiveLearningCampaignRequest, ActiveLearningCampaignRound, ActiveLearningCampaignStopReason,
    ActiveLearningExecutionFailure, DryRunActiveLearningCampaignExecutor,
};

pub use autonomous_engine::{
    execute_glioma_autonomous_research_engine, GliomaAutonomousResearchEngineCycle,
    GliomaAutonomousResearchEngineDisposition, GliomaAutonomousResearchEngineError,
    GliomaAutonomousResearchEngineRequest, GliomaAutonomousResearchEngineRun,
    GliomaAutonomousResearchEngineStopReason,
};

pub use autonomous_protocol::{
    execute_glioma_autonomous_protocol, AutonomousProtocolControllerDisposition,
    AutonomousProtocolControllerError, AutonomousProtocolControllerRequest,
    AutonomousProtocolControllerRun, AutonomousProtocolControllerStopReason,
    AutonomousProtocolRound,
};

pub use autonomous_campaign::{
    execute_glioma_autonomous_campaign, GliomaActionPlanner, GliomaAutonomousCampaign,
    GliomaAutonomousCampaignDisposition, GliomaAutonomousCampaignError,
    GliomaAutonomousCampaignRequest, GliomaAutonomousCampaignRound,
    GliomaAutonomousCampaignStopReason, GliomaAutonomousPlannerContext, GliomaPlannerFailure,
    StaticGliomaActionPlanner,
};

pub use clone_continuation::{
    plan_glioma_clone_continuation, CloneContinuationActionKind, CloneContinuationActionStatus,
    CloneContinuationCandidate, CloneContinuationDecision, CloneContinuationDisposition,
    CloneContinuationError, CloneContinuationPlan, CloneContinuationRequest,
};

pub use clone_campaign::{
    execute_glioma_adaptive_clone_campaign, execute_glioma_adaptive_clone_campaign_dry_run,
    AdaptiveCloneCampaign, AdaptiveCloneCampaignDisposition, AdaptiveCloneCampaignError,
    AdaptiveCloneCampaignExecutor, AdaptiveCloneCampaignRequest, AdaptiveCloneCampaignRound,
    AdaptiveCloneCampaignStopReason, AdaptiveCloneExecutionFailure,
    DryRunAdaptiveCloneCampaignExecutor,
};

pub use compensation::{
    plan_glioma_protocol_compensation, ProtocolCompensationCandidate,
    ProtocolCompensationDisposition, ProtocolCompensationError, ProtocolCompensationPlan,
    ProtocolCompensationRequest, ProtocolCompensationSelection,
};

pub use branch_optimizer::{
    materialize_glioma_protocol_branch, optimize_glioma_protocol_branches, ProtocolBranchCandidate,
    ProtocolBranchEvaluation, ProtocolBranchOptimizationDisposition,
    ProtocolBranchOptimizationError, ProtocolBranchOptimizationPlan,
    ProtocolBranchOptimizationRequest, ProtocolBranchWeights,
};

pub use director::{
    execute_glioma_research_director, plan_glioma_research_director, GliomaDirectorAction,
    GliomaDirectorCheckpoint, GliomaDirectorDisposition, GliomaDirectorFocus,
    GliomaResearchDirectorError, GliomaResearchDirectorRequest, GliomaResearchDirectorRun,
};

pub use evidence_gate::{
    execute_glioma_evidence_gated_research, EvidenceGatedResearchDisposition,
    GliomaEvidenceGatedResearchError, GliomaEvidenceGatedResearchRequest,
    GliomaEvidenceGatedResearchRun,
};

pub use execution::{
    execute_glioma_protocol, DryRunGliomaProtocolExecutor, GliomaProtocolExecutor,
    ProtocolExecution, ProtocolExecutionDisposition, ProtocolExecutionError,
    ProtocolExecutionFailure, ProtocolExecutionRequest, ProtocolExecutionStopReason,
    ProtocolTaskDisposition, ProtocolTaskResult, OUTPUT_SCHEMA as PROTOCOL_EXECUTION_OUTPUT_SCHEMA,
};

pub use evidence_surface::{
    compile_glioma_protocol_evidence_surface, ProtocolEvidenceCell, ProtocolEvidenceDisposition,
    ProtocolEvidenceSurface, ProtocolEvidenceSurfaceDisposition, ProtocolEvidenceSurfaceError,
    ProtocolEvidenceSurfaceRequest, ProtocolMeasurement,
};

pub use multistudy_fusion::{
    fuse_glioma_protocol_evidence, ProtocolEvidenceFusion, ProtocolEvidenceFusionDisposition,
    ProtocolEvidenceFusionError, ProtocolEvidenceFusionRequest, ProtocolEvidenceStudySurface,
    ProtocolFusionCell, ProtocolFusionDisposition,
};

pub use mechanism_campaign::{
    execute_glioma_multimodal_mechanism_campaign,
    execute_glioma_multimodal_mechanism_campaign_with_executor, MechanismCampaignDisposition,
    MechanismCampaignError, MechanismCampaignExecutionDisposition, MultimodalMechanismCampaign,
    MultimodalMechanismCampaignExecution, MultimodalMechanismCampaignRequest,
};

pub use mechanism_autopilot::{
    execute_glioma_mechanism_autopilot, GliomaMechanismAutopilotDisposition,
    GliomaMechanismAutopilotError, GliomaMechanismAutopilotRequest, GliomaMechanismAutopilotRound,
    GliomaMechanismAutopilotRun, GliomaMechanismAutopilotStopReason,
};

pub use mechanism_discovery_engine::{
    execute_glioma_mechanism_discovery_engine, GliomaMechanismDiscoveryDisposition,
    GliomaMechanismDiscoveryError, GliomaMechanismDiscoveryRequest, GliomaMechanismDiscoveryRound,
    GliomaMechanismDiscoveryRun, GliomaMechanismDiscoveryStopReason,
};

pub use mission::{
    execute_glioma_autonomous_research_mission, GliomaAutonomousResearchMission,
    GliomaMissionDisposition, GliomaMissionError, GliomaMissionGates, GliomaMissionRequest,
    GliomaMissionRound, GliomaMissionStopReason,
};

pub use mission_recovery::{
    execute_glioma_mission_recovery, GliomaMissionRecovery, GliomaMissionRecoveryDisposition,
    GliomaMissionRecoveryError, GliomaMissionRecoveryRequest, GliomaMissionRecoveryStopReason,
};

pub use multimodal_mission::{
    compile_glioma_multimodal_mission_candidates, execute_glioma_multimodal_mission,
    GliomaMultimodalMissionDisposition, GliomaMultimodalMissionError,
    GliomaMultimodalMissionRequest, GliomaMultimodalResearchMission,
};

pub use intent_mission::{
    compile_glioma_intent_mission_candidates, execute_glioma_intent_mission,
    GliomaIntentMissionDisposition, GliomaIntentMissionError, GliomaIntentMissionRequest,
    GliomaIntentResearchMission,
};

pub use program_cycle::{
    execute_glioma_autonomous_program_cycle, AutonomousProgramCycle,
    AutonomousProgramCycleDisposition, AutonomousProgramCycleError, AutonomousProgramCycleRequest,
    ProgramExecutionMode, ProgramGate, ProgramGateStatus,
};

pub use program_scheduler::{
    execute_glioma_program_scheduler, execute_glioma_program_scheduler_dry_run,
    GliomaProgramResourceCapacity, GliomaProgramResourceUsage, GliomaProgramScheduleExecution,
    GliomaProgramScheduleHold, GliomaProgramScheduleJob, GliomaProgramScheduleJobDisposition,
    GliomaProgramScheduleJobState, GliomaProgramSchedulerDisposition, GliomaProgramSchedulerError,
    GliomaProgramSchedulerRequest, GliomaProgramSchedulerRound, GliomaProgramSchedulerRun,
    GliomaProgramSchedulerStopReason,
};

pub use research_autopilot::{
    execute_glioma_research_autopilot, GliomaResearchAutopilotDisposition,
    GliomaResearchAutopilotError, GliomaResearchAutopilotRequest, GliomaResearchAutopilotRun,
};

pub use robust_active_learning_campaign::{
    execute_glioma_robust_active_learning_campaign, DryRunRobustActiveLearningCampaignExecutor,
    RobustActiveLearningCampaign, RobustActiveLearningCampaignDisposition,
    RobustActiveLearningCampaignError, RobustActiveLearningCampaignExecutor,
    RobustActiveLearningCampaignRequest, RobustActiveLearningCampaignRound,
    RobustActiveLearningCampaignStopReason, RobustActiveLearningExecutionFailure,
};

pub use scientific_frontier::{
    plan_glioma_scientific_frontier, FrontierCandidateGate, FrontierCandidateStatus,
    ScientificFrontierDisposition, ScientificFrontierError, ScientificFrontierPlan,
    ScientificFrontierRequest,
};

pub use evidence_campaign::{
    execute_glioma_evidence_campaign, GliomaEvidenceCampaignDisposition,
    GliomaEvidenceCampaignError, GliomaEvidenceCampaignExecution, GliomaEvidenceCampaignRequest,
};

pub use frontier_execution::{
    execute_glioma_scientific_frontier, ScientificFrontierExecution,
    ScientificFrontierExecutionDisposition, ScientificFrontierExecutionError,
    ScientificFrontierExecutionRequest,
};

pub use simulator::{
    protocol_request_from_experiment_design, simulate_glioma_protocol, ProtocolDisposition,
    ProtocolResource, ProtocolResourceKind, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest, ProtocolTask, ResourceUtilization, ScheduleEntry,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ProtocolSimulation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P07")
}
