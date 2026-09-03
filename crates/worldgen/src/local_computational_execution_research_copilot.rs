//! Worldgen P12 AFA-worldgen-P12-F09 computational_execution research copilot.
use super::computational_execution_copilot_support::{self,ExecutionCopilotRequest,ExecutionCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-computational_execution-copilot/1.0";
pub fn worldgen_local_computational_execution_research_copilot_manifest()->serde_json::Value{computational_execution_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn run_worldgen_local_computational_execution_research_copilot(request:&ExecutionCopilotRequest)->Result<ExecutionCopilotReceipt,computational_execution_copilot_support::ExecutionCopilotError>{computational_execution_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use computational_execution_copilot_support::{ExecutionCopilotError,ExecutionCopilotReceipt as WorldgenLocalProtocolSimulationresearchcopilotReceipt,ExecutionCopilotRequest as WorldgenLocalProtocolSimulationresearchcopilotRequest};

