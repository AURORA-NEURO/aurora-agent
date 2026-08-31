//! Federated continual context-compilation copilot (`AFA-worldgen-P03-F12`).
use super::context_copilot_support::{self,ContextCopilotRequest,ContextCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F12";pub const CONTRACT_VERSION:&str="worldgen-federated-continual-context-compilation-copilot/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion4@1";
pub fn worldgen_federated_continual_context_compilation_copilot_manifest()->serde_json::Value{context_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual/autonomous","A2")}
pub fn run_worldgen_federated_continual_context_compilation_copilot(r:&ContextCopilotRequest)->Result<ContextCopilotReceipt,context_copilot_support::ContextCopilotError>{context_copilot_support::run(r,FEATURE_ID,CONTRACT_VERSION,"federated continual/autonomous",true,true)}
pub use context_copilot_support::{ContextCopilotReceipt as WorldgenFederatedContinualContextCopilotReceipt,ContextCopilotRequest as WorldgenFederatedContinualContextCopilotRequest};
