//! Worldgen P05 AFA-worldgen-P05-F11 throughput research_copilot.
use super::resource_copilot_support::{self,ResourceCopilotRequest,ResourceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-resource-copilot/1.0";
pub fn worldgen_throughput_resource_discovery_research_copilot_manifest()->serde_json::Value{resource_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceCopilotRequest1@1","prospective high-throughput","A2")}
pub fn run_worldgen_throughput_resource_discovery_research_copilot(request:&ResourceCopilotRequest)->Result<ResourceCopilotReceipt,resource_copilot_support::ResourceCopilotError>{resource_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,false)}
pub use resource_copilot_support::{ResourceCopilotError,ResourceCopilotReceipt as WorldgenthroughputResourceresearchcopilotReceipt,ResourceCopilotRequest as WorldgenthroughputResourceresearchcopilotRequest};
