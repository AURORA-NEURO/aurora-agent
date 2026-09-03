//! IDs P32 multimodal multi-study inference feature F02.
use super::identity_continuity_support::{qualify,manifest,IdentityContinuityCard7,IdentityContinuityRequest4};
const FEATURE_ID:&str="AFA-ids-P32-F02";const CONTRACT_VERSION:&str="ids-multimodal-identity-continuity-inference/1.0";
pub fn ids_multimodal_identity_continuity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn qualify_ids_multimodal_identity_continuity(request:&IdentityContinuityRequest4)->Result<IdentityContinuityCard7,super::identity_continuity_support::IdentityContinuityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
