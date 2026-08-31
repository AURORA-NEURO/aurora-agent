//! The typed scope base.
//!
//! Implements blueprint 43.03 (typed scope base), the mapping taxonomy of 43.05
//! (restriction, transport, base change) and the structured-failure requirement of 43.06.
//!
//! Evidence in BioPRISM is valid only inside a scope. This crate supplies the type that says
//! *where* a claim holds, the partial order that says when one scope is narrower than another,
//! and the meet that says whether two scopes overlap at all — plus the vocabulary for moving
//! evidence between scopes without pretending the move was free.

#![allow(clippy::all)]

pub mod class;
pub mod error;
pub mod federated_commons_interoperability_gateway;
pub mod key;
pub mod meet;
pub mod throughput_federated_evidence_control_plane;
pub mod time;
pub mod transport;

pub use class::{DimensionRegistry, ScopeClass, DIMENSIONS_SCHEMA_VERSION};
pub use error::{ScopeError, TimeError};
pub use federated_commons_interoperability_gateway::{
    federated_scope_interoperability_manifest, operate_federated_scope_interoperability_gateway,
    ScopeFederationGatewayReceipt10, ScopeFederationGatewayRequest7,
    ScopeFederationReceiptArtifact10, ScopeGatewayArtifact4, ScopeGatewayError, ScopePeerManifest4,
    CONTRACT_VERSION as SCOPE_FEDERATED_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as SCOPE_FEDERATED_INTEROPERABILITY_FEATURE_ID,
};
pub use key::{ScopeKey, ScopeValue};
pub use meet::{meet, EmptyReason, Meet};
pub use throughput_federated_evidence_control_plane::{
    federated_evidence_control_manifest, operate_federated_evidence_control, EvidenceControlError,
    EvidenceControlRequest6, FederatedEvidenceControlArtifact9, FederatedEvidenceControlReceipt9,
    PeerEvidence4, ThroughputEvidence4,
    CONTRACT_VERSION as SCOPE_FEDERATED_EVIDENCE_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as SCOPE_FEDERATED_EVIDENCE_CONTROL_FEATURE_ID,
};
pub use time::{Interval, Timestamp};
pub use transport::{AggregationOperator, LossLedger, MappingCheck, MappingKind, ScopeMapping};
pub mod continuity_frontier_support;
pub mod federated_continual_continuity_frontier_contract_model;
pub mod federated_continual_continuity_frontier_inference;
pub mod federated_continual_continuity_frontier_research_copilot;
pub mod federated_continual_continuity_frontier_workflow_fabric;
pub mod local_continuity_frontier_contract_model;
pub mod local_continuity_frontier_inference;
pub mod local_continuity_frontier_research_copilot;
pub mod local_continuity_frontier_workflow_fabric;
pub mod multimodal_continuity_frontier_contract_model;
pub mod multimodal_continuity_frontier_inference;
pub mod multimodal_continuity_frontier_research_copilot;
pub mod multimodal_continuity_frontier_workflow_fabric;
pub mod throughput_continuity_frontier_contract_model;
pub mod throughput_continuity_frontier_inference;
pub mod throughput_continuity_frontier_research_copilot;
pub mod throughput_continuity_frontier_workflow_fabric;
pub use continuity_frontier_support::{
    ScopeAssertion4, ScopeContinuityArtifact4, ScopeContinuityCard7, ScopeContinuityError,
    ScopeContinuityRequest4,
};
pub use federated_continual_continuity_frontier_contract_model::{
    qualify_scope_federated_continuity_frontier_contract,
    scope_federated_continual_continuity_frontier_contract_model_manifest,
};
pub use federated_continual_continuity_frontier_inference::{
    qualify_scope_federated_continuity_frontier,
    scope_federated_continual_continuity_frontier_inference_manifest,
};
pub use federated_continual_continuity_frontier_research_copilot::{
    qualify_scope_federated_continuity_frontier_copilot,
    scope_federated_continual_continuity_frontier_research_copilot_manifest,
};
pub use federated_continual_continuity_frontier_workflow_fabric::{
    qualify_scope_federated_continuity_frontier_workflow,
    scope_federated_continual_continuity_frontier_workflow_fabric_manifest,
};
pub use local_continuity_frontier_contract_model::{
    qualify_scope_local_continuity_frontier_contract,
    scope_local_continuity_frontier_contract_model_manifest,
};
pub use local_continuity_frontier_inference::{
    qualify_scope_local_continuity_frontier, scope_local_continuity_frontier_inference_manifest,
};
pub use local_continuity_frontier_research_copilot::{
    qualify_scope_local_continuity_frontier_copilot,
    scope_local_continuity_frontier_research_copilot_manifest,
};
pub use local_continuity_frontier_workflow_fabric::{
    qualify_scope_local_continuity_frontier_workflow,
    scope_local_continuity_frontier_workflow_fabric_manifest,
};
pub use multimodal_continuity_frontier_contract_model::{
    qualify_scope_multimodal_continuity_frontier_contract,
    scope_multimodal_continuity_frontier_contract_model_manifest,
};
pub use multimodal_continuity_frontier_inference::{
    qualify_scope_multimodal_continuity_frontier,
    scope_multimodal_continuity_frontier_inference_manifest,
};
pub use multimodal_continuity_frontier_research_copilot::{
    qualify_scope_multimodal_continuity_frontier_copilot,
    scope_multimodal_continuity_frontier_research_copilot_manifest,
};
pub use multimodal_continuity_frontier_workflow_fabric::{
    qualify_scope_multimodal_continuity_frontier_workflow,
    scope_multimodal_continuity_frontier_workflow_fabric_manifest,
};
pub use throughput_continuity_frontier_contract_model::{
    qualify_scope_throughput_continuity_frontier_contract,
    scope_throughput_continuity_frontier_contract_model_manifest,
};
pub use throughput_continuity_frontier_inference::{
    qualify_scope_throughput_continuity_frontier,
    scope_throughput_continuity_frontier_inference_manifest,
};
pub use throughput_continuity_frontier_research_copilot::{
    qualify_scope_throughput_continuity_frontier_copilot,
    scope_throughput_continuity_frontier_research_copilot_manifest,
};
pub use throughput_continuity_frontier_workflow_fabric::{
    qualify_scope_throughput_continuity_frontier_workflow,
    scope_throughput_continuity_frontier_workflow_fabric_manifest,
};
