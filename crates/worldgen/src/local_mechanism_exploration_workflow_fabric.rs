//! Worldgen P08 AFA-worldgen-P08-F13 mechanism workflow fabric.
use super::mechanism_workflow_support::{self,MechanismWorkflowRequest,MechanismWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-mechanism-workflow/1.0";
pub fn worldgen_local_mechanism_exploration_workflow_fabric_manifest()->serde_json::Value{mechanism_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismWorkflowRequest1@1","local single-study","A0")}
pub fn schedule_worldgen_local_mechanism_exploration_workflow(request:&MechanismWorkflowRequest)->Result<MechanismWorkflowReceipt,mechanism_workflow_support::MechanismWorkflowError>{mechanism_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use mechanism_workflow_support::{MechanismWorkflowError,MechanismWorkflowReceipt as WorldgenLocalMechanismworkflowfabricReceipt,MechanismWorkflowRequest as WorldgenLocalMechanismworkflowfabricRequest};

