//! Worldgen P28 multimodal multi-study inference feature F02.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-semantic-parity-inference/1.0";
pub fn worldgen_multimodal_semantic_parity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn compare_worldgen_multimodal_semantic_parity(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}

