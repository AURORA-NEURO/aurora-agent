//! Worldgen P10 AFA-worldgen-P10-F16 protocol_simulation workflow fabric.
use super::protocol_simulation_workflow_support::{self,ProtocolWorkflowRequest,ProtocolWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-protocol_simulation-workflow/1.0";
pub fn worldgen_federated_continual_protocol_simulation_workflow_fabric_manifest()->serde_json::Value{protocol_simulation_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolWorkflowRequest1@1","federated continual autonomous","A1")}
pub fn schedule_worldgen_federated_continual_protocol_simulation_workflow(request:&ProtocolWorkflowRequest)->Result<ProtocolWorkflowReceipt,protocol_simulation_workflow_support::ProtocolWorkflowError>{protocol_simulation_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use protocol_simulation_workflow_support::{ProtocolWorkflowError,ProtocolWorkflowReceipt as WorldgenFederatedContinualProtocolSimulationworkflowfabricReceipt,ProtocolWorkflowRequest as WorldgenFederatedContinualProtocolSimulationworkflowfabricRequest};

