//! IDs P32 local single-study workflow fabric feature F13.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F13";const CONTRACT_VERSION:&str="ids-local-identity-continuity-workflow_fabric/1.0";
pub fn ids_local_identity_continuity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
pub fn qualify_ids_local_identity_continuity_workflow(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow fabric")}
