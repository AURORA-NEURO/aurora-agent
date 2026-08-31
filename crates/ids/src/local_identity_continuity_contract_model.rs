//! IDs P32 local single-study contract model feature F05.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F05";const CONTRACT_VERSION:&str="ids-local-identity-continuity-contract_model/1.0";
pub fn ids_local_identity_continuity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
pub fn qualify_ids_local_identity_continuity_contract(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract model")}
