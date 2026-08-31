//! Worldgen P03-F26 multimodal context-compilation assurance harness.
use super::context_assurance_support::{self, ContextAssuranceRequest, ContextAssuranceReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F26";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-context-assurance/1.0";
pub fn worldgen_multimodal_context_compilation_assurance_manifest() -> serde_json::Value { context_assurance_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextAssuranceRequest1@1", "multimodal multi-study", "A2") }
pub fn assure_worldgen_multimodal_context_compilation(request: &ContextAssuranceRequest) -> Result<ContextAssuranceReceipt, context_assurance_support::ContextAssuranceError> { context_assurance_support::assure(request, FEATURE_ID, CONTRACT_VERSION, "multimodal multi-study", true, false) }
pub use context_assurance_support::{ContextAssuranceError, ContextAssuranceReceipt as WorldgenMultimodalContextAssuranceReceipt, ContextAssuranceRequest as WorldgenMultimodalContextAssuranceRequest};
