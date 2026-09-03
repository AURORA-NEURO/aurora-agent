//! Worldgen P19 F10 statistical, causal, and ML research copilot.
use super::policy_autonomy_copilot_support::{self,PolicyAutonomyCopilotRequest,PolicyAutonomyCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P19-F10"; pub const CONTRACT_VERSION:&str="worldgen-multimodal-policy_autonomy-signing-copilot/1.0";
pub fn worldgen_multimodal_policy_autonomy_research_copilot_manifest()->serde_json::Value{policy_autonomy_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study")}
pub fn run_worldgen_multimodal_policy_autonomy_research_copilot(request:&PolicyAutonomyCopilotRequest)->Result<PolicyAutonomyCopilotReceipt,policy_autonomy_copilot_support::PolicyAutonomyCopilotError>{policy_autonomy_copilot_support::run(request,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use policy_autonomy_copilot_support::{PolicyAutonomyCopilotError,PolicyAutonomyCopilotRequest as WorldgenTypedPolicyAutonomyCopilotRequest,PolicyAutonomyCopilotReceipt as WorldgenTypedPolicyAutonomyCopilotReceipt};

