//! Worldgen P08 AFA-worldgen-P08-F09 mechanism research copilot.
use super::mechanism_copilot_support::{self,MechanismCopilotRequest,MechanismCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-mechanism-copilot/1.0";
pub fn worldgen_local_mechanism_exploration_research_copilot_manifest()->serde_json::Value{mechanism_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismCopilotRequest1@1","local single-study","A0")}
pub fn run_worldgen_local_mechanism_exploration_research_copilot(request:&MechanismCopilotRequest)->Result<MechanismCopilotReceipt,mechanism_copilot_support::MechanismCopilotError>{mechanism_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use mechanism_copilot_support::{MechanismCopilotError,MechanismCopilotReceipt as WorldgenLocalMechanismresearchcopilotReceipt,MechanismCopilotRequest as WorldgenLocalMechanismresearchcopilotRequest};

