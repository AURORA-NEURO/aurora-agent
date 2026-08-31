//! Worldgen P32 multimodal multi-study research copilot feature F10.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F10";const CONTRACT_VERSION:&str="worldgen-multimodal-bounded-evolution-research_copilot/1.0";
pub fn worldgen_multimodal_bounded_evolution_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
pub fn promote_worldgen_multimodal_bounded_evolution_copilot(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
