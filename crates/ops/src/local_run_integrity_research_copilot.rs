//! Ops P32 local single-study research_copilot run-integrity feature F03.
use super::run_integrity_support::{qualify,manifest,RunIntegrityCard7,RunIntegrityRequest4,RunIntegrityError};
const FEATURE_ID:&str="AFA-ops-P32-F03";const CONTRACT_VERSION:&str="ops-local-run-integrity-research_copilot/1.0";
pub fn ops_local_run_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
pub fn qualify_ops_local_run_integrity_research_copilot(request:&RunIntegrityRequest4)->Result<RunIntegrityCard7,RunIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
