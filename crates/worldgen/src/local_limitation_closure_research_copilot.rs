//! Worldgen P26 local single-study research copilot feature F09.
use super::limitation_closure_support::{close,manifest,LimitationClosureCard7,LimitationClosureRequest4};
const FEATURE_ID:&str="AFA-worldgen-P26-F09";const CONTRACT_VERSION:&str="worldgen-local-limitation-closure-research_copilot/1.0";
pub fn worldgen_local_limitation_closure_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn close_worldgen_local_limitation_closure_copilot(request:&LimitationClosureRequest4)->Result<LimitationClosureCard7,super::limitation_closure_support::LimitationClosureError>{close(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}

