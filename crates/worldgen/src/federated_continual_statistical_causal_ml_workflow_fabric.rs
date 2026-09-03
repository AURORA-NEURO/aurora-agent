//! Worldgen P13 F16 statistical, causal, and ML workflow fabric.
use super::statistical_causal_ml_workflow_support::{self,AnalysisWorkflowRequest,AnalysisWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-statistical-causal-ml-workflow/1.0";
pub fn worldgen_federated_continual_statistical_causal_ml_workflow_fabric_manifest()->serde_json::Value{statistical_causal_ml_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn schedule_worldgen_federated_continual_statistical_causal_ml_workflow(request:&AnalysisWorkflowRequest)->Result<AnalysisWorkflowReceipt,statistical_causal_ml_workflow_support::AnalysisWorkflowError>{statistical_causal_ml_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use statistical_causal_ml_workflow_support::{AnalysisWorkflowError,AnalysisWorkflowRequest as WorldgenStatisticalCausalMlWorkflowRequest,AnalysisWorkflowReceipt as WorldgenStatisticalCausalMlWorkflowReceipt};

