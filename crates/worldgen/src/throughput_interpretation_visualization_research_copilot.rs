//! Worldgen P14 F11 statistical, causal, and ML research copilot.
use super::interpretation_visualization_copilot_support::{self,InterpretationCopilotRequest,InterpretationCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-interpretation-visualization-copilot/1.0";
pub fn worldgen_throughput_interpretation_visualization_research_copilot_manifest()->serde_json::Value{interpretation_visualization_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn run_worldgen_throughput_interpretation_visualization_research_copilot(request:&InterpretationCopilotRequest)->Result<InterpretationCopilotReceipt,interpretation_visualization_copilot_support::InterpretationCopilotError>{interpretation_visualization_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use interpretation_visualization_copilot_support::{InterpretationCopilotError,InterpretationCopilotRequest as WorldgenInterpretationVisualizationCopilotRequest,InterpretationCopilotReceipt as WorldgenInterpretationVisualizationCopilotReceipt};

