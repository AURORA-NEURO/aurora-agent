//! Worldgen P27 multimodal multi-study inference feature F02.
use super::dependency_composition_support::{compose,manifest,DependencyCompositionCard7,DependencyCompositionRequest4};
const FEATURE_ID:&str="AFA-worldgen-P27-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-dependency-composition-inference/1.0";
pub fn worldgen_multimodal_dependency_composition_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn compose_worldgen_multimodal_dependency_composition(request:&DependencyCompositionRequest4)->Result<DependencyCompositionCard7,super::dependency_composition_support::DependencyCompositionError>{compose(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}

