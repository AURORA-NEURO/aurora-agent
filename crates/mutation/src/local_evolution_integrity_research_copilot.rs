//! Mutation P32 local research_copilot evolution-integrity feature.
use super::evolution_integrity_support::{manifest,qualify,EvolutionCard7,EvolutionIntegrityError,EvolutionRequest4};
pub const FEATURE_ID:&str="AFA-mutation-P32-F09";pub const CONTRACT_VERSION:&str="mutation-local_evolution_integrity_research_copilot/1.0";
pub fn local_evolution_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
pub fn qualify_local_evolution_integrity_research_copilot(request:&EvolutionRequest4)->Result<EvolutionCard7,EvolutionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
