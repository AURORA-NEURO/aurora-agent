//! Multimodal context compiler (`AFA-worldgen-P03-F02`).
use super::context_compilation_support::{self,ContextCompilationRequest,ContextCompilationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F02";pub const CONTRACT_VERSION:&str="worldgen-multimodal-research-context/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion2@1";
pub fn worldgen_multimodal_research_context_compilation_manifest()->serde_json::Value{context_compilation_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"multimodal multi-study","A1")}
pub fn compile_worldgen_multimodal_research_context(r:&ContextCompilationRequest)->Result<ContextCompilationReceipt,context_compilation_support::ContextCompilationError>{context_compilation_support::compile(r,FEATURE_ID,CONTRACT_VERSION,"multimodal multi-study",false)}
pub use context_compilation_support::{ContextCompilationReceipt as WorldgenMultimodalContextCompilationReceipt,ContextCompilationRequest as WorldgenMultimodalContextCompilationRequest};
