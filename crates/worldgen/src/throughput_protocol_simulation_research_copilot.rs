//! Worldgen P10 AFA-worldgen-P10-F11 protocol_simulation research copilot.
use super::protocol_simulation_copilot_support::{self,ProtocolCopilotRequest,ProtocolCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-protocol_simulation-copilot/1.0";
pub fn worldgen_throughput_protocol_simulation_research_copilot_manifest()->serde_json::Value{protocol_simulation_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolCopilotRequest1@1","prospective high-throughput","A1")}
pub fn run_worldgen_throughput_protocol_simulation_research_copilot(request:&ProtocolCopilotRequest)->Result<ProtocolCopilotReceipt,protocol_simulation_copilot_support::ProtocolCopilotError>{protocol_simulation_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false,false)}
pub use protocol_simulation_copilot_support::{ProtocolCopilotError,ProtocolCopilotReceipt as WorldgenThroughputProtocolSimulationresearchcopilotReceipt,ProtocolCopilotRequest as WorldgenThroughputProtocolSimulationresearchcopilotRequest};

