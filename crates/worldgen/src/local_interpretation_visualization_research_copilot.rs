//! Worldgen P14 F09 statistical, causal, and ML research copilot.
use super::interpretation_visualization_copilot_support::{self,InterpretationCopilotRequest,InterpretationCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-interpretation-visualization-copilot/1.0";
pub fn worldgen_local_interpretation_visualization_research_copilot_manifest()->serde_json::Value{interpretation_visualization_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn run_worldgen_local_interpretation_visualization_research_copilot(request:&InterpretationCopilotRequest)->Result<InterpretationCopilotReceipt,interpretation_visualization_copilot_support::InterpretationCopilotError>{interpretation_visualization_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use interpretation_visualization_copilot_support::{InterpretationCopilotError,InterpretationCopilotRequest as WorldgenInterpretationVisualizationCopilotRequest,InterpretationCopilotReceipt as WorldgenInterpretationVisualizationCopilotReceipt};

