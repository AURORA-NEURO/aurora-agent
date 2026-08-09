//! Benchmark packs: what a pack *is*, what the portfolio covers, and when a pack must not be
//! turned into a number.
//!
//! Implements blueprint 03.06 (Benchmark Pack IR), 15.00 (pack portfolio and taxonomy),
//! 15.01–15.25 (the agent and platform packs), 15.19 (benchmark meta-evaluation) and 29.00–29.21
//! (the biological capability taxonomy and its packs).
//!
//! A pack is the unit PRISM ships, cites and eventually retires. This crate fixes its shape and
//! encodes the portfolio; it does not build one. Instance generation is `worldgen` and `mutation`
//! work — a pack here references parents by id and generated instances by seed range, and never
//! materializes either. Trust tiers and release gating belong to `registry`; what is here is the
//! evidence a tiering decision consumes, not the decision.
//!
//! The part worth arguing about is [`health`]. Section 15.19 exists because a benchmark can fail
//! on its own terms while every run against it succeeds mechanically: everyone passes, or a
//! trivial heuristic passes, or the answers were in the training data. A saturated pack published
//! as "94% pass" is worse than publishing nothing, because the 94% moves with sampling noise and
//! reads as capability. So health is not a report attached to a score — it is the gate in front of
//! one. [`health::PackAssessment::reportable_score`] is the only way to obtain a number from a
//! pack, and it returns [`PackError::UnreportablePack`] rather than a score with a footnote. Every
//! assessment carries the digest of the pack revision it assessed, so a finding cannot outlive the
//! pack it was about.
//!
//! [`calibration`] answers the adjacent question: given some observed pass rates, does the pack
//! separate anything? A pack where everything passes and a pack where everything fails
//! discriminate equally little, and both are reported as their own verdict rather than as a range
//! of width zero.
//!
//! [`coverage`] maps packs onto the capability families they evidence and reports the complement.
//! The gaps are the output: families with one pack, families whose packs can only be judged by
//! human review, and — when the healthy subset is passed in — families the current release cannot
//! measure at all.
//!
//! What this crate does not decide: whether a pack is any good. Oracle *tier* is an upper bound on
//! what could be decided about an instance, never a measurement of how reliably it was; a
//! `Deterministic` tier on a pack with the wrong predicate is worse than a rubric. Detecting that
//! requires executing the pack against seeded defects, which is 15.19's own pack and the runtime's
//! job.

pub mod calibration;
pub mod coverage;
pub mod error;
pub mod health;
pub mod ir;
pub mod portfolio;
pub mod taxonomy;

pub use calibration::{
    CalibrationPolicy, DifficultyCalibration, Discrimination, SystemObservation,
};
pub use coverage::{coverage, matrix, portfolio_coverage, CoverageReport, CoverageRow, MatrixCell};
pub use error::PackError;
pub use health::{
    assess, ContaminationSignal, HealthFinding, HealthPolicy, HealthVerdict, Observations,
    PackAssessment, PackHealth, ReportedScore, Severity, TrivialBaseline,
};
pub use ir::{
    InstanceSource, PackContent, PackDependency, PackId, PackIr, PackManifest, PackVersion,
    ParentEnvironment, PublicCounts, SchemaRange, SeedRange,
};
pub use portfolio::PackDefinition;
pub use taxonomy::{
    AgentCapability, BioCapability, CapabilityFamily, Domain, OracleTier, PackAxis, ReleaseWave,
};

/// Re-exported because they appear in this crate's public types: a pack addresses itself with a
/// [`ContentHash`] and names its parents with [`WorldId`].
pub use bioprism_ids::{ContentHash, WorldId};
