//! Worldgen P03-F22 multimodal context-compilation interoperability gateway.
use super::context_interoperability_support::{self, ContextInteroperabilityRequest, ContextInteroperabilityReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F22";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-context-compilation-gateway/1.0";
pub fn worldgen_multimodal_context_compilation_interoperability_gateway_manifest() -> serde_json::Value { context_interoperability_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCompilationRequest1@1", "multimodal multi-study", "A2") }
pub fn negotiate_worldgen_multimodal_context_compilation_interoperability(request: &ContextInteroperabilityRequest) -> Result<ContextInteroperabilityReceipt, context_interoperability_support::ContextInteroperabilityError> { context_interoperability_support::negotiate(request, FEATURE_ID, CONTRACT_VERSION, "multimodal multi-study", true, false) }
pub use context_interoperability_support::{ContextInteroperabilityError, ContextInteroperabilityReceipt as WorldgenMultimodalContextInteroperabilityReceipt, ContextInteroperabilityRequest as WorldgenMultimodalContextInteroperabilityRequest};
