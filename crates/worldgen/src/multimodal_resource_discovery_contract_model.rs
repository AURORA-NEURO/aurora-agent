//! Worldgen P05 AFA-worldgen-P05-F06 multimodal contract_model.
use super::resource_contract_support::{self,ResourceContractRequest,ResourceContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F06"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-resource-contract/1.0";
pub fn worldgen_multimodal_resource_discovery_contract_model_manifest()->serde_json::Value{resource_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceContractRequest1@1","multimodal multi-study","A0")}
pub fn negotiate_worldgen_multimodal_resource_contract(request:&ResourceContractRequest)->Result<ResourceContractReceipt,resource_contract_support::ResourceContractError>{resource_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use resource_contract_support::{ResourceContractError,ResourceContractReceipt as WorldgenmultimodalResourcecontractmodelReceipt,ResourceContractRequest as WorldgenmultimodalResourcecontractmodelRequest};
