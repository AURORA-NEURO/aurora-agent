//! Metrics P32 local workflow_fabric discovery-rate integrity feature.
use super::discovery_rate_integrity_support::{manifest,qualify,DiscoveryRateCard7,DiscoveryRateIntegrityError,DiscoveryRateRequest4};
pub const FEATURE_ID:&str="AFA-metrics-P32-F13";pub const CONTRACT_VERSION:&str="metrics-local_discovery_rate_integrity_workflow_fabric/1.0";
pub fn local_discovery_rate_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
pub fn qualify_local_discovery_rate_integrity_workflow_fabric(request:&DiscoveryRateRequest4)->Result<DiscoveryRateCard7,DiscoveryRateIntegrityError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local","workflow_fabric")}
