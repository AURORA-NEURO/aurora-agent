//! Fiber P32 local single-study research-copilot fibration-integrity feature F03.
use super::fibration_integrity_support::{certify,manifest,FibrationIntegrityCard7,FibrationIntegrityRequest4};
const FEATURE_ID:&str="AFA-fiber-P32-F03";const CONTRACT_VERSION:&str="fiber-local-fibration-integrity-research-copilot/1.0";
pub fn fiber_local_fibration_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn certify_fiber_local_fibration_integrity_research_copilot(request:&FibrationIntegrityRequest4)->Result<FibrationIntegrityCard7,super::fibration_integrity_support::FibrationIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
