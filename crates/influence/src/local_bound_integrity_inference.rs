//! Influence P32 local single-study inference bound-integrity feature F01.
use super::bound_integrity_support::{certify,manifest,BoundIntegrityCard7,BoundIntegrityRequest4};
const FEATURE_ID:&str="AFA-influence-P32-F01";const CONTRACT_VERSION:&str="influence-local-bound-integrity-inference/1.0";
pub fn influence_local_bound_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn certify_influence_local_bound_integrity_inference(request:&BoundIntegrityRequest4)->Result<BoundIntegrityCard7,super::bound_integrity_support::BoundIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
