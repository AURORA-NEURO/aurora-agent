//! Ops P32 multimodal multi-study workflow_fabric run-integrity feature F08.
use super::run_integrity_support::{qualify,manifest,RunIntegrityCard7,RunIntegrityRequest4,RunIntegrityError};
const FEATURE_ID:&str="AFA-ops-P32-F08";const CONTRACT_VERSION:&str="ops-multimodal-run-integrity-workflow_fabric/1.0";
pub fn ops_multimodal_run_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
pub fn qualify_ops_multimodal_run_integrity_workflow_fabric(request:&RunIntegrityRequest4)->Result<RunIntegrityCard7,RunIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
