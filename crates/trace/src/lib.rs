//! Trajectory to Decision Cell.
//!
//! Implements blueprint section 06 (the benchmark compiler) and critical-path Gate 2. This is the
//! wedge the executive summary describes:
//!
//! > Given a failed or suspicious agent trajectory, identify the earliest decision worth testing,
//! > turn it into a reproducible microbenchmark, and compare two agent configurations from that
//! > exact state.
//!
//! `bioprism-prism` could already freeze a Decision Cell and fork architectures from it, but a cell
//! had to be authored by hand. This crate is the missing half: ingest a trajectory, locate where it
//! went wrong, and propose a cell for review.
//!
//! ## The pipeline
//!
//! ```text
//! ingest    JSONL -> Trace, with a mandatory loss report
//!    |      a trace that dropped events is not compilable, and says so
//! diverge   failing x passing -> the earliest step they stopped agreeing
//!    |      compared on content digests, so one insertion reports as one insertion
//! segment   Trace -> ranked candidates, with every score component shown
//!    |      only Choice and Action steps can host a cell
//! compile   candidate -> CellProposal -> approve(reviewer) -> DecisionCell
//! ```
//!
//! ## What is deliberately not here
//!
//! The OpenTelemetry adapter is JSON-only and deterministic: it maps recorded OTLP spans into this
//! Event IR and carries a semantic-loss report, but it does not export, contact a collector, or
//! claim support for every vendor convention. No model-assisted segmentation: Gate 2 permits it
//! but requires human approval regardless, and a transparent arithmetic ranker is auditable in a
//! way a model is not. No state minimization; that already exists in `bioprism_prism::minimize`.
//!
//! The ranking heuristic is *not* validated. Gate 0 asks whether experts agree with the located
//! decision on real trajectories, and no such study has been run here. The scores order candidates
//! for review; they do not establish that the top candidate is the right one.
//!
//! Blueprint module 04.02 is discharged here by [`otel`]: a bounded OTLP JSON importer with
//! explicit semantic-loss accounting. It is an importer only; export, collector transport, and
//! vendor-specific convention ownership remain outside this crate.

pub mod compile;
pub mod divergence;
pub mod error;
pub mod event;
pub mod ingest;
pub mod otel;
pub mod segment;

pub use compile::{Approved, CellProposal};
pub use divergence::{first_divergence, is_actionable, Divergence};
pub use error::TraceError;
pub use event::{Event, EventKind, Trace};
pub use ingest::{from_jsonl, validate, ImportLoss, Ingestion};
pub use otel::{from_otlp_json, OtelError, OtelIngestion, OtelLoss, OtelMapping, MAX_SPANS};
pub use segment::{excluded, review_reduction, segment, Candidate, CandidateScore};
