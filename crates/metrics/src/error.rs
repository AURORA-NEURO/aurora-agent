//! Typed metric errors, and the separate vocabulary for a refused comparison.
//!
//! Two enums, because they answer different questions. [`MetricsError`] means *the caller asked
//! for something that cannot be built* — an aggregate over nothing, an inverted interval, a
//! weighting over a capability nobody measured. [`ScoreIncomparability`] means *the question was
//! well formed and the answer is no*: these two numbers were produced under conditions that make
//! comparing them undefined rather than merely noisy.
//!
//! The split matters because the second is a result a report should print, not an exception a
//! caller should unwrap. `bioprism-standards` draws the same line between `StandardsError` and
//! `Incomparability`, and this module deliberately mirrors its shape so that a reader who has seen
//! one recognises the other.
//!
//! There is no `Other(String)` variant in either enum, for the reason `bioprism-atlas` gives: a
//! condition the crate cannot name is a gap in the enum, and the gap should surface as a
//! non-exhaustive match at compile time rather than as free text at runtime.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Something the caller asked for that cannot be constructed.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MetricsError {
    /// The load-bearing refusal of this crate, and the sibling of
    /// [`bioprism_atlas::AtlasError::EmptyDenominator`]. An aggregate needs at least one cell that
    /// actually contributed; the mean of no measurements is not zero, it does not exist.
    #[error("refusing to aggregate over {grid}: no cell contributed a measurement")]
    AggregateOverNothing { grid: String },

    /// 33.01 requires an aggregation whose coverage is stated. A deserialized aggregate whose
    /// contributing and unmeasured lists do not account for every cell of the grid it claims to
    /// summarise is not a coverage statement, it is a number with decoration.
    #[error(
        "aggregate over {grid} claims {cells} cells but accounts for {contributed} contributing \
         and {unmeasured} unmeasured"
    )]
    CoverageAccountingMismatch {
        grid: String,
        cells: usize,
        contributed: usize,
        unmeasured: usize,
    },

    /// The complete-grid rule was requested and the grid is not complete. Naming the holes is the
    /// whole value of the refusal.
    #[error("refusing a complete-grid aggregate over {grid}: {} cells are unmeasured ({})", .unmeasured.len(), .unmeasured.join(", "))]
    IncompleteGrid {
        grid: String,
        unmeasured: Vec<String>,
    },

    #[error("{subject} is not a finite number: {value}")]
    NotFinite { subject: &'static str, value: f64 },

    #[error("interval [{low}, {high}] is inverted")]
    InvertedInterval { low: f64, high: f64 },

    /// An interval that does not contain its own point estimate describes a different quantity
    /// from the one it is attached to.
    #[error("estimate {estimate} lies outside its own interval [{low}, {high}]")]
    IntervalExcludesEstimate { estimate: f64, low: f64, high: f64 },

    #[error("confidence level {0} is not strictly between 0 and 1")]
    ConfidenceLevelOutOfRange(f64),

    /// Interval arithmetic across confidence levels has no defined meaning: a 95% interval and a
    /// 50% interval combine into an interval at no stated level at all.
    #[error("cannot combine a {left} interval with a {right} interval")]
    MismatchedConfidenceLevel { left: String, right: String },

    /// 33.01 lists parent world, participant, site and mutation family as clustering units and
    /// gives no ordering over them. Combining intervals clustered at different units would require
    /// an ordering this crate refuses to invent.
    #[error("cannot combine an interval clustered at {left} with one clustered at {right}")]
    MismatchedClusteringUnit {
        left: &'static str,
        right: &'static str,
    },

    #[error("weighting policy is malformed: {detail}")]
    MalformedWeighting { detail: String },

    /// A weighting that names a capability the grid never measured cannot be applied. There is no
    /// path that scores the hole as zero; that is the entire reason this returns `Result`.
    #[error("weighting names {capability}, which {system} leaves unmeasured ({reason})")]
    WeightedCapabilityUnmeasured {
        system: String,
        capability: String,
        reason: String,
    },

    #[error("weighting names {capability}, which is absent from the grid of {system}")]
    WeightedCapabilityAbsent { system: String, capability: String },

    #[error("ranking needs at least two systems, got {0}")]
    RankingTooSmall(usize),

    #[error("system {0} appears twice in one ranking")]
    DuplicateSystem(String),

    /// Two capability vectors defined over different capability sets are not two readings of one
    /// grid. Ranking them would compare a subset against a superset.
    #[error("{left} and {right} are defined over different capability sets: {detail}")]
    DisjointCapabilitySets {
        left: String,
        right: String,
        detail: String,
    },

    #[error("failed to encode {subject} for hashing: {detail}")]
    Encoding { subject: String, detail: String },
}

/// Which side of a comparison failed to record a condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnrecordedSide {
    Left,
    Right,
    /// Both sides are silent. This still blocks. Silence does not match silence: two measurements
    /// that each failed to record their pack version are not thereby known to share one.
    Both,
}

impl UnrecordedSide {
    pub fn as_str(self) -> &'static str {
        match self {
            UnrecordedSide::Left => "left",
            UnrecordedSide::Right => "right",
            UnrecordedSide::Both => "both",
        }
    }
}

/// Why two scores may not be compared as they stand.
///
/// Returned as a value, never thrown: "these are incomparable" is a finding, and a report that
/// prints it has said something true. Mirrors `bioprism_standards::Incomparability`, including its
/// first-blocking-reason discipline — see [`crate::comparability::CHECK_ORDER`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "blocked_on", rename_all = "snake_case")]
pub enum ScoreIncomparability {
    /// Different questions. A verification score and a calibration score are not two readings of
    /// one quantity however similar their ranges.
    #[error("subjects differ: {left} versus {right}")]
    DifferentSubject { left: String, right: String },

    /// 33.01's release gate: "Direction and units are unambiguous." Two numbers produced by
    /// different scoring rules are not on one scale, and a rule whose direction differs inverts
    /// the meaning of "higher".
    #[error("scoring rules differ: {left} versus {right}")]
    DifferentScoringRule { left: String, right: String },

    /// 03.09 versions the capability hierarchy, so the same identifier can denote different
    /// capabilities across versions. `bioprism-atlas` refuses to reproject; so does this.
    #[error("ontology versions differ: {left} versus {right}")]
    DifferentOntologyVersion { left: String, right: String },

    #[error("pack versions differ: {left} versus {right}")]
    DifferentPackVersion { left: String, right: String },

    #[error("evidence bases differ: {left} versus {right}")]
    DifferentEvidenceBase { left: String, right: String },

    /// A score adjudicated by a model judge and one adjudicated by a deterministic oracle are not
    /// the same measurement, which is why 33.01 gates on "a model judge cannot override
    /// deterministic failures".
    #[error("oracle floors differ: {left} versus {right}")]
    DifferentOracleFloor { left: String, right: String },

    #[error("resource budgets differ: {left} versus {right}")]
    DifferentBudget { left: String, right: String },

    /// One coordinate of 33.01's stratification key disagrees.
    #[error("stratification dimension {dimension} differs: {left} versus {right}")]
    DifferentStratum {
        dimension: String,
        left: String,
        right: String,
    },

    /// The dimension was never recorded, so equality cannot be established. This is the variant
    /// that stops an unlabelled comparison from passing by default.
    #[error("{dimension} is unrecorded on {} side", .side.as_str())]
    ConditionUnrecorded {
        dimension: String,
        side: UnrecordedSide,
    },
}

impl ScoreIncomparability {
    /// The dimension that blocked, as a stable string for reports and gates.
    pub fn dimension(&self) -> String {
        match self {
            ScoreIncomparability::DifferentSubject { .. } => "subject".to_string(),
            ScoreIncomparability::DifferentScoringRule { .. } => "scoring rule".to_string(),
            ScoreIncomparability::DifferentOntologyVersion { .. } => "ontology version".to_string(),
            ScoreIncomparability::DifferentPackVersion { .. } => "pack version".to_string(),
            ScoreIncomparability::DifferentEvidenceBase { .. } => "evidence base".to_string(),
            ScoreIncomparability::DifferentOracleFloor { .. } => "oracle floor".to_string(),
            ScoreIncomparability::DifferentBudget { .. } => "budget".to_string(),
            ScoreIncomparability::DifferentStratum { dimension, .. } => dimension.clone(),
            ScoreIncomparability::ConditionUnrecorded { dimension, .. } => dimension.clone(),
        }
    }

    /// Whether the block is an absence of information rather than a disagreement.
    ///
    /// A gate needs the distinction: "these were measured under different packs" is a finding
    /// about the systems, "nobody wrote down which pack" is a finding about the evaluation.
    pub fn is_absence(&self) -> bool {
        matches!(self, ScoreIncomparability::ConditionUnrecorded { .. })
    }
}
