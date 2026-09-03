//! Worldgen P03-F21 local context-compilation interoperability gateway.
use super::context_interoperability_support::{self, ContextInteroperabilityRequest, ContextInteroperabilityReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F21";
pub const CONTRACT_VERSION: &str = "worldgen-local-context-compilation-gateway/1.0";
pub fn worldgen_local_context_compilation_interoperability_gateway_manifest() -> serde_json::Value { context_interoperability_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCompilationRequest1@1", "local single-study", "A1") }
pub fn negotiate_worldgen_local_context_compilation_interoperability(request: &ContextInteroperabilityRequest) -> Result<ContextInteroperabilityReceipt, context_interoperability_support::ContextInteroperabilityError> { context_interoperability_support::negotiate(request, FEATURE_ID, CONTRACT_VERSION, "local single-study", false, false) }
pub use context_interoperability_support::{ContextInteroperabilityError, ContextInteroperabilityReceipt as WorldgenLocalContextInteroperabilityReceipt, ContextInteroperabilityRequest as WorldgenLocalContextInteroperabilityRequest};
