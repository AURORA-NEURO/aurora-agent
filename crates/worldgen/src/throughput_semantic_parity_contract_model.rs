//! Worldgen P28 prospective high-throughput contract model feature F07.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F07";const CONTRACT_VERSION:&str="worldgen-throughput-semantic-parity-contract_model/1.0";
pub fn worldgen_throughput_semantic_parity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}
pub fn compare_worldgen_throughput_semantic_parity_contract(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}

