//! Scope P32 local single-study workflow fabric feature F13.
use super::continuity_frontier_support::{qualify,manifest,ScopeContinuityCard7,ScopeContinuityRequest4};
const FEATURE_ID:&str="AFA-scope-P32-F13";const CONTRACT_VERSION:&str="scope-local-continuity-frontier-workflow_fabric/1.0";
pub fn scope_local_continuity_frontier_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn qualify_scope_local_continuity_frontier_workflow(request:&ScopeContinuityRequest4)->Result<ScopeContinuityCard7,super::continuity_frontier_support::ScopeContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
