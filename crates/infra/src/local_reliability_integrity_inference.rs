//! Infra P32 local inference reliability-integrity feature.
use super::reliability_integrity_support::{manifest,qualify,ReliabilityIntegrityCard7,ReliabilityIntegrityError,ReliabilityIntegrityRequest4};
pub const FEATURE_ID:&str="AFA-infra-P32-F01";pub const CONTRACT_VERSION:&str="infra-local_reliability_integrity_inference/1.0";
pub fn local_reliability_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","inference")}
pub fn qualify_local_reliability_integrity_inference(request:&ReliabilityIntegrityRequest4)->Result<ReliabilityIntegrityCard7,ReliabilityIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","inference")}
