//! Decision Sections and Context Certificates.
//!
//! Implements blueprint 43.25 (Decision Section IR) and 43.26 (Context Certificate and omission
//! receipt), plus the plan descriptor of 43.36/43.37 and the oracle verdict shape of 43.41.
//!
//! Deliberately depends on neither the world model nor the compiler. A consumer — an MCP client,
//! an evaluator, a CI gate — must be able to read and verify a compiled context without linking
//! the engine that produced it.

pub mod certificate;
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
pub use interpretation_assurance::{
    assure_interpretations, interpretation_assurance_manifest, EvidenceBackedResult,
    EvidenceBackedState, InteractiveInterpretation, InterpretationAssuranceError,
    InterpretationAssuranceReceipt, InterpretationAssuranceRequest, InterpretationDisposition,
    CONTRACT_VERSION as INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_ASSURANCE_FEATURE_ID,
};
pub use layers::{Layer, RenderContext};
pub use omission::{InfluenceClass, OmissionGroup, OmissionManifest};
pub use plan::{Backend, Fallback, FallbackReason, PlanDescriptor};
pub use section::{
    DecisionSection, EvidenceCapsule, RefinementOption, UnresolvedObligation,
    SECTION_SCHEMA_VERSION,
};
pub use verdict::{LeakageWitness, OracleStatus, OracleVerdict};
