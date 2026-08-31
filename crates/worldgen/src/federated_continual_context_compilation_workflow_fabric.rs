//! Federated continual context-compilation workflow fabric (`AFA-worldgen-P03-F16`).
use super::context_workflow_support::{self,ContextWorkflowReceipt,ContextWorkflowRequest};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F16";pub const CONTRACT_VERSION:&str="worldgen-federated-continual-context-compilation-workflow/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion4@1";
pub fn worldgen_federated_continual_context_compilation_workflow_fabric_manifest()->serde_json::Value{context_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual/autonomous","A2")}
pub fn schedule_worldgen_federated_continual_context_compilation_workflow(r:&ContextWorkflowRequest)->Result<ContextWorkflowReceipt,context_workflow_support::ContextWorkflowError>{context_workflow_support::schedule(r,FEATURE_ID,CONTRACT_VERSION,"federated continual/autonomous",true,true)}
pub use context_workflow_support::{ContextWorkflowReceipt as WorldgenFederatedContinualContextWorkflowReceipt,ContextWorkflowRequest as WorldgenFederatedContinualContextWorkflowRequest};
