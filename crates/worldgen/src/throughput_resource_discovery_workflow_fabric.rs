//! Worldgen P05 AFA-worldgen-P05-F15 throughput workflow_fabric.
use super::resource_workflow_support::{self,ResourceWorkflowRequest,ResourceWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-resource-workflow/1.0";
pub fn worldgen_throughput_resource_discovery_workflow_fabric_manifest()->serde_json::Value{resource_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceWorkflowRequest1@1","prospective high-throughput","A2")}
pub fn schedule_worldgen_throughput_resource_discovery_workflow(request:&ResourceWorkflowRequest)->Result<ResourceWorkflowReceipt,resource_workflow_support::ResourceWorkflowError>{resource_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,false)}
pub use resource_workflow_support::{ResourceWorkflowError,ResourceWorkflowReceipt as WorldgenthroughputResourceworkflowfabricReceipt,ResourceWorkflowRequest as WorldgenthroughputResourceworkflowfabricRequest};
