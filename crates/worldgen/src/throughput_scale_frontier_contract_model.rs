//! Worldgen P29 prospective high-throughput contract model feature F07.
use super::scale_frontier_support::{evaluate,manifest,ScaleFrontierCard7,ScaleFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P29-F07";const CONTRACT_VERSION:&str="worldgen-throughput-scale-frontier-contract_model/1.0";
pub fn worldgen_throughput_scale_frontier_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}
pub fn evaluate_worldgen_throughput_scale_frontier_contract(request:&ScaleFrontierRequest4)->Result<ScaleFrontierCard7,super::scale_frontier_support::ScaleFrontierError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput","contract model")}

