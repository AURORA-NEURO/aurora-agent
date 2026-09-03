//! Worldgen P17 F13 statistical, causal, and ML workflow fabric.
use super::typed_determinism_workflow_support::{self,DeterminismWorkflowRequest,DeterminismWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P17-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-typed-determinism-workflow/1.0";
pub fn worldgen_local_typed_determinism_workflow_fabric_manifest()->serde_json::Value{typed_determinism_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_typed_determinism_workflow(request:&DeterminismWorkflowRequest)->Result<DeterminismWorkflowReceipt,typed_determinism_workflow_support::DeterminismWorkflowError>{typed_determinism_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use typed_determinism_workflow_support::{DeterminismWorkflowError,DeterminismWorkflowRequest as WorldgenTypedDeterminismWorkflowRequest,DeterminismWorkflowReceipt as WorldgenTypedDeterminismWorkflowReceipt};

