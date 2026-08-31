//! Megafactory P32 throughput factory-lineage integrity workflow fabric.
use super::factory_lineage_integrity_support::{
    manifest, qualify, FactoryLineageCard7, FactoryLineageIntegrityError, FactoryLineageRequest4,
};
pub const FEATURE_ID: &str = "AFA-megafactory-P32-F15";
pub const CONTRACT_VERSION: &str =
    "megafactory-throughput_factory_lineage_integrity_workflow_fabric/1.0";
pub fn throughput_factory_lineage_integrity_workflow_fabric_manifest() -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "workflow_fabric",
    )
}
pub fn qualify_throughput_factory_lineage_integrity_workflow_fabric(
    request: &FactoryLineageRequest4,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    qualify(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "workflow_fabric",
    )
}
