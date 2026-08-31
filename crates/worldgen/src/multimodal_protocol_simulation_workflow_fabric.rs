//! Worldgen P10 AFA-worldgen-P10-F14 protocol_simulation workflow fabric.
use super::protocol_simulation_workflow_support::{self,ProtocolWorkflowRequest,ProtocolWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-protocol_simulation-workflow/1.0";
pub fn worldgen_multimodal_protocol_simulation_workflow_fabric_manifest()->serde_json::Value{protocol_simulation_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolWorkflowRequest1@1","multimodal multi-study","A1")}
pub fn schedule_worldgen_multimodal_protocol_simulation_workflow(request:&ProtocolWorkflowRequest)->Result<ProtocolWorkflowReceipt,protocol_simulation_workflow_support::ProtocolWorkflowError>{protocol_simulation_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use protocol_simulation_workflow_support::{ProtocolWorkflowError,ProtocolWorkflowReceipt as WorldgenMultimodalProtocolSimulationworkflowfabricReceipt,ProtocolWorkflowRequest as WorldgenMultimodalProtocolSimulationworkflowfabricRequest};

