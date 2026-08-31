//! Metrics P32 federated_continual research_copilot discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F12";pub const CONTRACT_VERSION:&str="metrics-federated_continual_discovery_rate_integrity_research_copilot/1.0";
pub fn federated_continual_discovery_rate_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated_continual","research_copilot")}
pub fn qualify_federated_continual_discovery_rate_integrity_research_copilot(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated_continual","research_copilot")}
