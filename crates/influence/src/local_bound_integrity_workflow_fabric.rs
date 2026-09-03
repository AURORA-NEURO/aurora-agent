//! Influence P32 local single-study workflow-fabric bound-integrity feature F04.
use super::bound_integrity_support::{certify,manifest,BoundIntegrityCard7,BoundIntegrityRequest4};
const FEATURE_ID:&str="AFA-influence-P32-F04";const CONTRACT_VERSION:&str="influence-local-bound-integrity-workflow-fabric/1.0";
pub fn influence_local_bound_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn certify_influence_local_bound_integrity_workflow_fabric(request:&BoundIntegrityRequest4)->Result<BoundIntegrityCard7,super::bound_integrity_support::BoundIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
