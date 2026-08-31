//! Influence P32 local single-study research-copilot bound-integrity feature F03.
use super::bound_integrity_support::{certify,manifest,BoundIntegrityCard7,BoundIntegrityRequest4};
const FEATURE_ID:&str="AFA-influence-P32-F03";const CONTRACT_VERSION:&str="influence-local-bound-integrity-research-copilot/1.0";
pub fn influence_local_bound_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn certify_influence_local_bound_integrity_research_copilot(request:&BoundIntegrityRequest4)->Result<BoundIntegrityCard7,super::bound_integrity_support::BoundIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
