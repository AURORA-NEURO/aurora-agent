//! Worldgen P12 AFA-worldgen-P12-F06 computational_execution contract model.
use super::computational_execution_contract_support::{self,ExecutionContractRequest,ExecutionContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P12-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-computational_execution-contract/1.0";
pub fn worldgen_multimodal_computational_execution_contract_model_manifest()->serde_json::Value{computational_execution_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn negotiate_worldgen_multimodal_computational_execution_contract(request:&ExecutionContractRequest)->Result<ExecutionContractReceipt,computational_execution_contract_support::ExecutionContractError>{computational_execution_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use computational_execution_contract_support::{ExecutionContractError,ExecutionContractReceipt as WorldgenMultimodalProtocolSimulationcontractmodelReceipt,ExecutionContractRequest as WorldgenMultimodalProtocolSimulationcontractmodelRequest};

