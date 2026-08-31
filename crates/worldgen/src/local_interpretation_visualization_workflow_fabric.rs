//! Worldgen P14 F13 statistical, causal, and ML workflow fabric.
use super::interpretation_visualization_workflow_support::{self,InterpretationWorkflowRequest,InterpretationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-interpretation-visualization-workflow/1.0";
pub fn worldgen_local_interpretation_visualization_workflow_fabric_manifest()->serde_json::Value{interpretation_visualization_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_interpretation_visualization_workflow(request:&InterpretationWorkflowRequest)->Result<InterpretationWorkflowReceipt,interpretation_visualization_workflow_support::InterpretationWorkflowError>{interpretation_visualization_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use interpretation_visualization_workflow_support::{InterpretationWorkflowError,InterpretationWorkflowRequest as WorldgenInterpretationVisualizationWorkflowRequest,InterpretationWorkflowReceipt as WorldgenInterpretationVisualizationWorkflowReceipt};

