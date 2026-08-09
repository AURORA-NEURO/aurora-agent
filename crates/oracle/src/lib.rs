//! The biological oracle mesh.
//!
//! Implements blueprint section 31 (Biological Oracles and Reference Standards) and 40.21 (Oracle
//! Runtime and Mesh).
//!
//! # The problem
//!
//! 31.00 states it in one sentence: "Biological evaluation fails when a convenient label is
//! treated as the latent biological truth." The response is not a better label. It is a mesh of
//! *partially informative* evaluators whose assumptions, independence, scope, and failure modes
//! are visible, and a combination rule that refuses to average them into one.
//!
//! `crates/fiber/src/oracle.rs` already contains one oracle done right: a hardcoded
//! split-integrity checker that returns concrete leakage witnesses. This crate generalises it
//! without modifying it. [`Judgement::lift_verdict`] takes that oracle's
//! `bioprism_section::OracleVerdict` output and gives it the things it lacks — a rung on the
//! ladder, a version, a validity window, a declared scope — so it can stand beside other oracles
//! in a mesh.
//!
//! # The five things this crate is
//!
//! **1. A trait.** [`Oracle`] — [`Oracle::kind`], [`Oracle::tier`], [`Oracle::evaluate`], over an
//! [`OracleManifest`] that must state what it establishes and what it cannot (40.21 invariant 1).
//!
//! **2. A ladder.** [`EvidenceTier`]: `Deterministic > Execution > Property > Statistical >
//! Judge`. The 11.11 invariant is that **a nondeterministic judgement may never override a
//! deterministic or execution-grounded one** — 40.21 invariant 2, 31.01's failure containment,
//! 31.14's worked case. [`EvidenceTier::may_override`] is that rule, and it never reads
//! confidence.
//!
//! **3. Set-valued combination.** [`MeshPolicy::combine`] yields a *set* of acceptable positions,
//! not a winner. Two same-tier oracles that disagree yield both positions and
//! [`bioprism_section::OracleStatus::Underdetermined`] — never a majority, never an average.
//!
//! **4. Preserved disagreement.** [`Disagreement`] records who disagreed, at what tier, why, and
//! what would settle it. Adjudication requires a strictly stronger rung and never deletes the
//! position that lost (31.15's dissent-retention metric).
//!
//! **5. Expiry that speaks.** [`OracleManifest::admissibility`] turns 31.16's validity window and
//! supersession pointer into a per-evaluation answer. An expired oracle's judgement is retained on
//! [`CombinedVerdict::inadmissible`] carrying its reason, rather than being silently counted or
//! silently dropped.
//!
//! # Truth as a distribution (31.01)
//!
//! [`Judgement`] carries a [`Confidence`] and optionally a [`PositionDistribution`]. Neither buys
//! rank. A judge at 0.99 that contradicts a checksum produces a [`SuppressedOverride`] recording
//! the confidence it had and the rule that stopped it — the record exists so the refusal is
//! visible, not so the confidence is honoured.
//!
//! # What this crate does not do
//!
//! No isolation, no authorization, no execution. 40.21's execution path begins with "authorize
//! evaluator context" and its security section requires that evaluated agents never see holdouts
//! or hidden labels; 31.01 requires oracle code to run in an isolated grader environment. None of
//! that is here, because none of it can be provided by a library that runs in the caller's
//! process. It also reaches only three of the eight planes — artifact, analytical, policy — and
//! establishes nothing measurement-, biology-, causal-, longitudinal-, or translational-grade;
//! sections 31.04 through 31.13 need assays, cohorts, perturbations, and follow-up. Every shipped
//! manifest says so in its own `cannot_establish` set, which is the only form of that admission
//! a downstream consumer can act on.

pub mod combine;
pub mod disagreement;
pub mod error;
pub mod evidence;
pub mod judgement;
pub mod ladder;
pub mod manifest;
pub mod mesh;
pub mod oracle;
pub mod oracles;
pub mod plane;
pub mod time;

pub use combine::{
    CombinedVerdict, MeshPolicy, OracleFailure, OverrideRule, RetryClass, SuppressedOverride,
    VerdictBasis,
};
pub use disagreement::{
    Appeal, AppealGrounds, Disagreement, DisagreementSource, Resolution, Settlement,
};
pub use error::OracleError;
pub use evidence::Evidence;
pub use judgement::{
    Confidence, ConfidenceEnvelope, Finding, Judgement, Position, PositionDistribution,
};
pub use ladder::EvidenceTier;
pub use manifest::{
    Admissibility, Independence, OracleId, OracleManifest, OracleRef, OracleVersion,
    SharedResource, UncertaintyModel,
};
pub use mesh::OracleMesh;
pub use oracle::Oracle;
pub use oracles::{
    FieldType, MockJudgeOracle, NumericProperty, PropertyOracle, Recheck, ReexecutionOracle,
    SchemaOracle, WorldDocumentOracle,
};
pub use plane::{Determinism, Plane};
pub use time::{UtcTimestamp, ValidityWindow};
