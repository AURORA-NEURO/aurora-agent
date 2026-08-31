//! Worldgen P29 multimodal multi-study inference feature F02.
use super::scale_frontier_support::{evaluate,manifest,ScaleFrontierCard7,ScaleFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P29-F02";const CONTRACT_VERSION:&str="worldgen-multimodal-scale-frontier-inference/1.0";
pub fn worldgen_multimodal_scale_frontier_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn evaluate_worldgen_multimodal_scale_frontier(request:&ScaleFrontierRequest4)->Result<ScaleFrontierCard7,super::scale_frontier_support::ScaleFrontierError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}

