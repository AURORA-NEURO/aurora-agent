//! Worldgen P13 F13 statistical, causal, and ML workflow fabric.
use super::statistical_causal_ml_workflow_support::{self,AnalysisWorkflowRequest,AnalysisWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-statistical-causal-ml-workflow/1.0";
pub fn worldgen_local_statistical_causal_ml_workflow_fabric_manifest()->serde_json::Value{statistical_causal_ml_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_statistical_causal_ml_workflow(request:&AnalysisWorkflowRequest)->Result<AnalysisWorkflowReceipt,statistical_causal_ml_workflow_support::AnalysisWorkflowError>{statistical_causal_ml_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use statistical_causal_ml_workflow_support::{AnalysisWorkflowError,AnalysisWorkflowRequest as WorldgenStatisticalCausalMlWorkflowRequest,AnalysisWorkflowReceipt as WorldgenStatisticalCausalMlWorkflowReceipt};

