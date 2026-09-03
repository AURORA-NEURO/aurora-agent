//! Worldgen P25 prospective high-throughput inference feature F03.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F03";const CONTRACT_VERSION:&str="worldgen-throughput-contract-frontier-inference/1.0";
pub fn worldgen_throughput_contract_frontier_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","inference")}
pub fn admit_worldgen_throughput_contract_frontier(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","inference")}

