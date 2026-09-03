//! Worldgen P25 prospective high-throughput research copilot feature F11.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F11";const CONTRACT_VERSION:&str="worldgen-throughput-contract-frontier-research_copilot/1.0";
pub fn worldgen_throughput_contract_frontier_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}
pub fn admit_worldgen_throughput_contract_frontier_copilot(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}

