//! Evidence-to-typed-knowledge program ownership.

pub mod claim_frontier;
pub mod knowledge_graph;

pub use claim_frontier::{
    prioritize_knowledge_frontier, FrontierActionKind, KnowledgeFrontier,
    KnowledgeFrontierDisposition, KnowledgeFrontierError, KnowledgeFrontierRequest,
    KnowledgeFrontierScore, KnowledgeFrontierWeights,
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
