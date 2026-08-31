//! Worldgen P05 AFA-worldgen-P05-F08 federated_continual contract_model.
use super::resource_contract_support::{self,ResourceContractRequest,ResourceContractReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F08"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-resource-contract/1.0";
pub fn worldgen_federated_continual_resource_discovery_contract_model_manifest()->serde_json::Value{resource_contract_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceContractRequest1@1","federated continual autonomous","A2")}
pub fn negotiate_worldgen_federated_continual_resource_contract(request:&ResourceContractRequest)->Result<ResourceContractReceipt,resource_contract_support::ResourceContractError>{resource_contract_support::negotiate(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use resource_contract_support::{ResourceContractError,ResourceContractReceipt as Worldgenfederated_continualResourcecontractmodelReceipt,ResourceContractRequest as Worldgenfederated_continualResourcecontractmodelRequest};
