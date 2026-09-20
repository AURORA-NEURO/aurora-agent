//! Evidence-to-typed-knowledge program ownership.

pub mod action_bridge;
pub mod action_compiler;
pub mod autonomous_cycle;
pub mod belief_revision;
pub mod campaign;
pub mod claim_frontier;
pub mod composition;
pub mod dispatch;
pub mod gap_compiler;
pub mod knowledge_graph;
pub mod operating_cycle;
pub mod selection_cycle;

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

pub use claim_frontier::{
    prioritize_knowledge_frontier, FrontierActionKind, KnowledgeFrontier,
    KnowledgeFrontierDisposition, KnowledgeFrontierError, KnowledgeFrontierRequest,
    KnowledgeFrontierScore, KnowledgeFrontierWeights,
};
pub use composition::{
    compose_knowledge_graph, KnowledgeComponentDisposition, KnowledgeComposition,
    KnowledgeCompositionComponent, KnowledgeCompositionDisposition, KnowledgeCompositionError,
    KnowledgeCompositionPath, KnowledgeCompositionRequest, KnowledgePathDisposition,
    KnowledgeRelation, KnowledgeRelationKind,
};
pub use knowledge_graph::{
    compile_typed_knowledge, KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    KnowledgeError, KnowledgeRequest, TypedKnowledge,
};
pub use operating_cycle::{
    execute_glioma_knowledge_synthesis_operating_cycle, KnowledgeSynthesisOperatingCycle,
    KnowledgeSynthesisOperatingCycleDisposition, KnowledgeSynthesisOperatingCycleError,
    KnowledgeSynthesisOperatingCycleRequest,
};

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceKnowledge;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P02")
}
