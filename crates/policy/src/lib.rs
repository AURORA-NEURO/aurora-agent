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
pub mod label;
pub mod lattice;
pub mod protocol_assurance;
pub mod purpose;
pub mod redaction;
pub mod request;
pub mod residency;
pub mod trace;

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
pub use flow::{
    check_flow, derive, DeclassificationReceipt, DeclassificationRegistry, DeclassificationRule,
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
