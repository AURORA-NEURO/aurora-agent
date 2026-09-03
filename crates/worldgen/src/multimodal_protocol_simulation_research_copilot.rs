//! Worldgen P10 AFA-worldgen-P10-F10 protocol_simulation research copilot.
use super::protocol_simulation_copilot_support::{self,ProtocolCopilotRequest,ProtocolCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-protocol_simulation-copilot/1.0";
pub fn worldgen_multimodal_protocol_simulation_research_copilot_manifest()->serde_json::Value{protocol_simulation_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolCopilotRequest1@1","multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_protocol_simulation_research_copilot(request:&ProtocolCopilotRequest)->Result<ProtocolCopilotReceipt,protocol_simulation_copilot_support::ProtocolCopilotError>{protocol_simulation_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use protocol_simulation_copilot_support::{ProtocolCopilotError,ProtocolCopilotReceipt as WorldgenMultimodalProtocolSimulationresearchcopilotReceipt,ProtocolCopilotRequest as WorldgenMultimodalProtocolSimulationresearchcopilotRequest};

