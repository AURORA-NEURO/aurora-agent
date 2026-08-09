//! Structured failures.
//!
//! Blueprint 32.01 types every transformation by what it is allowed to change. A stress that
//! cannot be applied to a cohort must say *which precondition of the cohort it needed*, because
//! the caller's next move — repair the cohort, or drop the family from the program — depends on
//! the answer. A boolean `false` forces the caller to guess.

use serde::{Deserialize, Serialize};

/// Everything this crate can refuse to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum StressError {
    #[error("cohort {cohort} is empty")]
    EmptyCohort { cohort: String },

    #[error("cohort {cohort} repeats subject id {subject}")]
    DuplicateSubject { cohort: String, subject: String },

    #[error("subject {subject} has non-finite {field}")]
    NonFinite { subject: String, field: String },

    #[error("subject {subject} has non-positive weight {weight}")]
    NonPositiveWeight { subject: String, weight: String },

    #[error("subject {subject} has non-positive volume {volume} mm3")]
    NonPositiveVolume { subject: String, volume: String },

    #[error("cohort {cohort} has no {class} subjects, so class-conditional statistics are undefined")]
    ClassAbsent { cohort: String, class: String },

    #[error("cohort {cohort} has no batch named {batch}")]
    BatchAbsent { cohort: String, batch: String },

    #[error("target prevalence {target} is outside the open interval (0, 1)")]
    PrevalenceOutOfRange { target: String },

    #[error(
        "target prevalence {target} cannot be reached by reweighting a cohort whose observed \
         prevalence is {observed}: reweighting cannot create a class that is not present"
    )]
    PrevalenceUnreachable { target: String, observed: String },

    #[error("reproducibility coefficient of variation {cv} must lie in [0, 1)")]
    ReproducibilityOutOfRange { cv: String },

    #[error("standard-deviation multiplier {multiplier} must be at least 1.0: a stress widens uncertainty, it never narrows it")]
    NarrowingMultiplier { multiplier: String },

    #[error("cohort {cohort} could not be canonicalised for content addressing: {detail}")]
    NotAddressable { cohort: String, detail: String },

    #[error("magnitude {permille} permille exceeds the full declared stress, which is 1000")]
    MagnitudeOutOfRange { permille: u32 },

    #[error("procedure {procedure} has no value on cohort {cohort}: {reason}")]
    ConclusionUndefined {
        procedure: String,
        cohort: String,
        reason: String,
    },
}
