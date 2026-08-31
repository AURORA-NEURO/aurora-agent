//! Worldgen P04-F09 local knowledge-representation research copilot.
use super::knowledge_copilot_support::{self, KnowledgeCopilotRequest, KnowledgeCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-knowledge-copilot/1.0";
pub fn worldgen_local_knowledge_representation_research_copilot_manifest()->serde_json::Value{knowledge_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeCopilotRequest1@1","local single-study","A1")}
pub fn run_worldgen_local_knowledge_representation_research_copilot(request:&KnowledgeCopilotRequest)->Result<KnowledgeCopilotReceipt,knowledge_copilot_support::KnowledgeCopilotError>{knowledge_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use knowledge_copilot_support::{KnowledgeCopilotError,KnowledgeCopilotReceipt as WorldgenLocalKnowledgeCopilotReceipt,KnowledgeCopilotRequest as WorldgenLocalKnowledgeCopilotRequest};
