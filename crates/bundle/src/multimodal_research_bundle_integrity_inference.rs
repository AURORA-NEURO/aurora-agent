//! Bundle P32 multimodal inference signed research-object integrity feature.
use super::research_bundle_integrity_support::{manifest,release,BundleCard7,BundleReleaseRequest4,ResearchBundleIntegrityError};
pub const FEATURE_ID:&str="AFA-bundle-P32-F02";pub const CONTRACT_VERSION:&str="bundle-multimodal_research_bundle_integrity_inference/1.0";
pub fn multimodal_research_bundle_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
pub fn release_multimodal_research_bundle_integrity_inference(request:&BundleReleaseRequest4)->Result<BundleCard7,ResearchBundleIntegrityError>{release(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
