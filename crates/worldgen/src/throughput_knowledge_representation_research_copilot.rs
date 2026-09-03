//! Worldgen P04-F11 prospective high-throughput knowledge-representation research copilot.
use super::knowledge_copilot_support::{self, KnowledgeCopilotRequest, KnowledgeCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-knowledge-copilot/1.0";
pub fn worldgen_throughput_knowledge_representation_research_copilot_manifest()->serde_json::Value{knowledge_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeCopilotRequest1@1","prospective high-throughput","A2")}
pub fn run_worldgen_throughput_knowledge_representation_research_copilot(request:&KnowledgeCopilotRequest)->Result<KnowledgeCopilotReceipt,knowledge_copilot_support::KnowledgeCopilotError>{knowledge_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use knowledge_copilot_support::{KnowledgeCopilotError,KnowledgeCopilotReceipt as WorldgenThroughputKnowledgeCopilotReceipt,KnowledgeCopilotRequest as WorldgenThroughputKnowledgeCopilotRequest};
