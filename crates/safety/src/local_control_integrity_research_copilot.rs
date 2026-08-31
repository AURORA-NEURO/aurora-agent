//! Safety P32 local single-study research_copilot control-integrity feature F03.
use super::control_integrity_support::{qualify,manifest,SafetyIntegrityCard7,SafetyIntegrityRequest4,SafetyIntegrityError};
const FEATURE_ID:&str="AFA-safety-P32-F03";const CONTRACT_VERSION:&str="safety-local-control-integrity-research_copilot/1.0";
pub fn safety_local_control_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
pub fn qualify_safety_local_control_integrity_research_copilot(request:&SafetyIntegrityRequest4)->Result<SafetyIntegrityCard7,SafetyIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research_copilot")}
