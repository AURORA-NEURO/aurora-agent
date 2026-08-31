//! Worldgen P08 AFA-worldgen-P08-F11 mechanism research copilot.
use super::mechanism_copilot_support::{self,MechanismCopilotRequest,MechanismCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-mechanism-copilot/1.0";
pub fn worldgen_throughput_mechanism_exploration_research_copilot_manifest()->serde_json::Value{mechanism_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismCopilotRequest1@1","prospective high-throughput","A1")}
pub fn run_worldgen_throughput_mechanism_exploration_research_copilot(request:&MechanismCopilotRequest)->Result<MechanismCopilotReceipt,mechanism_copilot_support::MechanismCopilotError>{mechanism_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use mechanism_copilot_support::{MechanismCopilotError,MechanismCopilotReceipt as WorldgenThroughputMechanismresearchcopilotReceipt,MechanismCopilotRequest as WorldgenThroughputMechanismresearchcopilotRequest};

