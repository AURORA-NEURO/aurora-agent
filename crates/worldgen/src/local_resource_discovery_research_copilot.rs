//! Worldgen P05 AFA-worldgen-P05-F09 local research_copilot.
use super::resource_copilot_support::{self,ResourceCopilotRequest,ResourceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F09"; pub const CONTRACT_VERSION:&str="worldgen-local-resource-copilot/1.0";
pub fn worldgen_local_resource_discovery_research_copilot_manifest()->serde_json::Value{resource_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceCopilotRequest1@1","local single-study","A1")}
pub fn run_worldgen_local_resource_discovery_research_copilot(request:&ResourceCopilotRequest)->Result<ResourceCopilotReceipt,resource_copilot_support::ResourceCopilotError>{resource_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use resource_copilot_support::{ResourceCopilotError,ResourceCopilotReceipt as WorldgenlocalResourceresearchcopilotReceipt,ResourceCopilotRequest as WorldgenlocalResourceresearchcopilotRequest};
