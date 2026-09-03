//! Tokens P32 multimodal multi-study workflow-fabric compression-integrity feature F08.
use super::compression_integrity_support::{qualify,manifest,CompressionIntegrityCard7,CompressionIntegrityRequest4};
const FEATURE_ID:&str="AFA-tokens-P32-F08";const CONTRACT_VERSION:&str="tokens-multimodal-compression-integrity-workflow_fabric/1.0";
pub fn tokens_multimodal_compression_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow-fabric")}
pub fn qualify_tokens_multimodal_compression_integrity_workflow_fabric(request:&CompressionIntegrityRequest4)->Result<CompressionIntegrityCard7,super::compression_integrity_support::CompressionIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study","workflow-fabric")}
