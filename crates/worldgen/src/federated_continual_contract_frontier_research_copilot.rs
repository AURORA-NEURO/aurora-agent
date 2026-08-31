//! Worldgen P25 federated continual autonomous research copilot feature F12.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F12";const CONTRACT_VERSION:&str="worldgen-federated_continual-contract-frontier-research_copilot/1.0";
pub fn worldgen_federated_continual_contract_frontier_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","research copilot")}
pub fn admit_worldgen_federated_contract_frontier_copilot(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","research copilot")}

