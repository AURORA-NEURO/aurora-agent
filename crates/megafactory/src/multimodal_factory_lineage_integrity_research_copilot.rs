//! Megafactory P32 multimodal factory-lineage integrity research copilot.
use super::factory_lineage_integrity_support::{
    manifest, qualify, FactoryLineageCard7, FactoryLineageIntegrityError, FactoryLineageRequest4,
};
pub const FEATURE_ID: &str = "AFA-megafactory-P32-F10";
pub const CONTRACT_VERSION: &str =
    "megafactory-multimodal_factory_lineage_integrity_research_copilot/1.0";
pub fn multimodal_factory_lineage_integrity_research_copilot_manifest() -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "research_copilot",
    )
}
pub fn qualify_multimodal_factory_lineage_integrity_research_copilot(
    request: &FactoryLineageRequest4,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    qualify(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "research_copilot",
    )
}
