//! Worldgen P05 AFA-worldgen-P05-F13 local workflow_fabric.
use super::resource_workflow_support::{self,ResourceWorkflowRequest,ResourceWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-resource-workflow/1.0";
pub fn worldgen_local_resource_discovery_workflow_fabric_manifest()->serde_json::Value{resource_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceWorkflowRequest1@1","local single-study","A1")}
pub fn schedule_worldgen_local_resource_discovery_workflow(request:&ResourceWorkflowRequest)->Result<ResourceWorkflowReceipt,resource_workflow_support::ResourceWorkflowError>{resource_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use resource_workflow_support::{ResourceWorkflowError,ResourceWorkflowReceipt as WorldgenlocalResourceworkflowfabricReceipt,ResourceWorkflowRequest as WorldgenlocalResourceworkflowfabricRequest};
