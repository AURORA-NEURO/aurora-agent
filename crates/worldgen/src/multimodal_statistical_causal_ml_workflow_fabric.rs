//! Worldgen P13 F14 statistical, causal, and ML workflow fabric.
use super::statistical_causal_ml_workflow_support::{self,AnalysisWorkflowRequest,AnalysisWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P13-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-statistical-causal-ml-workflow/1.0";
pub fn worldgen_multimodal_statistical_causal_ml_workflow_fabric_manifest()->serde_json::Value{statistical_causal_ml_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn schedule_worldgen_multimodal_statistical_causal_ml_workflow(request:&AnalysisWorkflowRequest)->Result<AnalysisWorkflowReceipt,statistical_causal_ml_workflow_support::AnalysisWorkflowError>{statistical_causal_ml_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use statistical_causal_ml_workflow_support::{AnalysisWorkflowError,AnalysisWorkflowRequest as WorldgenStatisticalCausalMlWorkflowRequest,AnalysisWorkflowReceipt as WorldgenStatisticalCausalMlWorkflowReceipt};

