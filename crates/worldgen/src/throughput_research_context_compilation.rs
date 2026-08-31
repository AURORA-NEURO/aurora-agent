//! Prospective high-throughput context compiler (`AFA-worldgen-P03-F03`).
use super::context_compilation_support::{self,ContextCompilationRequest,ContextCompilationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F03";pub const CONTRACT_VERSION:&str="worldgen-throughput-research-context/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion3@1";
pub fn worldgen_throughput_research_context_compilation_manifest()->serde_json::Value{context_compilation_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"prospective high-throughput","A2")}
pub fn compile_worldgen_throughput_research_context(r:&ContextCompilationRequest)->Result<ContextCompilationReceipt,context_compilation_support::ContextCompilationError>{context_compilation_support::compile(r,FEATURE_ID,CONTRACT_VERSION,"prospective high-throughput",true)}
pub use context_compilation_support::{ContextCompilationReceipt as WorldgenThroughputContextCompilationReceipt,ContextCompilationRequest as WorldgenThroughputContextCompilationRequest};
