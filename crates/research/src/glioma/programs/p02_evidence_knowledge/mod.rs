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
pub mod closure;
pub mod composition;
pub mod consistency;
pub mod continual_agent;
pub mod dispatch;
pub mod federated_continual;
pub mod federated_knowledge;
pub mod gap_compiler;
pub mod knowledge_drift;
pub mod knowledge_graph;
pub mod multimodal_workflow;
pub mod operating_cycle;
pub mod prospective_monitor;
pub mod selection_cycle;
pub mod study_alignment;
pub mod workflow_admission;
pub mod workflow_compile;
pub mod workflow_recovery;

pub use gap_compiler::{
    compile_glioma_knowledge_gaps, KnowledgeGapClaimMapping, KnowledgeGapCompilerError,
    KnowledgeGapCompilerRequest, KnowledgeGapPortfolio, KnowledgeGapPortfolioDisposition,
    KnowledgeGapSourceTemplate,
};

pub use autonomous_cycle::{
    execute_glioma_autonomous_gap_cycle, AutonomousGapCycle, AutonomousGapCycleDisposition,
    AutonomousGapCycleError, AutonomousGapCycleRequest,
};

pub use action_compiler::{
    compile_glioma_knowledge_actions, CompiledActionDisposition, CompiledResearchAction,
    KnowledgeActionCompilerError, KnowledgeActionCompilerRequest, KnowledgeActionPlan,
    KnowledgeActionPlanDisposition, KnowledgeActionTemplate,
};

pub use action_bridge::{
    bridge_glioma_knowledge_actions, BridgedKnowledgeCandidate, KnowledgeActionBridge,
    KnowledgeActionBridgeError, KnowledgeActionBridgeRequest,
};

pub use selection_cycle::{
    execute_glioma_knowledge_selection_cycle, KnowledgeActionSelectionCycle,
    KnowledgeActionSelectionCycleError, KnowledgeActionSelectionCycleRequest,
};

pub use dispatch::{
    execute_glioma_knowledge_action_dispatch, DryRunKnowledgeActionExecutor,
    KnowledgeActionDispatchDisposition, KnowledgeActionDispatchError,
    KnowledgeActionDispatchRequest, KnowledgeActionDispatchResult, KnowledgeActionDispatchRun,
    KnowledgeActionExecutionFailure, KnowledgeActionExecutor, KnowledgeActionResultDisposition,
};

pub use campaign::{
    execute_glioma_knowledge_resolution_campaign, DryRunKnowledgeResolutionCampaignExecutor,
    KnowledgeResolutionCampaign, KnowledgeResolutionCampaignDisposition,
    KnowledgeResolutionCampaignError, KnowledgeResolutionCampaignExecutor,
    KnowledgeResolutionCampaignRequest, KnowledgeResolutionCampaignRound,
    KnowledgeResolutionCampaignStopReason, KnowledgeResolutionExecutionFailure,
};

pub use belief_revision::{
    revise_glioma_beliefs, BeliefConflict, BeliefRevision, BeliefRevisionDecision,
    BeliefRevisionDecisionKind, BeliefRevisionDisposition, BeliefRevisionError,
    BeliefRevisionRequest,
};

pub use action_outcome_assimilation::{
    assimilate_glioma_knowledge_action_outcomes, ActionOutcomeAssimilationError,
    ActionOutcomeAssimilationItem, ActionOutcomeSnapshot, KnowledgeActionOutcomeAssimilation,
    KnowledgeActionOutcomeAssimilationRequest, OutcomeAssimilationDecision,
    OutcomeAssimilationDisposition,
};
pub use claim_evidence_reconciliation::{
    reconcile_glioma_claim_evidence, ClaimEvidenceReconciliation, ClaimEvidenceReconciliationError,
    ClaimEvidenceReconciliationRequest, ClaimReconciliationDecision,
    ClaimReconciliationDisposition, ClaimReconciliationRow,
};
pub use claim_experiment_closure::{
    close_glioma_claims_to_experiments, ClaimExperimentClosure, ClaimExperimentClosureDisposition,
    ClaimExperimentClosureError, ClaimExperimentClosureRequest, ClaimExperimentDisposition,
    ClaimExperimentResult,
};
pub use claim_frontier::{
    prioritize_knowledge_frontier, FrontierActionKind, KnowledgeFrontier,
    KnowledgeFrontierDisposition, KnowledgeFrontierError, KnowledgeFrontierRequest,
    KnowledgeFrontierScore, KnowledgeFrontierWeights,
};
pub use closure::{
    compile_glioma_knowledge_closure, KnowledgeClaimClosure, KnowledgeClosure,
    KnowledgeClosureClaimDisposition, KnowledgeClosureDisposition, KnowledgeClosureError,
    KnowledgeClosureRequest,
};
pub use composition::{
    compose_knowledge_graph, KnowledgeComponentDisposition, KnowledgeComposition,
    KnowledgeCompositionComponent, KnowledgeCompositionDisposition, KnowledgeCompositionError,
    KnowledgeCompositionPath, KnowledgeCompositionRequest, KnowledgePathDisposition,
    KnowledgeRelation, KnowledgeRelationKind,
};
pub use consistency::{
    compile_glioma_knowledge_consistency, KnowledgeConsistencyClaimDisposition,
    KnowledgeConsistencyClaimScore, KnowledgeConsistencyClosure, KnowledgeConsistencyDisposition,
    KnowledgeConsistencyError, KnowledgeConsistencyRequest,
};
pub use continual_agent::{
    plan_federated_continual_agent, FederatedAgentActionKind, FederatedAgentCandidate,
    FederatedAgentDecision, FederatedAgentDisposition, FederatedAgentPlanItem,
    FederatedContinualAgentError, FederatedContinualAgentPlan, FederatedContinualAgentRequest,
};
pub use federated_continual::{
    analyze_federated_continual_knowledge, FederatedContinualClaim,
    FederatedContinualClaimDisposition, FederatedContinualDisposition, FederatedContinualKnowledge,
    FederatedContinualKnowledgeError, FederatedContinualKnowledgeRequest,
    FederatedContinualObservation, FederatedContinualTrend, FederatedEpochConsensus,
    FederatedEpochDisposition,
};
pub use federated_knowledge::{
    analyze_federated_knowledge, FederatedKnowledge, FederatedKnowledgeAction,
    FederatedKnowledgeDisposition, FederatedKnowledgeError, FederatedKnowledgeKind,
    FederatedKnowledgeRequest, FederatedKnowledgeSiteClaim,
};
pub use knowledge_drift::{
    detect_glioma_knowledge_drift, KnowledgeDrift, KnowledgeDriftAction, KnowledgeDriftDisposition,
    KnowledgeDriftError, KnowledgeDriftKind, KnowledgeDriftRequest,
};
pub use knowledge_graph::{
    compile_typed_knowledge, KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    KnowledgeError, KnowledgeRequest, TypedKnowledge,
};
pub use multimodal_workflow::{
    compile_multimodal_knowledge_workflow, ModalityWorkflowObservation,
    MultimodalKnowledgeWorkflow, MultimodalStudyReadiness, MultimodalWorkflowBarrier,
    MultimodalWorkflowBarrierKind, MultimodalWorkflowBranch, MultimodalWorkflowBranchKind,
    MultimodalWorkflowDisposition, MultimodalWorkflowError, MultimodalWorkflowRequest,
};
pub use operating_cycle::{
    execute_glioma_knowledge_synthesis_operating_cycle, KnowledgeSynthesisOperatingCycle,
    KnowledgeSynthesisOperatingCycleDisposition, KnowledgeSynthesisOperatingCycleError,
    KnowledgeSynthesisOperatingCycleRequest,
};
pub use prospective_monitor::{
    monitor_prospective_knowledge, ProspectiveKnowledgeAlert, ProspectiveKnowledgeDisposition,
    ProspectiveKnowledgeError, ProspectiveKnowledgeEvent, ProspectiveKnowledgeMonitor,
    ProspectiveKnowledgeRequest, ProspectiveKnowledgeRow, ProspectiveKnowledgeTrend,
};
pub use study_alignment::{
    compile_multi_study_knowledge, MultiStudyClaimDisposition, MultiStudyKnowledge,
    MultiStudyKnowledgeDisposition, MultiStudyKnowledgeError, MultiStudyKnowledgeRequest,
    MultiStudyKnowledgeRow, StudyClaimBinding, StudyClaimObservation, StudyKnowledgeSnapshot,
};
pub use workflow_admission::{
    admit_glioma_research_workflow, AdmittedResearchAction, ResearchAdmissionDisposition,
    ResearchAdmissionGate, ResearchAdmissionRoute, ResearchWorkflowAdmission,
    WorkflowAdmissionError, WorkflowAdmissionRequest,
};
pub use workflow_compile::{
    compile_local_research_workflow, LocalResearchWorkflow, LocalWorkflowDisposition,
    LocalWorkflowError, LocalWorkflowRequest, LocalWorkflowStep,
};
pub use workflow_recovery::{
    plan_glioma_workflow_recovery, WorkflowObservedStatus, WorkflowRecoveryAction,
    WorkflowRecoveryDecision, WorkflowRecoveryDisposition, WorkflowRecoveryError,
    WorkflowRecoveryPlan, WorkflowRecoveryRequest, WorkflowStepObservation,
};

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceKnowledge;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P02")
}
