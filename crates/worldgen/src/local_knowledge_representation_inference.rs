//! Worldgen P04-F01 local knowledge-representation inference engine.
use super::knowledge_representation_support::{self, KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-knowledge-representation/1.0";
pub fn worldgen_local_knowledge_representation_inference_manifest()->serde_json::Value{knowledge_representation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeRepresentationRequest1@1","local single-study","A0")}
pub fn represent_worldgen_local_knowledge(request:&KnowledgeRepresentationRequest)->Result<KnowledgeRepresentationReceipt,knowledge_representation_support::KnowledgeRepresentationError>{knowledge_representation_support::represent(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use knowledge_representation_support::{KnowledgeRepresentationError,KnowledgeRepresentationReceipt as WorldgenLocalKnowledgeRepresentationReceipt,KnowledgeRepresentationRequest as WorldgenLocalKnowledgeRepresentationRequest,KnowledgeNode as WorldgenKnowledgeNode,KnowledgeRelation as WorldgenKnowledgeRelation};
