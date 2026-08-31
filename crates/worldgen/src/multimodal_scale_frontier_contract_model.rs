//! Worldgen P29 multimodal multi-study contract model feature F06.
use super::scale_frontier_support::{evaluate,manifest,ScaleFrontierCard7,ScaleFrontierRequest4};
const FEATURE_ID:&str="AFA-worldgen-P29-F06";const CONTRACT_VERSION:&str="worldgen-multimodal-scale-frontier-contract_model/1.0";
pub fn worldgen_multimodal_scale_frontier_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
pub fn evaluate_worldgen_multimodal_scale_frontier_contract(request:&ScaleFrontierRequest4)->Result<ScaleFrontierCard7,super::scale_frontier_support::ScaleFrontierError>{evaluate(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}

