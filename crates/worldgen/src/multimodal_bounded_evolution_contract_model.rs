//! Worldgen P32 multimodal multi-study contract model feature F06.
use super::bounded_evolution_support::{promote,manifest,BoundedEvolutionCard7,BoundedEvolutionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P32-F06";const CONTRACT_VERSION:&str="worldgen-multimodal-bounded-evolution-contract_model/1.0";
pub fn worldgen_multimodal_bounded_evolution_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
pub fn promote_worldgen_multimodal_bounded_evolution_contract(request:&BoundedEvolutionRequest4)->Result<BoundedEvolutionCard7,super::bounded_evolution_support::BoundedEvolutionError>{promote(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
