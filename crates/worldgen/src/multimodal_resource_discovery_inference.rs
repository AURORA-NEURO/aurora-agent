//! Worldgen P05 AFA-worldgen-P05-F02 multimodal inference.
use super::resource_discovery_support::{self,ResourceDiscoveryRequest,ResourceDiscoveryReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F02"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-resource-discovery/1.0";
pub fn worldgen_multimodal_resource_discovery_inference_manifest()->serde_json::Value{resource_discovery_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceDiscoveryRequest1@1","multimodal multi-study","A1")}
pub fn discover_worldgen_multimodal_resources(request:&ResourceDiscoveryRequest)->Result<ResourceDiscoveryReceipt,resource_discovery_support::ResourceDiscoveryError>{resource_discovery_support::discover(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use resource_discovery_support::{ResourceDiscoveryError,ResourceDiscoveryReceipt as WorldgenmultimodalResourceinferenceReceipt,ResourceDiscoveryRequest as WorldgenmultimodalResourceinferenceRequest};
