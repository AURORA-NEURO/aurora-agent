//! Worldgen P08 AFA-worldgen-P08-F10 mechanism research copilot.
use super::mechanism_copilot_support::{self,MechanismCopilotRequest,MechanismCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P08-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-mechanism-copilot/1.0";
pub fn worldgen_multimodal_mechanism_exploration_research_copilot_manifest()->serde_json::Value{mechanism_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"MechanismCopilotRequest1@1","multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_mechanism_exploration_research_copilot(request:&MechanismCopilotRequest)->Result<MechanismCopilotReceipt,mechanism_copilot_support::MechanismCopilotError>{mechanism_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use mechanism_copilot_support::{MechanismCopilotError,MechanismCopilotReceipt as WorldgenMultimodalMechanismresearchcopilotReceipt,MechanismCopilotRequest as WorldgenMultimodalMechanismresearchcopilotRequest};

