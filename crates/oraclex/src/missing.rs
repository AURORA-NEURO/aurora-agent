//! Typed absence (32.10) and the permission boundaries that produce some of it (32.19).
//!
//! 32.10's first operator pair is "not collected versus technically failed" and its first failure risk
//! is "zero imputation invents biology". Those are the same sentence twice: an absence with no type
//! becomes a number, and the number is indistinguishable from a measurement.
//!
//! [`Observed`] is the type. It has two variants and no third; it has no `unwrap_or`, no
//! `unwrap_or_default`, no `or_zero`, no `Default`, and no `From<Observed<T>> for T`. A caller who
//! wants a number out of an [`Observed::Absent`] has to write the imputation themselves, at a call
//! site that names the reason they are imputing. That is the entire mechanism, and it works because
//! the convenience method is the thing that gets used.
//!
//! # Access denial is an absence, not an error
//!
//! 32.19's operators include "controlled-access denial" and 32.10's include it too. Modelling it as an
//! `Err` invites a retry; modelling it as [`MissingReason::AccessDenied`] puts it in the same place as
//! a failed assay, which is where an analysis has to handle it. 32.19's failure risk "permission
//! failure is bypassed" is what a retry loop looks like from outside.
//!
//! # Missingness itself carries information
//!
//! 32.10's worked relation: "Methylation is missing at a resource-limited site but not elsewhere; a
//! model must avoid using missingness itself as a hidden site and outcome proxy."
//! [`informativeness`] takes per-group present/absent counts the caller measured and answers whether
//! the absence pattern separates the groups. It hardcodes no rate: perfect separation is a
//! deterministic contradiction of "missingness is uninformative", and anything short of that is
//! returned as unresolved naming the mechanism evidence that would settle it.
//!
//! # Not implemented
//!
//! No imputation, no sensitivity analysis, no differential privacy. 32.10's validation program asks
//! for "sensitivity analyses" and 32.19's for "differential privacy or aggregation checks"; both need
//! an estimator, and an estimator that this crate wrote would be the model under test.

use std::collections::BTreeMap;

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Unresolved, Witness};

/// Why a value is absent. Every variant is a different downstream obligation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum MissingReason {
    /// The measurement was never ordered.
    NotCollected,
    /// It was ordered and the assay failed. Informative about the specimen in a way
    /// [`MissingReason::NotCollected`] is not.
    TechnicallyFailed { detail: String },
    /// The assay ran and the analyte was under its limit. This is a measurement, and the strongest
    /// case against collapsing absences: below-detection is bounded information, not no information.
    BelowDetection { limit: String },
    /// A field was removed before the data left its source (32.19).
    Redacted { policy: String },
    /// The caller was not permitted to read it (32.10, 32.19).
    AccessDenied { policy: String },
    /// A cell was suppressed because too few subjects fell in it (32.19).
    SmallCellSuppressed { floor: u64 },
    /// The subject stopped being followed (32.10).
    LostToFollowUp,
}

impl MissingReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            MissingReason::NotCollected => "not_collected",
            MissingReason::TechnicallyFailed { .. } => "technically_failed",
            MissingReason::BelowDetection { .. } => "below_detection",
            MissingReason::Redacted { .. } => "redacted",
            MissingReason::AccessDenied { .. } => "access_denied",
            MissingReason::SmallCellSuppressed { .. } => "small_cell_suppressed",
            MissingReason::LostToFollowUp => "lost_to_follow_up",
        }
    }

    /// Whether the absence is itself a bound on the value.
    ///
    /// True only for [`MissingReason::BelowDetection`]. An analysis may use that bound; it may not
    /// use "not collected" as a bound, and the difference is one method call away from being lost.
    pub fn bounds_the_value(&self) -> bool {
        matches!(self, MissingReason::BelowDetection { .. })
    }

    /// Whether a permission decision, rather than the specimen, produced this absence.
    pub fn is_policy(&self) -> bool {
        matches!(
            self,
            MissingReason::Redacted { .. }
                | MissingReason::AccessDenied { .. }
                | MissingReason::SmallCellSuppressed { .. }
        )
    }
}

/// A value that may be absent, with the absence typed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "observed", rename_all = "snake_case")]
pub enum Observed<T> {
    Present { value: T },
    Absent { reason: MissingReason },
}

impl<T> Observed<T> {
    pub fn present(value: T) -> Self {
        Observed::Present { value }
    }

    pub fn absent(reason: MissingReason) -> Self {
        Observed::Absent { reason }
    }

    pub fn is_present(&self) -> bool {
        matches!(self, Observed::Present { .. })
    }

    /// The value, if there is one.
    ///
    /// Returning `Option<&T>` rather than `T` is the whole design. There is deliberately no
    /// `unwrap_or`: `Option` already supplies one, and a caller reaching for it has at least written
    /// the default at a visible call site rather than inheriting it from this type.
    pub fn value(&self) -> Option<&T> {
        match self {
            Observed::Present { value } => Some(value),
            Observed::Absent { .. } => None,
        }
    }

    pub fn reason(&self) -> Option<&MissingReason> {
        match self {
            Observed::Present { .. } => None,
            Observed::Absent { reason } => Some(reason),
        }
    }
}

/// Why values are missing, as declared by whoever built the dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mechanism", rename_all = "snake_case")]
pub enum MissingnessMechanism {
    /// Absence is unrelated to anything.
    Random,
    /// Absence depends on a variable that is present in the data and can be conditioned on.
    DependsOnObserved { on: String },
    /// Absence depends on something not in the data, including possibly the missing value itself.
    DependsOnUnobserved { suspected: String },
    /// Nobody said. The default state of most datasets, and modelled explicitly so an analysis has to
    /// notice it rather than assume the first variant.
    Undeclared,
}

/// Whether dropping incomplete records is defensible.
///
/// 32.10's failure risk is "complete-case analysis changes population silently". The silence is what
/// this refuses: an undeclared mechanism returns unresolved naming the declaration as the gap, and a
/// dependence on unobserved variables returns a contradiction, because conditioning cannot fix it.
pub fn complete_case_admissible(mechanism: &MissingnessMechanism) -> Determination {
    match mechanism {
        MissingnessMechanism::Random => Determination::supported(
            EvidenceTier::Statistical,
            "absence was declared unrelated to the analysis variables",
        ),
        MissingnessMechanism::DependsOnObserved { on } => Determination::unresolved(
            format!("an analysis conditioning on '{on}'"),
            "complete-case is admissible only after conditioning on the variable absence depends on",
        ),
        MissingnessMechanism::DependsOnUnobserved { suspected } => Determination::contradicted(
            EvidenceTier::Statistical,
            Witness::RelationViolated {
                relation: "complete-case analysis preserves the population".to_string(),
                expected: "absence independent of unobserved variables".to_string(),
                observed: format!("absence is suspected to depend on {suspected}"),
            },
        ),
        MissingnessMechanism::Undeclared => Determination::unresolved(
            "a declared missingness mechanism",
            "dropping incomplete records without one changes the population by an unknown amount",
        ),
    }
}

/// Per-group counts of present and absent values for one field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsencePattern {
    counts: BTreeMap<String, (u64, u64)>,
}

impl Default for AbsencePattern {
    fn default() -> Self {
        AbsencePattern::new()
    }
}

impl AbsencePattern {
    pub fn new() -> Self {
        AbsencePattern {
            counts: BTreeMap::new(),
        }
    }

    pub fn observe(mut self, group: impl Into<String>, present: u64, absent: u64) -> Self {
        self.counts.insert(group.into(), (present, absent));
        self
    }

    pub fn groups(&self) -> impl Iterator<Item = &str> {
        self.counts.keys().map(String::as_str)
    }
}

/// Whether the pattern of absence identifies the group.
///
/// Perfect separation — some group entirely absent while another is entirely present — means a model
/// can read the group off the missingness indicator alone, which is exactly 32.10's "hidden site and
/// outcome proxy". That is a deterministic fact about the table, so it is a contradiction. Partial
/// separation is not decidable from counts without a model, so it comes back unresolved naming what
/// would settle it rather than guessing a cutoff this crate has no basis for.
pub fn informativeness(pattern: &AbsencePattern) -> Determination {
    if pattern.counts.len() < 2 {
        return Determination::not_evaluable(
            "informativeness compares groups, and fewer than two were supplied",
        );
    }
    let fully_absent: Vec<&str> = pattern
        .counts
        .iter()
        .filter(|(_, (present, absent))| *present == 0 && *absent > 0)
        .map(|(group, _)| group.as_str())
        .collect();
    let fully_present: Vec<&str> = pattern
        .counts
        .iter()
        .filter(|(_, (present, absent))| *absent == 0 && *present > 0)
        .map(|(group, _)| group.as_str())
        .collect();

    if let (Some(absent_group), Some(present_group)) = (fully_absent.first(), fully_present.first())
    {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "missingness carries no group information".to_string(),
                expected: "absence rates overlap across groups".to_string(),
                observed: format!(
                    "'{absent_group}' is entirely absent and '{present_group}' entirely present, so the missingness indicator recovers the group"
                ),
            },
        );
    }

    let rates_differ = {
        let mut seen: Option<(u64, u64)> = None;
        pattern.counts.values().any(|(present, absent)| {
            let current = (*present, *absent);
            match seen {
                None => {
                    seen = Some(current);
                    false
                }
                Some((p, a)) => {
                    (current.1 as u128) * ((p + a) as u128)
                        != (a as u128) * ((current.0 + current.1) as u128)
                }
            }
        })
    };

    if rates_differ {
        Determination::unresolved(
            "a declared missingness mechanism, or an analysis that conditions on the group",
            "absence rates differ across groups, so the missingness indicator carries group information",
        )
    } else {
        Determination::supported(
            EvidenceTier::Statistical,
            "absence rates are identical across the supplied groups",
        )
    }
}

/// How sensitive one field is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Safe to move: a count, a rate, a model coefficient over enough subjects.
    Aggregate,
    /// Attributable to one subject.
    Individual,
}

/// A named place data may or may not cross (32.19: "federated site execution", "no-egress
/// validation").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Boundary {
    pub name: String,
    /// The strongest sensitivity permitted to cross.
    pub permits: Sensitivity,
}

impl Boundary {
    /// A boundary that lets only aggregates out — 32.19's worked relation, where "individual genomics
    /// remain behind a federated worker".
    pub fn aggregate_only(name: impl Into<String>) -> Self {
        Boundary {
            name: name.into(),
            permits: Sensitivity::Aggregate,
        }
    }
}

/// A field about to cross a boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub sensitivity: Sensitivity,
    /// The number of subjects behind this value. `None` for a value that is not a count over
    /// subjects; a count-based aggregate over too few subjects is individual data with a mean on it.
    pub subjects: Option<u64>,
}

impl Field {
    pub fn individual(name: impl Into<String>) -> Self {
        Field {
            name: name.into(),
            sensitivity: Sensitivity::Individual,
            subjects: None,
        }
    }

    pub fn aggregate(name: impl Into<String>, subjects: u64) -> Self {
        Field {
            name: name.into(),
            sensitivity: Sensitivity::Aggregate,
            subjects: Some(subjects),
        }
    }
}

/// Whether a field may cross a boundary, given a caller-supplied small-cell floor.
///
/// The floor is a parameter with no default. Small-cell suppression thresholds are set by data
/// custodians and vary by jurisdiction and study; a library-chosen number would be wrong somewhere
/// and invisible everywhere.
pub fn egress(field: &Field, boundary: &Boundary, small_cell_floor: u64) -> Determination {
    if field.sensitivity == Sensitivity::Individual && boundary.permits == Sensitivity::Aggregate {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::EgressViolation {
                field: field.name.clone(),
                boundary: boundary.name.clone(),
            },
        );
    }
    match field.subjects {
        Some(subjects) if subjects < small_cell_floor => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::EgressViolation {
                field: format!(
                    "{} (over {subjects} subjects, below the declared floor of {small_cell_floor})",
                    field.name
                ),
                boundary: boundary.name.clone(),
            },
        ),
        Some(_) => Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "{} is aggregate over at least {small_cell_floor} subjects",
                field.name
            ),
        ),
        None => Determination::Unresolved(Unresolved::of(
            format!("a subject count for '{}'", field.name),
            "an aggregate with no denominator cannot be checked against the small-cell floor",
        )),
    }
}
