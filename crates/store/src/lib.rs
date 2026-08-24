//! Content-addressed indexed world storage.
//!
//! Implements blueprint 43.34 (storage layout, content addressing and multidimensional indexing).
//! The measured motivation is in `docs/ADR-001-language-strategy.md`: on a one-million-fact world
//! the eager path spent 16.5 s parsing and 5.5 s compiling to deliver eleven facts. Cost tracked
//! the corpus while the result tracked the query — the exact behaviour the project's thesis
//! rejects.
//!
//! A store is built once per world release and answers point queries by binary search over
//! on-disk sorted indices, with aggregates served from a manifest. Compiling against it costs what
//! the compiled region costs.

pub mod build;
pub mod error;
pub mod federated_gateway;
pub mod lazy;
pub mod sorted_index;

pub use build::{build, StoreManifest, STORE_SCHEMA_VERSION};
pub use error::StoreError;
pub use federated_gateway::{
    admit_federated_knowledge, FederatedKnowledgeGatewayReceipt, FederatedKnowledgeGatewayRequest,
    GatewayDisposition, GatewayError,
    CONTRACT_VERSION as FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID,
};
pub use lazy::LazyWorld;
pub use sorted_index::{SortedIndex, SortedIndexWriter};
