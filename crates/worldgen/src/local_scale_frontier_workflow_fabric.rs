//! Worldgen P29 local single-study workflow fabric feature F13.
use super::scale_frontier_support::{evaluate,manifest,ScaleFrontierCard7,ScaleFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P29-F13";const CONTRACT_VERSION:&str="worldgen-local-scale-frontier-workflow_fabric/1.0";
pub fn worldgen_local_scale_frontier_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn evaluate_worldgen_local_scale_frontier_workflow(request:&ScaleFrontierRequest4)->Result<ScaleFrontierCard7,super::scale_frontier_support::ScaleFrontierError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}

