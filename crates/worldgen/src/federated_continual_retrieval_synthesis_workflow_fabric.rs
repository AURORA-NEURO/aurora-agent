//! Federated continual retrieval-synthesis workflow fabric (`AFA-worldgen-P02-F16`).
use super::retrieval_workflow_support::{self, RetrievalWorkflowReceipt, RetrievalWorkflowRequest};
pub const FEATURE_ID:&str="AFA-worldgen-P02-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated-continual-retrieval-synthesis-workflow/1.0"; pub const INPUT_SCHEMA:&str="ScopedRetrievalQuery4@1";
pub fn worldgen_federated_continual_retrieval_synthesis_workflow_fabric_manifest()->serde_json::Value{retrieval_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual autonomous","A2")}
pub fn schedule_worldgen_federated_continual_retrieval_synthesis_workflow(r:&RetrievalWorkflowRequest)->Result<RetrievalWorkflowReceipt,retrieval_workflow_support::RetrievalWorkflowError>{retrieval_workflow_support::schedule(r,FEATURE_ID,CONTRACT_VERSION,true,true)}
pub use retrieval_workflow_support::{RetrievalWorkflowError,RetrievalWorkflowReceipt as WorldgenFederatedContinualRetrievalWorkflowReceipt,RetrievalWorkflowRequest as WorldgenFederatedContinualRetrievalWorkflowRequest};
