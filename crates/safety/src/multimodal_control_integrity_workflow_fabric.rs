//! Safety P32 multimodal multi-study workflow_fabric control-integrity feature F08.
use super::control_integrity_support::{qualify,manifest,SafetyIntegrityCard7,SafetyIntegrityRequest4,SafetyIntegrityError};
const FEATURE_ID:&str="AFA-safety-P32-F08";const CONTRACT_VERSION:&str="safety-multimodal-control-integrity-workflow_fabric/1.0";
pub fn safety_multimodal_control_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
pub fn qualify_safety_multimodal_control_integrity_workflow_fabric(request:&SafetyIntegrityRequest4)->Result<SafetyIntegrityCard7,SafetyIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow_fabric")}
