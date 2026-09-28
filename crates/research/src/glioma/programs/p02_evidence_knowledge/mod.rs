//! Evidence-to-typed-knowledge program ownership.

pub mod action_bridge;
pub mod action_compiler;
pub mod action_outcome_assimilation;
pub mod autonomous_cycle;
pub mod belief_revision;
pub mod campaign;
pub mod claim_evidence_reconciliation;
pub mod claim_experiment_closure;
pub mod claim_frontier;
pub mod closed_loop_frontier;
pub mod closure;
pub mod composition;
pub mod consistency;
pub mod continual_agent;
pub mod dispatch;
pub mod federated_continual;
pub mod federated_knowledge;
pub mod frontier_campaign;
pub mod gap_compiler;
pub mod knowledge_drift;
pub mod knowledge_graph;
pub mod knowledge_protocol_gateway;
pub mod multimodal_protocol_gateway;
pub mod multimodal_workflow;
pub mod operating_cycle;
pub mod prospective_belief_calibration;
pub mod prospective_monitor;
pub mod selection_cycle;
pub mod study_alignment;
pub mod workflow_admission;
pub mod workflow_compile;
pub mod workflow_recovery;

pub use gap_compiler::{
    KnowledgeGapClaimMapping, KnowledgeGapCompilerError, KnowledgeGapCompilerRequest,
    KnowledgeGapPortfolio, KnowledgeGapPortfolioDisposition, KnowledgeGapSourceTemplate,
    compile_glioma_knowledge_gaps,
};

pub use autonomous_cycle::{
    AutonomousGapCycle, AutonomousGapCycleDisposition, AutonomousGapCycleError,
    AutonomousGapCycleRequest, execute_glioma_autonomous_gap_cycle,
};

pub use action_compiler::{
    CompiledActionDisposition, CompiledResearchAction, KnowledgeActionCompilerError,
    KnowledgeActionCompilerRequest, KnowledgeActionPlan, KnowledgeActionPlanDisposition,
    KnowledgeActionTemplate, compile_glioma_knowledge_actions,
};

pub use action_bridge::{
    BridgedKnowledgeCandidate, KnowledgeActionBridge, KnowledgeActionBridgeError,
    KnowledgeActionBridgeRequest, bridge_glioma_knowledge_actions,
};

pub use selection_cycle::{
    KnowledgeActionSelectionCycle, KnowledgeActionSelectionCycleError,
    KnowledgeActionSelectionCycleRequest, execute_glioma_knowledge_selection_cycle,
};

pub use dispatch::{
    DryRunKnowledgeActionExecutor, KnowledgeActionDispatchDisposition,
    KnowledgeActionDispatchError, KnowledgeActionDispatchRequest, KnowledgeActionDispatchResult,
    KnowledgeActionDispatchRun, KnowledgeActionExecutionFailure, KnowledgeActionExecutor,
    KnowledgeActionResultDisposition, execute_glioma_knowledge_action_dispatch,
};

pub use campaign::{
    DryRunKnowledgeResolutionCampaignExecutor, KnowledgeResolutionCampaign,
    KnowledgeResolutionCampaignDisposition, KnowledgeResolutionCampaignError,
    KnowledgeResolutionCampaignExecutor, KnowledgeResolutionCampaignRequest,
    KnowledgeResolutionCampaignRound, KnowledgeResolutionCampaignStopReason,
    KnowledgeResolutionExecutionFailure, execute_glioma_knowledge_resolution_campaign,
};

pub use belief_revision::{
    BeliefConflict, BeliefRevision, BeliefRevisionDecision, BeliefRevisionDecisionKind,
    BeliefRevisionDisposition, BeliefRevisionError, BeliefRevisionRequest, revise_glioma_beliefs,
};

pub use action_outcome_assimilation::{
    ActionOutcomeAssimilationError, ActionOutcomeAssimilationItem, ActionOutcomeSnapshot,
    KnowledgeActionOutcomeAssimilation, KnowledgeActionOutcomeAssimilationRequest,
    OutcomeAssimilationDecision, OutcomeAssimilationDisposition,
    assimilate_glioma_knowledge_action_outcomes,
};
pub use claim_evidence_reconciliation::{
    ClaimEvidenceReconciliation, ClaimEvidenceReconciliationError,
    ClaimEvidenceReconciliationRequest, ClaimReconciliationDecision,
    ClaimReconciliationDisposition, ClaimReconciliationRow, reconcile_glioma_claim_evidence,
};
pub use claim_experiment_closure::{
    ClaimExperimentClosure, ClaimExperimentClosureDisposition, ClaimExperimentClosureError,
    ClaimExperimentClosureRequest, ClaimExperimentDisposition, ClaimExperimentResult,
    close_glioma_claims_to_experiments,
};
pub use claim_frontier::{
    FrontierActionKind, KnowledgeFrontier, KnowledgeFrontierDisposition, KnowledgeFrontierError,
    KnowledgeFrontierRequest, KnowledgeFrontierScore, KnowledgeFrontierWeights,
    prioritize_knowledge_frontier,
};
pub use closed_loop_frontier::{
    ClosedLoopFrontier, ClosedLoopFrontierDisposition, ClosedLoopFrontierError,
    ClosedLoopFrontierRequest, FrontierPromotionActionKind, FrontierPromotionCandidate,
    promote_glioma_closed_loop_frontier,
};
pub use closure::{
    KnowledgeClaimClosure, KnowledgeClosure, KnowledgeClosureClaimDisposition,
    KnowledgeClosureDisposition, KnowledgeClosureError, KnowledgeClosureRequest,
    compile_glioma_knowledge_closure,
};
pub use composition::{
    KnowledgeComponentDisposition, KnowledgeComposition, KnowledgeCompositionComponent,
    KnowledgeCompositionDisposition, KnowledgeCompositionError, KnowledgeCompositionPath,
    KnowledgeCompositionRequest, KnowledgePathDisposition, KnowledgeRelation,
    KnowledgeRelationKind, compose_knowledge_graph,
};
pub use consistency::{
    KnowledgeConsistencyClaimDisposition, KnowledgeConsistencyClaimScore,
    KnowledgeConsistencyClosure, KnowledgeConsistencyDisposition, KnowledgeConsistencyError,
    KnowledgeConsistencyRequest, compile_glioma_knowledge_consistency,
};
pub use continual_agent::{
    FederatedAgentActionKind, FederatedAgentCandidate, FederatedAgentDecision,
    FederatedAgentDisposition, FederatedAgentPlanItem, FederatedContinualAgentError,
    FederatedContinualAgentPlan, FederatedContinualAgentRequest, plan_federated_continual_agent,
};
pub use federated_continual::{
    FederatedContinualClaim, FederatedContinualClaimDisposition, FederatedContinualDisposition,
    FederatedContinualKnowledge, FederatedContinualKnowledgeError,
    FederatedContinualKnowledgeRequest, FederatedContinualObservation, FederatedContinualTrend,
    FederatedEpochConsensus, FederatedEpochDisposition, analyze_federated_continual_knowledge,
};
pub use federated_knowledge::{
    FederatedKnowledge, FederatedKnowledgeAction, FederatedKnowledgeDisposition,
    FederatedKnowledgeError, FederatedKnowledgeKind, FederatedKnowledgeRequest,
    FederatedKnowledgeSiteClaim, analyze_federated_knowledge,
};
pub use frontier_campaign::{
    FrontierCampaign, FrontierCampaignDisposition, FrontierCampaignError, FrontierCampaignRequest,
    FrontierCampaignRound, FrontierCampaignRoundStatus, schedule_glioma_frontier_campaign,
};
pub use knowledge_drift::{
    KnowledgeDrift, KnowledgeDriftAction, KnowledgeDriftDisposition, KnowledgeDriftError,
    KnowledgeDriftKind, KnowledgeDriftRequest, detect_glioma_knowledge_drift,
};
pub use knowledge_graph::{
    KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition, KnowledgeError,
    KnowledgeRequest, TypedKnowledge, compile_typed_knowledge,
};
pub use knowledge_protocol_gateway::{
    KnowledgeProtocolGatewayError, KnowledgeProtocolNegotiation, KnowledgeProtocolRequest,
    negotiate_glioma_knowledge_protocol,
};
pub use multimodal_protocol_gateway::{
    MultimodalKnowledgeProtocolRequest, MultimodalProtocolDisposition,
    MultimodalProtocolGatewayError, MultimodalProtocolNegotiation,
    negotiate_glioma_multimodal_knowledge_protocol,
};
pub use multimodal_workflow::{
    ModalityWorkflowObservation, MultimodalKnowledgeWorkflow, MultimodalStudyReadiness,
    MultimodalWorkflowBarrier, MultimodalWorkflowBarrierKind, MultimodalWorkflowBranch,
    MultimodalWorkflowBranchKind, MultimodalWorkflowDisposition, MultimodalWorkflowError,
    MultimodalWorkflowRequest, compile_multimodal_knowledge_workflow,
};
pub use operating_cycle::{
    KnowledgeSynthesisOperatingCycle, KnowledgeSynthesisOperatingCycleDisposition,
    KnowledgeSynthesisOperatingCycleError, KnowledgeSynthesisOperatingCycleRequest,
    execute_glioma_knowledge_synthesis_operating_cycle,
};
pub use prospective_belief_calibration::{
    BeliefCalibrationClaim, BeliefCalibrationClaimDisposition, BeliefForecastObservation,
    ProspectiveBeliefCalibration, ProspectiveBeliefCalibrationDisposition,
    ProspectiveBeliefCalibrationError, ProspectiveBeliefCalibrationRequest,
    calibrate_glioma_beliefs_prospectively,
};
pub use prospective_monitor::{
    ProspectiveKnowledgeAlert, ProspectiveKnowledgeDisposition, ProspectiveKnowledgeError,
    ProspectiveKnowledgeEvent, ProspectiveKnowledgeMonitor, ProspectiveKnowledgeRequest,
    ProspectiveKnowledgeRow, ProspectiveKnowledgeTrend, monitor_prospective_knowledge,
};
pub use study_alignment::{
    MultiStudyClaimDisposition, MultiStudyKnowledge, MultiStudyKnowledgeDisposition,
    MultiStudyKnowledgeError, MultiStudyKnowledgeRequest, MultiStudyKnowledgeRow,
    StudyClaimBinding, StudyClaimObservation, StudyKnowledgeSnapshot,
    compile_multi_study_knowledge,
};
pub use workflow_admission::{
    AdmittedResearchAction, ResearchAdmissionDisposition, ResearchAdmissionGate,
    ResearchAdmissionRoute, ResearchWorkflowAdmission, WorkflowAdmissionError,
    WorkflowAdmissionRequest, admit_glioma_research_workflow,
};
pub use workflow_compile::{
    LocalResearchWorkflow, LocalWorkflowDisposition, LocalWorkflowError, LocalWorkflowRequest,
    LocalWorkflowStep, compile_local_research_workflow,
};
pub use workflow_recovery::{
    WorkflowObservedStatus, WorkflowRecoveryAction, WorkflowRecoveryDecision,
    WorkflowRecoveryDisposition, WorkflowRecoveryError, WorkflowRecoveryPlan,
    WorkflowRecoveryRequest, WorkflowStepObservation, plan_glioma_workflow_recovery,
};

use crate::glioma::catalog::{GliomaProgramDescriptor, GliomaProgramId, glioma_program_catalog};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceKnowledge;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P02")
}
