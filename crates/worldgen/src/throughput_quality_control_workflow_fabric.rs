//! Worldgen P07 AFA-worldgen-P07-F15 quality workflow fabric.
use super::quality_workflow_support::{self,QualityWorkflowRequest,QualityWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-quality-workflow/1.0";
pub fn worldgen_throughput_quality_control_workflow_fabric_manifest()->serde_json::Value{quality_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityWorkflowRequest1@1","prospective high-throughput","A1")}
pub fn schedule_worldgen_throughput_quality_control_workflow(request:&QualityWorkflowRequest)->Result<QualityWorkflowReceipt,quality_workflow_support::QualityWorkflowError>{quality_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use quality_workflow_support::{QualityWorkflowError,QualityWorkflowReceipt as WorldgenThroughputQualitycontrolworkflowReceipt,QualityWorkflowRequest as WorldgenThroughputQualitycontrolworkflowRequest};

