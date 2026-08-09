//! Concrete oracles, one per rung that this crate can honestly reach.
//!
//! | oracle | tier | blueprint | establishes |
//! |---|---|---|---|
//! | [`SchemaOracle`] | deterministic | 31.02 | artifact |
//! | [`WorldDocumentOracle`] | deterministic | 31.02 | artifact |
//! | [`ReexecutionOracle`] | execution | 31.03 | analytical |
//! | [`PropertyOracle`] | property | 31.03 | analytical |
//! | [`MockJudgeOracle`] | judge | 31.14 | policy |
//!
//! Nothing here reaches the `statistical` rung, and nothing establishes the measurement,
//! biological, causal, longitudinal, or translational planes. Those need assay performance data,
//! cohorts, perturbation experiments, and follow-up — 31.04 through 31.13 — none of which a pure
//! library can supply. Every manifest below therefore calls
//! [`OracleManifest::disclaiming_the_rest`](crate::OracleManifest::disclaiming_the_rest), so a
//! verdict built from these five reports plainly that it establishes nothing biological.
//!
//! All five are deliberately buildable from a literal: they take their checks as data so a test
//! can construct an oracle that contradicts another and watch the ladder decide. That is the whole
//! reason [`MockJudgeOracle`] exists.

mod judge;
mod property;
mod reexecution;
mod schema;
mod world_document;

pub use judge::MockJudgeOracle;
pub use property::{NumericProperty, PropertyOracle};
pub use reexecution::{Recheck, ReexecutionOracle};
pub use schema::{FieldType, SchemaOracle};
pub use world_document::WorldDocumentOracle;
