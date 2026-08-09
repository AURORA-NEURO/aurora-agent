//! Observation status: the six non-collapsing absence states.
//!
//! This is the one closed enumeration the section states verbatim, and it is repeated
//! byte-identically in every module of 30. Blueprint 30.02:
//!
//! > Missing, not collected, technically failed, below detection, not applicable, and redacted
//! > are separate states.
//!
//! The rule has teeth in 30.10, which names *"assuming absent call means wild type"* as a
//! characteristic failure, and in 30.02, which requires that the absence of an event be
//! distinguished from missing documentation. A plain `Option<T>` erases exactly the
//! distinction that makes those two failures detectable, so this crate never uses `Option` to
//! mean "not observed" in evidence-carrying positions.

use serde::{Deserialize, Serialize};

/// Why a value is not present.
///
/// These are *not* orderable by severity and are not a scale. `BelowDetection` is a real
/// measurement outcome and carries information; `NotCollected` carries none. Treating them as
/// interchangeable is what produces spurious wild-type calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    /// The value should exist and does not. A defect in the record.
    Missing,
    /// The assay or study was never performed. Not a defect; simply no evidence.
    NotCollected,
    /// The assay ran and failed quality control. Evidence was attempted and is unusable.
    TechnicallyFailed,
    /// The assay ran successfully and the analyte was under the limit of detection.
    ///
    /// This is the state most often mistaken for a negative call. It bounds the value from
    /// above; it does not assert absence, because sensitivity is assay-specific (30.10 names
    /// ignoring assay sensitivity as a failure).
    BelowDetection,
    /// The value is undefined for this subject or specimen, not merely absent.
    NotApplicable,
    /// The value exists and was withheld under access policy (30.30 release gate).
    Redacted,
}

impl ObservationStatus {
    /// Whether this status carries evidence bearing on the value.
    ///
    /// Only [`ObservationStatus::BelowDetection`] does: the assay worked. Everything else is
    /// silence, and silence is not a negative result.
    pub const fn is_informative_about_the_value(self) -> bool {
        matches!(self, ObservationStatus::BelowDetection)
    }

    pub const fn describe(self) -> &'static str {
        match self {
            ObservationStatus::Missing => "missing from the record",
            ObservationStatus::NotCollected => "never collected",
            ObservationStatus::TechnicallyFailed => "assay failed quality control",
            ObservationStatus::BelowDetection => "below the assay limit of detection",
            ObservationStatus::NotApplicable => "not applicable to this specimen",
            ObservationStatus::Redacted => "redacted under access policy",
        }
    }
}

/// A value that may not have been observed, carrying *why* when it was not.
///
/// Blueprint 30.02 and 30.10. Deliberately offers no `unwrap_or_default`, no `Default`, and no
/// conversion to `bool`: every one of those would let a caller silently turn "never collected"
/// into a negative call in a single character of code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Observed<T> {
    Value(T),
    Unobserved(ObservationStatus),
}

impl<T> Observed<T> {
    pub const fn value(&self) -> Option<&T> {
        match self {
            Observed::Value(value) => Some(value),
            Observed::Unobserved(_) => None,
        }
    }

    pub const fn status(&self) -> Option<ObservationStatus> {
        match self {
            Observed::Value(_) => None,
            Observed::Unobserved(status) => Some(*status),
        }
    }

    pub const fn is_observed(&self) -> bool {
        matches!(self, Observed::Value(_))
    }
}
