//! Worldgen P08 AFA-worldgen-P08-F16 mechanism workflow fabric.
use super::mechanism_workflow_support::{self,MechanismWorkflowRequest,MechanismWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-mechanism-workflow/1.0";
pub fn worldgen_federated_continual_mechanism_exploration_workflow_fabric_manifest()->serde_json::Value{mechanism_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismWorkflowRequest1@1","federated continual autonomous","A1")}
pub fn schedule_worldgen_federated_continual_mechanism_exploration_workflow(request:&MechanismWorkflowRequest)->Result<MechanismWorkflowReceipt,mechanism_workflow_support::MechanismWorkflowError>{mechanism_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use mechanism_workflow_support::{MechanismWorkflowError,MechanismWorkflowReceipt as WorldgenFederatedContinualMechanismworkflowfabricReceipt,MechanismWorkflowRequest as WorldgenFederatedContinualMechanismworkflowfabricRequest};

