//! Policy P32 local single-study inference grant-integrity feature F01.
use super::grant_integrity_support::{qualify,manifest,GrantIntegrityCard7,GrantIntegrityRequest4};
const FEATURE_ID:&str="AFA-policy-P32-F01";const CONTRACT_VERSION:&str="policy-local-grant-integrity-inference/1.0";
pub fn policy_local_grant_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_policy_local_grant_integrity_inference(request:&GrantIntegrityRequest4)->Result<GrantIntegrityCard7,super::grant_integrity_support::GrantIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
