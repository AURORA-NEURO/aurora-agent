//! Megafactory P32 local factory-lineage integrity workflow fabric.
use super::factory_lineage_integrity_support::{
    manifest, qualify, FactoryLineageCard7, FactoryLineageIntegrityError, FactoryLineageRequest4,
};
pub const FEATURE_ID: &str = "AFA-megafactory-P32-F13";
pub const CONTRACT_VERSION: &str =
    "megafactory-local_factory_lineage_integrity_workflow_fabric/1.0";
pub fn local_factory_lineage_integrity_workflow_fabric_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "workflow_fabric")
}
pub fn qualify_local_factory_lineage_integrity_workflow_fabric(
    request: &FactoryLineageRequest4,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    qualify(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "local",
        "workflow_fabric",
    )
}
