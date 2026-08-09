//! Typed failures of the biological evaluation engine.
//!
//! Every variant here exists because the alternative was to emit a number. Section 26.20 lists
//! "missing metrics imputed optimistically" and "overall score rewards unsafe action" as the
//! failure modes that make a leaderboard worthless; the defence is that the engine refuses,
//! loudly and with a reason, rather than substituting a plausible float.

use thiserror::Error;

use crate::comparability::Incomparability;
use crate::wrongness::BiologicalErrorClass;

/// Rejections raised while building a reference standard (31.01).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReferenceError {
    #[error("a reference standard must admit at least one state")]
    NoAdmissibleState,

    #[error("state {state:?} carries a non-finite mass")]
    NonFiniteMass { state: String },

    #[error("state {state:?} carries negative mass {mass}")]
    NegativeMass { state: String, mass: f64 },

    /// The masses must be a distribution. Rescaling silently would let an oracle assert more
    /// confidence than it has by under-declaring the alternatives.
    #[error("reference masses sum to {total}, not 1.0 (tolerance {tolerance})")]
    MassNotNormalised { total: f64, tolerance: f64 },

    #[error("state {state:?} declared more than once")]
    DuplicateState { state: String },

    #[error("aleatoric fraction {fraction} is outside [0, 1]")]
    AleatoricFractionOutOfRange { fraction: f64 },
}

/// Rejections raised while building a prediction.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PredictionError {
    #[error("a distributional prediction must place mass on at least one state")]
    NoPredictedState,

    #[error("predicted state {state:?} carries a non-finite mass")]
    NonFiniteMass { state: String },

    #[error("predicted state {state:?} carries negative mass {mass}")]
    NegativeMass { state: String, mass: f64 },

    #[error("predicted masses sum to {total}, not 1.0 (tolerance {tolerance})")]
    MassNotNormalised { total: f64, tolerance: f64 },

    #[error("state {state:?} declared more than once")]
    DuplicateState { state: String },
}

/// Refusals raised while grading.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ScoreError {
    /// 26.10. The prediction and the reference were not measured on the same footing, so any
    /// number comparing them would be an artefact of the mismatch.
    #[error("measurements are not comparable: {0:?}")]
    Incomparable(Vec<Incomparability>),

    /// 31.01: `unresolved` "is not scored as a hidden negative". A reference that declines to
    /// decide yields no score at all, not a zero.
    #[error("the reference standard is unresolved ({reason}); this is not a zero")]
    ReferenceUnresolved { reason: String },

    /// The case sits outside the reference standard's declared scope.
    #[error("the reference standard is not evaluable here ({reason})")]
    ReferenceNotEvaluable { reason: String },

    /// The prediction names an outcome the reference standard never enumerated. Treating it as
    /// mass zero would quietly assert that the reference is exhaustive, which is the thing under
    /// test.
    #[error("predicted state {state:?} is outside the reference standard's admissible set")]
    StateOutsideReference { state: String },

    /// A witness earned under a laxer gate was offered to a stricter grader. Without this check
    /// the comparability gate is bypassable by anyone willing to construct their own requirement.
    #[error(
        "comparability witness was earned under requirement {found:?}, grader requires {expected:?}"
    )]
    WitnessFromDifferentRequirement { expected: String, found: String },
}

/// Refusals raised when a caller asks for a single number (31.01, 26.20).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CollapseError {
    #[error(
        "policy {policy_id:?} requires reference confidence >= {required}, the reference offers \
         {available}"
    )]
    ReferenceBelowPolicyFloor {
        policy_id: String,
        required: f64,
        available: f64,
    },

    /// The policy proposes to read the reference's spread as noise when the reference declared it
    /// to be biology. That is not a scoring choice, it is a contradiction of the reference
    /// standard, and it always moves the number upward.
    #[error(
        "policy {policy_id:?} discharges reference uncertainty as {discharge}, but the reference \
         declares its dispersion {dispersion}; the resulting number would deny the uncertainty it \
         was derived from"
    )]
    DischargeContradictsDispersion {
        policy_id: String,
        discharge: String,
        dispersion: String,
    },

    #[error("policy {policy_id:?} requires attributed dispersion; this reference is unattributed")]
    UnattributedDispersion { policy_id: String },

    #[error("policy {policy_id:?} declares a confidence floor {floor} outside [0, 1]")]
    MalformedPolicy { policy_id: String, floor: f64 },
}

/// Refusals raised while awarding partial credit (26.02 protocol step 6).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CreditError {
    /// "Retain partial credit only when the remaining conclusion is meaningful." A wrong
    /// molecular subtype does not leave a meaningful remainder.
    #[error("error class {class} is critical; the remaining conclusion is not meaningful")]
    CriticalErrorForfeitsCredit { class: BiologicalErrorClass },

    /// Credit for an unclassified failure is credit whose rule cannot be stated.
    #[error("an unclassified error cannot earn partial credit under rule {rule_id:?}")]
    UnclassifiedError { rule_id: String },

    #[error("rule {rule_id:?} produced fraction {fraction}, outside [0, 1]")]
    FractionOutOfRange { rule_id: String, fraction: f64 },

    #[error("rule {rule_id:?} awarded credit without naming a single earning term")]
    NoBasis { rule_id: String },

    #[error("rule {rule_id:?} could not be digested for replay: {detail}")]
    NotDigestible { rule_id: String, detail: String },
}

/// Refusals raised while aggregating (26.15, 26.20).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum AggregationError {
    #[error("an empty panel has no aggregate; absence of raters is not consensus")]
    EmptyPanel,

    /// 26.15 failure mode: "reviewers see different evidence", generalised to scores. Results
    /// that passed different comparability gates are not results about the same thing, and
    /// 26.20 permits normalising only "within defensible groups".
    #[error(
        "pooled scores mix comparability requirements: expected {expected:?}, found {found:?}"
    )]
    MixedRequirements { expected: String, found: String },

    /// One case refused to collapse and the pool refused with it, rather than quietly averaging
    /// over the cases whose references happened to be confident.
    #[error("case {subject:?} could not be collapsed, so the pool cannot be: {detail}")]
    CaseNotCollapsible { subject: String, detail: String },

    #[error("rater {rater:?} appears twice in the panel")]
    DuplicateRater { rater: String },
}
