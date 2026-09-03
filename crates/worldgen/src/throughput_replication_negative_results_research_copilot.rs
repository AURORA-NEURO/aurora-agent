//! Worldgen P15 F11 statistical, causal, and ML research copilot.
use super::replication_negative_results_copilot_support::{self,ReplicationCopilotRequest,ReplicationCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-replication-negative-results-copilot/1.0";
pub fn worldgen_throughput_replication_negative_results_research_copilot_manifest()->serde_json::Value{replication_negative_results_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn run_worldgen_throughput_replication_negative_results_research_copilot(request:&ReplicationCopilotRequest)->Result<ReplicationCopilotReceipt,replication_negative_results_copilot_support::ReplicationCopilotError>{replication_negative_results_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use replication_negative_results_copilot_support::{ReplicationCopilotError,ReplicationCopilotRequest as WorldgenReplicationNegativeResultsCopilotRequest,ReplicationCopilotReceipt as WorldgenReplicationNegativeResultsCopilotReceipt};

