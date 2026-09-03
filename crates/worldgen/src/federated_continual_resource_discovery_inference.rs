//! Worldgen P05 AFA-worldgen-P05-F04 federated_continual inference.
use super::resource_discovery_support::{self,ResourceDiscoveryRequest,ResourceDiscoveryReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-resource-discovery/1.0";
pub fn worldgen_federated_continual_resource_discovery_inference_manifest()->serde_json::Value{resource_discovery_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceDiscoveryRequest1@1","federated continual autonomous","A1")}
pub fn discover_worldgen_federated_continual_resources(request:&ResourceDiscoveryRequest)->Result<ResourceDiscoveryReceipt,resource_discovery_support::ResourceDiscoveryError>{resource_discovery_support::discover(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true)}
pub use resource_discovery_support::{ResourceDiscoveryError,ResourceDiscoveryReceipt as Worldgenfederated_continualResourceinferenceReceipt,ResourceDiscoveryRequest as Worldgenfederated_continualResourceinferenceRequest};
