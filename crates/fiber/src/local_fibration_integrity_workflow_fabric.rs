//! Fiber P32 local single-study workflow-fabric fibration-integrity feature F04.
use super::fibration_integrity_support::{certify,manifest,FibrationIntegrityCard7,FibrationIntegrityRequest4};
const FEATURE_ID:&str="AFA-fiber-P32-F04";const CONTRACT_VERSION:&str="fiber-local-fibration-integrity-workflow-fabric/1.0";
pub fn fiber_local_fibration_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn certify_fiber_local_fibration_integrity_workflow_fabric(request:&FibrationIntegrityRequest4)->Result<FibrationIntegrityCard7,super::fibration_integrity_support::FibrationIntegrityError>{certify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
