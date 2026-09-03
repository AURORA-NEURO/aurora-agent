//! Infra P32 multimodal contract-model reliability-integrity feature.
use super::reliability_integrity_support::{manifest,qualify,ReliabilityIntegrityCard7,ReliabilityIntegrityError,ReliabilityIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-infra-P32-F06";pub const CONTRACT_VERSION:&str="infra-multimodal_reliability_integrity_contract_model/1.0";
pub fn multimodal_reliability_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","contract-model")}
pub fn qualify_multimodal_reliability_integrity_contract_model(request:&ReliabilityIntegrityRequest4)->Result<ReliabilityIntegrityCard7,ReliabilityIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","contract-model")}
