//! Worldgen P04-F03 prospective high-throughput knowledge-representation inference engine.
use super::knowledge_representation_support::{self, KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-knowledge-representation/1.0";
pub fn worldgen_throughput_knowledge_representation_inference_manifest()->serde_json::Value{knowledge_representation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeRepresentationRequest1@1","prospective high-throughput","A1")}
pub fn represent_worldgen_throughput_knowledge(request:&KnowledgeRepresentationRequest)->Result<KnowledgeRepresentationReceipt,knowledge_representation_support::KnowledgeRepresentationError>{knowledge_representation_support::represent(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true)}
pub use knowledge_representation_support::{KnowledgeRepresentationError,KnowledgeRepresentationReceipt as WorldgenThroughputKnowledgeRepresentationReceipt,KnowledgeRepresentationRequest as WorldgenThroughputKnowledgeRepresentationRequest};
