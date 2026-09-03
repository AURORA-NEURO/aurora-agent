//! Worldgen P11 AFA-worldgen-P11-F07 laboratory_integration contract model.
use super::laboratory_integration_contract_support::{self,InstrumentContractRequest,InstrumentContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F07"; pub const CONTRACT_VERSION:&str="worldgen-throughput-laboratory_integration-contract/1.0";
pub fn worldgen_throughput_laboratory_integration_contract_model_manifest()->serde_json::Value{laboratory_integration_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentContractRequest1@1","prospective high-throughput","A1")}
pub fn negotiate_worldgen_throughput_laboratory_integration_contract(request:&InstrumentContractRequest)->Result<InstrumentContractReceipt,laboratory_integration_contract_support::InstrumentContractError>{laboratory_integration_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use laboratory_integration_contract_support::{InstrumentContractError,InstrumentContractReceipt as WorldgenThroughputLaboratoryIntegrationcontractmodelReceipt,InstrumentContractRequest as WorldgenThroughputLaboratoryIntegrationcontractmodelRequest};

