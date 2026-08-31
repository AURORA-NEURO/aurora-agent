//! Worldgen P27 local single-study contract model feature F05.
use super::dependency_composition_support::{compose,manifest,DependencyCompositionCard7,DependencyCompositionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P27-F05";const CONTRACT_VERSION:&str="worldgen-local-dependency-composition-contract_model/1.0";
pub fn worldgen_local_dependency_composition_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn compose_worldgen_local_dependency_composition_contract(request:&DependencyCompositionRequest4)->Result<DependencyCompositionCard7,super::dependency_composition_support::DependencyCompositionError>{compose(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}

