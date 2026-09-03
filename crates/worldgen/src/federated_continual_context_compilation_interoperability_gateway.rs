//! Worldgen P03-F24 federated continual context-compilation interoperability gateway.
use super::context_interoperability_support::{self, ContextInteroperabilityRequest, ContextInteroperabilityReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F24";
pub const CONTRACT_VERSION: &str = "worldgen-federated-continual-context-compilation-gateway/1.0";
pub fn worldgen_federated_continual_context_compilation_interoperability_gateway_manifest() -> serde_json::Value { context_interoperability_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextCompilationRequest1@1", "federated continual autonomous", "A2") }
pub fn negotiate_worldgen_federated_continual_context_compilation_interoperability(request: &ContextInteroperabilityRequest) -> Result<ContextInteroperabilityReceipt, context_interoperability_support::ContextInteroperabilityError> { context_interoperability_support::negotiate(request, FEATURE_ID, CONTRACT_VERSION, "federated continual autonomous", true, true) }
pub use context_interoperability_support::{ContextInteroperabilityError, ContextInteroperabilityReceipt as WorldgenFederatedContinualContextInteroperabilityReceipt, ContextInteroperabilityRequest as WorldgenFederatedContinualContextInteroperabilityRequest};
