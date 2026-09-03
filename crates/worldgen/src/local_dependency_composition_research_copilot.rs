//! Worldgen P27 local single-study research copilot feature F09.
use super::dependency_composition_support::{compose,manifest,DependencyCompositionCard7,DependencyCompositionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P27-F09";const CONTRACT_VERSION:&str="worldgen-local-dependency-composition-research_copilot/1.0";
pub fn worldgen_local_dependency_composition_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn compose_worldgen_local_dependency_composition_copilot(request:&DependencyCompositionRequest4)->Result<DependencyCompositionCard7,super::dependency_composition_support::DependencyCompositionError>{compose(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}

