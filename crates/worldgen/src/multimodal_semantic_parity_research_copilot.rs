//! Worldgen P28 multimodal multi-study research copilot feature F10.
use super::semantic_parity_support::{compare,manifest,SemanticParityCard7,SemanticParityRequest4};
const FEATURE_ID:&str="AFA-worldgen-P28-F10";const CONTRACT_VERSION:&str="worldgen-multimodal-semantic-parity-research_copilot/1.0";
pub fn worldgen_multimodal_semantic_parity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
pub fn compare_worldgen_multimodal_semantic_parity_copilot(request:&SemanticParityRequest4)->Result<SemanticParityCard7,super::semantic_parity_support::SemanticParityError>{compare(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}

