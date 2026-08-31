//! Worldgen P10 AFA-worldgen-P10-F12 protocol_simulation research copilot.
use super::protocol_simulation_copilot_support::{self,ProtocolCopilotRequest,ProtocolCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-protocol_simulation-copilot/1.0";
pub fn worldgen_federated_continual_protocol_simulation_research_copilot_manifest()->serde_json::Value{protocol_simulation_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolCopilotRequest1@1","federated continual autonomous","A1")}
pub fn run_worldgen_federated_continual_protocol_simulation_research_copilot(request:&ProtocolCopilotRequest)->Result<ProtocolCopilotReceipt,protocol_simulation_copilot_support::ProtocolCopilotError>{protocol_simulation_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use protocol_simulation_copilot_support::{ProtocolCopilotError,ProtocolCopilotReceipt as WorldgenFederatedContinualProtocolSimulationresearchcopilotReceipt,ProtocolCopilotRequest as WorldgenFederatedContinualProtocolSimulationresearchcopilotRequest};

