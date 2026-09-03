//! Worldgen P05 AFA-worldgen-P05-F10 multimodal research_copilot.
use super::resource_copilot_support::{self,ResourceCopilotRequest,ResourceCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P05-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-resource-copilot/1.0";
pub fn worldgen_multimodal_resource_discovery_research_copilot_manifest()->serde_json::Value{resource_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"ResourceCopilotRequest1@1","multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_resource_discovery_research_copilot(request:&ResourceCopilotRequest)->Result<ResourceCopilotReceipt,resource_copilot_support::ResourceCopilotError>{resource_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false,false)}
pub use resource_copilot_support::{ResourceCopilotError,ResourceCopilotReceipt as WorldgenmultimodalResourceresearchcopilotReceipt,ResourceCopilotRequest as WorldgenmultimodalResourceresearchcopilotRequest};
