//! Tokens P32 local single-study inference compression-integrity feature F01.
use super::compression_integrity_support::{qualify,manifest,CompressionIntegrityCard7,CompressionIntegrityRequest4};
const FEATURE_ID:&str="AFA-tokens-P32-F01";const CONTRACT_VERSION:&str="tokens-local-compression-integrity-inference/1.0";
pub fn tokens_local_compression_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
pub fn qualify_tokens_local_compression_integrity_inference(request:&CompressionIntegrityRequest4)->Result<CompressionIntegrityCard7,super::compression_integrity_support::CompressionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","inference")}
