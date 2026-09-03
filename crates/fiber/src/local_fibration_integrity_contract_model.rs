//! Fiber P32 local single-study contract-model fibration-integrity feature F02.
use super::fibration_integrity_support::{certify,manifest,FibrationIntegrityCard7,FibrationIntegrityRequest4};
const FEATURE_ID:&str="AFA-fiber-P32-F02";const CONTRACT_VERSION:&str="fiber-local-fibration-integrity-contract-model/1.0";
pub fn fiber_local_fibration_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn certify_fiber_local_fibration_integrity_contract_model(request:&FibrationIntegrityRequest4)->Result<FibrationIntegrityCard7,super::fibration_integrity_support::FibrationIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
