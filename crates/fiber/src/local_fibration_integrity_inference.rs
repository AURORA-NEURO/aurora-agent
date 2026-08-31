//! Fiber P32 local single-study inference fibration-integrity feature F01.
use super::fibration_integrity_support::{certify,manifest,FibrationIntegrityCard7,FibrationIntegrityRequest4};
const FEATURE_ID:&str="AFA-fiber-P32-F01";const CONTRACT_VERSION:&str="fiber-local-fibration-integrity-inference/1.0";
pub fn fiber_local_fibration_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn certify_fiber_local_fibration_integrity_inference(request:&FibrationIntegrityRequest4)->Result<FibrationIntegrityCard7,super::fibration_integrity_support::FibrationIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
