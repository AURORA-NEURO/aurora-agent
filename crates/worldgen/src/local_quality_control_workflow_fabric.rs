//! Worldgen P07 AFA-worldgen-P07-F13 quality workflow fabric.
use super::quality_workflow_support::{self,QualityWorkflowRequest,QualityWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-quality-workflow/1.0";
pub fn worldgen_local_quality_control_workflow_fabric_manifest()->serde_json::Value{quality_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityWorkflowRequest1@1","local single-study","A0")}
pub fn schedule_worldgen_local_quality_control_workflow(request:&QualityWorkflowRequest)->Result<QualityWorkflowReceipt,quality_workflow_support::QualityWorkflowError>{quality_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use quality_workflow_support::{QualityWorkflowError,QualityWorkflowReceipt as WorldgenLocalQualitycontrolworkflowReceipt,QualityWorkflowRequest as WorldgenLocalQualitycontrolworkflowRequest};

