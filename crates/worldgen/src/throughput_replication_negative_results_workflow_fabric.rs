//! Worldgen P15 F15 statistical, causal, and ML workflow fabric.
use super::replication_negative_results_workflow_support::{self,ReplicationWorkflowRequest,ReplicationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P15-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-replication-negative-results-workflow/1.0";
pub fn worldgen_throughput_replication_negative_results_workflow_fabric_manifest()->serde_json::Value{replication_negative_results_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn schedule_worldgen_throughput_replication_negative_results_workflow(request:&ReplicationWorkflowRequest)->Result<ReplicationWorkflowReceipt,replication_negative_results_workflow_support::ReplicationWorkflowError>{replication_negative_results_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use replication_negative_results_workflow_support::{ReplicationWorkflowError,ReplicationWorkflowRequest as WorldgenReplicationNegativeResultsWorkflowRequest,ReplicationWorkflowReceipt as WorldgenReplicationNegativeResultsWorkflowReceipt};

