//! Worldgen P28 local single-study contract model feature F05.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F05";const CONTRACT_VERSION:&str="worldgen-local-semantic-parity-contract_model/1.0";
pub fn worldgen_local_semantic_parity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn compare_worldgen_local_semantic_parity_contract(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}

