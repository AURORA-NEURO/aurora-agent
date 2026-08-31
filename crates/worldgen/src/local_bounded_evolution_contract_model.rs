//! Worldgen P32 local single-study contract model feature F05.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F05";const CONTRACT_VERSION:&str="worldgen-local-bounded-evolution-contract_model/1.0";
pub fn worldgen_local_bounded_evolution_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn promote_worldgen_local_bounded_evolution_contract(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
