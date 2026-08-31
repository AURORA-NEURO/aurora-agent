//! Worldgen P10 AFA-worldgen-P10-F05 protocol_simulation contract model.
use super::protocol_simulation_contract_support::{self,ProtocolContractRequest,ProtocolContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P10-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-protocol_simulation-contract/1.0";
pub fn worldgen_local_protocol_simulation_contract_model_manifest()->serde_json::Value{protocol_simulation_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ProtocolContractRequest1@1","local single-study","A0")}
pub fn negotiate_worldgen_local_protocol_simulation_contract(request:&ProtocolContractRequest)->Result<ProtocolContractReceipt,protocol_simulation_contract_support::ProtocolContractError>{protocol_simulation_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use protocol_simulation_contract_support::{ProtocolContractError,ProtocolContractReceipt as WorldgenLocalProtocolSimulationcontractmodelReceipt,ProtocolContractRequest as WorldgenLocalProtocolSimulationcontractmodelRequest};

