//! Multimodal context-compilation copilot (`AFA-worldgen-P03-F10`).
use super::context_copilot_support::{self,ContextCopilotRequest,ContextCopilotReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F10";pub const CONTRACT_VERSION:&str="worldgen-multimodal-context-compilation-copilot/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion2@1";
pub fn worldgen_multimodal_context_compilation_copilot_manifest()->serde_json::Value{context_copilot_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn run_worldgen_multimodal_context_compilation_copilot(r:&ContextCopilotRequest)->Result<ContextCopilotReceipt,context_copilot_support::ContextCopilotError>{context_copilot_support::run(r,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",true,false)}
pub use context_copilot_support::{ContextCopilotReceipt as WorldgenMultimodalContextCopilotReceipt,ContextCopilotRequest as WorldgenMultimodalContextCopilotRequest};
