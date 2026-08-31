//! Metrics P32 multimodal inference discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F02";pub const CONTRACT_VERSION:&str="metrics-multimodal_discovery_rate_integrity_inference/1.0";
pub fn multimodal_discovery_rate_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
pub fn qualify_multimodal_discovery_rate_integrity_inference(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
