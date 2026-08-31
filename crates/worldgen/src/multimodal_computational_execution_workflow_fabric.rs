//! Worldgen P12 AFA-worldgen-P12-F14 computational_execution workflow fabric.
use super::computational_execution_workflow_support::{self,ExecutionWorkflowRequest,ExecutionWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-computational_execution-workflow/1.0";
pub fn worldgen_multimodal_computational_execution_workflow_fabric_manifest()->serde_json::Value{computational_execution_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn schedule_worldgen_multimodal_computational_execution_workflow(request:&ExecutionWorkflowRequest)->Result<ExecutionWorkflowReceipt,computational_execution_workflow_support::ExecutionWorkflowError>{computational_execution_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use computational_execution_workflow_support::{ExecutionWorkflowError,ExecutionWorkflowReceipt as WorldgenMultimodalProtocolSimulationworkflowfabricReceipt,ExecutionWorkflowRequest as WorldgenMultimodalProtocolSimulationworkflowfabricRequest};

