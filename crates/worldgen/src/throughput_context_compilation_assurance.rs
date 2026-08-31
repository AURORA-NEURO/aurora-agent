//! Worldgen P03-F27 prospective high-throughput context-compilation assurance harness.
use super::context_assurance_support::{self, ContextAssuranceRequest, ContextAssuranceReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F27";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-context-assurance/1.0";
pub fn worldgen_throughput_context_compilation_assurance_manifest() -> serde_json::Value { context_assurance_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextAssuranceRequest1@1", "prospective high-throughput", "A2") }
pub fn assure_worldgen_throughput_context_compilation(request: &ContextAssuranceRequest) -> Result<ContextAssuranceReceipt, context_assurance_support::ContextAssuranceError> { context_assurance_support::assure(request, FEATURE_ID, CONTRACT_VERSION, "prospective high-throughput", true, true) }
pub use context_assurance_support::{ContextAssuranceError, ContextAssuranceReceipt as WorldgenThroughputContextAssuranceReceipt, ContextAssuranceRequest as WorldgenThroughputContextAssuranceRequest};
