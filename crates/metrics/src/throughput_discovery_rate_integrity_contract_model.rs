//! Metrics P32 throughput contract_model discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F07";pub const CONTRACT_VERSION:&str="metrics-throughput_discovery_rate_integrity_contract_model/1.0";
pub fn throughput_discovery_rate_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
pub fn qualify_throughput_discovery_rate_integrity_contract_model(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"throughput","contract_model")}
