//! Policy, privacy and information-flow fibers.
//!
//! Implements blueprint 43.33 (policy, privacy and information-flow fibers), together with the
//! parts of section 13 (13.16 data classification and minimisation, 13.17 PII/PHI governance),
//! section 36 (36.01 classification and information-flow labels, 36.04 consent and purpose
//! limitation, 36.06 federated evaluation, 36.18 retention and withdrawal) and 39.19 (context
//! security, privacy and role filters) that the fiber has to satisfy to be more than a diagram.
//!
//! # The one decision this crate exists to enforce
//!
//! 43.33's normative sentence: constraints are "enforced during compilation and execution—not as a
//! final response filter". A response filter is the natural design and the wrong one, because by
//! the time it runs the compiler has already read, ranked, joined and cached the evidence it is
//! about to hide. [`PolicyLattice::admits`] is therefore callable with nothing but an evidence
//! scope and a bound [`Request`] — no Decision Section, no rendered answer, no scores — so a
//! selector can consult it per candidate and never bring refused evidence into scoring range at
//! all. [`PolicyLattice::screen`] is the same check over a candidate set.
//!
//! # Shape
//!
//! * [`label`] — the [`PolicyLabel`] lattice with `join`, `meet` and [`PolicyLabel::flows_to`].
//!   Seven axes; three of them run backwards, and each says so where it is defined.
//! * [`lattice`] — policy as a fiber over `bioprism_scope::ScopeKey`. Rules attach at scopes and
//!   restrict along refinement; the effective label at a scope is the join of every rule that
//!   governs it; a scope no rule claims is unlabelled and refuses everything.
//! * [`purpose`] and [`consent`] — purpose binding, with a closed purpose enumeration and no
//!   implication order, so purpose creep has no accidental spelling.
//! * [`residency`] — jurisdictions, and the [`residency::propose_transport`] declaration that turns
//!   a policy-crossing move into a `ScopeMapping` with a loss ledger instead of a silent copy.
//! * [`flow`] — [`flow::derive`] for the safe direction, and a versioned, authority-gated,
//!   leakage-analysed [`flow::DeclassificationRegistry`] for the unsafe one.
//! * [`redaction`] — semantic replacement with receipts, and small-cell suppression that returns a
//!   bounded unknown instead of an unsafe count.
//! * [`trace`] — the policy trace that goes in the certificate, convertible into the
//!   `bioprism_section` omission and unresolved-obligation vocabulary.
//!
//! # Relationship to `bioprism_weave`
//!
//! `bioprism_weave::ContextCapsule` projects a *compiled* Decision Section for one recipient by
//! clearance label, and reports what the projection withheld. This crate is the layer beneath it
//! and answers a different question: which evidence was ever eligible to be compiled, for this
//! principal, this purpose, this decision time and this destination. A capsule can only withhold
//! from a set the compiler already assembled; the lattice decides what goes into that set. Neither
//! subsumes the other, and running only the capsule projection would mean the controlled sections
//! were read, joined and cached before anyone checked whether they could be.
//!
//! # What this crate deliberately does not do
//!
//! No cryptography, no key management, no tokenisation, no real IAM. A [`Principal`] is an
//! already-authenticated claim and this crate cannot tell a forged one from a real one. There is
//! no network egress control, no sandbox, no audit storage, and no verification that a remote pod
//! honoured an obligation it was handed. There is no inference-attack model: small cells are
//! suppressed by threshold, and whether a set of thresholded releases reconstructs a suppressed
//! one is a question 36.02 asks and this crate cannot answer. Obligations are *stated*, not
//! discharged — a caller that ignores them produces an illegal result that nothing here will
//! notice.
//!
//! In 36.01's enforcement architecture this crate is the policy-intersection step, not the trusted
//! kernel that surrounds it.

pub mod autonomy;
pub mod autonomy_batch;
pub mod consent;
pub mod decision;
pub mod error;
pub mod flow;
pub mod interoperability_control;
pub mod label;
pub mod lattice;
pub mod protocol_assurance;
pub mod purpose;
pub mod redaction;
pub mod request;
pub mod residency;
pub mod trace;
pub mod federated_continual_evidence_surveillance_contract_model;

pub use autonomy::{
    admit_autonomy, AutonomyAdmissionReceipt, AutonomyAdmissionRequest, AutonomyError,
};
pub use autonomy_batch::{
    admit_autonomy_batch, autonomy_batch_manifest, BatchActionDecision, BatchActionReceipt,
    BatchAdmissionAction, BatchAdmissionError, BatchAdmissionReceipt, BatchAdmissionRequest,
    FEATURE_ID as AUTONOMY_BATCH_FEATURE_ID, FEATURE_VERSION as AUTONOMY_BATCH_FEATURE_VERSION,
};
pub use consent::{Consent, ConsentStatus};
pub use decision::{Admission, Decision, ExecutionMode, Obligation, Refusal};
pub use error::PolicyError;
pub use federated_continual_evidence_surveillance_contract_model::{
    federated_continual_evidence_surveillance_contract_model_manifest,
    model_federated_continual_evidence_surveillance_contract,
    FederatedContinualContractClaim,
    FederatedContinualContractCompatibility,
    FederatedContinualContractDisposition,
    FederatedContinualEvidenceSurveillanceContractError,
    FederatedContinualEvidenceSurveillanceContractReceipt,
    FederatedContinualEvidenceSurveillanceContractRequest,
    CONTRACT_VERSION as POLICY_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as POLICY_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    INPUT_SCHEMA as POLICY_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_INPUT_SCHEMA,
    OUTPUT_SCHEMA as POLICY_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_OUTPUT_SCHEMA,
};
pub use flow::{
    check_flow, derive, DeclassificationReceipt, DeclassificationRegistry, DeclassificationRule,
};
pub use interoperability_control::{
    interoperability_control_manifest, negotiate_interoperability, ExternalCapabilityOffer,
    IntegrationDisposition, InteroperabilityControlError, InteroperabilityControlReceipt,
    InteroperabilityControlRequest, OfferEvidenceState,
    CONTRACT_VERSION as INTEROPERABILITY_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as INTEROPERABILITY_CONTROL_FEATURE_ID,
};
pub use label::{Classification, ExportPolicy, PolicyLabel, Retention};
pub use lattice::{
    AdmittedFact, LabelResolution, PolicyLattice, PolicyRule, RefusedFact, Screening,
};
pub use protocol_assurance::{
    assess_protocol_assurance, ProtocolAssuranceDisposition, ProtocolAssuranceError,
    ProtocolAssuranceReceipt, ProtocolAssuranceRequest,
    CONTRACT_VERSION as PROTOCOL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as PROTOCOL_ASSURANCE_FEATURE_ID,
};
pub mod analysis_copilot;
pub mod federated_commons_interoperability_gateway;
pub mod federated_continual_grant_integrity_contract_model;
pub mod federated_continual_grant_integrity_inference;
pub mod federated_continual_grant_integrity_research_copilot;
pub mod federated_continual_grant_integrity_workflow_fabric;
pub mod grant_integrity_support;
pub mod local_grant_integrity_contract_model;
pub mod local_grant_integrity_inference;
pub mod local_grant_integrity_research_copilot;
pub mod local_grant_integrity_workflow_fabric;
pub mod multimodal_grant_integrity_contract_model;
pub mod multimodal_grant_integrity_inference;
pub mod multimodal_grant_integrity_research_copilot;
pub mod multimodal_grant_integrity_workflow_fabric;
pub mod throughput_grant_integrity_contract_model;
pub mod throughput_grant_integrity_inference;
pub mod throughput_grant_integrity_research_copilot;
pub mod throughput_grant_integrity_workflow_fabric;
pub use analysis_copilot::{
    analysis_copilot_manifest, qualify_analysis_question, AnalysisCandidate5, AnalysisCopilotError,
    AnalysisDisposition, AnalysisQuestion4, QualifiedAnalysisResult3,
    CONTRACT_VERSION as ANALYSIS_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as ANALYSIS_COPILOT_FEATURE_ID,
};
pub use grant_integrity_support::{qualify as qualify_grant_integrity, manifest as grant_integrity_manifest, AutonomyGrant4, GrantIntegrityArtifact4, GrantIntegrityCard7, GrantIntegrityError, GrantIntegrityRequest4, BOUNDARY as GRANT_INTEGRITY_BOUNDARY, CONTENT_TYPE as GRANT_INTEGRITY_CONTENT_TYPE};
pub use local_grant_integrity_inference::*;
pub use multimodal_grant_integrity_inference::*;
pub use throughput_grant_integrity_inference::*;
pub use federated_continual_grant_integrity_inference::*;
pub use local_grant_integrity_contract_model::*;
pub use multimodal_grant_integrity_contract_model::*;
pub use throughput_grant_integrity_contract_model::*;
pub use federated_continual_grant_integrity_contract_model::*;
pub use local_grant_integrity_research_copilot::*;
pub use multimodal_grant_integrity_research_copilot::*;
pub use throughput_grant_integrity_research_copilot::*;
pub use federated_continual_grant_integrity_research_copilot::*;
pub use local_grant_integrity_workflow_fabric::*;
pub use multimodal_grant_integrity_workflow_fabric::*;
pub use throughput_grant_integrity_workflow_fabric::*;
pub use federated_continual_grant_integrity_workflow_fabric::*;
pub use federated_commons_interoperability_gateway::{
    admit as admit_policy_federation, admit_json as admit_policy_federation_json,
    capability_manifest as federated_commons_manifest, FederationAdmission,
    FederationArtifactCandidate, PolicyFederationEnvelope, PolicyFederationError,
    PolicyFederationRequest, CONTRACT_VERSION as FEDERATED_COMMONS_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_COMMONS_FEATURE_ID, INPUT_SCHEMA as FEDERATED_COMMONS_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_COMMONS_OUTPUT_SCHEMA,
};
pub use purpose::{Purpose, PurposeSet};
pub use redaction::{
    CellRelease, RedactedView, RedactionPlan, RedactionReceipt, RedactionRule, Replacement,
    SmallCellRule,
};
pub use request::{Authority, Channel, Clearance, Principal, Request};
pub use residency::{
    declared_residency, propose_transport, Jurisdiction, Residency, RESIDENCY_DIMENSION,
};
pub use trace::{PolicyTrace, TraceEntry, TraceOutcome};
