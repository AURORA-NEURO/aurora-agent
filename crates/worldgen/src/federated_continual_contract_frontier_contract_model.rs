//! Worldgen P25 federated continual autonomous contract model feature F08.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F08";const CONTRACT_VERSION:&str="worldgen-federated_continual-contract-frontier-contract_model/1.0";
pub fn worldgen_federated_continual_contract_frontier_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}
pub fn admit_worldgen_federated_contract_frontier_contract(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}

