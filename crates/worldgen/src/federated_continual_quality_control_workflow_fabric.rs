//! Worldgen P07 AFA-worldgen-P07-F16 quality workflow fabric.
use super::quality_workflow_support::{self,QualityWorkflowRequest,QualityWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P07-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-quality-workflow/1.0";
pub fn worldgen_federated_continual_quality_control_workflow_fabric_manifest()->serde_json::Value{quality_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"QualityWorkflowRequest1@1","federated continual autonomous","A1")}
pub fn schedule_worldgen_federated_continual_quality_control_workflow(request:&QualityWorkflowRequest)->Result<QualityWorkflowReceipt,quality_workflow_support::QualityWorkflowError>{quality_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use quality_workflow_support::{QualityWorkflowError,QualityWorkflowReceipt as WorldgenFederatedContinualQualitycontrolworkflowReceipt,QualityWorkflowRequest as WorldgenFederatedContinualQualitycontrolworkflowRequest};

