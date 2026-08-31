//! Worldgen P05 AFA-worldgen-P05-F12 federated_continual research_copilot.
use super::resource_copilot_support::{self,ResourceCopilotRequest,ResourceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-resource-copilot/1.0";
pub fn worldgen_federated_continual_resource_discovery_research_copilot_manifest()->serde_json::Value{resource_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceCopilotRequest1@1","federated continual autonomous","A2")}
pub fn run_worldgen_federated_continual_resource_discovery_research_copilot(request:&ResourceCopilotRequest)->Result<ResourceCopilotReceipt,resource_copilot_support::ResourceCopilotError>{resource_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true,true)}
pub use resource_copilot_support::{ResourceCopilotError,ResourceCopilotReceipt as Worldgenfederated_continualResourceresearchcopilotReceipt,ResourceCopilotRequest as Worldgenfederated_continualResourceresearchcopilotRequest};
