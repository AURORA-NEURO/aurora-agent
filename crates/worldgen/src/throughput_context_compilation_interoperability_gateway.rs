//! Worldgen P03-F23 prospective high-throughput context-compilation interoperability gateway.
use super::context_interoperability_support::{self, ContextInteroperabilityRequest, ContextInteroperabilityReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F23";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-context-compilation-gateway/1.0";
pub fn worldgen_throughput_context_compilation_interoperability_gateway_manifest() -> serde_json::Value { context_interoperability_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCompilationRequest1@1", "prospective high-throughput", "A2") }
pub fn negotiate_worldgen_throughput_context_compilation_interoperability(request: &ContextInteroperabilityRequest) -> Result<ContextInteroperabilityReceipt, context_interoperability_support::ContextInteroperabilityError> { context_interoperability_support::negotiate(request, FEATURE_ID, CONTRACT_VERSION, "prospective high-throughput", true, true) }
pub use context_interoperability_support::{ContextInteroperabilityError, ContextInteroperabilityReceipt as WorldgenThroughputContextInteroperabilityReceipt, ContextInteroperabilityRequest as WorldgenThroughputContextInteroperabilityRequest};
