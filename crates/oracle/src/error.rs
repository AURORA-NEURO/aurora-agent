//! Typed failures of the oracle runtime.
//!
//! Blueprint 40.21 ("Failure semantics") requires every failure to be a typed event rather than a
//! panic or a silently degraded result, and to say whether it invalidates the current projection
//! or the underlying result. The distinction this module encodes is between a *fault of the
//! oracle* and a *judgement by the oracle*:
//!
//! * an `OracleError` means the oracle, its manifest, or the harness is wrong — a malformed
//!   validity window, a property configured over a field that cannot be ordered, two oracles
//!   registered under one identity;
//! * [`crate::Position::NotEvaluable`] means the oracle is fine and declines to judge *this*
//!   evidence.
//!
//! Collapsing the two would let a broken grader read as an abstention, which is exactly the
//! "grader bug" failure mode that 31.15 asks to be surfaced rather than absorbed.

use crate::ladder::EvidenceTier;
use crate::plane::Plane;
use bioprism_ids::CanonicalError;
use thiserror::Error;

/// Every way the oracle runtime can fail as opposed to judge.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum OracleError {
    /// An oracle identifier did not have the `namespace:name` shape 31.01 mandates.
    #[error("malformed oracle id {value:?}: {reason}")]
    MalformedOracleId { value: String, reason: &'static str },

    /// 40.21 invariant 1 requires each oracle to state what it establishes *and* what it cannot.
    /// Declaring the same plane on both sides is a contradiction, not a conservative default.
    #[error("oracle {kind} declares plane {plane} as both established and not established")]
    ContradictoryPlaneDeclaration { kind: String, plane: Plane },

    /// An oracle that establishes nothing cannot contribute evidence and must not be registered.
    #[error("oracle {kind} declares no plane that it establishes")]
    NoEstablishedPlane { kind: String },

    /// Timestamps are constrained so that lexical order *is* chronological order; see
    /// [`crate::UtcTimestamp`].
    #[error("malformed timestamp {value:?}: {reason}")]
    MalformedTimestamp { value: String, reason: &'static str },

    /// A validity window (31.16) that closes before it opens admits nothing and is a manifest bug.
    #[error("validity window ends at {valid_until} before it begins at {valid_from}")]
    InvertedValidityWindow {
        valid_from: String,
        valid_until: String,
    },

    /// Confidence is a probability, not a score (31.01).
    #[error("confidence {value} is outside the closed unit interval")]
    ConfidenceOutOfRange { value: f64 },

    /// A reference distribution that is not a distribution cannot be scored by a proper scoring
    /// rule, which is the only scoring 31.01 permits for distributional truth.
    #[error("malformed position distribution: {reason}")]
    MalformedDistribution { reason: String },

    /// The evidence lacks a field the caller asserted would be present.
    #[error("evidence for {subject:?} has no field {pointer:?}")]
    MissingEvidenceField { subject: String, pointer: String },

    /// An oracle was configured to compare fields that cannot be compared. This is a
    /// configuration fault, not a contradiction in the artifact.
    #[error("oracle {kind} cannot compare {pointer:?}: expected {expected}, found {actual}")]
    NonComparableField {
        kind: String,
        pointer: String,
        expected: &'static str,
        actual: String,
    },

    /// Two oracles sharing one `kind` and version make observations unattributable.
    #[error("oracle {oracle} is already registered in this mesh")]
    DuplicateOracle { oracle: String },

    /// Combination over no oracles would produce a verdict with no evidence behind it.
    #[error("the mesh holds no oracle")]
    EmptyMesh,

    /// 31.15 appeals move *up* the ladder. An adjudicator at or below the disputed tier cannot
    /// settle anything; treating it as decisive would be majority voting with extra steps.
    #[error(
        "an adjudicator at tier {offered} cannot settle a disagreement at tier {dispute}: \
         adjudication requires a strictly stronger tier"
    )]
    AdjudicationTierTooLow {
        dispute: EvidenceTier,
        offered: EvidenceTier,
    },

    /// An abstention resolves nothing, so it cannot close a disagreement record.
    #[error("oracle {oracle} abstained and so cannot settle a disagreement")]
    AdjudicationAbstains { oracle: String },

    /// An inadmissible judgement (expired, superseded, not yet valid) is inadmissible everywhere,
    /// including in adjudication (31.16).
    #[error("oracle {oracle} is inadmissible ({reason}) and so cannot settle a disagreement")]
    InadmissibleAdjudicator { oracle: String, reason: String },

    /// Canonical serialisation failed while content-hashing an artifact.
    #[error("canonical serialization failed: {0}")]
    Canonical(#[from] CanonicalError),
}
