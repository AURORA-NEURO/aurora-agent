//! Worldgen P27 local single-study workflow fabric feature F13.
use super::dependency_composition_support::{compose,manifest,DependencyCompositionCard7,DependencyCompositionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P27-F13";const CONTRACT_VERSION:&str="worldgen-local-dependency-composition-workflow_fabric/1.0";
pub fn worldgen_local_dependency_composition_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn compose_worldgen_local_dependency_composition_workflow(request:&DependencyCompositionRequest4)->Result<DependencyCompositionCard7,super::dependency_composition_support::DependencyCompositionError>{compose(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}

