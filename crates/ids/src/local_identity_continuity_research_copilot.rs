//! IDs P32 local single-study research copilot feature F09.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F09";const CONTRACT_VERSION:&str="ids-local-identity-continuity-research_copilot/1.0";
pub fn ids_local_identity_continuity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
pub fn qualify_ids_local_identity_continuity_copilot(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research copilot")}
