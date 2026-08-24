//! Decision obligations, action gates, and the budget and sufficiency machinery around them.
//!
//! Implements blueprint section 39 (Token-Efficient Biological Inference): the decision obligation
//! graph of 39.06, the non-compressible invariants of 39.05, the token budget controller of 39.16,
//! the omission ledger and sufficiency certificate of 39.17, and the capsule contract of 39.03
//! that carries all four to a consumer.
//!
//! The section's thesis is that a smaller prompt is not a success if it increases overclaiming or
//! drops the facts that made an inference valid. This crate is the enforcement half of that claim.
//! Everything in it answers one of two questions:
//!
//! - **May this action run?** [`gate::may_perform`] is the crate's most valuable function. A
//!   high-regret action declares prerequisite obligation predicates and is blocked when they are
//!   unmet — including when it declares none at all, because an undeclared gate must not be an
//!   open one.
//! - **Is this context adequate, or merely small?** [`ledger::SufficiencyCertificate::decide`]
//!   answers only from what was actually established: an unmet obligation whose influence was
//!   never analysed yields `unknown`, never `sufficient`.
//!
//! ## Reuse rather than reimplementation
//!
//! Influence classification and the omission manifest come from `bioprism_section`
//! ([`bioprism_section::InfluenceClass`], [`bioprism_section::OmissionManifest`]) and the layer
//! ladder from [`bioprism_section::Layer`]. Those types already encode the distinction between
//! *provably cannot matter* and *nobody checked*, which is the distinction a sufficiency claim
//! rests on, and a second copy of it in this crate would be a second place for it to drift.
//!
//! ## What this crate does not do
//!
//! It does not compile goals into obligations — that is 39.02's evidence-graph compiler. It does
//! not select evidence: the scoring function of 39.07 is out of scope, and only the greedy
//! value-per-token ordering it implies appears here, inside the budget controller. It does not
//! authenticate actors or enforce role authority (39.19). And it contains no tokenizer: every
//! token count is an estimate that names its own method, per [`budget::EstimationMethod`].

pub mod budget;
pub mod capsule;
pub mod error;
pub mod gate;
pub mod graph;
pub mod invariants;
pub mod ledger;
pub mod release_harness;
pub mod state;

pub use budget::{
    Allocation, BudgetEvent, BudgetLedger, BudgetPlan, BudgetRequest, EstimationMethod, Lease,
    LeaseId, Rejection, RejectionReason, StopDecision, TokenBudgetController, TokenEstimate,
};
pub use capsule::{BioContextCapsule, CAPSULE_SCHEMA_VERSION};
pub use error::{BudgetError, CapsuleError, LedgerError, ObligationError};
pub use gate::{
    may_perform, Action, BlockReason, Gate, ObligationPredicate, RegretClass, UnmetPrerequisite,
};
pub use graph::{FrontierEntry, Obligation, ObligationGraph};
pub use invariants::{
    ClosureReport, ClosureSelection, ClosureViolation, ProtectedClass, Representation,
};
pub use ledger::{
    CertificateCheck, OmissionLedger, OmissionReason, OmittedCandidate, RelevanceIndex,
    RelevanceRecord, SufficiencyCertificate, SufficiencyInputs, SufficiencyStatus, UnmetObligation,
};
pub use release_harness::{
    assess_release_harness, HarnessCheck, HarnessDisposition, ReleaseHarnessError,
    ReleaseHarnessReceipt, ReleaseHarnessRequest,
    CONTRACT_VERSION as RELEASE_HARNESS_CONTRACT_VERSION, FEATURE_ID as RELEASE_HARNESS_FEATURE_ID,
};
pub use state::{ObligationState, StateRecord};
