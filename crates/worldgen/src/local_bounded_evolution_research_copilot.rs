//! Worldgen P32 local single-study research copilot feature F09.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F09";const CONTRACT_VERSION:&str="worldgen-local-bounded-evolution-research_copilot/1.0";
pub fn worldgen_local_bounded_evolution_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn promote_worldgen_local_bounded_evolution_copilot(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
