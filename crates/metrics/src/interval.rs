//! Uncertainty that cannot be dropped on the way to a report.
//!
//! Blueprint 33.01: "Report confidence intervals and practical effect sizes, not only
//! significance", and "Confidence intervals cluster at the highest dependency level, usually parent
//! world, participant, site, or mutation family." 33.04 adds "interval coverage and width" to its
//! metric list. Section 33 says all of this and defines none of it, which is the gap this module
//! stands in — carefully, and without pretending to be a statistics library.
//!
//! # What this module is not
//!
//! **There is no estimator here.** No bootstrap, no Wilson interval, no cluster-robust variance.
//! An interval arrives from the caller with a stated method and a stated clustering unit, and this
//! module's job is to make sure it then travels with the number it qualifies. `bioprism-atlas`
//! declines intervals for the same reason and says so in its own docs: the estimator needs
//! per-trial outcomes retained past aggregation. Computing one from the aggregate counts an atlas
//! keeps would produce an interval clustered at the trial, which is precisely the inflated
//! precision 33.01's clustering rule exists to prevent.
//!
//! # The two type-level facts
//!
//! 1. **An [`IntervalEstimate`] cannot be built with an interval that excludes its own point.**
//!    Private fields, one gated constructor, and serde routed through
//!    [`IntervalEstimateFields`] so a JSON document cannot bypass the check — the same mechanism
//!    `bioprism_atlas::Measurement` uses against the empty denominator.
//! 2. **There is no conversion that drops an interval.** [`PointEstimate`] requires a
//!    [`NoIntervalReason`], and every variant of that enum *asserts that no interval existed*.
//!    There is no `NoIntervalReason::Dropped` and there will not be one: restating an interval
//!    estimate as a point estimate therefore requires the caller to write down a false statement,
//!    which is the strongest guarantee a type can offer against a lossy conversion whose input is a
//!    plain `f64`.
//!
//! # Interval arithmetic
//!
//! [`Interval::add`], [`Interval::scale`] and [`weighted_mean`] are ordinary interval arithmetic:
//! the result's width is derived from the inputs' widths and is never narrower than arithmetic
//! allows. Two refusals are built in, both because section 33 under-specifies rather than because
//! the arithmetic is hard: combining across confidence levels, and combining across clustering
//! units. 33.01 lists four candidate clustering units and gives no ordering over them, so there is
//! no defensible answer to "what unit does a parent-world interval plus a site interval cluster
//! at", and inventing one would be the kind of quiet assumption this crate exists to refuse.

use crate::error::MetricsError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The dependency level an interval clusters at — 33.01's list, closed.
///
/// Deliberately **not** `Ord`. The blueprint names these four without ranking them, and a derived
/// ordering would silently become a policy: "site is stronger than participant" is a claim about
/// study design that section 33 does not make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusteringUnit {
    /// The individual scored trial. Almost always wrong for a generated pack: 33.19's worked
    /// example is a million instances descending from three hundred parent worlds, and clustering
    /// at the trial reports a precision the evidence does not have.
    Trial,
    ParentWorld,
    Participant,
    Site,
    MutationFamily,
}

impl ClusteringUnit {
    pub const ALL: [ClusteringUnit; 5] = [
        ClusteringUnit::Trial,
        ClusteringUnit::ParentWorld,
        ClusteringUnit::Participant,
        ClusteringUnit::Site,
        ClusteringUnit::MutationFamily,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ClusteringUnit::Trial => "trial",
            ClusteringUnit::ParentWorld => "parent_world",
            ClusteringUnit::Participant => "participant",
            ClusteringUnit::Site => "site",
            ClusteringUnit::MutationFamily => "mutation_family",
        }
    }

    /// Whether the unit is one of the dependency levels 33.01 calls "highest".
    ///
    /// [`ClusteringUnit::Trial`] is the only one that is not, and it is in the enum precisely so a
    /// gate can refuse it by name rather than by its absence.
    pub fn is_dependency_level(self) -> bool {
        !matches!(self, ClusteringUnit::Trial)
    }
}

impl fmt::Display for ClusteringUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A nominal coverage probability, strictly inside `(0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct ConfidenceLevel(f64);

impl ConfidenceLevel {
    pub fn new(level: f64) -> Result<Self, MetricsError> {
        if !level.is_finite() || level <= 0.0 || level >= 1.0 {
            return Err(MetricsError::ConfidenceLevelOutOfRange(level));
        }
        Ok(ConfidenceLevel(level))
    }

    /// The conventional default, provided as a named constructor rather than a `Default` impl:
    /// a confidence level that appeared without anyone choosing it is not a stated one.
    pub fn ninety_five() -> Self {
        ConfidenceLevel(0.95)
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for ConfidenceLevel {
    type Error = MetricsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        ConfidenceLevel::new(value)
    }
}

impl From<ConfidenceLevel> for f64 {
    fn from(value: ConfidenceLevel) -> Self {
        value.0
    }
}

impl fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

/// Where an interval came from, and what it clusters at.
///
/// `method` is free text on purpose: this crate does not implement the methods and cannot validate
/// a name it does not know. What it can do is refuse to let the field be absent, so a reader always
/// learns whether they are looking at a bootstrap over parent worlds or a normal approximation over
/// trials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalBasis {
    pub method: String,
    pub clustering_unit: ClusteringUnit,
    /// Independent units behind the interval — the number `bioprism_scale::EffectiveSize` calls
    /// effective, not the instance count.
    pub effective_size: usize,
}

impl IntervalBasis {
    pub fn new(
        method: impl Into<String>,
        clustering_unit: ClusteringUnit,
        effective_size: usize,
    ) -> Self {
        IntervalBasis {
            method: method.into(),
            clustering_unit,
            effective_size,
        }
    }

    /// Checks the metadata that gives an interval its evidentiary meaning.  A zero effective size
    /// is not an independent sample, and an empty method leaves the interval's provenance
    /// unknowable; both are refused before an interval can carry them into a report.
    pub fn validate(&self) -> Result<(), MetricsError> {
        if self.method.trim().is_empty() {
            return Err(MetricsError::MalformedIntervalBasis {
                detail: "method must not be empty".to_string(),
            });
        }
        if self.effective_size == 0 {
            return Err(MetricsError::MalformedIntervalBasis {
                detail: "effective size must be greater than zero".to_string(),
            });
        }
        Ok(())
    }
}

impl fmt::Display for IntervalBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} clustered at {} over {} units",
            self.method, self.clustering_unit, self.effective_size
        )
    }
}

/// The wire form of an [`Interval`]. Public because it is the only route in — construction and
/// deserialization share one validating path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalFields {
    pub low: f64,
    pub high: f64,
    pub level: ConfidenceLevel,
    pub basis: IntervalBasis,
}

/// An interval, with the level and basis that make it mean something.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IntervalFields", into = "IntervalFields")]
pub struct Interval {
    low: f64,
    high: f64,
    level: ConfidenceLevel,
    basis: IntervalBasis,
}

impl TryFrom<IntervalFields> for Interval {
    type Error = MetricsError;

    fn try_from(fields: IntervalFields) -> Result<Self, Self::Error> {
        Interval::new(fields.low, fields.high, fields.level, fields.basis)
    }
}

impl From<Interval> for IntervalFields {
    fn from(interval: Interval) -> Self {
        IntervalFields {
            low: interval.low,
            high: interval.high,
            level: interval.level,
            basis: interval.basis,
        }
    }
}

impl Interval {
    pub fn new(
        low: f64,
        high: f64,
        level: ConfidenceLevel,
        basis: IntervalBasis,
    ) -> Result<Self, MetricsError> {
        basis.validate()?;
        if !low.is_finite() {
            return Err(MetricsError::NotFinite {
                subject: "interval lower bound",
                value: low,
            });
        }
        if !high.is_finite() {
            return Err(MetricsError::NotFinite {
                subject: "interval upper bound",
                value: high,
            });
        }
        if low > high {
            return Err(MetricsError::InvertedInterval { low, high });
        }
        Ok(Interval {
            low,
            high,
            level,
            basis,
        })
    }

    pub fn low(&self) -> f64 {
        self.low
    }

    pub fn high(&self) -> f64 {
        self.high
    }

    pub fn level(&self) -> ConfidenceLevel {
        self.level
    }

    pub fn basis(&self) -> &IntervalBasis {
        &self.basis
    }

    /// The number 33.04 asks for by name. Never derived on the fly by a consumer, because a
    /// consumer holding only `low` would have to reconstruct it.
    pub fn width(&self) -> f64 {
        self.high - self.low
    }

    pub fn midpoint(&self) -> f64 {
        (self.low + self.high) / 2.0
    }

    pub fn contains(&self, value: f64) -> bool {
        value >= self.low && value <= self.high
    }

    pub fn overlaps(&self, other: &Interval) -> bool {
        self.low <= other.high && other.low <= self.high
    }

    /// Whether the interval excludes a reference value entirely.
    ///
    /// This is the honest form of "significant": 33.01 asks for "practical effect sizes, not only
    /// significance", and an interval that excludes the null still says nothing about whether the
    /// difference matters. The method is named for what it checks.
    pub fn excludes(&self, value: f64) -> bool {
        !self.contains(value)
    }

    /// Interval sum. The result's width is the sum of the widths — uncertainty accumulates, and
    /// there is no arrangement of this arithmetic that makes it accumulate less.
    pub fn add(&self, other: &Interval) -> Result<Interval, MetricsError> {
        self.check_combinable(other)?;
        Interval::new(
            self.low + other.low,
            self.high + other.high,
            self.level,
            self.combined_basis(other),
        )
    }

    /// Interval difference, in the sense of `self - other`. Widths add here too: subtracting an
    /// uncertain quantity does not cancel uncertainty.
    pub fn sub(&self, other: &Interval) -> Result<Interval, MetricsError> {
        self.check_combinable(other)?;
        Interval::new(
            self.low - other.high,
            self.high - other.low,
            self.level,
            self.combined_basis(other),
        )
    }

    /// Multiplies by a non-negative constant. A negative factor would flip the bounds and, with
    /// them, the direction of the underlying scoring rule, so it is refused rather than reordered.
    pub fn scale(&self, factor: f64) -> Result<Interval, MetricsError> {
        if !factor.is_finite() || factor < 0.0 {
            return Err(MetricsError::NotFinite {
                subject: "interval scale factor",
                value: factor,
            });
        }
        Interval::new(
            self.low * factor,
            self.high * factor,
            self.level,
            self.basis.clone(),
        )
    }

    fn check_combinable(&self, other: &Interval) -> Result<(), MetricsError> {
        if self.level != other.level {
            return Err(MetricsError::MismatchedConfidenceLevel {
                left: self.level.to_string(),
                right: other.level.to_string(),
            });
        }
        if self.basis.clustering_unit != other.basis.clustering_unit {
            return Err(MetricsError::MismatchedClusteringUnit {
                left: self.basis.clustering_unit.as_str(),
                right: other.basis.clustering_unit.as_str(),
            });
        }
        Ok(())
    }

    /// The combined interval rests on the smaller of the two effective sizes: a combination is no
    /// better supported than its least-supported component.
    fn combined_basis(&self, other: &Interval) -> IntervalBasis {
        IntervalBasis {
            method: format!("{} combined with {}", self.basis.method, other.basis.method),
            clustering_unit: self.basis.clustering_unit,
            effective_size: self.basis.effective_size.min(other.basis.effective_size),
        }
    }
}

/// Weighted mean of intervals, carrying the width through.
///
/// Used by [`crate::aggregate`] when a grid's cells have intervals. Weights must be positive and
/// finite; the result's bounds are the weighted means of the bounds, which is the correct interval
/// arithmetic for a convex combination and is never narrower than the narrowest input.
pub fn weighted_mean(parts: &[(f64, Interval)]) -> Result<Interval, MetricsError> {
    let Some((_, first)) = parts.first() else {
        return Err(MetricsError::AggregateOverNothing {
            grid: "interval weighted mean".to_string(),
        });
    };
    let mut total = 0.0f64;
    let mut low = 0.0f64;
    let mut high = 0.0f64;
    let mut effective = usize::MAX;
    for (weight, interval) in parts {
        if !weight.is_finite() || *weight <= 0.0 {
            return Err(MetricsError::NotFinite {
                subject: "interval weight",
                value: *weight,
            });
        }
        first.check_combinable(interval)?;
        total += weight;
        low += weight * interval.low;
        high += weight * interval.high;
        effective = effective.min(interval.basis.effective_size);
    }
    Interval::new(
        low / total,
        high / total,
        first.level,
        IntervalBasis {
            method: format!("weighted mean of {} intervals", parts.len()),
            clustering_unit: first.basis.clustering_unit,
            effective_size: effective,
        },
    )
}

/// Why a number arrives without an interval.
///
/// Every variant asserts that **no interval existed**. None of them means "an interval existed and
/// was discarded", and adding such a variant would defeat the module: it is the absence of an
/// honest way to say "I dropped it" that stops an [`IntervalEstimate`] being laundered into a
/// [`PointEstimate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoIntervalReason {
    /// One evaluable trial. There is no dispersion to estimate, and a zero-width interval would
    /// claim certainty rather than report an absence.
    SingleTrial,
    /// The quantity is exact — a count, a version, a conformance check. An interval would be
    /// meaningless rather than missing.
    DeterministicQuantity,
    /// No estimator was available at the point the number was produced. This is the reason
    /// `bioprism-atlas` measurements carry, because that crate deliberately implements none.
    EstimatorNotAvailable,
    /// The dependency structure is unknown, so 33.01's clustering requirement cannot be satisfied
    /// and any interval would be clustered at the wrong level.
    ClusteringUnitUnknown,
    /// Policy withholds the dispersion — a small protected subgroup, per 33.01's "protect small
    /// groups". The estimate survives; the interval does not.
    WithheldToProtectSmallGroup,
}

impl NoIntervalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            NoIntervalReason::SingleTrial => "single_trial",
            NoIntervalReason::DeterministicQuantity => "deterministic_quantity",
            NoIntervalReason::EstimatorNotAvailable => "estimator_not_available",
            NoIntervalReason::ClusteringUnitUnknown => "clustering_unit_unknown",
            NoIntervalReason::WithheldToProtectSmallGroup => "withheld_to_protect_small_group",
        }
    }

    /// Whether the absence is a property of the quantity rather than of the evaluation.
    ///
    /// Only [`NoIntervalReason::DeterministicQuantity`] is: everything else is a gap that better
    /// evaluation could close, and a coverage report should treat it as debt.
    pub fn is_intrinsic(self) -> bool {
        matches!(self, NoIntervalReason::DeterministicQuantity)
    }
}

impl fmt::Display for NoIntervalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The wire form of an [`IntervalEstimate`], and the only route in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalEstimateFields {
    pub value: f64,
    pub interval: Interval,
}

/// A number and the interval it must never travel without.
///
/// Fields are private and the constructor validates containment, so
/// `IntervalEstimate { value: 0.9, interval: [0.1, 0.2] }` cannot exist. Serde routes through
/// [`IntervalEstimateFields`], so the `interval` key is mandatory in JSON as well as in Rust: there
/// is no serialized form of this type that omits it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IntervalEstimateFields", into = "IntervalEstimateFields")]
pub struct IntervalEstimate {
    value: f64,
    interval: Interval,
}

impl TryFrom<IntervalEstimateFields> for IntervalEstimate {
    type Error = MetricsError;

    fn try_from(fields: IntervalEstimateFields) -> Result<Self, Self::Error> {
        IntervalEstimate::new(fields.value, fields.interval)
    }
}

impl From<IntervalEstimate> for IntervalEstimateFields {
    fn from(estimate: IntervalEstimate) -> Self {
        IntervalEstimateFields {
            value: estimate.value,
            interval: estimate.interval,
        }
    }
}

impl IntervalEstimate {
    pub fn new(value: f64, interval: Interval) -> Result<Self, MetricsError> {
        if !value.is_finite() {
            return Err(MetricsError::NotFinite {
                subject: "point estimate",
                value,
            });
        }
        if !interval.contains(value) {
            return Err(MetricsError::IntervalExcludesEstimate {
                estimate: value,
                low: interval.low(),
                high: interval.high(),
            });
        }
        Ok(IntervalEstimate { value, interval })
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn interval(&self) -> &Interval {
        &self.interval
    }

    pub fn width(&self) -> f64 {
        self.interval.width()
    }
}

/// A number with a stated reason for having no interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PointEstimateFields", into = "PointEstimateFields")]
pub struct PointEstimate {
    value: f64,
    no_interval: NoIntervalReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct PointEstimateFields {
    value: f64,
    no_interval: NoIntervalReason,
}

impl TryFrom<PointEstimateFields> for PointEstimate {
    type Error = MetricsError;

    fn try_from(fields: PointEstimateFields) -> Result<Self, Self::Error> {
        PointEstimate::stated(fields.value, fields.no_interval)
    }
}

impl From<PointEstimate> for PointEstimateFields {
    fn from(estimate: PointEstimate) -> Self {
        PointEstimateFields {
            value: estimate.value,
            no_interval: estimate.no_interval,
        }
    }
}

impl PointEstimate {
    /// The only constructor, and it demands the reason.
    ///
    /// Named `stated` rather than `new` because that is what the caller is doing: stating, on the
    /// record, that no interval existed for this number.
    pub fn stated(value: f64, no_interval: NoIntervalReason) -> Result<Self, MetricsError> {
        if !value.is_finite() {
            return Err(MetricsError::NotFinite {
                subject: "point estimate",
                value,
            });
        }
        Ok(PointEstimate { value, no_interval })
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn no_interval(&self) -> NoIntervalReason {
        self.no_interval
    }
}

/// A metric value: with an interval, or with a reason there is none.
///
/// There is no third state. A number that has neither an interval nor an explanation for its
/// absence is not representable, which is the modest but complete guarantee this enum makes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "uncertainty", rename_all = "snake_case")]
pub enum Estimate {
    Interval { estimate: IntervalEstimate },
    Point { estimate: PointEstimate },
}

impl Estimate {
    pub fn with_interval(value: f64, interval: Interval) -> Result<Self, MetricsError> {
        Ok(Estimate::Interval {
            estimate: IntervalEstimate::new(value, interval)?,
        })
    }

    pub fn point(value: f64, no_interval: NoIntervalReason) -> Result<Self, MetricsError> {
        Ok(Estimate::Point {
            estimate: PointEstimate::stated(value, no_interval)?,
        })
    }

    pub fn value(&self) -> f64 {
        match self {
            Estimate::Interval { estimate } => estimate.value(),
            Estimate::Point { estimate } => estimate.value(),
        }
    }

    /// The interval, or `None`. A gate that needs one and finds `None` is *unevaluable*, not
    /// failed — see [`crate::gate`].
    pub fn interval(&self) -> Option<&Interval> {
        match self {
            Estimate::Interval { estimate } => Some(estimate.interval()),
            Estimate::Point { .. } => None,
        }
    }

    pub fn width(&self) -> Option<f64> {
        self.interval().map(Interval::width)
    }

    pub fn no_interval_reason(&self) -> Option<NoIntervalReason> {
        match self {
            Estimate::Point { estimate } => Some(estimate.no_interval()),
            Estimate::Interval { .. } => None,
        }
    }
}
