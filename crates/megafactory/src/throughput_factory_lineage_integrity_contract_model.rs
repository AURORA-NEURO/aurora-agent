//! Megafactory P32 throughput factory-lineage integrity contract model.
use super::factory_lineage_integrity_support::{
    manifest, qualify, FactoryLineageCard7, FactoryLineageIntegrityError, FactoryLineageRequest4,
};
pub const FEATURE_ID: &str = "AFA-megafactory-P32-F07";
pub const CONTRACT_VERSION: &str =
    "megafactory-throughput_factory_lineage_integrity_contract_model/1.0";
pub fn throughput_factory_lineage_integrity_contract_model_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "throughput", "contract_model")
}
pub fn qualify_throughput_factory_lineage_integrity_contract_model(
    request: &FactoryLineageRequest4,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    qualify(
        request,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "contract_model",
    )
}
