//! Evidence-to-typed-knowledge program ownership.

pub mod campaign;
pub mod claim_frontier;
pub mod composition;
pub mod knowledge_graph;

pub use campaign::{
    execute_glioma_knowledge_resolution_campaign, DryRunKnowledgeResolutionCampaignExecutor,
    KnowledgeResolutionCampaign, KnowledgeResolutionCampaignDisposition,
    KnowledgeResolutionCampaignError, KnowledgeResolutionCampaignExecutor,
    KnowledgeResolutionCampaignRequest, KnowledgeResolutionCampaignRound,
    KnowledgeResolutionCampaignStopReason, KnowledgeResolutionExecutionFailure,
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

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::EvidenceKnowledge;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P02")
}
