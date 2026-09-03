//! Metrics P32 throughput inference discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F03";pub const CONTRACT_VERSION:&str="metrics-throughput_discovery_rate_integrity_inference/1.0";
pub fn throughput_discovery_rate_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
pub fn qualify_throughput_discovery_rate_integrity_inference(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","inference")}
