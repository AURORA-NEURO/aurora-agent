//! The evaluation engine: scoring, attribution and the capability posterior.
//!
//! Implements blueprint section 07 (Evaluation Engine) and the scoring half of section 06
//! (Benchmark Compiler) — the part that consumes what the compiler produced rather than the part
//! that produces it.
//!
//! # What this crate is for
//!
//! Blueprint 00.01 says the output of PRISM is "a capability posterior rather than a single
//! score", and that a matched fork isolates *which component* explains a difference. Both claims
//! are easy to state and easy to quietly violate, so this crate is organised around making the
//! violations impossible to reach rather than merely discouraged:
//!
//! - [`ladder`] composes tiered evidence strongest-first, so a judge has no code path by which to
//!   raise a deterministic conclusion (07.01);
//! - [`score`] keeps outcome and justification on separate axes, so a right answer for the wrong
//!   reason is its own category rather than a pass or a fail (07.05);
//! - [`attribution`] refuses to attribute a difference to any component when more than one
//!   component varied (07.07);
//! - [`cluster`] reports an effective sample size beside every aggregate, so instances descended
//!   from one parent cannot be counted as independent evidence (07.06);
//! - [`posterior`] returns a capability vector, and its `overall()` is a `Result` that refuses
//!   unless declared coverage floors were met (07.05, 07.12).
//!
//! # What this crate does not do
//!
//! It does not *produce* judgements. The evidence ladder over oracle verdicts lives in
//! `bioprism-oracle`; running evaluators, sandboxing untrusted benchmark code, segmenting
//! trajectories and synthesising oracles all live elsewhere. This crate takes tiered
//! [`ladder::Contribution`]s as given and answers what may be concluded, attributed and published
//! from them.
//!
//! It also does not implement the parts of section 07 that need machinery this crate deliberately
//! has no access to: cost and latency accounting (07.08), calibration curves and reliability
//! diagrams (07.06), benchmark-health drift detection (07.11), and the human resolution workflow
//! that a disputed result should enter (07.01). Where those are load-bearing for an invariant here,
//! the invariant is stated and the measurement is left to the caller.

pub mod attribution;
pub mod bridge;
pub mod cluster;
pub mod error;
pub mod ladder;
pub mod posterior;
pub mod score;

pub use bridge::{contribution_from_verdict, digest, Provenance};
pub use attribution::{
    attribute, ArmSpec, Attribution, AttributionClaim, AttributionReport, ComponentEffect,
    EffectDirection, MatchedFork, RefusalReason,
};
pub use cluster::{ClusteredEstimate, ClusteredSample, IccEstimate};
pub use error::EvalError;
pub use ladder::{
    compose, Contribution, Detail, Disagreement, EvidenceRef, ScoreTier, ScoredResult,
    SuppressedRaise, UnknownPolicy,
};
pub use posterior::{
    unprovenanced, CapabilityEstimate, CapabilityPosterior, CoverageFloor, Dominance, GateScalar,
    Observation, ReleaseGate,
};
pub use score::{
    credit_for, Conclusion, Constraint, Credit, CreditBasis, CreditPolicy, Justification, Outcome,
    ResultScore, Rubric, RubricProgress, Satisfaction, UnknownCredit, Veto, VetoKind,
};

/// Schema version for everything this crate serializes.
///
/// Blueprint 07.01's invariant: "changes to schemas and scoring semantics are versioned and cannot
/// retroactively rewrite already published results." Consumers pin this and refuse to merge
/// reports across a bump rather than silently reinterpreting old ones.
pub const EVALENGINE_SCHEMA_VERSION: &str = "07.0.1";
