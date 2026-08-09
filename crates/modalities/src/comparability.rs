//! Comparability with the modality dimension added.
//!
//! `bioprism-standards` already decides whether two measurements agree on units, coordinate frame,
//! reference build and ontology binding, and returns the first blocking dimension. This module
//! does not reimplement any of that. It adds the one question the standards layer has no view on:
//! whether the two numbers are numbers **about the same kind of thing**.
//!
//! The case that motivates the module is two measurements "of the same gene", one by RNA-seq and
//! one by mass spectrometry. They can agree on every dimension `bioprism-standards` checks — same
//! ontology term at the same release, same specimen, dimensionless units — and still not be two
//! estimates of one quantity. 28.06 lists treating one as the other among its characteristic
//! failure modes, and [`comparable_across`] refuses with the blueprint's own words for it.
//!
//! # Order of the checks
//!
//! [`CHECK_ORDER`] is measurand, then reported axis, then the status of that axis, then everything
//! `bioprism-standards` checks. The modality dimensions come first because they subsume the
//! others: if the two values are of different quantities, telling a caller their units differ
//! would send them to write a conversion factor for a conversion that does not exist. The same
//! argument the standards crate makes for checking frame before magnitude, one level up.
//!
//! # What agreement here does and does not mean
//!
//! `Ok(())` means the two measurements may be placed side by side without a category error. It is
//! not a statement that they agree, that either is correct, or that a difference between them is
//! biological. Nothing in this module reads a value.

use crate::catalog::substitution_failure_mode;
use crate::descriptor::{Measurand, Modality, ModalityDescriptor, Resolution, ResolutionStatus};
use crate::error::{CrossModalIncomparability, ModalityError};
use bioprism_ids::ContentHash;
use bioprism_standards::{
    comparable_under, report as standards_report, ComparabilityPolicy, ComparabilityReport,
    Measurement,
};
use serde::{Deserialize, Serialize};

/// The dimensions checked, in the order they are checked.
///
/// The last entry defers to [`bioprism_standards::CHECK_ORDER`] rather than repeating it, which is
/// the same relationship the code has: this crate owns the first three and delegates the rest.
pub const CHECK_ORDER: &[&str] = &[
    "measurand",
    "reported resolution axis",
    "status of that axis",
    "everything in bioprism_standards::CHECK_ORDER",
];

/// A measurement together with the modality that produced it and the axis it is reported at.
///
/// The `reported_at` axis is not decoration. A single-cell experiment can report per-cell values
/// or a pseudobulk average, and those are different objects even though the experiment was the
/// same; carrying the axis is what lets a comparison notice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModalMeasurement {
    pub descriptor: ModalityDescriptor,
    pub reported_at: Resolution,
    pub measurement: Measurement,
}

impl ModalMeasurement {
    pub fn new(
        descriptor: ModalityDescriptor,
        reported_at: Resolution,
        measurement: Measurement,
    ) -> Self {
        ModalMeasurement {
            descriptor,
            reported_at,
            measurement,
        }
    }

    pub fn modality(&self) -> Modality {
        self.descriptor.modality
    }

    pub fn measurand(&self) -> Measurand {
        self.descriptor.measurand
    }

    pub fn axis_status(&self) -> ResolutionStatus {
        self.descriptor.resolution(self.reported_at)
    }
}

/// Whether two measurements from different modalities may be compared as they stand.
///
/// "As they stand" carries the same weight it does in [`bioprism_standards::comparable`]: two
/// lengths in different units are blocked pending a recorded conversion, and two values at
/// different resolution axes are blocked pending a recorded aggregation. Both are acts, and both
/// must leave a trace.
pub fn comparable_across(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
) -> Result<(), CrossModalIncomparability> {
    comparable_across_under(left, right, ComparabilityPolicy::default())
}

/// [`comparable_across`] with an explicit standards policy.
pub fn comparable_across_under(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
    policy: ComparabilityPolicy,
) -> Result<(), CrossModalIncomparability> {
    check_measurands(left, right)?;
    check_axes(left, right)?;
    check_axis_status(left, right)?;
    comparable_under(&left.measurement, &right.measurement, policy)?;
    Ok(())
}

fn check_measurands(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
) -> Result<(), CrossModalIncomparability> {
    if left.measurand() == right.measurand() {
        return Ok(());
    }
    let note = substitution_failure_mode(left.measurand(), right.measurand())
        .or_else(|| substitution_failure_mode(right.measurand(), left.measurand()))
        .map(|mode| format!("{} names this — {}: {}", mode.module, mode.label, mode.statement))
        .unwrap_or_else(|| {
            "two measurements of different quantities are not two estimates of one".to_string()
        });
    Err(CrossModalIncomparability::MeasurandMismatch {
        left: left.modality(),
        right: right.modality(),
        left_measurand: left.measurand(),
        right_measurand: right.measurand(),
        note,
    })
}

fn check_axes(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
) -> Result<(), CrossModalIncomparability> {
    if left.reported_at == right.reported_at {
        return Ok(());
    }
    Err(CrossModalIncomparability::ResolutionMismatch {
        left: left.modality(),
        right: right.modality(),
        left_axis: left.reported_at.to_string(),
        right_axis: right.reported_at.to_string(),
    })
}

fn check_axis_status(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
) -> Result<(), CrossModalIncomparability> {
    for side in [left, right] {
        match side.axis_status() {
            ResolutionStatus::Undeclared => {
                return Err(CrossModalIncomparability::UndeclaredAxis {
                    side: side.modality(),
                    axis: side.reported_at,
                })
            }
            ResolutionStatus::Unresolved => {
                return Err(CrossModalIncomparability::UnreportableAxis {
                    side: side.modality(),
                    axis: side.reported_at,
                })
            }
            _ => {}
        }
    }
    match (left.axis_status(), right.axis_status()) {
        (ResolutionStatus::Imputed { source, by }, ResolutionStatus::Resolved) => {
            Err(CrossModalIncomparability::ImputedAgainstMeasured {
                side: left.modality(),
                axis: left.reported_at,
                imputed_by: format!("{by} from {source}"),
            })
        }
        (ResolutionStatus::Resolved, ResolutionStatus::Imputed { source, by }) => {
            Err(CrossModalIncomparability::ImputedAgainstMeasured {
                side: right.modality(),
                axis: right.reported_at,
                imputed_by: format!("{by} from {source}"),
            })
        }
        _ => Ok(()),
    }
}

/// A cross-modal comparison and everything that had to be true or assumed for it.
///
/// Wraps rather than replaces [`bioprism_standards::ComparabilityReport`], so a reader can see
/// which layer said what. The caveats are the reason the type exists: a verdict of comparable
/// reached between two estimates, or between two compositional readouts, is a different object
/// from one reached between two direct measurements, and a receipt that hid the difference would
/// be the silent coercion 28.00 forbids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossModalReport {
    pub left: Modality,
    pub right: Modality,
    pub verdict: CrossModalVerdict,
    /// The standards layer's own report, present when the modality checks passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standards: Option<ComparabilityReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CrossModalVerdict {
    Comparable,
    Blocked {
        reason: CrossModalIncomparability,
    },
}

impl CrossModalVerdict {
    pub fn is_comparable(&self) -> bool {
        matches!(self, CrossModalVerdict::Comparable)
    }
}

/// Builds a full report, delegating to the standards layer once the modality checks pass.
pub fn report(
    left: &ModalMeasurement,
    right: &ModalMeasurement,
    policy: ComparabilityPolicy,
) -> CrossModalReport {
    let mut caveats = Vec::new();
    if left.measurand().is_compositional() && right.measurand().is_compositional() {
        caveats.push(
            "both sides are compositional: a component can move because it changed or because \
             something else did"
                .to_string(),
        );
    }
    for side in [left, right] {
        if let ResolutionStatus::Imputed { source, by } = side.axis_status() {
            caveats.push(format!(
                "{} reports at {} via {by} from {source}, which is an estimate",
                side.modality(),
                side.reported_at
            ));
        }
    }

    match comparable_across_under(left, right, policy) {
        Ok(()) => CrossModalReport {
            left: left.modality(),
            right: right.modality(),
            verdict: CrossModalVerdict::Comparable,
            standards: Some(standards_report(&left.measurement, &right.measurement, policy)),
            caveats,
        },
        Err(CrossModalIncomparability::Standards(_)) => {
            let standards = standards_report(&left.measurement, &right.measurement, policy);
            let reason = match &standards.verdict {
                bioprism_standards::Verdict::Blocked { reason } => {
                    CrossModalIncomparability::Standards(reason.clone())
                }
                bioprism_standards::Verdict::Comparable => {
                    unreachable!("the standards layer blocked and then did not")
                }
            };
            CrossModalReport {
                left: left.modality(),
                right: right.modality(),
                verdict: CrossModalVerdict::Blocked { reason },
                standards: Some(standards),
                caveats,
            }
        }
        Err(reason) => CrossModalReport {
            left: left.modality(),
            right: right.modality(),
            verdict: CrossModalVerdict::Blocked { reason },
            standards: None,
            caveats,
        },
    }
}

impl CrossModalReport {
    /// A content hash of the report, so a downstream artefact can cite this verdict.
    pub fn digest(&self) -> Result<ContentHash, ModalityError> {
        let value = serde_json::to_value(self).map_err(|error| ModalityError::Encoding {
            subject: "cross-modal comparability report".to_string(),
            detail: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error| ModalityError::Encoding {
            subject: "cross-modal comparability report".to_string(),
            detail: error.to_string(),
        })
    }
}
