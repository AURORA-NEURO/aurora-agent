//! Policy P32 local single-study workflow-fabric grant-integrity feature F04.
use super::grant_integrity_support::{qualify,manifest,GrantIntegrityCard7,GrantIntegrityRequest4};
const FEATURE_ID:&str="AFA-policy-P32-F04";const CONTRACT_VERSION:&str="policy-local-grant-integrity-workflow_fabric/1.0";
pub fn policy_local_grant_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn qualify_policy_local_grant_integrity_workflow_fabric(request:&GrantIntegrityRequest4)->Result<GrantIntegrityCard7,super::grant_integrity_support::GrantIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
