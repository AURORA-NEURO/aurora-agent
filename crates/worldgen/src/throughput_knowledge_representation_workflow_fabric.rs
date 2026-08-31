//! Worldgen P04-F15 prospective high-throughput knowledge-representation workflow fabric.
use super::knowledge_workflow_support::{self, KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F15"; pub const CONTRACT_VERSION:&str="worldgen-throughput-knowledge-workflow/1.0";
pub fn worldgen_throughput_knowledge_representation_workflow_fabric_manifest()->serde_json::Value{knowledge_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeWorkflowRequest1@1","prospective high-throughput","A2")}
pub fn schedule_worldgen_throughput_knowledge_representation_workflow(request:&KnowledgeWorkflowRequest)->Result<KnowledgeWorkflowReceipt,knowledge_workflow_support::KnowledgeWorkflowError>{knowledge_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use knowledge_workflow_support::{KnowledgeWorkflowError,KnowledgeWorkflowReceipt as WorldgenThroughputKnowledgeWorkflowReceipt,KnowledgeWorkflowRequest as WorldgenThroughputKnowledgeWorkflowRequest};
