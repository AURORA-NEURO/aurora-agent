//! Adaptive P32 local single-study inference posterior-integrity feature F01.
use super::posterior_integrity_support::{qualify,manifest,PosteriorIntegrityCard7,PosteriorIntegrityRequest4};
const FEATURE_ID:&str="AFA-adaptive-P32-F01";const CONTRACT_VERSION:&str="adaptive-local-posterior-integrity-inference/1.0";
pub fn adaptive_local_posterior_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_adaptive_local_posterior_integrity_inference(request:&PosteriorIntegrityRequest4)->Result<PosteriorIntegrityCard7,super::posterior_integrity_support::PosteriorIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
