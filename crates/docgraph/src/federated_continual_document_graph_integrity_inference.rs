//! Docgraph P32 federated continual document graph integrity feature.
use super::document_graph_integrity_support::{
    manifest, qualify, DocumentGraphIntegrityCard7, DocumentGraphIntegrityError,
    DocumentGraphIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-docgraph-P32-F04";
pub const CONTRACT_VERSION: &str =
    "docgraph-federated_continual_document_graph_integrity_inference/1.0";
pub fn federated_continual_document_graph_integrity_inference_manifest() -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "federated continual",
        "inference",
    )
}
pub fn qualify_federated_continual_document_graph_integrity_inference(
    q: &DocumentGraphIntegrityRequest4,
) -> Result<DocumentGraphIntegrityCard7, DocumentGraphIntegrityError> {
    qualify(
        q,
        FEATURE_ID,
        CONTRACT_VERSION,
        "federated continual",
        "inference",
    )
}
