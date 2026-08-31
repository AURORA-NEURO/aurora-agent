//! Worldgen P03-F25 local context-compilation assurance harness.
use super::context_assurance_support::{self, ContextAssuranceRequest, ContextAssuranceReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F25";
pub const CONTRACT_VERSION: &str = "worldgen-local-context-assurance/1.0";
pub fn worldgen_local_context_compilation_assurance_manifest() -> serde_json::Value { context_assurance_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextAssuranceRequest1@1", "local single-study", "A1") }
pub fn assure_worldgen_local_context_compilation(request: &ContextAssuranceRequest) -> Result<ContextAssuranceReceipt, context_assurance_support::ContextAssuranceError> { context_assurance_support::assure(request, FEATURE_ID, CONTRACT_VERSION, "local single-study", false, false) }
pub use context_assurance_support::{ContextAssuranceError, ContextAssuranceReceipt as WorldgenLocalContextAssuranceReceipt, ContextAssuranceRequest as WorldgenLocalContextAssuranceRequest};
