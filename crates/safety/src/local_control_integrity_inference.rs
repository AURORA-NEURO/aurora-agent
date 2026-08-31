//! Safety P32 local single-study inference control-integrity feature F01.
use super::control_integrity_support::{qualify,manifest,SafetyIntegrityCard7,SafetyIntegrityRequest4,SafetyIntegrityError};
const FEATURE_ID:&str="AFA-safety-P32-F01";const CONTRACT_VERSION:&str="safety-local-control-integrity-inference/1.0";
pub fn safety_local_control_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_safety_local_control_integrity_inference(request:&SafetyIntegrityRequest4)->Result<SafetyIntegrityCard7,SafetyIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
