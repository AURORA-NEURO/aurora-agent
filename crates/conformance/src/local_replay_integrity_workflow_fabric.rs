//! Conformance P32 local single-study workflow_fabric replay-integrity feature F04.
use super::replay_integrity_support::{qualify,manifest,ReplayIntegrityCard7,ReplayIntegrityRequest4,ReplayIntegrityError};
const FEATURE_ID:&str="AFA-conformance-P32-F04";const CONTRACT_VERSION:&str="conformance-local-replay-integrity-workflow_fabric/1.0";
pub fn conformance_local_replay_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
pub fn qualify_conformance_local_replay_integrity_workflow_fabric(request:&ReplayIntegrityRequest4)->Result<ReplayIntegrityCard7,ReplayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
