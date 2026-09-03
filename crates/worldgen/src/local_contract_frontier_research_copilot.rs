//! Worldgen P25 local single-study research copilot feature F09.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F09";const CONTRACT_VERSION:&str="worldgen-local-contract-frontier-research_copilot/1.0";
pub fn worldgen_local_contract_frontier_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn admit_worldgen_local_contract_frontier_copilot(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}

