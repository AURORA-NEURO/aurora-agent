//! Infra P32 local contract-model reliability-integrity feature.
use super::reliability_integrity_support::{manifest,qualify,ReliabilityIntegrityCard7,ReliabilityIntegrityError,ReliabilityIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-infra-P32-F02";pub const CONTRACT_VERSION:&str="infra-local_reliability_integrity_contract_model/1.0";
pub fn local_reliability_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
pub fn qualify_local_reliability_integrity_contract_model(request:&ReliabilityIntegrityRequest4)->Result<ReliabilityIntegrityCard7,ReliabilityIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","contract-model")}
