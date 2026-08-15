//! Bounded descriptive analytics for capability observations.
//!
//! This module is the executable middle layer between the metric contracts in this crate and a
//! caller's measured inventory. It deliberately computes *descriptive* quantities only. A mean,
//! percentile, retention ratio, calibration error, or paired contrast is useful input to a claim,
//! but none of them is a causal estimator, a clinical validation, or a universal score.
//!
//! The input model is intentionally domain-neutral. `dimension` and `domain` are caller-owned
//! vocabulary, so the same kernel can inspect verification, oncology, multimodal, infrastructure,
//! coordination, or any future capability family without forking arithmetic. The provenance fields
//! remain attached to each observation, and declared/missing/blocked observations are counted but
//! never smuggled into measured summaries.
//!
//! The kernel covers the common arithmetic behind section 33's metric families:
//!
//! - descriptive performance, cost, and latency summaries;
//! - paired robustness and cross-modal agreement summaries;
//! - calibration error for bounded probability forecasts;
//! - reproducibility spread over caller-declared repeated observations;
//! - translation or treatment/control contrasts, reported as contrasts rather than causal effects;
//! - coordination gain and overhead, without pretending to execute agents.
//!
//! It does not choose an estimator, infer missing data, pool correlated observations, or acquire
//! evidence. Every such boundary is returned in the report's caveats so a consumer cannot confuse
//! a complete arithmetic report with a complete scientific study.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;

/// Schema version for [`AnalyticsReport`].
pub const ANALYTICS_SCHEMA_VERSION: &str = "bioprism-metrics-analytics/0.1";
/// Maximum number of rows accepted by the pure in-memory kernel.
pub const MAX_ANALYTICS_ROWS: usize = 10_000;

fn default_max_bins() -> usize {
    10
}

/// Whether a larger or smaller value is favourable for a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HigherIsBetter,
    LowerIsBetter,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::HigherIsBetter => "higher_is_better",
            Direction::LowerIsBetter => "lower_is_better",
        }
    }

    fn orient(self, value: f64) -> f64 {
        match self {
            Direction::HigherIsBetter => value,
            Direction::LowerIsBetter => -value,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Provenance posture of one row. Only observed and reproduced rows contribute to measured
/// summaries; declared rows are visible evidence of intent, not measured support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Observed,
    Reproduced,
    Declared,
    Missing,
    Blocked,
    NotApplicable,
}

impl EvidenceState {
    pub fn is_measured(self) -> bool {
        matches!(self, EvidenceState::Observed | EvidenceState::Reproduced)
    }

    pub fn is_excluded(self) -> bool {
        !self.is_measured()
    }
}

/// One scalar observation with the coordinates needed to interpret it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricObservation {
    pub id: String,
    pub dimension: String,
    pub domain: String,
    pub system: String,
    pub value: f64,
    pub direction: Direction,
    pub unit: String,
    pub condition: String,
    pub replicate_group: Option<String>,
    pub cost: Option<f64>,
    pub latency_ms: Option<f64>,
    pub evidence: EvidenceState,
}

impl MetricObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        dimension: impl Into<String>,
        domain: impl Into<String>,
        system: impl Into<String>,
        value: f64,
        direction: Direction,
        unit: impl Into<String>,
        condition: impl Into<String>,
        evidence: EvidenceState,
    ) -> Result<Self, AnalyticsError> {
        let observation = MetricObservation {
            id: id.into(),
            dimension: dimension.into(),
            domain: domain.into(),
            system: system.into(),
            value,
            direction,
            unit: unit.into(),
            condition: condition.into(),
            replicate_group: None,
            cost: None,
            latency_ms: None,
            evidence,
        };
        observation.validate()?;
        Ok(observation)
    }

    pub fn with_replicate_group(
        mut self,
        group: impl Into<String>,
    ) -> Result<Self, AnalyticsError> {
        self.replicate_group = Some(group.into());
        self.validate()?;
        Ok(self)
    }

    pub fn with_cost(mut self, cost: f64) -> Result<Self, AnalyticsError> {
        self.cost = Some(cost);
        self.validate()?;
        Ok(self)
    }

    pub fn with_latency_ms(mut self, latency_ms: f64) -> Result<Self, AnalyticsError> {
        self.latency_ms = Some(latency_ms);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), AnalyticsError> {
        for (field, value) in [
            ("id", self.id.as_str()),
            ("dimension", self.dimension.as_str()),
            ("domain", self.domain.as_str()),
            ("system", self.system.as_str()),
            ("unit", self.unit.as_str()),
            ("condition", self.condition.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(AnalyticsError::EmptyField { field });
            }
        }
        finite_nonnegative("value", self.value, false)?;
        if let Some(cost) = self.cost {
            finite_nonnegative("cost", cost, true)?;
        }
        if let Some(latency_ms) = self.latency_ms {
            finite_nonnegative("latency_ms", latency_ms, true)?;
        }
        Ok(())
    }
}

/// A paired baseline/variant row. This is the shared shape for robustness, cross-modal agreement,
/// translation gaps, treatment/control contrasts, and coordination comparisons. The report calls
/// it a contrast and never upgrades it to a causal effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedObservation {
    pub id: String,
    pub dimension: String,
    pub domain: String,
    pub baseline: f64,
    pub variant: f64,
    pub direction: Direction,
    pub tolerance: f64,
    pub evidence: EvidenceState,
}

impl PairedObservation {
    pub fn validate(&self) -> Result<(), AnalyticsError> {
        if self.id.trim().is_empty() {
            return Err(AnalyticsError::EmptyField { field: "id" });
        }
        if self.dimension.trim().is_empty() {
            return Err(AnalyticsError::EmptyField { field: "dimension" });
        }
        if self.domain.trim().is_empty() {
            return Err(AnalyticsError::EmptyField { field: "domain" });
        }
        finite_nonnegative("baseline", self.baseline, false)?;
        finite_nonnegative("variant", self.variant, false)?;
        finite_nonnegative("tolerance", self.tolerance, true)?;
        Ok(())
    }
}

/// One probability forecast paired with a bounded outcome. `observed` may be fractional when the
/// caller has a soft reference distribution, but it must remain in `[0, 1]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationObservation {
    pub id: String,
    pub domain: String,
    pub group: Option<String>,
    pub predicted: f64,
    pub observed: f64,
    pub evidence: EvidenceState,
}

impl CalibrationObservation {
    fn validate(&self) -> Result<(), AnalyticsError> {
        if self.id.trim().is_empty() {
            return Err(AnalyticsError::EmptyField { field: "id" });
        }
        if self.domain.trim().is_empty() {
            return Err(AnalyticsError::EmptyField { field: "domain" });
        }
        bounded_probability("predicted", self.predicted)?;
        bounded_probability("observed", self.observed)?;
        Ok(())
    }
}

/// Bounded input to [`analyse`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsInput {
    pub observations: Vec<MetricObservation>,
    #[serde(default)]
    pub pairs: Vec<PairedObservation>,
    #[serde(default)]
    pub calibration: Vec<CalibrationObservation>,
    #[serde(default = "default_max_bins")]
    pub calibration_bins: usize,
}

/// A finite descriptive summary. `count == 0` is a meaningful empty result, not a zero score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DescriptiveStats {
    pub count: usize,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub p95: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub population_variance: Option<f64>,
}

impl DescriptiveStats {
    fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return DescriptiveStats {
                count: 0,
                mean: None,
                median: None,
                p95: None,
                minimum: None,
                maximum: None,
                population_variance: None,
            };
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
        let variance = sorted
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / sorted.len() as f64;
        DescriptiveStats {
            count: sorted.len(),
            mean: Some(mean),
            median: Some(quantile(&sorted, 0.5)),
            p95: Some(quantile(&sorted, 0.95)),
            minimum: sorted.first().copied(),
            maximum: sorted.last().copied(),
            population_variance: Some(variance),
        }
    }
}

/// Summary for one dimension over observed and reproduced rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionSummary {
    pub dimension: String,
    pub direction: Direction,
    pub observations: usize,
    pub measured: usize,
    pub reproduced: usize,
    pub declared: usize,
    pub missing: usize,
    pub blocked: usize,
    pub not_applicable: usize,
    pub domains: Vec<String>,
    pub systems: Vec<String>,
    pub replicate_groups: Vec<String>,
    pub values: DescriptiveStats,
    pub cost: Option<DescriptiveStats>,
    pub latency_ms: Option<DescriptiveStats>,
}

/// Summary of paired variation. `mean_oriented_delta` is positive when the variant moved in the
/// declared favourable direction; it is not a causal effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairSummary {
    pub dimension: String,
    pub direction: Direction,
    pub observations: usize,
    pub measured: usize,
    pub excluded: usize,
    pub domains: Vec<String>,
    pub mean_oriented_delta: Option<f64>,
    pub mean_absolute_delta: Option<f64>,
    pub mean_retention: Option<f64>,
    pub worst_retention: Option<f64>,
    pub positive_fraction: Option<f64>,
    pub negative_fraction: Option<f64>,
    pub agreement_fraction: Option<f64>,
    pub tolerance: Option<f64>,
    pub tolerance_minimum: Option<f64>,
    pub tolerance_maximum: Option<f64>,
}

/// One equal-width calibration bin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub lower: f64,
    pub upper: f64,
    pub count: usize,
    pub mean_prediction: Option<f64>,
    pub observed_rate: Option<f64>,
    pub absolute_error: Option<f64>,
}

/// Calibration summary with Brier score and equal-width expected calibration error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationSummary {
    pub observations: usize,
    pub measured: usize,
    pub excluded: usize,
    pub brier_score: Option<f64>,
    pub expected_calibration_error: Option<f64>,
    pub bins: Vec<CalibrationBin>,
}

/// Overall coverage and provenance counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsCoverage {
    pub scalar_observations: usize,
    pub measured_observations: usize,
    pub excluded_observations: usize,
    pub paired_observations: usize,
    pub measured_pairs: usize,
    pub calibration_observations: usize,
    pub measured_calibration_observations: usize,
    pub dimensions: usize,
    pub domains: usize,
    pub systems: usize,
}

/// Result of the bounded analysis. All rows are sorted deterministically by their identifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsReport {
    pub schema_version: String,
    pub coverage: AnalyticsCoverage,
    pub dimensions: Vec<DimensionSummary>,
    pub paired: Vec<PairSummary>,
    pub calibration: Option<CalibrationSummary>,
    pub caveats: Vec<String>,
}

/// Errors that prevent a descriptive report from being built.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum AnalyticsError {
    #[error("{field} is empty")]
    EmptyField { field: &'static str },
    #[error("{field} is not finite: {value}")]
    NonFinite { field: &'static str, value: f64 },
    #[error("{field} must be non-negative: {value}")]
    Negative { field: &'static str, value: f64 },
    #[error("{field} must be between 0 and 1: {value}")]
    ProbabilityOutOfRange { field: &'static str, value: f64 },
    #[error("{kind} contains too many rows: {count}; maximum is {maximum}")]
    TooManyRows {
        kind: &'static str,
        count: usize,
        maximum: usize,
    },
    #[error("duplicate observation id {id}")]
    DuplicateId { id: String },
    #[error("dimension {dimension} declares both {left} and {right} direction")]
    DirectionMismatch {
        dimension: String,
        left: Direction,
        right: Direction,
    },
    #[error("calibration_bins must be between 2 and 100, got {0}")]
    InvalidBinCount(usize),
}

fn finite_nonnegative(
    field: &'static str,
    value: f64,
    nonnegative: bool,
) -> Result<(), AnalyticsError> {
    if !value.is_finite() {
        return Err(AnalyticsError::NonFinite { field, value });
    }
    if nonnegative && value < 0.0 {
        return Err(AnalyticsError::Negative { field, value });
    }
    Ok(())
}

fn bounded_probability(field: &'static str, value: f64) -> Result<(), AnalyticsError> {
    if !value.is_finite() {
        return Err(AnalyticsError::NonFinite { field, value });
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(AnalyticsError::ProbabilityOutOfRange { field, value });
    }
    Ok(())
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability * (sorted.len().saturating_sub(1) as f64);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    let fraction = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * fraction
}

fn sorted_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort();
    values
}

/// Build a bounded descriptive report from caller-supplied observations.
pub fn analyse(input: &AnalyticsInput) -> Result<AnalyticsReport, AnalyticsError> {
    if input.observations.len() > MAX_ANALYTICS_ROWS {
        return Err(AnalyticsError::TooManyRows {
            kind: "observations",
            count: input.observations.len(),
            maximum: MAX_ANALYTICS_ROWS,
        });
    }
    if input.pairs.len() > MAX_ANALYTICS_ROWS {
        return Err(AnalyticsError::TooManyRows {
            kind: "pairs",
            count: input.pairs.len(),
            maximum: MAX_ANALYTICS_ROWS,
        });
    }
    if input.calibration.len() > MAX_ANALYTICS_ROWS {
        return Err(AnalyticsError::TooManyRows {
            kind: "calibration",
            count: input.calibration.len(),
            maximum: MAX_ANALYTICS_ROWS,
        });
    }
    if !(2..=100).contains(&input.calibration_bins) {
        return Err(AnalyticsError::InvalidBinCount(input.calibration_bins));
    }

    let mut ids = BTreeSet::new();
    let mut dimensions: BTreeMap<String, Vec<&MetricObservation>> = BTreeMap::new();
    let mut systems = BTreeSet::new();
    let mut domains = BTreeSet::new();
    for observation in &input.observations {
        observation.validate()?;
        if !ids.insert(observation.id.clone()) {
            return Err(AnalyticsError::DuplicateId {
                id: observation.id.clone(),
            });
        }
        systems.insert(observation.system.clone());
        domains.insert(observation.domain.clone());
        dimensions
            .entry(observation.dimension.clone())
            .or_default()
            .push(observation);
    }
    for pair in &input.pairs {
        pair.validate()?;
    }
    for row in &input.calibration {
        row.validate()?;
    }

    let mut dimension_rows = Vec::with_capacity(dimensions.len());
    for (dimension, rows) in dimensions {
        let direction = rows[0].direction;
        if let Some(conflict) = rows.iter().find(|row| row.direction != direction) {
            return Err(AnalyticsError::DirectionMismatch {
                dimension,
                left: direction,
                right: conflict.direction,
            });
        }
        let mut values = Vec::new();
        let mut costs = Vec::new();
        let mut latencies = Vec::new();
        let mut domains_in_dimension = Vec::new();
        let mut systems_in_dimension = Vec::new();
        let mut replicate_groups = Vec::new();
        let mut reproduced = 0;
        let mut declared = 0;
        let mut missing = 0;
        let mut blocked = 0;
        let mut not_applicable = 0;
        for row in &rows {
            domains_in_dimension.push(row.domain.clone());
            systems_in_dimension.push(row.system.clone());
            if let Some(group) = &row.replicate_group {
                replicate_groups.push(group.clone());
            }
            match row.evidence {
                EvidenceState::Observed => {
                    values.push(row.value);
                    if let Some(cost) = row.cost {
                        costs.push(cost);
                    }
                    if let Some(latency) = row.latency_ms {
                        latencies.push(latency);
                    }
                }
                EvidenceState::Reproduced => {
                    values.push(row.value);
                    reproduced += 1;
                    if let Some(cost) = row.cost {
                        costs.push(cost);
                    }
                    if let Some(latency) = row.latency_ms {
                        latencies.push(latency);
                    }
                }
                EvidenceState::Declared => declared += 1,
                EvidenceState::Missing => missing += 1,
                EvidenceState::Blocked => blocked += 1,
                EvidenceState::NotApplicable => not_applicable += 1,
            }
        }
        dimension_rows.push(DimensionSummary {
            dimension,
            direction,
            observations: rows.len(),
            measured: values.len(),
            reproduced,
            declared,
            missing,
            blocked,
            not_applicable,
            domains: sorted_strings(domains_in_dimension),
            systems: sorted_strings(systems_in_dimension),
            replicate_groups: sorted_strings(replicate_groups),
            values: DescriptiveStats::from_values(&values),
            cost: if costs.is_empty() {
                None
            } else {
                Some(DescriptiveStats::from_values(&costs))
            },
            latency_ms: if latencies.is_empty() {
                None
            } else {
                Some(DescriptiveStats::from_values(&latencies))
            },
        });
    }

    let mut pair_groups: BTreeMap<String, Vec<&PairedObservation>> = BTreeMap::new();
    for pair in &input.pairs {
        pair_groups
            .entry(pair.dimension.clone())
            .or_default()
            .push(pair);
    }
    let mut paired_rows = Vec::with_capacity(pair_groups.len());
    for (dimension, rows) in pair_groups {
        let direction = rows[0].direction;
        if let Some(conflict) = rows.iter().find(|row| row.direction != direction) {
            return Err(AnalyticsError::DirectionMismatch {
                dimension,
                left: direction,
                right: conflict.direction,
            });
        }
        let mut oriented_deltas = Vec::new();
        let mut absolute_deltas = Vec::new();
        let mut retentions = Vec::new();
        let mut positive = 0usize;
        let mut negative = 0usize;
        let mut agreements = 0usize;
        let mut tolerance_values = Vec::new();
        let mut domains_in_pairs = Vec::new();
        let mut excluded = 0;
        for row in &rows {
            domains_in_pairs.push(row.domain.clone());
            if row.evidence.is_excluded() {
                excluded += 1;
                continue;
            }
            let delta = direction.orient(row.variant) - direction.orient(row.baseline);
            let absolute_delta = (row.variant - row.baseline).abs();
            let retention = match direction {
                Direction::HigherIsBetter if row.baseline != 0.0 => {
                    Some(row.variant / row.baseline)
                }
                Direction::LowerIsBetter if row.variant != 0.0 => Some(row.baseline / row.variant),
                _ => None,
            };
            oriented_deltas.push(delta);
            absolute_deltas.push(absolute_delta);
            if delta > 0.0 {
                positive += 1;
            } else if delta < 0.0 {
                negative += 1;
            }
            if absolute_delta <= row.tolerance {
                agreements += 1;
            }
            if let Some(retention) = retention {
                retentions.push(retention);
            }
            tolerance_values.push(row.tolerance);
        }
        let count = oriented_deltas.len();
        paired_rows.push(PairSummary {
            dimension,
            direction,
            observations: rows.len(),
            measured: count,
            excluded,
            domains: sorted_strings(domains_in_pairs),
            mean_oriented_delta: mean(&oriented_deltas),
            mean_absolute_delta: mean(&absolute_deltas),
            mean_retention: mean(&retentions),
            worst_retention: retentions.iter().copied().reduce(f64::min),
            positive_fraction: fraction(positive, count),
            negative_fraction: fraction(negative, count),
            agreement_fraction: fraction(agreements, count),
            tolerance: if tolerance_values
                .windows(2)
                .all(|window| (window[0] - window[1]).abs() < f64::EPSILON)
            {
                tolerance_values.first().copied()
            } else {
                None
            },
            tolerance_minimum: tolerance_values.iter().copied().reduce(f64::min),
            tolerance_maximum: tolerance_values.iter().copied().reduce(f64::max),
        });
    }

    let calibration = if input.calibration.is_empty() {
        None
    } else {
        Some(calibration_summary(
            &input.calibration,
            input.calibration_bins,
        ))
    };
    let measured_observations = input
        .observations
        .iter()
        .filter(|row| row.evidence.is_measured())
        .count();
    let measured_pairs = input
        .pairs
        .iter()
        .filter(|row| row.evidence.is_measured())
        .count();
    let measured_calibration_observations = input
        .calibration
        .iter()
        .filter(|row| row.evidence.is_measured())
        .count();
    Ok(AnalyticsReport {
        schema_version: ANALYTICS_SCHEMA_VERSION.to_string(),
        coverage: AnalyticsCoverage {
            scalar_observations: input.observations.len(),
            measured_observations,
            excluded_observations: input.observations.len() - measured_observations,
            paired_observations: input.pairs.len(),
            measured_pairs,
            calibration_observations: input.calibration.len(),
            measured_calibration_observations,
            dimensions: dimension_rows.len(),
            domains: domains.len(),
            systems: systems.len(),
        },
        dimensions: dimension_rows,
        paired: paired_rows,
        calibration,
        caveats: vec![
            "summaries include only observed and reproduced rows; declared, missing, blocked, and not_applicable rows remain visible but are excluded".into(),
            "paired deltas and retention are descriptive contrasts, not causal effects or clinical validation".into(),
            "calibration uses caller-supplied bounded outcomes and equal-width bins; it does not fit or validate a probabilistic model".into(),
            "no missing-value imputation, dependency correction, interval estimation, acquisition, or external data access occurs".into(),
        ],
    })
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn fraction(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

fn calibration_summary(rows: &[CalibrationObservation], bins: usize) -> CalibrationSummary {
    let measured = rows
        .iter()
        .filter(|row| row.evidence.is_measured())
        .collect::<Vec<_>>();
    let excluded = rows.len() - measured.len();
    let brier_score = mean(
        &measured
            .iter()
            .map(|row| (row.predicted - row.observed).powi(2))
            .collect::<Vec<_>>(),
    );
    let mut bin_rows = (0..bins)
        .map(|index| CalibrationBin {
            lower: index as f64 / bins as f64,
            upper: (index + 1) as f64 / bins as f64,
            count: 0,
            mean_prediction: None,
            observed_rate: None,
            absolute_error: None,
        })
        .collect::<Vec<_>>();
    for row in &measured {
        let index = ((row.predicted * bins as f64).floor() as usize).min(bins - 1);
        let bin = &mut bin_rows[index];
        bin.count += 1;
        bin.mean_prediction = Some(bin.mean_prediction.unwrap_or(0.0) + row.predicted);
        bin.observed_rate = Some(bin.observed_rate.unwrap_or(0.0) + row.observed);
    }
    let mut expected_calibration_error = 0.0;
    if !measured.is_empty() {
        for bin in &mut bin_rows {
            if bin.count == 0 {
                continue;
            }
            bin.mean_prediction = bin.mean_prediction.map(|sum| sum / bin.count as f64);
            bin.observed_rate = bin.observed_rate.map(|sum| sum / bin.count as f64);
            bin.absolute_error =
                Some((bin.mean_prediction.unwrap_or(0.0) - bin.observed_rate.unwrap_or(0.0)).abs());
            expected_calibration_error +=
                bin.absolute_error.unwrap_or(0.0) * bin.count as f64 / measured.len() as f64;
        }
    }
    CalibrationSummary {
        observations: rows.len(),
        measured: measured.len(),
        excluded,
        brier_score,
        expected_calibration_error: (!measured.is_empty()).then_some(expected_calibration_error),
        bins: bin_rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, value: f64, evidence: EvidenceState) -> MetricObservation {
        MetricObservation::new(
            id,
            "verification",
            "oncology",
            "agent-a",
            value,
            Direction::HigherIsBetter,
            "fraction",
            "pack/1",
            evidence,
        )
        .unwrap()
    }

    #[test]
    fn summaries_keep_missingness_and_compute_descriptive_values() {
        let report = analyse(&AnalyticsInput {
            observations: vec![
                observation("one", 0.5, EvidenceState::Observed),
                observation("two", 0.9, EvidenceState::Reproduced),
                observation("three", 0.0, EvidenceState::Missing),
                observation("four", 1.0, EvidenceState::Declared),
            ],
            pairs: Vec::new(),
            calibration: Vec::new(),
            calibration_bins: 10,
        })
        .unwrap();
        let row = &report.dimensions[0];
        assert_eq!(row.measured, 2);
        assert_eq!(row.reproduced, 1);
        assert_eq!(row.missing, 1);
        assert_eq!(row.declared, 1);
        assert_eq!(row.values.mean, Some(0.7));
        assert_eq!(report.coverage.excluded_observations, 2);
    }

    #[test]
    fn paired_lower_is_better_rows_report_retention_and_agreement() {
        let report = analyse(&AnalyticsInput {
            observations: Vec::new(),
            pairs: vec![
                PairedObservation {
                    id: "a".into(),
                    dimension: "latency".into(),
                    domain: "runtime".into(),
                    baseline: 10.0,
                    variant: 5.0,
                    direction: Direction::LowerIsBetter,
                    tolerance: 1.0,
                    evidence: EvidenceState::Observed,
                },
                PairedObservation {
                    id: "b".into(),
                    dimension: "latency".into(),
                    domain: "runtime".into(),
                    baseline: 10.0,
                    variant: 10.5,
                    direction: Direction::LowerIsBetter,
                    tolerance: 1.0,
                    evidence: EvidenceState::Observed,
                },
            ],
            calibration: Vec::new(),
            calibration_bins: 10,
        })
        .unwrap();
        let row = &report.paired[0];
        assert_eq!(row.measured, 2);
        assert_eq!(row.positive_fraction, Some(0.5));
        assert_eq!(row.agreement_fraction, Some(0.5));
        assert_eq!(row.mean_retention, Some((2.0 + 10.0 / 10.5) / 2.0));
    }

    #[test]
    fn calibration_is_bounded_and_excludes_declarations() {
        let report = analyse(&AnalyticsInput {
            observations: Vec::new(),
            pairs: Vec::new(),
            calibration: vec![
                CalibrationObservation {
                    id: "one".into(),
                    domain: "verification".into(),
                    group: None,
                    predicted: 0.9,
                    observed: 1.0,
                    evidence: EvidenceState::Observed,
                },
                CalibrationObservation {
                    id: "two".into(),
                    domain: "verification".into(),
                    group: None,
                    predicted: 0.1,
                    observed: 0.0,
                    evidence: EvidenceState::Declared,
                },
            ],
            calibration_bins: 2,
        })
        .unwrap();
        let calibration = report.calibration.unwrap();
        assert_eq!(calibration.measured, 1);
        assert_eq!(calibration.excluded, 1);
        assert!((calibration.brier_score.unwrap() - 0.01).abs() < 1e-12);
        assert!(calibration.expected_calibration_error.unwrap() > 0.0);
    }

    #[test]
    fn duplicate_ids_and_mixed_directions_refuse() {
        let mut first = observation("same", 0.5, EvidenceState::Observed);
        let mut second = observation("same", 0.6, EvidenceState::Observed);
        assert!(matches!(
            analyse(&AnalyticsInput {
                observations: vec![first.clone(), second.clone()],
                pairs: Vec::new(),
                calibration: Vec::new(),
                calibration_bins: 10,
            }),
            Err(AnalyticsError::DuplicateId { .. })
        ));
        first.id = "first".into();
        second.id = "second".into();
        second.direction = Direction::LowerIsBetter;
        assert!(matches!(
            analyse(&AnalyticsInput {
                observations: vec![first, second],
                pairs: Vec::new(),
                calibration: Vec::new(),
                calibration_bins: 10,
            }),
            Err(AnalyticsError::DirectionMismatch { .. })
        ));
    }
}
