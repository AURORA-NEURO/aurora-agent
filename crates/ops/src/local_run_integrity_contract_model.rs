//! Ops P32 local single-study contract_model run-integrity feature F02.
use super::run_integrity_support::{qualify,manifest,RunIntegrityCard7,RunIntegrityRequest4,RunIntegrityError};
const FEATURE_ID:&str="AFA-ops-P32-F02";const CONTRACT_VERSION:&str="ops-local-run-integrity-contract_model/1.0";
pub fn ops_local_run_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract_model")}
pub fn qualify_ops_local_run_integrity_contract_model(request:&RunIntegrityRequest4)->Result<RunIntegrityCard7,RunIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract_model")}
