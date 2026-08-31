//! Docgraph P32 local document graph integrity feature.
use super::document_graph_integrity_support::{
    manifest, qualify, DocumentGraphIntegrityCard7, DocumentGraphIntegrityError,
    DocumentGraphIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-docgraph-P32-F01";
pub const CONTRACT_VERSION: &str = "docgraph-local_document_graph_integrity_inference/1.0";
pub fn local_document_graph_integrity_inference_manifest() -> serde_json::Value {
    manifest(FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
pub fn qualify_local_document_graph_integrity_inference(
    q: &DocumentGraphIntegrityRequest4,
) -> Result<DocumentGraphIntegrityCard7, DocumentGraphIntegrityError> {
    qualify(q, FEATURE_ID, CONTRACT_VERSION, "local", "inference")
}
