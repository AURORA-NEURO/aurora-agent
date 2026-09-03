//! Factory P32 local lease/fencing integrity research copilot.
use super::lease_fencing_integrity_support::{
    manifest, qualify, LeaseFencingIntegrityCard7, LeaseFencingIntegrityError,
    LeaseFencingIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-factory-P32-F09";
pub const CONTRACT_VERSION: &str = "factory-local_lease_fencing_integrity_research_copilot/1.0";
pub fn local_lease_fencing_integrity_research_copilot_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "research_copilot")
}
pub fn qualify_local_lease_fencing_integrity_research_copilot(
    q: &LeaseFencingIntegrityRequest4,
) -> Result<LeaseFencingIntegrityCard7, LeaseFencingIntegrityError> {
    qualify(q, FEATURE_ID, CONTRACT_VERSION, "local", "research_copilot")
}
