//! Worldgen P26 multimodal multi-study contract model feature F06.
use super::limitation_closure_support::{close,manifest,LimitationClosureCard7,LimitationClosureRequest4};
const FEATURE_ID:&str="AFA-worldgen-P26-F06";const CONTRACT_VERSION:&str="worldgen-multimodal-limitation-closure-contract_model/1.0";
pub fn worldgen_multimodal_limitation_closure_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}
pub fn close_worldgen_multimodal_limitation_closure_contract(request:&LimitationClosureRequest4)->Result<LimitationClosureCard7,super::limitation_closure_support::LimitationClosureError>{close(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract model")}

