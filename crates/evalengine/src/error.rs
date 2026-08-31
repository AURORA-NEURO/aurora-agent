//! Typed failures.
//!
//! The engine distinguishes two things that are easy to conflate. A **refusal** is a legitimate
//! answer: an attribution that cannot be made, a scalar that must not be published. Refusals are
//! values — [`crate::attribution::Attribution::Refused`] and the `Err` arm of
//! [`crate::posterior::CapabilityPosterior::overall`] — and they carry their reason into the
//! serialized report. An **error** is a malformed input: a rubric with two constraints of the same
//! name, a coverage contract with no rationale, a clustered estimate over nothing.
//!
//! Both are `Err` for the caller. The difference is what the caller should do: fix the input, or
//! accept that the evidence does not support the claim.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Everything this crate can refuse or reject.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum EvalError {
    /// Internal maps or derived state no longer agree. Refuse the aggregate rather than
    /// fabricating a missing capability estimate.
    #[error("evaluation engine invariant violated: {detail}")]
    InvariantViolation { detail: String },

    /// Composition was asked to score a result with no evidence at all.
    ///
    /// The absence of evaluators is not a failing result. It is an unscored one, and the caller
    /// must say which it meant.
    #[error("result `{result_id}` has no evaluator contributions; an unevaluated result has no score, not a zero")]
    NoContributions { result_id: String },

    /// Two rubric constraints share a name, so partial credit would double-count or silently drop
    /// one of them.
    #[error("rubric constraint `{name}` is declared more than once")]
    DuplicateConstraint { name: String },

    /// A report could not be reduced to canonical bytes, so it cannot be content-addressed.
    #[error("value could not be canonicalized for content addressing: {detail}")]
    NotCanonicalizable { detail: String },

    /// A clustered estimate was requested over an empty sample.
    #[error("clustered estimate `{label}` has no observations")]
    EmptySample { label: String },

    /// A cluster was declared with no members.
    #[error("parent `{parent}` in sample `{label}` contributes no instances")]
    EmptyCluster { label: String, parent: String },

    /// A release gate was declared without saying why the scalar is the right one.
    ///
    /// Blueprint 07.05 permits a scalar "for a specific release gate only with its formula,
    /// rationale, and sensitivity analysis". An empty rationale fails that condition before any
    /// data is looked at.
    #[error("release gate `{gate}` declares no rationale for collapsing a capability vector to a scalar")]
    GateWithoutRationale { gate: String },

    /// A gate declared no coverage floors at all.
    #[error("release gate `{gate}` declares no coverage floors; a scalar with no coverage contract is not obtainable")]
    GateWithoutCoverageFloors { gate: String },

    /// The gate names a capability the posterior never measured.
    #[error("release gate `{gate}` requires capability `{capability}`, which the posterior does not contain")]
    CapabilityUnobserved { gate: String, capability: String },

    /// Too few independent parents behind a capability estimate.
    #[error("capability `{capability}` rests on {observed} parent cluster(s), below the floor of {required} declared by gate `{gate}`")]
    ClusterFloorUnmet {
        gate: String,
        capability: String,
        observed: usize,
        required: usize,
    },

    /// Enough instances, but not enough *independent* information in them.
    #[error("capability `{capability}` has effective sample size {observed:.2}, below the floor of {required:.2} declared by gate `{gate}`")]
    EffectiveSampleFloorUnmet {
        gate: String,
        capability: String,
        observed: f64,
        required: f64,
    },

    /// An outstanding veto. Blueprint 07.01 makes vetoes individually visible and fail-closed;
    /// they are never averaged away.
    #[error("capability `{capability}` holds an outstanding {kind} veto (`{detail}`); gate `{gate}` fails closed")]
    VetoOutstanding {
        gate: String,
        capability: String,
        kind: String,
        detail: String,
    },

    /// More of the sample was unknown than the gate declared it would tolerate.
    #[error("capability `{capability}` is {observed:.3} unknown by fraction, above the {tolerated:.3} tolerated by gate `{gate}`")]
    UnknownFractionExceeded {
        gate: String,
        capability: String,
        observed: f64,
        tolerated: f64,
    },

    /// The gate demanded grounded evidence and got opinion.
    #[error("capability `{capability}` rests on {weakest} evidence, weaker than the {required} floor declared by gate `{gate}`")]
    TierFloorUnmet {
        gate: String,
        capability: String,
        weakest: String,
        required: String,
    },
}
