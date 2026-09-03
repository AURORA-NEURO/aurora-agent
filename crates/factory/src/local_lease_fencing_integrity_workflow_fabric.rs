//! Factory P32 local lease/fencing integrity workflow fabric.
use super::lease_fencing_integrity_support::{
    manifest, qualify, LeaseFencingIntegrityCard7, LeaseFencingIntegrityError,
    LeaseFencingIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-factory-P32-F13";
pub const CONTRACT_VERSION: &str = "factory-local_lease_fencing_integrity_workflow_fabric/1.0";
pub fn local_lease_fencing_integrity_workflow_fabric_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "workflow_fabric")
}
pub fn qualify_local_lease_fencing_integrity_workflow_fabric(
    q: &LeaseFencingIntegrityRequest4,
) -> Result<LeaseFencingIntegrityCard7, LeaseFencingIntegrityError> {
    qualify(q, FEATURE_ID, CONTRACT_VERSION, "local", "workflow_fabric")
}
