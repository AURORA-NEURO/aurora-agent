//! Worldgen P19 F12 statistical, causal, and ML research copilot.
use super::policy_autonomy_copilot_support::{self,PolicyAutonomyCopilotRequest,PolicyAutonomyCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F12"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-policy_autonomy-signing-copilot/1.0";
pub fn worldgen_federated_continual_policy_autonomy_research_copilot_manifest()->serde_json::Value{policy_autonomy_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn run_worldgen_federated_continual_policy_autonomy_research_copilot(request:&PolicyAutonomyCopilotRequest)->Result<PolicyAutonomyCopilotReceipt,policy_autonomy_copilot_support::PolicyAutonomyCopilotError>{policy_autonomy_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous",false,true)}
pub use policy_autonomy_copilot_support::{PolicyAutonomyCopilotError,PolicyAutonomyCopilotRequest as WorldgenTypedPolicyAutonomyCopilotRequest,PolicyAutonomyCopilotReceipt as WorldgenTypedPolicyAutonomyCopilotReceipt};

