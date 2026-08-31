//! IDs P32 federated continual autonomous workflow fabric feature F16.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F16";const CONTRACT_VERSION:&str="ids-federated_continual-identity-continuity-workflow_fabric/1.0";
pub fn ids_federated_continual_identity_continuity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","workflow fabric")}
pub fn qualify_ids_federated_identity_continuity_workflow(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous","workflow fabric")}
