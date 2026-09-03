//! Adaptive P32 local single-study contract-model posterior-integrity feature F02.
use super::posterior_integrity_support::{qualify,manifest,PosteriorIntegrityCard7,PosteriorIntegrityRequest4};
const FEATURE_ID:&str="AFA-adaptive-P32-F02";const CONTRACT_VERSION:&str="adaptive-local-posterior-integrity-contract_model/1.0";
pub fn adaptive_local_posterior_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn qualify_adaptive_local_posterior_integrity_contract_model(request:&PosteriorIntegrityRequest4)->Result<PosteriorIntegrityCard7,super::posterior_integrity_support::PosteriorIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
