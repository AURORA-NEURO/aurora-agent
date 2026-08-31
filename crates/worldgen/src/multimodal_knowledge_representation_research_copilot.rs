//! Worldgen P04-F10 multimodal knowledge-representation research copilot.
use super::knowledge_copilot_support::{self, KnowledgeCopilotRequest, KnowledgeCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-knowledge-copilot/1.0";
pub fn worldgen_multimodal_knowledge_representation_research_copilot_manifest()->serde_json::Value{knowledge_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeCopilotRequest1@1","multimodal multi-study","A2")}
pub fn run_worldgen_multimodal_knowledge_representation_research_copilot(request:&KnowledgeCopilotRequest)->Result<KnowledgeCopilotReceipt,knowledge_copilot_support::KnowledgeCopilotError>{knowledge_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use knowledge_copilot_support::{KnowledgeCopilotError,KnowledgeCopilotReceipt as WorldgenMultimodalKnowledgeCopilotReceipt,KnowledgeCopilotRequest as WorldgenMultimodalKnowledgeCopilotRequest};
