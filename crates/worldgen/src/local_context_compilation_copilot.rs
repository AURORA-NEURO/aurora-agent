//! Local context-compilation copilot (`AFA-worldgen-P03-F09`).
use super::context_copilot_support::{self,ContextCopilotRequest,ContextCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F09";pub const CONTRACT_VERSION:&str="worldgen-local-context-compilation-copilot/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion1@1";
pub fn worldgen_local_context_compilation_copilot_manifest()->serde_json::Value{context_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"local single-study","A1")}
pub fn run_worldgen_local_context_compilation_copilot(r:&ContextCopilotRequest)->Result<ContextCopilotReceipt,context_copilot_support::ContextCopilotError>{context_copilot_support::run(r,FEATURE_ID,CONTRACT_VERSION,"local single-study",false,false)}
pub use context_copilot_support::{ContextCopilotError,ContextCopilotReceipt as WorldgenLocalContextCopilotReceipt,ContextCopilotRequest as WorldgenLocalContextCopilotRequest};
