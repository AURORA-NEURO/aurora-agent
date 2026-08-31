//! Tokens P32 local single-study contract-model compression-integrity feature F02.
use super::compression_integrity_support::{qualify,manifest,CompressionIntegrityCard7,CompressionIntegrityRequest4};
const FEATURE_ID:&str="AFA-tokens-P32-F02";const CONTRACT_VERSION:&str="tokens-local-compression-integrity-contract_model/1.0";
pub fn tokens_local_compression_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
pub fn qualify_tokens_local_compression_integrity_contract_model(request:&CompressionIntegrityRequest4)->Result<CompressionIntegrityCard7,super::compression_integrity_support::CompressionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","contract-model")}
