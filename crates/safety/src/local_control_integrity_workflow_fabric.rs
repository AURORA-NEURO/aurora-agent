//! Safety P32 local single-study workflow_fabric control-integrity feature F04.
use super::control_integrity_support::{qualify,manifest,SafetyIntegrityCard7,SafetyIntegrityRequest4,SafetyIntegrityError};
const FEATURE_ID:&str="AFA-safety-P32-F04";const CONTRACT_VERSION:&str="safety-local-control-integrity-workflow_fabric/1.0";
pub fn safety_local_control_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
pub fn qualify_safety_local_control_integrity_workflow_fabric(request:&SafetyIntegrityRequest4)->Result<SafetyIntegrityCard7,SafetyIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow_fabric")}
