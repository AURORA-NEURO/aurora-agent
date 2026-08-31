//! Ops P32 local single-study inference run-integrity feature F01.
use super::run_integrity_support::{qualify,manifest,RunIntegrityCard7,RunIntegrityRequest4,RunIntegrityError};
const FEATURE_ID:&str="AFA-ops-P32-F01";const CONTRACT_VERSION:&str="ops-local-run-integrity-inference/1.0";
pub fn ops_local_run_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_ops_local_run_integrity_inference(request:&RunIntegrityRequest4)->Result<RunIntegrityCard7,RunIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
