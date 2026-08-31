//! Worldgen P28 local single-study inference feature F01.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F01";const CONTRACT_VERSION:&str="worldgen-local-semantic-parity-inference/1.0";
pub fn worldgen_local_semantic_parity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn compare_worldgen_local_semantic_parity(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}

