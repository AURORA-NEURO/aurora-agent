//! Federated continual context compiler (`AFA-worldgen-P03-F04`).
use super::context_compilation_support::{self,ContextCompilationRequest,ContextCompilationReceipt};
pub const FEATURE_ID:&str="AFA-worldgen-P03-F04";pub const CONTRACT_VERSION:&str="worldgen-federated-continual-research-context/1.0";pub const INPUT_SCHEMA:&str="ContextCompilationQuestion4@1";
pub fn worldgen_federated_continual_research_context_compilation_manifest()->serde_json::Value{context_compilation_support::manifest(FEATURE_ID,CONTRACT_VERSION,INPUT_SCHEMA,"federated continual/autonomous","A2")}
pub fn compile_worldgen_federated_continual_research_context(r:&ContextCompilationRequest)->Result<ContextCompilationReceipt,context_compilation_support::ContextCompilationError>{context_compilation_support::compile(r,FEATURE_ID,CONTRACT_VERSION,"federated continual/autonomous",true)}
pub use context_compilation_support::{ContextCompilationReceipt as WorldgenFederatedContinualContextCompilationReceipt,ContextCompilationRequest as WorldgenFederatedContinualContextCompilationRequest};
