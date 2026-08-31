//! Factory P32 throughput lease/fencing integrity contract model.
use super::lease_fencing_integrity_support::{
    manifest, qualify, LeaseFencingIntegrityCard7, LeaseFencingIntegrityError,
    LeaseFencingIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-factory-P32-F07";
pub const CONTRACT_VERSION: &str = "factory-throughput_lease_fencing_integrity_contract_model/1.0";
pub fn throughput_lease_fencing_integrity_contract_model_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "throughput", "contract_model")
}
pub fn qualify_throughput_lease_fencing_integrity_contract_model(
    q: &LeaseFencingIntegrityRequest4,
) -> Result<LeaseFencingIntegrityCard7, LeaseFencingIntegrityError> {
    qualify(
        q,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "contract_model",
    )
}
