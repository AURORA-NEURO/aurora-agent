//! Influence P32 local single-study contract-model bound-integrity feature F02.
use super::bound_integrity_support::{certify,manifest,BoundIntegrityCard7,BoundIntegrityRequest4};
const FEATURE_ID:&str="AFA-influence-P32-F02";const CONTRACT_VERSION:&str="influence-local-bound-integrity-contract-model/1.0";
pub fn influence_local_bound_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn certify_influence_local_bound_integrity_contract_model(request:&BoundIntegrityRequest4)->Result<BoundIntegrityCard7,super::bound_integrity_support::BoundIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
