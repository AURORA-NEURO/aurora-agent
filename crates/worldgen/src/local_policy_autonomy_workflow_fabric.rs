//! Worldgen P19 F13 statistical, causal, and ML workflow fabric.
use super::policy_autonomy_workflow_support::{self,PolicyAutonomyWorkflowRequest,PolicyAutonomyWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F13"; pub const CONTRACT_VERSION:&str="worldgen-local-policy_autonomy-signing-workflow/1.0";
pub fn worldgen_local_policy_autonomy_workflow_fabric_manifest()->serde_json::Value{policy_autonomy_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn schedule_worldgen_local_policy_autonomy_workflow(request:&PolicyAutonomyWorkflowRequest)->Result<PolicyAutonomyWorkflowReceipt,policy_autonomy_workflow_support::PolicyAutonomyWorkflowError>{policy_autonomy_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"local single-study",true,false)}
pub use policy_autonomy_workflow_support::{PolicyAutonomyWorkflowError,PolicyAutonomyWorkflowRequest as WorldgenTypedPolicyAutonomyWorkflowRequest,PolicyAutonomyWorkflowReceipt as WorldgenTypedPolicyAutonomyWorkflowReceipt};

