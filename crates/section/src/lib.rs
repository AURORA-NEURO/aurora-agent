//! Decision Sections and Context Certificates.
//!
//! Implements blueprint 43.25 (Decision Section IR) and 43.26 (Context Certificate and omission
//! receipt), plus the plan descriptor of 43.36/43.37 and the oracle verdict shape of 43.41.
//!
//! Deliberately depends on neither the world model nor the compiler. A consumer — an MCP client,
//! an evaluator, a CI gate — must be able to read and verify a compiled context without linking
//! the engine that produced it.

pub mod certificate;
pub mod closure_integrity_support;
pub mod local_closure_integrity_inference;
pub mod multimodal_closure_integrity_inference;
pub mod throughput_closure_integrity_inference;
pub mod federated_continual_closure_integrity_inference;
pub mod local_closure_integrity_contract_model;
pub mod multimodal_closure_integrity_contract_model;
pub mod throughput_closure_integrity_contract_model;
pub mod federated_continual_closure_integrity_contract_model;
pub mod local_closure_integrity_research_copilot;
pub mod multimodal_closure_integrity_research_copilot;
pub mod throughput_closure_integrity_research_copilot;
pub mod federated_continual_closure_integrity_research_copilot;
pub mod local_closure_integrity_workflow_fabric;
pub mod multimodal_closure_integrity_workflow_fabric;
pub mod throughput_closure_integrity_workflow_fabric;
pub mod federated_continual_closure_integrity_workflow_fabric;
pub mod interpretation_assurance;
pub mod layers;
pub mod omission;
pub mod plan;
pub mod section;
pub mod verdict;

pub use certificate::{
    CertificateProfile, CertificateVerification, ContextCertificate, ReferenceOmissions,
    SourceHashes, CERTIFICATE_SCHEMA_VERSION, CERTIFICATE_SCHEMA_VERSION_EXTENDED,
};
pub use closure_integrity_support::{compile as compile_closure_integrity, manifest as closure_integrity_manifest, ClosureIntegrityArtifact4, ClosureIntegrityCard7, ClosureIntegrityError, ClosureIntegrityRequest4, SectionClaim4, BOUNDARY as CLOSURE_INTEGRITY_BOUNDARY, CONTENT_TYPE as CLOSURE_INTEGRITY_CONTENT_TYPE};
pub use local_closure_integrity_inference::*;
pub use multimodal_closure_integrity_inference::*;
pub use throughput_closure_integrity_inference::*;
pub use federated_continual_closure_integrity_inference::*;
pub use local_closure_integrity_contract_model::*;
pub use multimodal_closure_integrity_contract_model::*;
pub use throughput_closure_integrity_contract_model::*;
pub use federated_continual_closure_integrity_contract_model::*;
pub use local_closure_integrity_research_copilot::*;
pub use multimodal_closure_integrity_research_copilot::*;
pub use throughput_closure_integrity_research_copilot::*;
pub use federated_continual_closure_integrity_research_copilot::*;
pub use local_closure_integrity_workflow_fabric::*;
pub use multimodal_closure_integrity_workflow_fabric::*;
pub use throughput_closure_integrity_workflow_fabric::*;
pub use federated_continual_closure_integrity_workflow_fabric::*;
pub use interpretation_assurance::{
    assure_interpretations, interpretation_assurance_manifest, EvidenceBackedResult,
    EvidenceBackedState, InteractiveInterpretation, InterpretationAssuranceError,
    InterpretationAssuranceReceipt, InterpretationAssuranceRequest, InterpretationDisposition,
    CONTRACT_VERSION as INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_ASSURANCE_FEATURE_ID,
};
pub use layers::{Layer, RenderContext};
pub use omission::{InfluenceClass, OmissionGroup, OmissionManifest};
pub use omission::{InformativeBound, OmissionAccountingError, ProvenUnreachable};
pub use plan::{Backend, Fallback, FallbackReason, PlanDescriptor};
pub use section::{
    DecisionSection, EvidenceCapsule, RefinementOption, UnresolvedObligation,
    SECTION_SCHEMA_VERSION,
};
pub use verdict::{LeakageWitness, OracleStatus, OracleVerdict};
