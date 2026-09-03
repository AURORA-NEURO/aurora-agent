//! Conformance P32 multimodal multi-study contract_model replay-integrity feature F06.
use super::replay_integrity_support::{qualify,manifest,ReplayIntegrityCard7,ReplayIntegrityRequest4,ReplayIntegrityError};
const FEATURE_ID:&str="AFA-conformance-P32-F06";const CONTRACT_VERSION:&str="conformance-multimodal-replay-integrity-contract_model/1.0";
pub fn conformance_multimodal_replay_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
pub fn qualify_conformance_multimodal_replay_integrity_contract_model(request:&ReplayIntegrityRequest4)->Result<ReplayIntegrityCard7,ReplayIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","contract_model")}
