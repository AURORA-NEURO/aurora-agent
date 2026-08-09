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
//! No OpenTelemetry adapter — the MVP cut line asks for one, and it needs an OTel dependency this
//! offline workspace does not carry. No model-assisted segmentation: Gate 2 permits it but requires
//! human approval regardless, and a transparent arithmetic ranker is auditable in a way a model is
//! not. No state minimization; that already exists in `bioprism_prism::minimize`.
//!
//! The ranking heuristic is *not* validated. Gate 0 asks whether experts agree with the located
//! decision on real trajectories, and no such study has been run here. The scores order candidates
//! for review; they do not establish that the top candidate is the right one.

pub mod compile;
pub mod divergence;
pub mod error;
pub mod event;
pub mod ingest;
pub mod segment;

pub use compile::{Approved, CellProposal};
pub use divergence::{first_divergence, is_actionable, Divergence};
pub use error::TraceError;
pub use event::{Event, EventKind, Trace};
pub use ingest::{from_jsonl, validate, ImportLoss, Ingestion};
pub use segment::{excluded, review_reduction, segment, Candidate, CandidateScore};
