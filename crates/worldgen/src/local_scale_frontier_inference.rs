//! Worldgen P29 local single-study inference feature F01.
use super::scale_frontier_support::{evaluate,manifest,ScaleFrontierCard7,ScaleFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P29-F01";const CONTRACT_VERSION:&str="worldgen-local-scale-frontier-inference/1.0";
pub fn worldgen_local_scale_frontier_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn evaluate_worldgen_local_scale_frontier(request:&ScaleFrontierRequest4)->Result<ScaleFrontierCard7,super::scale_frontier_support::ScaleFrontierError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}

