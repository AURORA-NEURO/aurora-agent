//! Worldgen P03-F28 federated continual context-compilation assurance harness.
use super::context_assurance_support::{self, ContextAssuranceRequest, ContextAssuranceReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F28";
pub const CONTRACT_VERSION: &str = "worldgen-federated-continual-context-assurance/1.0";
pub fn worldgen_federated_continual_context_compilation_assurance_manifest() -> serde_json::Value { context_assurance_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextAssuranceRequest1@1", "federated continual autonomous", "A2") }
pub fn assure_worldgen_federated_continual_context_compilation(request: &ContextAssuranceRequest) -> Result<ContextAssuranceReceipt, context_assurance_support::ContextAssuranceError> { context_assurance_support::assure(request, FEATURE_ID, CONTRACT_VERSION, "federated continual autonomous", true, true) }
pub use context_assurance_support::{ContextAssuranceError, ContextAssuranceReceipt as WorldgenFederatedContinualContextAssuranceReceipt, ContextAssuranceRequest as WorldgenFederatedContinualContextAssuranceRequest};
