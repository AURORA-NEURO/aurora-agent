//! Metrics P32 federated_continual contract_model discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F08";pub const CONTRACT_VERSION:&str="metrics-federated_continual_discovery_rate_integrity_contract_model/1.0";
pub fn federated_continual_discovery_rate_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated_continual","contract_model")}
pub fn qualify_federated_continual_discovery_rate_integrity_contract_model(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"federated_continual","contract_model")}
