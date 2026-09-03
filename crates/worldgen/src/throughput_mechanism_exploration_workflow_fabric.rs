//! Worldgen P08 AFA-worldgen-P08-F15 mechanism workflow fabric.
use super::mechanism_workflow_support::{self,MechanismWorkflowRequest,MechanismWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-mechanism-workflow/1.0";
pub fn worldgen_throughput_mechanism_exploration_workflow_fabric_manifest()->serde_json::Value{mechanism_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismWorkflowRequest1@1","prospective high-throughput","A1")}
pub fn schedule_worldgen_throughput_mechanism_exploration_workflow(request:&MechanismWorkflowRequest)->Result<MechanismWorkflowReceipt,mechanism_workflow_support::MechanismWorkflowError>{mechanism_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use mechanism_workflow_support::{MechanismWorkflowError,MechanismWorkflowReceipt as WorldgenThroughputMechanismworkflowfabricReceipt,MechanismWorkflowRequest as WorldgenThroughputMechanismworkflowfabricRequest};

