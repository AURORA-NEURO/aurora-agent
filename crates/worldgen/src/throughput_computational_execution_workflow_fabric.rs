//! Worldgen P12 AFA-worldgen-P12-F15 computational_execution workflow fabric.
use super::computational_execution_workflow_support::{self,ExecutionWorkflowRequest,ExecutionWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-computational_execution-workflow/1.0";
pub fn worldgen_throughput_computational_execution_workflow_fabric_manifest()->serde_json::Value{computational_execution_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn schedule_worldgen_throughput_computational_execution_workflow(request:&ExecutionWorkflowRequest)->Result<ExecutionWorkflowReceipt,computational_execution_workflow_support::ExecutionWorkflowError>{computational_execution_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use computational_execution_workflow_support::{ExecutionWorkflowError,ExecutionWorkflowReceipt as WorldgenThroughputProtocolSimulationworkflowfabricReceipt,ExecutionWorkflowRequest as WorldgenThroughputProtocolSimulationworkflowfabricRequest};

