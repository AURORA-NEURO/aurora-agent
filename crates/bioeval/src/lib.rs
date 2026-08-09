//! The biological evaluation engine.
//!
//! Implements the load-bearing half of blueprint section 26 (Biological Evaluation Engine) on top
//! of 31.01 (truth as a distribution, not a label). Section 26 is twenty-four modules of
//! evaluation design averaging 57 substantive lines each, of which 28 are a template repeated
//! verbatim across the section — the identical "Diagnostic outputs", "Required baselines",
//! "Statistical analysis" and "Release gates" blocks. That half is a process requirement for
//! whoever runs a study, not behaviour a library can hold. What a library can hold is the part
//! that makes *biological* evaluation different from software evaluation, and that part reduces to
//! two claims.
//!
//! **The reference standard is itself uncertain.** A unit test's expected value is the truth. A
//! pathology panel's majority read is not. 31.01: reference truth is "distributions, intervals,
//! sets, and partial orders whenever the biology or measurement does not justify one categorical
//! answer", and its worked case says a calibrated `0.55/0.35/0.10` forecast "can be more correct
//! than an unqualified categorical label". Everything in [`reference`] and [`score`] follows from
//! taking that literally: agreement with a reference that is 60% confident yields a *band*, not a
//! number, and the only way to a scalar is [`score::BioScore::collapse`] under a named policy that
//! can refuse.
//!
//! **Being wrong has structure.** A failing assertion is a failing assertion; a wrong molecular
//! subtype, a flipped laterality, a milligram-for-microgram slip and a right-direction-wrong-
//! magnitude estimate are four different events with four different consequences, and a benchmark
//! that reports one accuracy figure has told an engineer nothing about which one happened.
//! [`wrongness`] classifies them with a severity fixed by the taxonomy rather than by the grader,
//! and [`layer`] records *where* in a pipeline the first failure sat, so that a specimen swap and
//! a reasoning error stop looking identical when they produce the same wrong sentence.
//!
//! # The four refusals
//!
//! Each is a place where the engine returns a typed error instead of a plausible float.
//!
//! | Refusal | Module | Because |
//! |---|---|---|
//! | No scalar without a discharge policy | [`score`] | A number that has forgotten the reference was 60% confident is worse than no number. |
//! | No score across incomparable measurements | [`comparability`] | GRCh37 against GRCh38 is a different locus; the comparison is an artefact of the mismatch. |
//! | No partial credit without a stated rule | [`credit`] | Grader discretion is the surface a system optimises against (26.14). |
//! | No consensus over a safety-reaching dissent | [`aggregate`] | One reader calling the wrong side of a body is not 11% wrong. |
//!
//! # What this crate is not
//!
//! It does not schedule evaluations (26.21), detect contamination (26.13), localise first causal
//! divergence (26.22), or compute Pareto frontiers over the outcome vector (26.20) — those need a
//! decision trace and a registry that live elsewhere in the workspace. It has no ontology, so
//! 31.01's "distance on taxonomic or causal graphs" is unavailable and a near-miss subtype must be
//! expressed as reference mass on both states rather than as a graph distance. It knows one proper
//! scoring rule. Each omission is named again at the module that would have owned it.

pub mod aggregate;
pub mod comparability;
pub mod credit;
pub mod error;
pub mod layer;
pub mod reference;
pub mod score;
pub mod wrongness;

pub use aggregate::{ConsensusPolicy, ConsensusState, PanelAggregate, PooledScore, Rating, Veto};
pub use comparability::{
    gate, Bridge, ComparabilityRequirement, ComparabilityWitness, FrameDimension, FrameSide,
    Incomparability, MeasurementFrame,
};
pub use credit::{Credit, CreditEvidence, CreditRule, CreditTerm};
pub use error::{
    AggregationError, CollapseError, CreditError, PredictionError, ReferenceError, ScoreError,
};
pub use layer::{
    ClassifiedError, Conclusion, CorrectnessLayer, FailureSignature, LayerVerdict, LayeredOutcome,
};
pub use reference::{Dispersion, ReferenceDistribution, ReferenceStandard, Resolution};
pub use score::{
    BioScore, CollapsePolicy, Grade, Grader, PredictedDistribution, Prediction, ReferenceDischarge,
    ScoreInterval, ScoringRule,
};
pub use wrongness::{BiologicalErrorClass, Severity};
