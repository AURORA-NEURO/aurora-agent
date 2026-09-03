//! Typed failures.
//!
//! Blueprint 39.16 and 39.17 both require that every declared error class be observable as a
//! typed event rather than inferred from a log line, so each failure named in those modules —
//! unclassified omission, mandatory edge omitted, budget exhaustion, branch duplicating a lease,
//! stale graph — has a variant here.
//!
//! Errors are split by concern rather than pooled into one enum: a caller reconciling a token
//! ledger should not have to match on capsule field validation, and the compiler should not be
//! able to convert a closure violation into a budget problem by accident.

use bioprism_ids::CanonicalError;
use thiserror::Error;

/// Failures of the decision obligation graph (39.06).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ObligationError {
    #[error("obligation graph invariant violated: {detail}")]
    InvariantViolation { detail: String },

    #[error("obligation `{0}` is already in the graph")]
    DuplicateObligation(String),

    #[error("obligation `{0}` is not in the graph")]
    UnknownObligation(String),

    #[error("obligation `{obligation}` depends on `{missing}`, which is not in the graph")]
    DanglingDependency { obligation: String, missing: String },

    #[error("dependency cycle reachable from obligation `{0}`")]
    DependencyCycle(String),

    #[error("state `{state}` on obligation `{obligation}` requires an explicit reason")]
    ReasonRequired { obligation: String, state: String },

    #[error("state `{state}` on obligation `{obligation}` requires at least one evidence locator")]
    EvidenceRequired { obligation: String, state: String },

    #[error("obligation `{0}` was recorded as unseen while carrying evidence")]
    EvidenceOnUnseen(String),

    #[error("state record for obligation `{0}` names no actor")]
    ActorRequired(String),

    #[error("confidence {confidence} on obligation `{obligation}` is outside 0.0..=1.0")]
    ConfidenceOutOfRange { obligation: String, confidence: f64 },

    #[error("obligation `{obligation}` carries a non-finite value {value}")]
    NonFiniteValue { obligation: String, value: f64 },
}

/// Failures of the token, latency, privacy and tool budget controller (39.16).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BudgetError {
    #[error(
        "mandatory closure needs {required} tokens but the run budget is {total}; \
         mandatory invariants are not tradeable for tokens"
    )]
    MandatoryClosureUnaffordable { required: usize, total: usize },

    #[error("lease for `{holder}` requested {requested} tokens but only {available} remain")]
    LeaseExceedsHolding {
        holder: String,
        requested: usize,
        available: usize,
    },

    #[error("lease {0} is not held by this controller")]
    UnknownLease(usize),

    #[error("branch `{0}` was rejected and may not spend further; its ledger is retained")]
    LeaseRejected(String),
}

/// Failures of the omission ledger and sufficiency certificate (39.17).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum LedgerError {
    #[error("candidate `{0}` appears twice in the ledger")]
    DuplicateCandidate(String),

    #[error("mandatory candidate `{0}` was omitted without a classified reason")]
    UnclassifiedMandatoryOmission(String),

    #[error("candidate `{0}` claims zero or bounded influence without an argument")]
    UnsupportedInfluenceClaim(String),

    #[error("candidate `{0}` claims bounded influence without a bound")]
    MissingInfluenceBound(String),

    #[error("candidate `{0}` is policy-redacted but names no policy")]
    PolicyReasonNotReproducible(String),
}

/// Failures of the BioContext capsule contract (39.03).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CapsuleError {
    #[error("capsule is missing the required field `{0}`")]
    MissingField(&'static str),

    #[error(
        "visibility cutoff {cutoff} is later than valid time {valid_at}; \
         the capsule would expose evidence released after the decision cut"
    )]
    CutAfterValidTime { cutoff: String, valid_at: String },

    #[error(
        "sufficiency certificate is scoped to decision `{decision}` and role `{role}`, \
         but the capsule is for decision `{capsule_decision}` and role `{capsule_role}`"
    )]
    CertificateScopeMismatch {
        decision: String,
        role: String,
        capsule_decision: String,
        capsule_role: String,
    },

    #[error("omission ledger is invalid: {0}")]
    Ledger(#[from] LedgerError),

    #[error("capsule is not canonically serialisable: {0}")]
    Canonical(#[from] CanonicalError),
}
