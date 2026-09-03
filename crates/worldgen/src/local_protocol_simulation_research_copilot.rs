//! Worldgen P10 AFA-worldgen-P10-F09 protocol_simulation research copilot.
use super::protocol_simulation_copilot_support::{self,ProtocolCopilotRequest,ProtocolCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-protocol_simulation-copilot/1.0";
pub fn worldgen_local_protocol_simulation_research_copilot_manifest()->serde_json::Value{protocol_simulation_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolCopilotRequest1@1","local single-study","A0")}
pub fn run_worldgen_local_protocol_simulation_research_copilot(request:&ProtocolCopilotRequest)->Result<ProtocolCopilotReceipt,protocol_simulation_copilot_support::ProtocolCopilotError>{protocol_simulation_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use protocol_simulation_copilot_support::{ProtocolCopilotError,ProtocolCopilotReceipt as WorldgenLocalProtocolSimulationresearchcopilotReceipt,ProtocolCopilotRequest as WorldgenLocalProtocolSimulationresearchcopilotRequest};

