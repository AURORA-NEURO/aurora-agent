//! Policy P32 local single-study contract-model grant-integrity feature F02.
use super::grant_integrity_support::{qualify,manifest,GrantIntegrityCard7,GrantIntegrityRequest4};
const FEATURE_ID:&str="AFA-policy-P32-F02";const CONTRACT_VERSION:&str="policy-local-grant-integrity-contract_model/1.0";
pub fn policy_local_grant_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn qualify_policy_local_grant_integrity_contract_model(request:&GrantIntegrityRequest4)->Result<GrantIntegrityCard7,super::grant_integrity_support::GrantIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
