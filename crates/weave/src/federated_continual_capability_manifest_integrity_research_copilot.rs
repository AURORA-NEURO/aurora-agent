//! Weave P32 local inference capability-manifest integrity feature.
use super::capability_manifest_integrity_support::{
    admit, manifest, CapabilityManifestCard7, CapabilityManifestIntegrityError,
    CapabilityManifestRequest4,
};
pub const FEATURE_ID: &str = "AFA-weave-P32-F01";
pub const CONTRACT_VERSION: &str = "weave-local_capability_manifest_integrity_inference/1.0";
pub fn local_capability_manifest_integrity_inference_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
pub fn admit_local_capability_manifest_integrity_inference(
    request: &CapabilityManifestRequest4,
) -> Result<CapabilityManifestCard7, CapabilityManifestIntegrityError> {
    admit(request, FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
