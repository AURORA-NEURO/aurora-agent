//! Worldgen P15 F10 statistical, causal, and ML research copilot.
use super::replication_negative_results_copilot_support::{self,ReplicationCopilotRequest,ReplicationCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-replication-negative-results-copilot/1.0";
pub fn worldgen_multimodal_replication_negative_results_research_copilot_manifest()->serde_json::Value{replication_negative_results_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_replication_negative_results_research_copilot(request:&ReplicationCopilotRequest)->Result<ReplicationCopilotReceipt,replication_negative_results_copilot_support::ReplicationCopilotError>{replication_negative_results_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use replication_negative_results_copilot_support::{ReplicationCopilotError,ReplicationCopilotRequest as WorldgenReplicationNegativeResultsCopilotRequest,ReplicationCopilotReceipt as WorldgenReplicationNegativeResultsCopilotReceipt};

