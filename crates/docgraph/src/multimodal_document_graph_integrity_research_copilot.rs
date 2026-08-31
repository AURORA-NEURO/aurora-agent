//! Docgraph P32 multimodal document graph integrity research copilot.
use super::document_graph_integrity_support::{
    manifest, qualify, DocumentGraphIntegrityCard7, DocumentGraphIntegrityError,
    DocumentGraphIntegrityRequest4,
};
pub const FEATURE_ID: &str = "AFA-docgraph-P32-F10";
pub const CONTRACT_VERSION: &str =
    "docgraph-multimodal_document_graph_integrity_research_copilot/1.0";
pub fn multimodal_document_graph_integrity_research_copilot_manifest() -> serde_json::Value {
    manifest(
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "research_copilot",
    )
}
pub fn qualify_multimodal_document_graph_integrity_research_copilot(
    q: &DocumentGraphIntegrityRequest4,
) -> Result<DocumentGraphIntegrityCard7, DocumentGraphIntegrityError> {
    qualify(
        q,
        FEATURE_ID,
        CONTRACT_VERSION,
        "multimodal",
        "research_copilot",
    )
}
