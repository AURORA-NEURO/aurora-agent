//! Worldgen P14 F14 statistical, causal, and ML workflow fabric.
use super::interpretation_visualization_workflow_support::{self,InterpretationWorkflowRequest,InterpretationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F14"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-interpretation-visualization-workflow/1.0";
pub fn worldgen_multimodal_interpretation_visualization_workflow_fabric_manifest()->serde_json::Value{interpretation_visualization_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn schedule_worldgen_multimodal_interpretation_visualization_workflow(request:&InterpretationWorkflowRequest)->Result<InterpretationWorkflowReceipt,interpretation_visualization_workflow_support::InterpretationWorkflowError>{interpretation_visualization_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use interpretation_visualization_workflow_support::{InterpretationWorkflowError,InterpretationWorkflowRequest as WorldgenInterpretationVisualizationWorkflowRequest,InterpretationWorkflowReceipt as WorldgenInterpretationVisualizationWorkflowReceipt};

