//! Worldgen P04-F16 federated continual knowledge-representation workflow fabric.
use super::knowledge_workflow_support::{self, KnowledgeWorkflowRequest, KnowledgeWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P04-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated-continual-knowledge-workflow/1.0";
pub fn worldgen_federated_continual_knowledge_representation_workflow_fabric_manifest()->serde_json::Value{knowledge_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"KnowledgeWorkflowRequest1@1","federated continual autonomous","A2")}
pub fn schedule_worldgen_federated_continual_knowledge_representation_workflow(request:&KnowledgeWorkflowRequest)->Result<KnowledgeWorkflowReceipt,knowledge_workflow_support::KnowledgeWorkflowError>{knowledge_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",true,true)}
pub use knowledge_workflow_support::{KnowledgeWorkflowError,KnowledgeWorkflowReceipt as WorldgenFederatedContinualKnowledgeWorkflowReceipt,KnowledgeWorkflowRequest as WorldgenFederatedContinualKnowledgeWorkflowRequest};
