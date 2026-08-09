//! Aggregation that carries its own coverage, because the two cannot be separated.
//!
//! This module answers the one question a capability atlas invites and must not answer carelessly:
//! *what may a single number over a grid of measurements claim?*
//!
//! Blueprint 33.01 sets the terms — "A composite score is publishable only when the intended use
//! declares a weighting policy, every mandatory safety gate passes, coverage is sufficient, and
//! uncertainty remains visible" — and ADR-006 makes the general case a decision rather than a
//! preference: "no universal 'agent IQ' will be canonical", with pack-specific aggregates allowed
//! "when their construction is transparent".
//!
//! # The mechanism, copied from `bioprism-scale`
//!
//! `bioprism-scale` faced the same problem for instance counts and solved it structurally:
//! `NominalCount` implements neither `Serialize` nor `Deserialize`, so a raw count reaches a report
//! only inside `EffectiveSize`, which carries the inflation ratio beside it. The equivalent here is
//! [`BareScore`]: it is what [`CoveredAggregate::value`] returns, it holds one `f64`, and it
//! implements no serialization at all. There is no way to put an aggregate into a document except
//! as a [`CoveredAggregate`], and a `CoveredAggregate` cannot exist — in memory or in JSON —
//! without the list of cells that contributed and the list of cells that did not.
//!
//! Concretely, the guarantees are:
//!
//! - **No bare-score constructor.** Every path to a [`CoveredAggregate`] walks a
//!   [`CapabilityGrid`] and derives the coverage in the same pass as the value, so the two cannot
//!   describe different populations.
//! - **Deserialization revalidates.** [`CoveredAggregateFields`] is the only route in, and it
//!   recomputes the coverage fractions rather than trusting them, so a hand-written document
//!   cannot claim 100% coverage over a grid with holes.
//! - **An empty contribution refuses.** [`MetricsError::AggregateOverNothing`] is the sibling of
//!   `bioprism_atlas::AtlasError::EmptyDenominator`: the mean of no measurements is not zero.
//! - **Uncertainty survives.** The aggregate's own [`Estimate`] is an interval when every
//!   contributing cell had one at a common level and clustering unit, and otherwise a point
//!   estimate with a stated [`NoIntervalReason`]. There is no arm that quietly loses the interval.
//!
//! # The rule that does not exist
//!
//! There is no `AggregationRule::UnmeasuredAsZero`, and no argument that enables it. That is the
//! entire subject of the crate: an unmeasured capability rendered as a zero is indistinguishable
//! on a chart from a capability that was measured and failed, and the reader draws a conclusion
//! about the system when the truth is a conclusion about the evaluation.
//!
//! # Not implemented
//!
//! No statistical pooling — a random-effects meta-analysis over cells would need per-trial data and
//! a variance model, and section 33 specifies neither. No calibration, no regret, no cost curves:
//! §33's nineteen modules name 222 metrics and dimensions and define none of them, so this crate
//! implements the *arithmetic that governs any of them* rather than guessing at particular
//! estimators.

use crate::conditions::MeasurementConditions;
use crate::error::MetricsError;
use crate::grid::CapabilityGrid;
use crate::interval::{weighted_mean, Estimate, Interval, NoIntervalReason};
use crate::weighting::DeclaredWeighting;
use bioprism_atlas::{CapabilityId, UnmeasuredReason};
use serde::{Deserialize, Serialize};

/// An aggregate value with nothing attached.
///
/// Implements neither `Serialize` nor `Deserialize`, and that omission is the point: a caller may
/// compute with this number, but there is no way to write it into a document without going back
/// through the [`CoveredAggregate`] it came from, which brings its coverage along. The pattern is
/// `bioprism_scale::corpus::NominalCount`'s, for the same reason.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[must_use = "a bare score may be computed with but not reported; report the CoveredAggregate"]
pub struct BareScore(f64);

impl BareScore {
    pub fn get(self) -> f64 {
        self.0
    }
}

/// One hole, as it appears in an aggregate's coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnmeasuredCell {
    pub capability: CapabilityId,
    pub reason: UnmeasuredReason,
}

/// Which cells contributed, which did not, and what that leaves the aggregate covering.
///
/// The fractions are derived, never supplied: [`CoveredAggregateFields`] recomputes them on the way
/// in. Two of them, because the honest answer depends on a question the caller must be forced to
/// notice — a grid can be fully covered *within its declared scope* while covering half the
/// capabilities in the ontology.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    /// Every capability in the grid, contributing or not.
    pub cells: usize,
    /// The cells whose measurements are actually inside the number.
    pub contributed: Vec<CapabilityId>,
    /// Holes that block: nobody looked, or looking failed. These are why the aggregate is not an
    /// aggregate over the grid.
    pub blocking_holes: Vec<UnmeasuredCell>,
    /// Holes the declared intended use closes, in the sense of
    /// `bioprism_atlas::UnmeasuredReason::supports_claim`. Counted separately because they are the
    /// only ones that do not reduce coverage.
    pub holes_closed_by_declaration: Vec<UnmeasuredCell>,
    /// Cells that were measured and that the rule did not use — a weighting that names four of
    /// eleven capabilities excludes seven measurements. Kept apart from the holes because "we did
    /// not weight it" and "nobody measured it" are different statements about the same blank space
    /// on a chart.
    pub measured_but_excluded: Vec<CapabilityId>,
    /// Contributing cells over all cells. The number a reader compares against the ontology.
    pub fraction_of_grid: f64,
    /// Contributing cells over cells that were in scope at all.
    pub fraction_of_scope: f64,
}

impl Coverage {
    /// Whether every in-scope cell contributed. The only state in which "an aggregate over the
    /// grid" is a true description of the number.
    pub fn is_complete(&self) -> bool {
        self.blocking_holes.is_empty()
    }

    /// The sentence a report leads with, in the shape of
    /// `bioprism_scale::EffectiveSize::headline`.
    pub fn headline(&self) -> String {
        format!(
            "{} of {} cells contributed ({:.2} of the grid, {:.2} of declared scope); {} blocking \
             holes, {} closed by declaration, {} measured but excluded. An aggregate over a grid \
             with an unmeasured cell is not an aggregate over the grid.",
            self.contributed.len(),
            self.cells,
            self.fraction_of_grid,
            self.fraction_of_scope,
            self.blocking_holes.len(),
            self.holes_closed_by_declaration.len(),
            self.measured_but_excluded.len()
        )
    }

    fn derive(
        cells: usize,
        contributed: Vec<CapabilityId>,
        blocking_holes: Vec<UnmeasuredCell>,
        holes_closed_by_declaration: Vec<UnmeasuredCell>,
        measured_but_excluded: Vec<CapabilityId>,
    ) -> Result<Self, MetricsError> {
        let accounted = contributed.len()
            + blocking_holes.len()
            + holes_closed_by_declaration.len()
            + measured_but_excluded.len();
        if accounted != cells {
            return Err(MetricsError::CoverageAccountingMismatch {
                grid: "coverage".to_string(),
                cells,
                contributed: contributed.len(),
                unmeasured: accounted - contributed.len(),
            });
        }
        let in_scope = contributed.len() + blocking_holes.len();
        Ok(Coverage {
            fraction_of_grid: fraction(contributed.len(), cells),
            fraction_of_scope: fraction(contributed.len(), in_scope),
            cells,
            contributed,
            blocking_holes,
            holes_closed_by_declaration,
            measured_but_excluded,
        })
    }
}

/// How the contributing cells were combined.
///
/// Four rules, and the absence of a fifth. Every one of them is a *stated* handling of holes, which
/// is why the rule travels inside the aggregate: "mean of 0.8" and "mean of 0.8 over the four cells
/// out of eleven that were measured" are different claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum AggregationRule {
    /// Unweighted mean over measured cells. Coverage records what was left out.
    MeanOverMeasured,
    /// Weighted mean under a predeclared policy. Refuses if any weighted capability is a hole,
    /// which is 33.01's composite gate restated at the arithmetic level.
    WeightedMean { intended_use: String },
    /// The worst measured cell. 33.01: "Show distribution and worst-domain behavior, not only the
    /// mean." Under a lower-is-better scoring rule this is the maximum, not the minimum.
    WorstMeasured,
    /// Mean over a grid with no blocking holes. Refuses otherwise, naming them.
    CompleteMean,
}

impl AggregationRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            AggregationRule::MeanOverMeasured => "mean_over_measured",
            AggregationRule::WeightedMean { .. } => "weighted_mean",
            AggregationRule::WorstMeasured => "worst_measured",
            AggregationRule::CompleteMean => "complete_mean",
        }
    }
}

/// The worst cell in the grid, under the grid's own direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorstCell {
    pub capability: CapabilityId,
    pub value: f64,
}

/// The wire form of a [`CoveredAggregate`]. Public because it is the only route in: construction
/// and deserialization share one validating path, exactly as `bioprism_atlas::MeasurementFields`
/// does for the empty-denominator refusal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoveredAggregateFields {
    pub grid: String,
    pub rule: AggregationRule,
    pub conditions: MeasurementConditions,
    pub estimate: Estimate,
    pub coverage: Coverage,
    /// The smallest effective size among contributing cells. An aggregate is no better supported
    /// than its least-supported component, so this is a minimum and never a sum.
    pub effective_size: usize,
    pub worst: WorstCell,
    /// Range across contributing cells. A mean of 0.7 over `[0.69, 0.71]` and a mean of 0.7 over
    /// `[0.1, 0.99]` are different findings and 33.01 asks for both to be visible.
    pub spread: f64,
    /// The applied weighting's digest, when there was one. Present so the arbitrary step is
    /// citable rather than described.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weighting_digest: Option<String>,
}

/// An aggregate over a capability grid, inseparable from its coverage.
///
/// Fields are private and serde routes through [`CoveredAggregateFields`], which re-derives the
/// coverage fractions. A document claiming a number over a grid it did not cover does not
/// deserialize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CoveredAggregateFields", into = "CoveredAggregateFields")]
pub struct CoveredAggregate {
    grid: String,
    rule: AggregationRule,
    conditions: MeasurementConditions,
    estimate: Estimate,
    coverage: Coverage,
    effective_size: usize,
    worst: WorstCell,
    spread: f64,
    weighting_digest: Option<String>,
}

impl TryFrom<CoveredAggregateFields> for CoveredAggregate {
    type Error = MetricsError;

    fn try_from(fields: CoveredAggregateFields) -> Result<Self, Self::Error> {
        if fields.coverage.contributed.is_empty() {
            return Err(MetricsError::AggregateOverNothing { grid: fields.grid });
        }
        let coverage = Coverage::derive(
            fields.coverage.cells,
            fields.coverage.contributed,
            fields.coverage.blocking_holes,
            fields.coverage.holes_closed_by_declaration,
            fields.coverage.measured_but_excluded,
        )
        .map_err(|error| match error {
            MetricsError::CoverageAccountingMismatch {
                cells,
                contributed,
                unmeasured,
                ..
            } => MetricsError::CoverageAccountingMismatch {
                grid: fields.grid.clone(),
                cells,
                contributed,
                unmeasured,
            },
            other => other,
        })?;
        Ok(CoveredAggregate {
            grid: fields.grid,
            rule: fields.rule,
            conditions: fields.conditions,
            estimate: fields.estimate,
            coverage,
            effective_size: fields.effective_size,
            worst: fields.worst,
            spread: fields.spread,
            weighting_digest: fields.weighting_digest,
        })
    }
}

impl From<CoveredAggregate> for CoveredAggregateFields {
    fn from(value: CoveredAggregate) -> Self {
        CoveredAggregateFields {
            grid: value.grid,
            rule: value.rule,
            conditions: value.conditions,
            estimate: value.estimate,
            coverage: value.coverage,
            effective_size: value.effective_size,
            worst: value.worst,
            spread: value.spread,
            weighting_digest: value.weighting_digest,
        }
    }
}

impl CoveredAggregate {
    /// Unweighted mean over every measured cell.
    pub fn mean(grid: &CapabilityGrid) -> Result<Self, MetricsError> {
        let weights = uniform_weights(grid)?;
        combine(
            grid,
            AggregationRule::MeanOverMeasured,
            &weights,
            Reduction::Mean,
            None,
        )
    }

    /// Mean over a grid whose every in-scope cell is measured.
    ///
    /// Refuses otherwise and names the holes. This is the only rule under which "an aggregate over
    /// the grid" is a literally true description of the result.
    pub fn complete_mean(grid: &CapabilityGrid) -> Result<Self, MetricsError> {
        let blocking: Vec<String> = grid
            .holes()
            .filter(|(_, reason)| !reason.supports_claim())
            .map(|(capability, _)| capability.to_string())
            .collect();
        if !blocking.is_empty() {
            return Err(MetricsError::IncompleteGrid {
                grid: grid.label.clone(),
                unmeasured: blocking,
            });
        }
        let weights = uniform_weights(grid)?;
        combine(
            grid,
            AggregationRule::CompleteMean,
            &weights,
            Reduction::Mean,
            None,
        )
    }

    /// Weighted mean under a predeclared policy.
    ///
    /// Refuses when a weighted capability is absent or unmeasured. There is no fallback that
    /// renormalises the surviving weights: silently dropping a weighted capability changes the
    /// intended use the policy declared, which makes the result a different composite wearing the
    /// original's name.
    pub fn weighted(
        grid: &CapabilityGrid,
        weighting: &DeclaredWeighting,
    ) -> Result<Self, MetricsError> {
        let mut weights = Vec::new();
        for (capability, weight) in weighting.weights() {
            let Some(cell) = grid.cell(capability) else {
                return Err(MetricsError::WeightedCapabilityAbsent {
                    system: grid.label.clone(),
                    capability: capability.to_string(),
                });
            };
            if let Some(reason) = cell.unmeasured_reason() {
                return Err(MetricsError::WeightedCapabilityUnmeasured {
                    system: grid.label.clone(),
                    capability: capability.to_string(),
                    reason: reason.to_string(),
                });
            }
            weights.push((capability.clone(), weight));
        }
        combine(
            grid,
            AggregationRule::WeightedMean {
                intended_use: weighting.intended_use().to_string(),
            },
            &weights,
            Reduction::Mean,
            Some(weighting),
        )
    }

    /// The worst measured cell, under the grid's declared direction.
    ///
    /// Every measured cell contributes — the worst is a statement about all of them — but the
    /// reported value and interval are the losing cell's. 33.01 asks for "worst-domain behavior,
    /// not only the mean", and a worst-domain number whose coverage listed one cell would hide the
    /// fact that the other ten were checked.
    pub fn worst(grid: &CapabilityGrid) -> Result<Self, MetricsError> {
        let weights = uniform_weights(grid)?;
        combine(
            grid,
            AggregationRule::WorstMeasured,
            &weights,
            Reduction::Worst,
            None,
        )
    }

    /// The number, wrapped so it cannot be reported alone.
    pub fn value(&self) -> BareScore {
        BareScore(self.estimate.value())
    }

    pub fn grid(&self) -> &str {
        &self.grid
    }

    pub fn rule(&self) -> &AggregationRule {
        &self.rule
    }

    pub fn conditions(&self) -> &MeasurementConditions {
        &self.conditions
    }

    pub fn estimate(&self) -> &Estimate {
        &self.estimate
    }

    pub fn interval(&self) -> Option<&Interval> {
        self.estimate.interval()
    }

    /// The coverage, which no accessor can detach from the value: both come off the same object,
    /// and the object cannot be built with one and not the other.
    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    pub fn effective_size(&self) -> usize {
        self.effective_size
    }

    pub fn worst_cell(&self) -> &WorstCell {
        &self.worst
    }

    pub fn spread(&self) -> f64 {
        self.spread
    }

    pub fn weighting_digest(&self) -> Option<&str> {
        self.weighting_digest.as_deref()
    }

    /// Whether the aggregate may be described as covering the grid it names.
    pub fn covers_its_grid(&self) -> bool {
        self.coverage.is_complete()
    }
}

/// How the contributing values become one value. Separate from [`AggregationRule`] because the
/// rule is what a reader is told and this is what the arithmetic does; `CompleteMean` and
/// `MeanOverMeasured` differ in their precondition, not in their reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reduction {
    Mean,
    Worst,
}

fn uniform_weights(grid: &CapabilityGrid) -> Result<Vec<(CapabilityId, f64)>, MetricsError> {
    let weights: Vec<(CapabilityId, f64)> = grid
        .measured()
        .map(|(capability, _)| (capability.clone(), 1.0))
        .collect();
    if weights.is_empty() {
        return Err(MetricsError::AggregateOverNothing {
            grid: grid.label.clone(),
        });
    }
    Ok(weights)
}

fn combine(
    grid: &CapabilityGrid,
    rule: AggregationRule,
    weights: &[(CapabilityId, f64)],
    reduction: Reduction,
    weighting: Option<&DeclaredWeighting>,
) -> Result<CoveredAggregate, MetricsError> {
    if weights.is_empty() {
        return Err(MetricsError::AggregateOverNothing {
            grid: grid.label.clone(),
        });
    }

    let mut total = 0.0f64;
    let mut accumulated = 0.0f64;
    let mut effective_size = usize::MAX;
    let mut contributed = Vec::new();
    let mut interval_parts: Vec<(f64, Interval)> = Vec::new();
    let mut every_cell_had_an_interval = true;
    let mut lowest = f64::INFINITY;
    let mut highest = f64::NEG_INFINITY;
    let mut worst: Option<WorstCell> = None;
    let mut worst_interval: Option<Interval> = None;
    let direction = grid.conditions.scoring_rule.direction;

    for (capability, weight) in weights {
        if !weight.is_finite() || *weight <= 0.0 {
            return Err(MetricsError::MalformedWeighting {
                detail: format!("weight {weight} for {capability} is not positive and finite"),
            });
        }
        let Some(cell) = grid.cell(capability) else {
            return Err(MetricsError::WeightedCapabilityAbsent {
                system: grid.label.clone(),
                capability: capability.to_string(),
            });
        };
        let Some(estimate) = cell.estimate() else {
            let reason = cell
                .unmeasured_reason()
                .unwrap_or(UnmeasuredReason::NotAttempted);
            return Err(MetricsError::WeightedCapabilityUnmeasured {
                system: grid.label.clone(),
                capability: capability.to_string(),
                reason: reason.to_string(),
            });
        };
        let value = estimate.value();
        total += weight;
        accumulated += weight * value;
        effective_size = effective_size.min(cell.effective_size().unwrap_or(0));
        contributed.push(capability.clone());
        match estimate.interval() {
            Some(interval) => interval_parts.push((*weight, interval.clone())),
            None => every_cell_had_an_interval = false,
        }
        lowest = lowest.min(value);
        highest = highest.max(value);
        let replaces = worst
            .as_ref()
            .is_none_or(|current| direction.is_better(current.value, value));
        if replaces {
            worst = Some(WorstCell {
                capability: capability.clone(),
                value,
            });
            worst_interval = estimate.interval().cloned();
        }
    }

    let worst = worst.ok_or_else(|| MetricsError::AggregateOverNothing {
        grid: grid.label.clone(),
    })?;

    let estimate = match reduction {
        Reduction::Mean => {
            let value = accumulated / total;
            if every_cell_had_an_interval {
                match weighted_mean(&interval_parts) {
                    Ok(interval) => Estimate::with_interval(value, interval)?,
                    Err(_) => Estimate::point(value, NoIntervalReason::ClusteringUnitUnknown)?,
                }
            } else {
                Estimate::point(value, NoIntervalReason::EstimatorNotAvailable)?
            }
        }
        Reduction::Worst => match worst_interval {
            Some(interval) => Estimate::with_interval(worst.value, interval)?,
            None => Estimate::point(worst.value, NoIntervalReason::EstimatorNotAvailable)?,
        },
    };

    let mut blocking_holes = Vec::new();
    let mut closed_holes = Vec::new();
    for (capability, reason) in grid.holes() {
        let cell = UnmeasuredCell {
            capability: capability.clone(),
            reason,
        };
        if reason.supports_claim() {
            closed_holes.push(cell);
        } else {
            blocking_holes.push(cell);
        }
    }
    let measured_but_excluded: Vec<CapabilityId> = grid
        .measured()
        .map(|(capability, _)| capability)
        .filter(|capability| !contributed.contains(capability))
        .cloned()
        .collect();

    let coverage = Coverage::derive(
        grid.len(),
        contributed,
        blocking_holes,
        closed_holes,
        measured_but_excluded,
    )
    .map_err(|error| match error {
        MetricsError::CoverageAccountingMismatch {
            cells,
            contributed,
            unmeasured,
            ..
        } => MetricsError::CoverageAccountingMismatch {
            grid: grid.label.clone(),
            cells,
            contributed,
            unmeasured,
        },
        other => other,
    })?;

    Ok(CoveredAggregate {
        grid: grid.label.clone(),
        rule,
        conditions: grid.conditions.clone(),
        estimate,
        coverage,
        effective_size: if effective_size == usize::MAX {
            0
        } else {
            effective_size
        },
        worst,
        spread: if highest >= lowest {
            highest - lowest
        } else {
            0.0
        },
        weighting_digest: weighting.map(|w| w.digest().as_str().to_string()),
    })
}

fn fraction(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}
