//! Worldgen P05 AFA-worldgen-P05-F16 federated_continual workflow_fabric.
use super::resource_workflow_support::{self,ResourceWorkflowRequest,ResourceWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-resource-workflow/1.0";
pub fn worldgen_federated_continual_resource_discovery_workflow_fabric_manifest()->serde_json::Value{resource_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceWorkflowRequest1@1","federated continual autonomous","A2")}
pub fn schedule_worldgen_federated_continual_resource_discovery_workflow(request:&ResourceWorkflowRequest)->Result<ResourceWorkflowReceipt,resource_workflow_support::ResourceWorkflowError>{resource_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true,true)}
pub use resource_workflow_support::{ResourceWorkflowError,ResourceWorkflowReceipt as Worldgenfederated_continualResourceworkflowfabricReceipt,ResourceWorkflowRequest as Worldgenfederated_continualResourceworkflowfabricRequest};
