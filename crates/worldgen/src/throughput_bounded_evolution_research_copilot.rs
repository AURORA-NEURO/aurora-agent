//! Worldgen P32 prospective high-throughput research copilot feature F11.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F11";const CONTRACT_VERSION:&str="worldgen-throughput-bounded-evolution-research_copilot/1.0";
pub fn worldgen_throughput_bounded_evolution_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}
pub fn promote_worldgen_throughput_bounded_evolution_copilot(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","research copilot")}
