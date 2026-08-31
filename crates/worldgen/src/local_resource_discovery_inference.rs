//! Worldgen P05 AFA-worldgen-P05-F01 local inference.
use super::resource_discovery_support::{self,ResourceDiscoveryRequest,ResourceDiscoveryReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-resource-discovery/1.0";
pub fn worldgen_local_resource_discovery_inference_manifest()->serde_json::Value{resource_discovery_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceDiscoveryRequest1@1","local single-study","A0")}
pub fn discover_worldgen_local_resources(request:&ResourceDiscoveryRequest)->Result<ResourceDiscoveryReceipt,resource_discovery_support::ResourceDiscoveryError>{resource_discovery_support::discover(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false)}
pub use resource_discovery_support::{ResourceDiscoveryError,ResourceDiscoveryReceipt as WorldgenlocalResourceinferenceReceipt,ResourceDiscoveryRequest as WorldgenlocalResourceinferenceRequest};
