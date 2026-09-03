//! Worldgen P32 multimodal multi-study inference feature F02.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-bounded-evolution-inference/1.0";
pub fn worldgen_multimodal_bounded_evolution_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn promote_worldgen_multimodal_bounded_evolution(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
