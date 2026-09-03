//! Adaptive P32 local single-study workflow-fabric posterior-integrity feature F04.
use super::posterior_integrity_support::{qualify,manifest,PosteriorIntegrityCard7,PosteriorIntegrityRequest4};
const FEATURE_ID:&str="AFA-adaptive-P32-F04";const CONTRACT_VERSION:&str="adaptive-local-posterior-integrity-workflow_fabric/1.0";
pub fn adaptive_local_posterior_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn qualify_adaptive_local_posterior_integrity_workflow_fabric(request:&PosteriorIntegrityRequest4)->Result<PosteriorIntegrityCard7,super::posterior_integrity_support::PosteriorIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
