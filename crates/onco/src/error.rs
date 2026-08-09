//! Typed failures for OncoWorld.
//!
//! Blueprint 30.30 requires that a refusal be legible. A caller told "no" must be able to tell
//! three very different situations apart, because only one of them is fixed by collecting more
//! evidence:
//!
//! * a **boundary refusal** — the request was for individualized clinical direction, and no
//!   amount of evidence will make this platform answer it;
//! * a **data defect** — clocks disagree, a measurement is not a number, a scope names a cohort
//!   where a single subject was required;
//! * an **unmet obligation** — the evidence is well formed but does not yet license the claim
//!   that was asked for.
//!
//! Collapsing these into one stringly-typed failure would let a caller retry a boundary refusal
//! as though it were a transient parse error, which is the overreach 30.30 exists to prevent.

use crate::boundary::OutputUse;
use bioprism_scope::Timestamp;
use thiserror::Error;

/// Every way an OncoWorld operation can refuse.
///
/// Deliberately `Clone` and `PartialEq` so that tests assert on the *shape* of a refusal rather
/// than on its rendered message, which is documentation and may be reworded.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum OncoError {
    /// Blueprint 30.02, four time axes.
    ///
    /// Event validity, recording, release and agent visibility are ordered: a fact cannot be
    /// recorded before it happened, released before it was recorded, or visible to an agent
    /// before it was released. Half of all clock mix-ups at the source trip this immediately.
    #[error(
        "timepoint {timepoint:?} violates clock order: {later_axis} {later} precedes \
         {earlier_axis} {earlier}; the four time axes of 30.02 are ordered and are not \
         interchangeable"
    )]
    ClockOrderViolation {
        timepoint: String,
        earlier_axis: &'static str,
        earlier: Timestamp,
        later_axis: &'static str,
        later: Timestamp,
    },

    /// Blueprint 30.02. Labels index a worldline, so they must be unique within it.
    #[error("timepoint label {0:?} already exists on this worldline")]
    DuplicateTimepoint(String),

    /// Blueprint 30.02. A worldline is anchored to exactly one baseline.
    #[error("worldline references baseline {0:?}, which is not among its timepoints")]
    UnknownBaseline(String),

    /// Blueprint 30.02. Identifiers must be usable as study pseudonyms.
    #[error("subject reference is empty or contains control characters")]
    MalformedSubjectRef,

    /// Blueprint 30.02. A worldline belongs to one subject.
    ///
    /// A scope that binds `subject` to a set, to an interval, or not at all describes a cohort.
    /// Picking a member of that set would fabricate a worldline that no one observed.
    #[error(
        "scope does not identify a single subject on dimension {dimension:?}: {found}; a tumour \
         worldline is per-subject and cannot be built from a cohort scope"
    )]
    SubjectNotSingular {
        dimension: &'static str,
        found: String,
    },

    /// Blueprint 30.06. Measurements feed ratios, so they must be finite and non-negative.
    #[error("measurement {field} must be finite and non-negative, got {value}")]
    InvalidMeasurement { field: &'static str, value: f64 },

    /// Blueprint 30.02. Karnofsky is a decile scale, not an arbitrary percentage.
    #[error("Karnofsky performance status must be a multiple of 10 in 0..=100, got {0}")]
    InvalidKarnofsky(u8),

    /// Blueprint 30.26. Follow-up time must be non-negative on the event-validity clock.
    #[error("last contact {last_contact} precedes risk-set entry {entry}")]
    FollowUpEndsBeforeItStarts {
        entry: Timestamp,
        last_contact: Timestamp,
    },

    /// Blueprint 30.26, left truncation.
    ///
    /// Risk-set entry may follow the index date (delayed entry), but it may never precede it:
    /// that would credit the subject with at-risk time before the clock started.
    #[error("risk-set entry {entry} precedes the index date {index}; delayed entry is allowed, negative entry is not")]
    RiskSetEntryBeforeIndex { index: Timestamp, entry: Timestamp },

    /// Blueprint 30.26 with 30.07.
    ///
    /// An outcome event may only be recorded from a *confirmed* progression. A not-evaluable or
    /// stable assessment is not an event and does not become one by being written down.
    #[error(
        "response call {call} is not a confirmed progression and cannot be recorded as an outcome event"
    )]
    ResponseCallIsNotProgression { call: &'static str },

    /// Blueprint 30.30. The research boundary refused the requested use.
    #[error(
        "output use {attempted:?} is individualized clinical use and lies outside the research-only \
         boundary; permitted uses are {permitted:?}. This platform does not diagnose a person, \
         prognosticate for an individual, recommend treatment, or triage care"
    )]
    OutsideResearchBoundary {
        attempted: OutputUse,
        permitted: Vec<OutputUse>,
    },

    /// Blueprint 30.30, release gate.
    ///
    /// "Controlled or identifiable data never enter public trace bundles." A request that
    /// carries direct identifiers is refused before any analysis runs, so that the refusal
    /// cannot be reported alongside an echo of the identifiers.
    #[error(
        "request carries {count} direct identifier field(s) and cannot be processed; \
         controlled or identifiable data never enter research outputs"
    )]
    IdentifiersPresent { count: usize },

    /// Blueprint 30.02. A fact's payload did not decode into a timepoint document.
    #[error("fact {fact} does not carry a well-formed timepoint document: {message}")]
    MalformedObservation { fact: String, message: String },

    /// Blueprint 30.02. Ingest requires exactly one baseline-tagged fact per subject.
    #[error("expected exactly one fact tagged {tag:?} to anchor the worldline baseline, found {found}")]
    BaselineTagAmbiguous { tag: &'static str, found: usize },

    /// Blueprint 30.02. Facts from different subjects cannot share a worldline.
    #[error("facts describe more than one subject: {first:?} and {second:?}")]
    MixedSubjects { first: String, second: String },
}
