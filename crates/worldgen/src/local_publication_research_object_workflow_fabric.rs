//! Worldgen P16 F13 statistical, causal, and ML workflow fabric.
use super::publication_research_object_workflow_support::{self,ReleaseWorkflowRequest,ReleaseWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P16-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-publication-research-object-workflow/1.0";
pub fn worldgen_local_publication_research_object_workflow_fabric_manifest()->serde_json::Value{publication_research_object_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_publication_research_object_workflow(request:&ReleaseWorkflowRequest)->Result<ReleaseWorkflowReceipt,publication_research_object_workflow_support::ReleaseWorkflowError>{publication_research_object_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use publication_research_object_workflow_support::{ReleaseWorkflowError,ReleaseWorkflowRequest as WorldgenPublicationResearchObjectWorkflowRequest,ReleaseWorkflowReceipt as WorldgenPublicationResearchObjectWorkflowReceipt};

