//! Protocol simulation and adaptive workflow program ownership.
//!
//! The workflow planner is re-exported here so callers can discover the P07 surface through the
//! folder-owned program module while the shared glioma namespace retains a stable API.

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};
pub use crate::glioma::workflow::{
    GliomaWorkflowBranch,
    GliomaWorkflowError,
    GliomaWorkflowExecution,
    GliomaWorkflowMode,
    GliomaWorkflowNode,
    GliomaWorkflowPlan,
    GliomaWorkflowRequest,
    WorkflowNodeDecision,
    execute_glioma_workflow,
    plan_glioma_workflow,
};
pub mod action_execution;
pub mod active_learning_campaign;
pub mod adaptive_scheduler;
pub mod adaptive_scientific_mission;
pub mod autonomous_campaign;
pub mod autonomous_engine;
pub mod autonomous_protocol;
pub mod autonomous_workflow;
pub mod branch_optimizer;
pub mod clone_campaign;
pub mod clone_continuation;
pub mod compensation;
pub mod cross_model_replication_mission;
pub mod director;
pub mod engine_evaluation;
pub mod evidence_campaign;
pub mod evidence_gate;
pub mod evidence_gated_stage_execution;
pub mod evidence_surface;
pub mod execution;
pub mod frontier_execution;
pub mod heterogeneity_portfolio_mission;
pub mod intent_mission;
pub mod mechanism_autopilot;
pub mod mechanism_campaign;
pub mod mechanism_discovery_engine;
pub mod mechanism_validation_execution;
pub mod mission;
pub mod mission_recovery;
pub mod multimodal_mission;
pub mod multistudy_fusion;
pub mod posterior_batch_campaign;
pub mod program_cycle;
pub mod program_scheduler;
pub mod research_autopilot;
pub mod robust_active_learning_campaign;
pub mod scenario_ensemble;
pub mod scientific_frontier;
pub mod simulator;
pub mod stage_executor_adapter;
pub mod stage_worker_registry;
pub mod transport_gate;
pub use action_execution::{
    ActionExecutionDisposition,
    ActionExecutionFailure,
    ActionExecutionResult,
    ActionPortfolioExecution,
    ActionPortfolioExecutionDisposition,
    ActionPortfolioExecutionError,
    ActionPortfolioExecutionRequest,
    ActionPortfolioStopReason,
    DryRunGliomaActionExecutor,
    GliomaActionArtifactInput,
    GliomaActionExecutionContext,
    GliomaActionExecutor,
    GliomaActionWorkflowScope,
    execute_glioma_action_portfolio,
    execute_glioma_action_portfolio_with_context,
    execute_glioma_action_portfolio_with_selection_and_context,
};
pub use adaptive_scientific_mission::{
    execute_glioma_adaptive_scientific_mission,
    AdaptiveScientificMissionDisposition,
    AdaptiveScientificMissionError,
    AdaptiveScientificMissionRequest,
    AdaptiveScientificMissionRound,
    AdaptiveScientificMissionRun,
    AdaptiveScientificMissionStopReason,
    AdaptiveScientificMissionUpdateContext,
    GliomaScientificStateBuilder,
    ScientificStateBuildFailure,
};
pub use adaptive_scheduler::{
    GliomaAdaptiveWorkflowSchedulerDisposition,
    GliomaAdaptiveWorkflowSchedulerError,
    GliomaAdaptiveWorkflowSchedulerPlan,
    GliomaAdaptiveWorkflowSchedulerRequest,
    SchedulerDecision,
    SchedulerObservation,
    SchedulerOutcome,
    plan_glioma_adaptive_workflow,
};
pub use active_learning_campaign::{
    ActiveLearningCampaign,
    ActiveLearningCampaignDisposition,
    ActiveLearningCampaignError,
    ActiveLearningCampaignExecutor,
    ActiveLearningCampaignRequest,
    ActiveLearningCampaignRound,
    ActiveLearningCampaignStopReason,
    ActiveLearningExecutionFailure,
    DryRunActiveLearningCampaignExecutor,
    execute_glioma_active_learning_campaign,
};
pub use posterior_batch_campaign::{
    execute_glioma_posterior_batch_campaign,
    PosteriorBatchAssayOutcome,
    PosteriorBatchCampaign,
    PosteriorBatchCampaignDisposition,
    PosteriorBatchCampaignError,
    PosteriorBatchCampaignExecutor,
    PosteriorBatchCampaignObservation,
    PosteriorBatchCampaignRequest,
    PosteriorBatchCampaignRound,
    PosteriorBatchCampaignStopReason,
    PosteriorBatchModel,
    PosteriorBatchModelContext,
    PosteriorBatchModelState,
    PosteriorBatchProviderFailure,
};
pub use autonomous_engine::{
    GliomaAutonomousResearchEngineCycle,
    GliomaAutonomousResearchEngineDisposition,
    GliomaAutonomousResearchEngineError,
    GliomaAutonomousResearchEngineRequest,
    GliomaAutonomousResearchEngineRun,
    GliomaAutonomousResearchEngineStopReason,
    execute_glioma_autonomous_research_engine,
    resume_glioma_autonomous_research_engine,
    GliomaAdaptiveReplanningPolicy,
};
pub use engine_evaluation::{
    evaluate_glioma_autonomous_research_engine,
    evaluate_glioma_autonomous_research_engine_scenarios,
    evaluate_glioma_autonomous_research_engine_traces,
    GliomaAutonomousResearchEngineEvaluation,
    GliomaAutonomousResearchEngineStressEvaluation,
    GliomaAutonomousResearchEngineTraceEvaluation,
    GliomaEngineEvaluationError,
    GliomaEngineEvaluationMetric,
    GliomaEngineEvaluationPolicy,
    GliomaEngineStressPolicyMetric,
    GliomaEngineStressScenarioSummary,
    GliomaEngineTraceDisposition,
    GliomaEngineTraceOutcome,
    GliomaEngineTracePolicyMetric,
    GliomaEngineTraceScenarioSummary,
};
pub use autonomous_protocol::{
    AutonomousProtocolControllerDisposition,
    AutonomousProtocolControllerError,
    AutonomousProtocolControllerRequest,
    AutonomousProtocolControllerRun,
    AutonomousProtocolControllerStopReason,
    AutonomousProtocolRound,
    execute_glioma_autonomous_protocol,
};
pub use autonomous_campaign::{
    GliomaActionPlanner,
    GliomaAutonomousCampaign,
    GliomaAutonomousCampaignDisposition,
    GliomaAutonomousCampaignError,
    GliomaAutonomousCampaignRequest,
    GliomaAutonomousCampaignRound,
    GliomaAutonomousCampaignStopReason,
    GliomaAutonomousPlannerContext,
    GliomaPlannerFailure,
    StaticGliomaActionPlanner,
    execute_glioma_autonomous_campaign,
};
pub use clone_continuation::{
    CloneContinuationActionKind,
    CloneContinuationActionStatus,
    CloneContinuationCandidate,
    CloneContinuationDecision,
    CloneContinuationDisposition,
    CloneContinuationError,
    CloneContinuationPlan,
    CloneContinuationRequest,
    plan_glioma_clone_continuation,
};
pub use clone_campaign::{
    AdaptiveCloneCampaign,
    AdaptiveCloneCampaignDisposition,
    AdaptiveCloneCampaignError,
    AdaptiveCloneCampaignExecutor,
    AdaptiveCloneCampaignRequest,
    AdaptiveCloneCampaignRound,
    AdaptiveCloneCampaignStopReason,
    AdaptiveCloneExecutionFailure,
    DryRunAdaptiveCloneCampaignExecutor,
    execute_glioma_adaptive_clone_campaign,
    execute_glioma_adaptive_clone_campaign_dry_run,
};
pub use compensation::{
    ProtocolCompensationCandidate,
    ProtocolCompensationDisposition,
    ProtocolCompensationError,
    ProtocolCompensationPlan,
    ProtocolCompensationRequest,
    ProtocolCompensationSelection,
    plan_glioma_protocol_compensation,
};
pub use cross_model_replication_mission::{
    execute_glioma_cross_model_replication_mission_dry_run,
    execute_glioma_cross_model_replication_mission_with_executor,
    plan_glioma_cross_model_replication_mission,
    CrossModelReplicationMissionDisposition,
    CrossModelReplicationMissionError,
    CrossModelReplicationMissionExecution,
    CrossModelReplicationMissionExecutionRequest,
    CrossModelReplicationMissionPlan,
    CrossModelReplicationMissionRequest,
};
pub use branch_optimizer::{
    ProtocolBranchCandidate,
    ProtocolBranchEvaluation,
    ProtocolBranchOptimizationDisposition,
    ProtocolBranchOptimizationError,
    ProtocolBranchOptimizationPlan,
    ProtocolBranchOptimizationRequest,
    ProtocolBranchWeights,
    materialize_glioma_protocol_branch,
    optimize_glioma_protocol_branches,
};
pub use director::{
    GliomaDirectorAction,
    GliomaDirectorCheckpoint,
    GliomaDirectorDisposition,
    GliomaDirectorFocus,
    GliomaResearchDirectorError,
    GliomaResearchDirectorRequest,
    GliomaResearchDirectorRun,
    execute_glioma_research_director,
    plan_glioma_research_director,
};
pub use evidence_gate::{
    EvidenceGatedResearchDisposition,
    GliomaEvidenceGatedResearchError,
    GliomaEvidenceGatedResearchRequest,
    GliomaEvidenceGatedResearchRun,
    execute_glioma_evidence_gated_research,
};
pub use autonomous_workflow::{
    execute_glioma_autonomous_research_workflow_dry_run,
    GliomaAutonomousResearchWorkflowError,
    GliomaAutonomousResearchWorkflowRequest,
};
pub use evidence_gated_stage_execution::{
    execute_glioma_evidence_gated_stage_engine,
    GliomaEvidenceGatedStageExecution,
    GliomaEvidenceGatedStageExecutionDisposition,
    GliomaEvidenceGatedStageExecutionError,
    GliomaEvidenceGatedStageExecutionRequest,
};
pub use heterogeneity_portfolio_mission::{
    plan_glioma_heterogeneity_portfolio_mission,
    HeterogeneityPortfolioActionBinding,
    HeterogeneityPortfolioMissionDisposition,
    HeterogeneityPortfolioMissionError,
    HeterogeneityPortfolioMissionPlan,
    HeterogeneityPortfolioMissionRequest,
};
pub use execution::{
    DryRunGliomaProtocolExecutor,
    GliomaProtocolExecutor,
    OUTPUT_SCHEMA as PROTOCOL_EXECUTION_OUTPUT_SCHEMA,
    ProtocolExecution,
    ProtocolExecutionDisposition,
    ProtocolExecutionError,
    ProtocolExecutionFailure,
    ProtocolExecutionRequest,
    ProtocolExecutionStopReason,
    ProtocolTaskDisposition,
    ProtocolTaskResult,
    execute_glioma_protocol,
};
pub use mechanism_validation_execution::{
    MechanismValidationExecution,
    MechanismValidationExecutionDisposition,
    MechanismValidationExecutionError,
    MechanismValidationExecutionRequest,
    execute_glioma_mechanism_validation_protocol,
};
pub use evidence_surface::{
    ProtocolEvidenceCell,
    ProtocolEvidenceDisposition,
    ProtocolEvidenceSurface,
    ProtocolEvidenceSurfaceDisposition,
    ProtocolEvidenceSurfaceError,
    ProtocolEvidenceSurfaceRequest,
    ProtocolMeasurement,
    compile_glioma_protocol_evidence_surface,
};
pub use multistudy_fusion::{
    ProtocolEvidenceFusion,
    ProtocolEvidenceFusionDisposition,
    ProtocolEvidenceFusionError,
    ProtocolEvidenceFusionRequest,
    ProtocolEvidenceStudySurface,
    ProtocolFusionCell,
    ProtocolFusionDisposition,
    fuse_glioma_protocol_evidence,
};
pub use transport_gate::{
    ProtocolTransportEndpoint,
    ProtocolTransportEndpointDisposition,
    ProtocolTransportGate,
    ProtocolTransportGateDisposition,
    ProtocolTransportGateError,
    ProtocolTransportGateRequest,
    gate_glioma_protocol_transport,
};
pub use mechanism_campaign::{
    MechanismCampaignDisposition,
    MechanismCampaignError,
    MechanismCampaignExecutionDisposition,
    MultimodalMechanismCampaign,
    MultimodalMechanismCampaignExecution,
    MultimodalMechanismCampaignRequest,
    execute_glioma_multimodal_mechanism_campaign,
    execute_glioma_multimodal_mechanism_campaign_with_executor,
};
pub use mechanism_autopilot::{
    GliomaMechanismAutopilotDisposition,
    GliomaMechanismAutopilotError,
    GliomaMechanismAutopilotRequest,
    GliomaMechanismAutopilotRound,
    GliomaMechanismAutopilotRun,
    GliomaMechanismAutopilotStopReason,
    execute_glioma_mechanism_autopilot,
    execute_glioma_mechanism_autopilot_with_feedback,
    GliomaMechanismAutopilotFeedback,
    GliomaMechanismFeedbackInterpreter,
    MAX_FEEDBACK_OBSERVATIONS_PER_ACTION,
};
pub use mechanism_discovery_engine::{
    GliomaMechanismDiscoveryDisposition,
    GliomaMechanismDiscoveryError,
    GliomaMechanismDiscoveryRequest,
    GliomaMechanismDiscoveryRound,
    GliomaMechanismDiscoveryRun,
    GliomaMechanismDiscoveryStopReason,
    execute_glioma_mechanism_discovery_engine,
};
pub use mission::{
    GliomaAutonomousResearchMission,
    GliomaMissionDisposition,
    GliomaMissionError,
    GliomaMissionGates,
    GliomaMissionRequest,
    GliomaMissionRound,
    GliomaMissionStopReason,
    execute_glioma_autonomous_research_mission,
    execute_glioma_autonomous_research_mission_with_context,
    GliomaMissionExecutionContext,
};
pub use mission_recovery::{
    GliomaMissionRecovery,
    GliomaMissionRecoveryDisposition,
    GliomaMissionRecoveryError,
    GliomaMissionRecoveryRequest,
    GliomaMissionRecoveryStopReason,
    execute_glioma_mission_recovery,
};
pub use multimodal_mission::{
    GliomaMultimodalMissionDisposition,
    GliomaMultimodalMissionError,
    GliomaMultimodalMissionRequest,
    GliomaMultimodalResearchMission,
    compile_glioma_multimodal_mission_candidates,
    execute_glioma_multimodal_mission,
};
pub use intent_mission::{
    GliomaIntentMissionDisposition,
    GliomaIntentMissionError,
    GliomaIntentMissionRequest,
    GliomaIntentResearchMission,
    compile_glioma_intent_mission_candidates,
    execute_glioma_intent_mission,
};
pub use program_cycle::{
    AutonomousProgramCycle,
    AutonomousProgramCycleDisposition,
    AutonomousProgramCycleError,
    AutonomousProgramCycleRequest,
    ProgramExecutionMode,
    ProgramGate,
    ProgramGateStatus,
    execute_glioma_autonomous_program_cycle,
};
pub use program_scheduler::{
    GliomaProgramResourceCapacity,
    GliomaProgramResourceUsage,
    GliomaProgramScheduleExecution,
    GliomaProgramScheduleHold,
    GliomaProgramScheduleJob,
    GliomaProgramScheduleJobDisposition,
    GliomaProgramScheduleJobState,
    GliomaProgramSchedulerDisposition,
    GliomaProgramSchedulerError,
    GliomaProgramSchedulerRequest,
    GliomaProgramSchedulerRound,
    GliomaProgramSchedulerRun,
    GliomaProgramSchedulerStopReason,
    execute_glioma_program_scheduler,
    execute_glioma_program_scheduler_dry_run,
};
pub use research_autopilot::{
    GliomaResearchAutopilotDisposition,
    GliomaResearchAutopilotError,
    GliomaResearchAutopilotRequest,
    GliomaResearchAutopilotRun,
    execute_glioma_research_autopilot,
};
pub use robust_active_learning_campaign::{
    DryRunRobustActiveLearningCampaignExecutor,
    RobustActiveLearningCampaign,
    RobustActiveLearningCampaignDisposition,
    RobustActiveLearningCampaignError,
    RobustActiveLearningCampaignExecutor,
    RobustActiveLearningCampaignRequest,
    RobustActiveLearningCampaignRound,
    RobustActiveLearningCampaignStopReason,
    RobustActiveLearningExecutionFailure,
    execute_glioma_robust_active_learning_campaign,
};
pub use scientific_frontier::{
    FrontierCandidateGate,
    FrontierCandidateStatus,
    ScientificFrontierDisposition,
    ScientificFrontierError,
    ScientificFrontierPlan,
    ScientificFrontierRequest,
    plan_glioma_scientific_frontier,
    ScientificFrontierCandidateClaimLink,
};
pub use scenario_ensemble::{
    ProtocolScenario,
    ProtocolScenarioEnsemble,
    ProtocolScenarioEnsembleDisposition,
    ProtocolScenarioEnsembleError,
    ProtocolScenarioEnsembleRequest,
    ProtocolScenarioResult,
    ScenarioFailureClass,
    simulate_glioma_protocol_scenario_ensemble,
};

pub use stage_executor_adapter::GliomaStageActionExecutor;
pub use stage_worker_registry::{
    compile_glioma_stage_worker_routes,
    execute_glioma_autonomous_research_engine_with_stage_workers,
    DryRunGliomaStageWorker,
    GliomaAutonomousResearchStageExecution,
    GliomaStageExecutorRegistry,
    GliomaStageWorkerExecutionError,
    GliomaStageWorkerProfile,
    GliomaStageWorkerRoute,
    GliomaStageWorkerRouteDisposition,
    GliomaStageWorkerRouteError,
    GliomaStageWorkerRoutePlan,
    GliomaStageWorkerRouteRequest,
};
pub use evidence_campaign::{
    GliomaEvidenceCampaignDisposition,
    GliomaEvidenceCampaignError,
    GliomaEvidenceCampaignExecution,
    GliomaEvidenceCampaignRequest,
    execute_glioma_evidence_campaign,
};
pub use frontier_execution::{
    ScientificFrontierExecution,
    ScientificFrontierExecutionDisposition,
    ScientificFrontierExecutionError,
    ScientificFrontierExecutionRequest,
    execute_glioma_scientific_frontier,
};
pub use simulator::{
    ProtocolDisposition,
    ProtocolResource,
    ProtocolResourceKind,
    ProtocolSimulation,
    ProtocolSimulationError,
    ProtocolSimulationRequest,
    ProtocolTask,
    ResourceUtilization,
    ScheduleEntry,
    protocol_request_from_experiment_design,
    simulate_glioma_protocol,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::ProtocolSimulation;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P07")
}
