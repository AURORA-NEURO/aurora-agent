//! Worldgen P04-F02 multimodal knowledge-representation inference engine.
use super::knowledge_representation_support::{self, KnowledgeRepresentationRequest, KnowledgeRepresentationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-knowledge-representation/1.0";
pub fn worldgen_multimodal_knowledge_representation_inference_manifest()->serde_json::Value{knowledge_representation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeRepresentationRequest1@1","multimodal multi-study","A1")}
pub fn represent_worldgen_multimodal_knowledge(request:&KnowledgeRepresentationRequest)->Result<KnowledgeRepresentationReceipt,knowledge_representation_support::KnowledgeRepresentationError>{knowledge_representation_support::represent(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use knowledge_representation_support::{KnowledgeRepresentationError,KnowledgeRepresentationReceipt as WorldgenMultimodalKnowledgeRepresentationReceipt,KnowledgeRepresentationRequest as WorldgenMultimodalKnowledgeRepresentationRequest};
