//! Worldgen P26 multimodal multi-study workflow fabric feature F14.
use super::limitation_closure_support::{close,manifest,LimitationClosureCard7,LimitationClosureRequest4};
const FEATURE_ID:&str="AFA-worldgen-P26-F14";const CONTRACT_VERSION:&str="worldgen-multimodal-limitation-closure-workflow_fabric/1.0";
pub fn worldgen_multimodal_limitation_closure_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}
pub fn close_worldgen_multimodal_limitation_closure_workflow(request:&LimitationClosureRequest4)->Result<LimitationClosureCard7,super::limitation_closure_support::LimitationClosureError>{close(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow fabric")}

