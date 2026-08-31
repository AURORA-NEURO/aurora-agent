//! Megafactory P32 local factory-lineage integrity feature.
use super::factory_lineage_integrity_support::{
    manifest, qualify, FactoryLineageCard7, FactoryLineageIntegrityError, FactoryLineageRequest4,
};
pub const FEATURE_ID: &str = "AFA-megafactory-P32-F01";
pub const CONTRACT_VERSION: &str = "megafactory-local_factory_lineage_integrity_inference/1.0";
pub fn local_factory_lineage_integrity_inference_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
pub fn qualify_local_factory_lineage_integrity_inference(
    request: &FactoryLineageRequest4,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    qualify(request, FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
