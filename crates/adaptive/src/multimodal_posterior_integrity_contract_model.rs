//! Adaptive P32 multimodal multi-study contract-model posterior-integrity feature F06.
use super::posterior_integrity_support::{qualify,manifest,PosteriorIntegrityCard7,PosteriorIntegrityRequest4};
const FEATURE_ID:&str="AFA-adaptive-P32-F06";const CONTRACT_VERSION:&str="adaptive-multimodal-posterior-integrity-contract_model/1.0";
pub fn adaptive_multimodal_posterior_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract-model")}
pub fn qualify_adaptive_multimodal_posterior_integrity_contract_model(request:&PosteriorIntegrityRequest4)->Result<PosteriorIntegrityCard7,super::posterior_integrity_support::PosteriorIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract-model")}
