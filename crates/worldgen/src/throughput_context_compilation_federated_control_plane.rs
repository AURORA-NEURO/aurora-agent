//! Worldgen P03-F31 prospective high-throughput context-compilation federated control plane.
use super::context_control_plane_support::{self, ContextControlPlaneRequest, ContextControlPlaneReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F31";
pub const CONTRACT_VERSION: &str = "worldgen-throughput-context-control-plane/1.0";
pub fn worldgen_throughput_context_compilation_federated_control_plane_manifest() -> serde_json::Value { context_control_plane_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextControlPlaneRequest1@1", "prospective high-throughput", "A2") }
pub fn control_worldgen_throughput_context_compilation(request: &ContextControlPlaneRequest) -> Result<ContextControlPlaneReceipt, context_control_plane_support::ContextControlPlaneError> { context_control_plane_support::control(request, FEATURE_ID, CONTRACT_VERSION, "prospective high-throughput", true, true) }
pub use context_control_plane_support::{ContextControlPlaneError, ContextControlPlaneReceipt as WorldgenThroughputContextControlPlaneReceipt, ContextControlPlaneRequest as WorldgenThroughputContextControlPlaneRequest, ContextControlAttestation as WorldgenThroughputContextControlAttestation};
