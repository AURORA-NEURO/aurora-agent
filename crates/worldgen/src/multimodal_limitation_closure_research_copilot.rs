//! Worldgen P26 multimodal multi-study research copilot feature F10.
use super::limitation_closure_support::{close,manifest,LimitationClosureCard7,LimitationClosureRequest4};
const FEATURE_ID:&str="AFA-worldgen-P26-F10";const CONTRACT_VERSION:&str="worldgen-multimodal-limitation-closure-research_copilot/1.0";
pub fn worldgen_multimodal_limitation_closure_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}
pub fn close_worldgen_multimodal_limitation_closure_copilot(request:&LimitationClosureRequest4)->Result<LimitationClosureCard7,super::limitation_closure_support::LimitationClosureError>{close(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research copilot")}

