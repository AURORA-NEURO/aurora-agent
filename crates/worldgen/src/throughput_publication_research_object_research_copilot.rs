//! Worldgen P16 F11 statistical, causal, and ML research copilot.
use super::publication_research_object_copilot_support::{self,ReleaseCopilotRequest,ReleaseCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P16-F11"; pub const CONTRACT_VERSION:&str="worldgen-throughput-publication-research-object-copilot/1.0";
pub fn worldgen_throughput_publication_research_object_research_copilot_manifest()->serde_json::Value{publication_research_object_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput")}
pub fn run_worldgen_throughput_publication_research_object_research_copilot(request:&ReleaseCopilotRequest)->Result<ReleaseCopilotReceipt,publication_research_object_copilot_support::ReleaseCopilotError>{publication_research_object_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use publication_research_object_copilot_support::{ReleaseCopilotError,ReleaseCopilotRequest as WorldgenPublicationResearchObjectCopilotRequest,ReleaseCopilotReceipt as WorldgenPublicationResearchObjectCopilotReceipt};

