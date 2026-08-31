//! Worldgen P05 AFA-worldgen-P05-F03 throughput inference.
use super::resource_discovery_support::{self,ResourceDiscoveryRequest,ResourceDiscoveryReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F03"; pub const CONTRACT_VERSION:&str="worldgen-throughput-resource-discovery/1.0";
pub fn worldgen_throughput_resource_discovery_inference_manifest()->serde_json::Value{resource_discovery_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceDiscoveryRequest1@1","prospective high-throughput","A1")}
pub fn discover_worldgen_throughput_resources(request:&ResourceDiscoveryRequest)->Result<ResourceDiscoveryReceipt,resource_discovery_support::ResourceDiscoveryError>{resource_discovery_support::discover(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",false)}
pub use resource_discovery_support::{ResourceDiscoveryError,ResourceDiscoveryReceipt as WorldgenthroughputResourceinferenceReceipt,ResourceDiscoveryRequest as WorldgenthroughputResourceinferenceRequest};
