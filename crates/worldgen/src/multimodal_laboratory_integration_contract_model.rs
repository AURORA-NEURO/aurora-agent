//! Worldgen P11 AFA-worldgen-P11-F06 laboratory_integration contract model.
use super::laboratory_integration_contract_support::{self,InstrumentContractRequest,InstrumentContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P11-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-laboratory_integration-contract/1.0";
pub fn worldgen_multimodal_laboratory_integration_contract_model_manifest()->serde_json::Value{laboratory_integration_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"InstrumentContractRequest1@1","multimodal multi-study","A1")}
pub fn negotiate_worldgen_multimodal_laboratory_integration_contract(request:&InstrumentContractRequest)->Result<InstrumentContractReceipt,laboratory_integration_contract_support::InstrumentContractError>{laboratory_integration_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use laboratory_integration_contract_support::{InstrumentContractError,InstrumentContractReceipt as WorldgenMultimodalLaboratoryIntegrationcontractmodelReceipt,InstrumentContractRequest as WorldgenMultimodalLaboratoryIntegrationcontractmodelRequest};

