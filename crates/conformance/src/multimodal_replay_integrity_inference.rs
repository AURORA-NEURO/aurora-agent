//! Conformance P32 multimodal multi-study inference replay-integrity feature F05.
use super::replay_integrity_support::{qualify,manifest,ReplayIntegrityCard7,ReplayIntegrityRequest4,ReplayIntegrityError};
const FEATURE_ID:&str="AFA-conformance-P32-F05";const CONTRACT_VERSION:&str="conformance-multimodal-replay-integrity-inference/1.0";
pub fn conformance_multimodal_replay_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
pub fn qualify_conformance_multimodal_replay_integrity_inference(request:&ReplayIntegrityRequest4)->Result<ReplayIntegrityCard7,ReplayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","inference")}
