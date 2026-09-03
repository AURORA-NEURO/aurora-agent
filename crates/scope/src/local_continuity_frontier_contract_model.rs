//! Scope P32 local single-study contract model feature F05.
use super::continuity_frontier_support::{qualify,manifest,ScopeContinuityCard7,ScopeContinuityRequest4};
const FEATURE_ID:&str="AFA-scope-P32-F05";const CONTRACT_VERSION:&str="scope-local-continuity-frontier-contract_model/1.0";
pub fn scope_local_continuity_frontier_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn qualify_scope_local_continuity_frontier_contract(request:&ScopeContinuityRequest4)->Result<ScopeContinuityCard7,super::continuity_frontier_support::ScopeContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
