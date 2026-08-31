//! Worldgen P04-F13 local knowledge-representation workflow fabric.
use super::knowledge_workflow_support::{self, KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-knowledge-workflow/1.0";
pub fn worldgen_local_knowledge_representation_workflow_fabric_manifest()->serde_json::Value{knowledge_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeWorkflowRequest1@1","local single-study","A1")}
pub fn schedule_worldgen_local_knowledge_representation_workflow(request:&KnowledgeWorkflowRequest)->Result<KnowledgeWorkflowReceipt,knowledge_workflow_support::KnowledgeWorkflowError>{knowledge_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use knowledge_workflow_support::{KnowledgeWorkflowError,KnowledgeWorkflowReceipt as WorldgenLocalKnowledgeWorkflowReceipt,KnowledgeWorkflowRequest as WorldgenLocalKnowledgeWorkflowRequest};
