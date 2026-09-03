//! Worldgen P14 F15 statistical, causal, and ML workflow fabric.
use super::interpretation_visualization_workflow_support::{self,InterpretationWorkflowRequest,InterpretationWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-interpretation-visualization-workflow/1.0";
pub fn worldgen_throughput_interpretation_visualization_workflow_fabric_manifest()->serde_json::Value{interpretation_visualization_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn schedule_worldgen_throughput_interpretation_visualization_workflow(request:&InterpretationWorkflowRequest)->Result<InterpretationWorkflowReceipt,interpretation_visualization_workflow_support::InterpretationWorkflowError>{interpretation_visualization_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use interpretation_visualization_workflow_support::{InterpretationWorkflowError,InterpretationWorkflowRequest as WorldgenInterpretationVisualizationWorkflowRequest,InterpretationWorkflowReceipt as WorldgenInterpretationVisualizationWorkflowReceipt};

