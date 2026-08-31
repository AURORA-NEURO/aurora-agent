//! Worldgen P25 local single-study contract model feature F05.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F05";const CONTRACT_VERSION:&str="worldgen-local-contract-frontier-contract_model/1.0";
pub fn worldgen_local_contract_frontier_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn admit_worldgen_local_contract_frontier_contract(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}

