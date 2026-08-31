//! Local context-compilation workflow fabric (`AFA-worldgen-P03-F13`).
use super::context_workflow_support::{self,ContextWorkflowReceipt,ContextWorkflowRequest};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F13";pub const CONTRACT_VERSION:&str="worldgen-local-context-compilation-workflow/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion1@1";
pub fn worldgen_local_context_compilation_workflow_fabric_manifest()->serde_json::Value{context_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A1")}
pub fn schedule_worldgen_local_context_compilation_workflow(r:&ContextWorkflowRequest)->Result<ContextWorkflowReceipt,context_workflow_support::ContextWorkflowError>{context_workflow_support::schedule(r,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use context_workflow_support::{ContextWorkflowError,ContextWorkflowReceipt as WorldgenLocalContextWorkflowReceipt,ContextWorkflowRequest as WorldgenLocalContextWorkflowRequest};
