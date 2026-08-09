//! Typed failures.
//!
//! Every 08 module repeats the same mitigation: "fail closed where integrity or safety is
//! affected, preserve the underlying evidence, and emit an actionable diagnostic rather than
//! silently repairing or discarding state." For this crate that mostly means refusing to
//! *report*. An estimate whose coverage floor was never reached is not a slightly worse
//! estimate; it is an answer to a question nobody asked, and returning it with a caveat string
//! is exactly how caveats get dropped downstream.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AdaptiveError {
    #[error("empty identifier for {kind}")]
    EmptyId { kind: &'static str },

    #[error("identifier for {kind} contains a control character: {value:?}")]
    ControlCharacterId { kind: &'static str, value: String },

    #[error("beta parameters must be finite and strictly positive, got alpha={alpha}, beta={beta}")]
    InvalidBetaParameters { alpha: f64, beta: f64 },

    #[error("credibility must lie strictly between 0 and 1, got {0}")]
    InvalidCredibility(f64),

    #[error("probability must lie in [0, 1], got {0}")]
    InvalidProbability(f64),

    #[error("intraclass correlation must lie in [0, 1], got {0}")]
    InvalidIcc(f64),

    #[error("candidate {instance} declares cost {cost}, which is not finite and strictly positive")]
    InvalidCost { instance: String, cost: f64 },

    #[error(
        "instance {instance} already has a scored trial for capability {capability}; the \
         clustered model has one level (parent), so a second trial on the same instance would be \
         counted as independent evidence when it is not"
    )]
    DuplicateTrial {
        capability: String,
        instance: String,
    },

    #[error("capability {capability} has no recorded trials")]
    UnknownCapability { capability: String },

    #[error("capability {capability} is not reportable: {shortfalls}")]
    CoverageFloorNotMet {
        capability: String,
        shortfalls: String,
    },

    #[error(
        "no candidate can reduce the outstanding coverage shortfall for capability {capability} \
         ({shortfalls}); the candidate registry cannot satisfy the floor and the panel must not \
         quietly proceed without it"
    )]
    CoverageUnsatisfiable {
        capability: String,
        shortfalls: String,
    },

    #[error(
        "nothing selectable: the candidate set is empty, or every candidate in it already carries \
         scored evidence for its capability"
    )]
    NoCandidates,

    #[error("bootstrap requires at least two parent clusters, got {0}")]
    BootstrapNeedsClusters(usize),

    #[error("bootstrap requires at least one draw")]
    BootstrapNeedsDraws,

    #[error("canonical serialization failed: {0}")]
    Canonical(String),
}
