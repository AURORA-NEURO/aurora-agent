//! Prospective high-throughput context-compilation copilot (`AFA-worldgen-P03-F11`).
use super::context_copilot_support::{self,ContextCopilotRequest,ContextCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F11";pub const CONTRACT_VERSION:&str="worldgen-throughput-context-compilation-copilot/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion3@1";
pub fn worldgen_throughput_context_compilation_copilot_manifest()->serde_json::Value{context_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A2")}
pub fn run_worldgen_throughput_context_compilation_copilot(r:&ContextCopilotRequest)->Result<ContextCopilotReceipt,context_copilot_support::ContextCopilotError>{context_copilot_support::run(r,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true,true)}
pub use context_copilot_support::{ContextCopilotReceipt as WorldgenThroughputContextCopilotReceipt,ContextCopilotRequest as WorldgenThroughputContextCopilotRequest};
