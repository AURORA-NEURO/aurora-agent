//! Worldgen P10 AFA-worldgen-P10-F02 protocol_simulation exploration inference.
use super::protocol_simulation_support::{self,ProtocolDraft,ProtocolSimulationReport};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-protocol_simulation-exploration/1.0";
pub fn worldgen_multimodal_protocol_simulation_inference_manifest()->serde_json::Value{protocol_simulation_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolDraft1@1","multimodal multi-study","A1")}
pub fn simulate_worldgen_multimodal_protocol_simulations(request:&ProtocolDraft)->Result<ProtocolSimulationReport,protocol_simulation_support::ProtocolSimulationError>{protocol_simulation_support::simulate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use protocol_simulation_support::{ProtocolStep,ProtocolSimulationError,ProtocolSimulationReport as WorldgenMultimodalProtocolSimulationportfolioInference,ProtocolDraft as WorldgenMultimodalProtocolSimulationquestionInference};

