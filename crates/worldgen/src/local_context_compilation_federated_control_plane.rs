//! Worldgen P03-F29 local context-compilation federated control plane.
use super::context_control_plane_support::{self, ContextControlPlaneRequest, ContextControlPlaneReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F29";
pub const CONTRACT_VERSION: &str = "worldgen-local-context-control-plane/1.0";
pub fn worldgen_local_context_compilation_federated_control_plane_manifest() -> serde_json::Value { context_control_plane_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextControlPlaneRequest1@1", "local single-study", "A1") }
pub fn control_worldgen_local_context_compilation(request: &ContextControlPlaneRequest) -> Result<ContextControlPlaneReceipt, context_control_plane_support::ContextControlPlaneError> { context_control_plane_support::control(request, FEATURE_ID, CONTRACT_VERSION, "local single-study", false, false) }
pub use context_control_plane_support::{ContextControlPlaneError, ContextControlPlaneReceipt as WorldgenLocalContextControlPlaneReceipt, ContextControlPlaneRequest as WorldgenLocalContextControlPlaneRequest, ContextControlAttestation as WorldgenLocalContextControlAttestation};
