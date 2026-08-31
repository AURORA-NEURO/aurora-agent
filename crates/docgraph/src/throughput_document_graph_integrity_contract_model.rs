//! Docgraph P32 throughput document graph integrity contract model.
use super::document_graph_integrity_support::{
    manifest, qualify, DocumentGraphIntegrityCard7, DocumentGraphIntegrityError,
    DocumentGraphIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-docgraph-P32-F07";
pub const CONTRACT_VERSION: &str =
    "docgraph-throughput_document_graph_integrity_contract_model/1.0";
pub fn throughput_document_graph_integrity_contract_model_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "throughput", "contract_model")
}
pub fn qualify_throughput_document_graph_integrity_contract_model(
    q: &DocumentGraphIntegrityRequest4,
) -> Result<DocumentGraphIntegrityCard7, DocumentGraphIntegrityError> {
    qualify(
        q,
        FEATURE_ID,
        CONTRACT_VERSION,
        "throughput",
        "contract_model",
    )
}
