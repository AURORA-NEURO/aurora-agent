//! Policy P32 multimodal multi-study research-copilot grant-integrity feature F07.
use super::grant_integrity_support::{qualify,manifest,GrantIntegrityCard7,GrantIntegrityRequest4};
const FEATURE_ID:&str="AFA-policy-P32-F07";const CONTRACT_VERSION:&str="policy-multimodal-grant-integrity-research_copilot/1.0";
pub fn policy_multimodal_grant_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research-copilot")}
pub fn qualify_policy_multimodal_grant_integrity_research_copilot(request:&GrantIntegrityRequest4)->Result<GrantIntegrityCard7,super::grant_integrity_support::GrantIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","research-copilot")}
