//! IDs P32 federated continual autonomous contract model feature F08.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F08";const CONTRACT_VERSION:&str="ids-federated_continual-identity-continuity-contract_model/1.0";
pub fn ids_federated_continual_identity_continuity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}
pub fn qualify_ids_federated_identity_continuity_contract(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","contract model")}
