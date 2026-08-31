//! Worldgen P12 AFA-worldgen-P12-F10 computational_execution research copilot.
use super::computational_execution_copilot_support::{self,ExecutionCopilotRequest,ExecutionCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-computational_execution-copilot/1.0";
pub fn worldgen_multimodal_computational_execution_research_copilot_manifest()->serde_json::Value{computational_execution_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_computational_execution_research_copilot(request:&ExecutionCopilotRequest)->Result<ExecutionCopilotReceipt,computational_execution_copilot_support::ExecutionCopilotError>{computational_execution_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use computational_execution_copilot_support::{ExecutionCopilotError,ExecutionCopilotReceipt as WorldgenMultimodalProtocolSimulationresearchcopilotReceipt,ExecutionCopilotRequest as WorldgenMultimodalProtocolSimulationresearchcopilotRequest};

