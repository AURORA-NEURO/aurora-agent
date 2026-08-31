//! Worldgen P19 F16 statistical, causal, and ML workflow fabric.
use super::policy_autonomy_workflow_support::{self,PolicyAutonomyWorkflowRequest,PolicyAutonomyWorkflowReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F16"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-policy_autonomy-signing-workflow/1.0";
pub fn worldgen_federated_continual_policy_autonomy_workflow_fabric_manifest()->serde_json::Value{policy_autonomy_workflow_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn schedule_worldgen_federated_continual_policy_autonomy_workflow(request:&PolicyAutonomyWorkflowRequest)->Result<PolicyAutonomyWorkflowReceipt,policy_autonomy_workflow_support::PolicyAutonomyWorkflowError>{policy_autonomy_workflow_support::schedule(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use policy_autonomy_workflow_support::{PolicyAutonomyWorkflowError,PolicyAutonomyWorkflowRequest as WorldgenTypedPolicyAutonomyWorkflowRequest,PolicyAutonomyWorkflowReceipt as WorldgenTypedPolicyAutonomyWorkflowReceipt};

