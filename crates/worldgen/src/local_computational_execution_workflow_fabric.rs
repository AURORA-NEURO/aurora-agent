//! Worldgen P12 AFA-worldgen-P12-F13 computational_execution workflow fabric.
use super::computational_execution_workflow_support::{self,ExecutionWorkflowRequest,ExecutionWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-computational_execution-workflow/1.0";
pub fn worldgen_local_computational_execution_workflow_fabric_manifest()->serde_json::Value{computational_execution_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_computational_execution_workflow(request:&ExecutionWorkflowRequest)->Result<ExecutionWorkflowReceipt,computational_execution_workflow_support::ExecutionWorkflowError>{computational_execution_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use computational_execution_workflow_support::{ExecutionWorkflowError,ExecutionWorkflowReceipt as WorldgenLocalProtocolSimulationworkflowfabricReceipt,ExecutionWorkflowRequest as WorldgenLocalProtocolSimulationworkflowfabricRequest};

