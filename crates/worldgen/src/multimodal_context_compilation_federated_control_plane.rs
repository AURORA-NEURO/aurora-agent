//! Worldgen P03-F30 multimodal context-compilation federated control plane.
use super::context_control_plane_support::{self, ContextControlPlaneRequest, ContextControlPlaneReceipt};
pub const FEATURE_ID: &str = "AFA-worldgen-P03-F30";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-context-control-plane/1.0";
pub fn worldgen_multimodal_context_compilation_federated_control_plane_manifest() -> serde_json::Value { context_control_plane_support::manifest(FEATURE_ID, CONTRACT_VERSION, "ContextControlPlaneRequest1@1", "multimodal multi-study", "A2") }
pub fn control_worldgen_multimodal_context_compilation(request: &ContextControlPlaneRequest) -> Result<ContextControlPlaneReceipt, context_control_plane_support::ContextControlPlaneError> { context_control_plane_support::control(request, FEATURE_ID, CONTRACT_VERSION, "multimodal multi-study", true, false) }
pub use context_control_plane_support::{ContextControlPlaneError, ContextControlPlaneReceipt as WorldgenMultimodalContextControlPlaneReceipt, ContextControlPlaneRequest as WorldgenMultimodalContextControlPlaneRequest, ContextControlAttestation as WorldgenMultimodalContextControlAttestation};
