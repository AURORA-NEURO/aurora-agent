//! Typed failures for the biological IR.
//!
//! Each variant names the offending subject — a specimen id, a lens id, a reviewer — because
//! the consumer of these errors is an adapter author staring at a ten-thousand-row extract.
//! "invalid lineage" is not actionable; "specimen `blk-1.s3` was drawn at 2026-03-04 from
//! `blk-1`, consumed at 2026-03-01" is.
//!
//! Findings that are *not* failures live next to the code that produces them:
//! [`crate::lineage::LineageIssue`], [`crate::cohort::LeakageFinding`] and
//! [`crate::lens::Incomparability`] are diagnostics returned in bulk, not `Err` values, and a
//! validation pass that stopped at the first one would hide the rest.

use crate::uncertainty::UncertaintyKind;
use bioprism_ids::CanonicalError;
use thiserror::Error;

/// Failures in specimen and aliquot lineage (25.04).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LineageError {
    #[error("specimen {specimen:?} is not in the lineage graph")]
    UnknownSpecimen { specimen: String },

    #[error("specimen {specimen:?} was inserted twice; 25.04 requires duplicate identifier detection")]
    DuplicateSpecimen { specimen: String },

    #[error("specimen {child:?} declares parent {parent:?}, which is not in the lineage graph")]
    UnknownParent { child: String, parent: String },

    #[error("lineage through specimen {specimen:?} is cyclic; 25.04 requires acyclic lineage")]
    Cycle { specimen: String },

    #[error("cannot combine quantities for {subject:?} across units {left:?} and {right:?}; this crate implements no unit conversion")]
    UnitMismatch {
        subject: String,
        left: String,
        right: String,
    },
}

/// Failures in cohort, eligibility and split definitions (25.13).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CohortError {
    #[error("observation {observation:?} is not in the candidate frame")]
    UnknownObservation { observation: String },

    #[error("observation {observation:?} appears twice in the candidate frame")]
    DuplicateObservation { observation: String },

    #[error("cohort {cohort:?} declares no eligibility rules; 25.13 requires rules to be executable and counted")]
    NoRules { cohort: String },

    #[error("cohort {cohort:?} analyses {cohort_unit} but its estimand targets {estimand_unit}")]
    EstimandUnitMismatch {
        cohort: String,
        cohort_unit: String,
        estimand_unit: String,
    },

    #[error("cohort {cohort:?} splits on {split_unit} but groups on {grouping}, so the split unit cannot honour the grouping")]
    SplitUnitFinerThanGrouping {
        cohort: String,
        split_unit: String,
        grouping: String,
    },

    #[error("rule {rule:?} is declared twice in cohort {cohort:?}")]
    DuplicateRule { cohort: String, rule: String },
}

/// Failures in AssayLens declarations and measurements (25.05).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LensError {
    #[error("lens {lens:?} is not in the catalog")]
    UnknownLens { lens: String },

    #[error("lens {lens:?} version {version:?} is registered twice")]
    DuplicateLens { lens: String, version: String },

    #[error("lens {lens:?} claims {claimed} identifiability but its calibration is {calibration}; 25.05 forbids claiming more identifiability than calibration supports")]
    UncalibratedAbsoluteClaim {
        lens: String,
        claimed: String,
        calibration: String,
    },

    #[error("lens {lens:?} reports a negative or below-threshold reading with no limit of detection; 25.05 requires negative results to include sensitivity context")]
    NegativeWithoutSensitivity { lens: String },

    #[error("lens {lens:?} requires {required} but specimen {specimen:?} has {available} left")]
    InsufficientMaterial {
        lens: String,
        specimen: String,
        required: String,
        available: String,
    },

    #[error("lens {lens:?} requires material {required:?} but specimen {specimen:?} is {found:?}")]
    WrongMaterial {
        lens: String,
        specimen: String,
        required: String,
        found: String,
    },

    #[error("QC metric {metric:?} on lens {lens:?} has an empty acceptance band")]
    EmptyQcBand { lens: String, metric: String },

    #[error(transparent)]
    Lineage(#[from] LineageError),

    #[error("lens {lens:?} could not be canonically serialised: {message}")]
    Canonical { lens: String, message: String },
}

impl LensError {
    pub(crate) fn canonical(lens: &str, source: CanonicalError) -> Self {
        LensError::Canonical {
            lens: lens.to_string(),
            message: source.to_string(),
        }
    }
}

/// Failures in uncertainty and reference-standard objects (25.12).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum UncertaintyError {
    #[error("categorical distribution for {subject:?} sums to {sum:?}, not 1")]
    UnnormalizedDistribution { subject: String, sum: String },

    #[error("probability {value:?} for label {label:?} in {subject:?} is outside [0, 1]")]
    ProbabilityOutOfRange {
        subject: String,
        label: String,
        value: String,
    },

    #[error("interval for {subject:?} runs from {lower:?} down to {upper:?}")]
    InvertedInterval {
        subject: String,
        lower: String,
        upper: String,
    },

    #[error("interval coverage {coverage:?} for {subject:?} is outside (0, 1]")]
    InvalidCoverage { subject: String, coverage: String },

    #[error("refusing to pool {left} uncertainty with {right} uncertainty; 25.12 forbids reducing uncertainty type to one generic confidence")]
    CrossKindPooling {
        left: UncertaintyKind,
        right: UncertaintyKind,
    },

    #[error("two {kind} components are written as {left} and {right}, which this crate does not know how to combine")]
    RepresentationsNotCombinable {
        kind: UncertaintyKind,
        left: String,
        right: String,
    },

    #[error("budget for {subject:?} declares {kind} uncertainty twice")]
    DuplicateKind { subject: String, kind: UncertaintyKind },

    #[error("decision {subject:?} requires {kind} uncertainty and the budget does not account for it")]
    UnaccountedKind { subject: String, kind: UncertaintyKind },

    #[error("adjudication by {adjudicator:?} drops reviewer {reviewer:?}; 25.12 requires expert disagreement to remain visible")]
    DissentErased {
        adjudicator: String,
        reviewer: String,
    },

    #[error("reviewer distribution for {subject:?} has no assessments")]
    NoReviewers { subject: String },

    #[error("calibration curve fitted in scope {curve_scope:?} does not cover query scope {query_scope:?}; 25.12 states calibration is contextual")]
    CalibrationOutOfContext {
        curve_scope: String,
        query_scope: String,
    },
}

/// Failures in biological evidence objects (25.11).
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum EvidenceError {
    #[error("evidence {evidence:?} is not in the ledger")]
    UnknownEvidence { evidence: String },

    #[error("evidence {evidence:?} is inserted twice")]
    DuplicateEvidence { evidence: String },

    #[error("evidence {evidence:?} derives from {ancestor:?}, which is not in the ledger; 25.11 requires derived evidence to preserve ancestors")]
    UnknownAncestor { evidence: String, ancestor: String },

    #[error("evidence {evidence:?} lists itself as its own ancestor")]
    SelfDerivation { evidence: String },

    #[error("evidence {evidence:?} drops access label {label:?} carried by ancestor {ancestor:?}")]
    AccessLabelDropped {
        evidence: String,
        ancestor: String,
        label: String,
    },

    #[error("locator for evidence {evidence:?} is not resolvable: {reason}")]
    UnresolvableLocator { evidence: String, reason: String },

    #[error("relation from {subject:?} to {object:?} names no asserter; 25.11 makes support and contradiction claims with provenance")]
    UnattributedRelation { subject: String, object: String },

    #[error("evidence {evidence:?} declares artifact hash {declared:?} but the bytes hash to {actual:?}")]
    ArtifactHashMismatch {
        evidence: String,
        declared: String,
        actual: String,
    },

    #[error("evidence {evidence:?} has an empty validity interval, so it is valid at no instant")]
    EmptyValidityInterval { evidence: String },

    #[error("evidence {evidence:?} could not be canonically serialised: {message}")]
    Canonical { evidence: String, message: String },
}

impl EvidenceError {
    pub(crate) fn canonical(evidence: &str, source: CanonicalError) -> Self {
        EvidenceError::Canonical {
            evidence: evidence.to_string(),
            message: source.to_string(),
        }
    }
}
