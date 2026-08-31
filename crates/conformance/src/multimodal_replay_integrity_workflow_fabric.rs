//! Conformance P32 multimodal multi-study workflow_fabric replay-integrity feature F08.
use super::replay_integrity_support::{qualify,manifest,ReplayIntegrityCard7,ReplayIntegrityRequest4,ReplayIntegrityError};
const FEATURE_ID:&str="AFA-conformance-P32-F08";const CONTRACT_VERSION:&str="conformance-multimodal-replay-integrity-workflow_fabric/1.0";
pub fn conformance_multimodal_replay_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
pub fn qualify_conformance_multimodal_replay_integrity_workflow_fabric(request:&ReplayIntegrityRequest4)->Result<ReplayIntegrityCard7,ReplayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
