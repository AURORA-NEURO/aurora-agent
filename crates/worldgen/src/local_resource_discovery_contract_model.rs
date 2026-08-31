//! Worldgen P05 AFA-worldgen-P05-F05 local contract_model.
use super::resource_contract_support::{self,ResourceContractRequest,ResourceContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F05"; pub const CONTRACT_VERSION:&str="worldgen-local-resource-contract/1.0";
pub fn worldgen_local_resource_discovery_contract_model_manifest()->serde_json::Value{resource_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceContractRequest1@1","local single-study","A0")}
pub fn negotiate_worldgen_local_resource_contract(request:&ResourceContractRequest)->Result<ResourceContractReceipt,resource_contract_support::ResourceContractError>{resource_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use resource_contract_support::{ResourceContractError,ResourceContractReceipt as WorldgenlocalResourcecontractmodelReceipt,ResourceContractRequest as WorldgenlocalResourcecontractmodelRequest};
