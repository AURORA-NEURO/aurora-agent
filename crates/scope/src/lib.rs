//! The typed scope base.
//!
//! Implements blueprint 43.03 (typed scope base), the mapping taxonomy of 43.05
//! (restriction, transport, base change) and the structured-failure requirement of 43.06.
//!
//! Evidence in BioPRISM is valid only inside a scope. This crate supplies the type that says
//! *where* a claim holds, the partial order that says when one scope is narrower than another,
//! and the meet that says whether two scopes overlap at all — plus the vocabulary for moving
//! evidence between scopes without pretending the move was free.

pub mod class;
pub mod error;
pub mod key;
pub mod meet;
pub mod time;
pub mod transport;
pub mod throughput_federated_evidence_control_plane;
pub mod federated_commons_interoperability_gateway;

pub use class::{DimensionRegistry, ScopeClass};
pub use error::{ScopeError, TimeError};
pub use key::{ScopeKey, ScopeValue};
pub use meet::{meet, EmptyReason, Meet};
pub use time::{Interval, Timestamp};
pub use transport::{
    AggregationOperator, LossLedger, MappingCheck, MappingKind, ScopeMapping,
};
pub use throughput_federated_evidence_control_plane::{
    operate_federated_evidence_control, federated_evidence_control_manifest,
    EvidenceControlError, EvidenceControlRequest6, FederatedEvidenceControlArtifact9,
    FederatedEvidenceControlReceipt9, PeerEvidence4, ThroughputEvidence4,
    CONTRACT_VERSION as SCOPE_FEDERATED_EVIDENCE_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as SCOPE_FEDERATED_EVIDENCE_CONTROL_FEATURE_ID,
};
pub use federated_commons_interoperability_gateway::{
    operate_federated_scope_interoperability_gateway,
    federated_scope_interoperability_manifest,
    ScopeGatewayArtifact4, ScopeGatewayError, ScopeFederationGatewayRequest7,
    ScopeFederationGatewayReceipt10, ScopeFederationReceiptArtifact10, ScopePeerManifest4,
    CONTRACT_VERSION as SCOPE_FEDERATED_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as SCOPE_FEDERATED_INTEROPERABILITY_FEATURE_ID,
};
pub mod continuity_frontier_support;
pub mod local_continuity_frontier_inference;
pub mod multimodal_continuity_frontier_inference;
pub mod throughput_continuity_frontier_inference;
pub mod federated_continual_continuity_frontier_inference;
pub mod local_continuity_frontier_contract_model;
pub mod multimodal_continuity_frontier_contract_model;
pub mod throughput_continuity_frontier_contract_model;
pub mod federated_continual_continuity_frontier_contract_model;
pub mod local_continuity_frontier_research_copilot;
pub mod multimodal_continuity_frontier_research_copilot;
pub mod throughput_continuity_frontier_research_copilot;
pub mod federated_continual_continuity_frontier_research_copilot;
pub mod local_continuity_frontier_workflow_fabric;
pub mod multimodal_continuity_frontier_workflow_fabric;
pub mod throughput_continuity_frontier_workflow_fabric;
pub mod federated_continual_continuity_frontier_workflow_fabric;
pub use continuity_frontier_support::{ScopeAssertion4,ScopeContinuityRequest4,ScopeContinuityCard7,ScopeContinuityArtifact4,ScopeContinuityError};
pub use local_continuity_frontier_inference::{scope_local_continuity_frontier_inference_manifest,qualify_scope_local_continuity_frontier};
pub use multimodal_continuity_frontier_inference::{scope_multimodal_continuity_frontier_inference_manifest,qualify_scope_multimodal_continuity_frontier};
pub use throughput_continuity_frontier_inference::{scope_throughput_continuity_frontier_inference_manifest,qualify_scope_throughput_continuity_frontier};
pub use federated_continual_continuity_frontier_inference::{scope_federated_continual_continuity_frontier_inference_manifest,qualify_scope_federated_continuity_frontier};
pub use local_continuity_frontier_contract_model::{scope_local_continuity_frontier_contract_model_manifest,qualify_scope_local_continuity_frontier_contract};
pub use multimodal_continuity_frontier_contract_model::{scope_multimodal_continuity_frontier_contract_model_manifest,qualify_scope_multimodal_continuity_frontier_contract};
pub use throughput_continuity_frontier_contract_model::{scope_throughput_continuity_frontier_contract_model_manifest,qualify_scope_throughput_continuity_frontier_contract};
pub use federated_continual_continuity_frontier_contract_model::{scope_federated_continual_continuity_frontier_contract_model_manifest,qualify_scope_federated_continuity_frontier_contract};
pub use local_continuity_frontier_research_copilot::{scope_local_continuity_frontier_research_copilot_manifest,qualify_scope_local_continuity_frontier_copilot};
pub use multimodal_continuity_frontier_research_copilot::{scope_multimodal_continuity_frontier_research_copilot_manifest,qualify_scope_multimodal_continuity_frontier_copilot};
pub use throughput_continuity_frontier_research_copilot::{scope_throughput_continuity_frontier_research_copilot_manifest,qualify_scope_throughput_continuity_frontier_copilot};
pub use federated_continual_continuity_frontier_research_copilot::{scope_federated_continual_continuity_frontier_research_copilot_manifest,qualify_scope_federated_continuity_frontier_copilot};
pub use local_continuity_frontier_workflow_fabric::{scope_local_continuity_frontier_workflow_fabric_manifest,qualify_scope_local_continuity_frontier_workflow};
pub use multimodal_continuity_frontier_workflow_fabric::{scope_multimodal_continuity_frontier_workflow_fabric_manifest,qualify_scope_multimodal_continuity_frontier_workflow};
pub use throughput_continuity_frontier_workflow_fabric::{scope_throughput_continuity_frontier_workflow_fabric_manifest,qualify_scope_throughput_continuity_frontier_workflow};
pub use federated_continual_continuity_frontier_workflow_fabric::{scope_federated_continual_continuity_frontier_workflow_fabric_manifest,qualify_scope_federated_continuity_frontier_workflow};
