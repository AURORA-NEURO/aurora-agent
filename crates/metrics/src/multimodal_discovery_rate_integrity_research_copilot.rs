//! Metrics P32 multimodal research_copilot discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F10";pub const CONTRACT_VERSION:&str="metrics-multimodal_discovery_rate_integrity_research_copilot/1.0";
pub fn multimodal_discovery_rate_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","research_copilot")}
pub fn qualify_multimodal_discovery_rate_integrity_research_copilot(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","research_copilot")}
