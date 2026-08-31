//! Worldgen P25 multimodal multi-study workflow fabric feature F14.
use super::contract_frontier_support::{admit,manifest,ContractFrontierCard7,ContractFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P25-F14";const CONTRACT_VERSION:&str="worldgen-multimodal-contract-frontier-workflow_fabric/1.0";
pub fn worldgen_multimodal_contract_frontier_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}
pub fn admit_worldgen_multimodal_contract_frontier_workflow(request:&ContractFrontierRequest4)->Result<ContractFrontierCard7,super::contract_frontier_support::ContractFrontierError>{admit(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}

