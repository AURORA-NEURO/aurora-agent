//! Deterministic longitudinal trajectory analysis for preclinical glioma studies.
//!
//! Endpoint-only comparisons can hide adaptation, rebound, or model-specific kinetics.  This
//! feature fits an integer least-squares slope for every independently tracked unit, summarizes
//! the eligible trajectories by arm, and compares the arm-level slopes.  It is intentionally
//! conservative: duplicate timepoints, too few observations, unbalanced time grids, and noisy or
//! non-monotone trajectories remain visible as uncertainty or negative evidence.  No trajectory
//! is interpreted as a diagnosis, prognosis, or treatment recommendation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaTrajectoryAnalysis1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_UNITS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub timepoint: u32,
    pub outcome_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub min_timepoints_per_unit: u16,
    pub min_units_per_arm: usize,
    pub slope_threshold_milli_per_tick: u64,
    pub max_residual_milli: u64,
    pub max_monotonicity_violations: u16,
    pub require_balanced_timepoints: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitTrajectoryDisposition {
    Eligible,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitTrajectory {
    pub unit_id: String,
    pub arm_id: String,
    pub observation_order: Vec<String>,
    pub timepoint_order: Vec<u32>,
    pub slope_milli_per_tick: i64,
    pub intercept_milli: i64,
    pub residual_mad_milli: u64,
    pub monotonicity_violations: u16,
    pub disposition: UnitTrajectoryDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryArmSummary {
    pub arm_id: String,
    pub unit_order: Vec<String>,
    pub eligible_unit_order: Vec<String>,
    pub timepoint_order: Vec<u32>,
    pub mean_slope_milli_per_tick: i64,
    pub min_slope_milli_per_tick: i64,
    pub max_slope_milli_per_tick: i64,
    pub mean_residual_mad_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryDisposition {
    Qualified,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub control: TrajectoryArmSummary,
    pub treatment: TrajectoryArmSummary,
    pub unit_order: Vec<String>,
    pub unresolved_unit_order: Vec<String>,
    pub slope_effect_milli_per_tick: i64,
    pub interval_low_milli_per_tick: i64,
    pub interval_high_milli_per_tick: i64,
    pub eligible_unit_count: usize,
    pub slope_direction_concordance_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: TrajectoryDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TrajectoryError {
    #[error("trajectory request is invalid: {0}")]
    InvalidRequest(String),
    #[error("trajectory observations are invalid: {0}")]
    InvalidObservation(String),
    #[error("trajectory output is invalid: {0}")]
    InvalidOutput(String),
    #[error("trajectory digest failed: {0}")]
    Digest(String),
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn fit_unit(
    observations: &[&TrajectoryObservation],
    request: &TrajectoryRequest,
) -> UnitTrajectory {
    let mut ordered = observations.to_vec();
    ordered.sort_by(|left, right| {
        left.timepoint
            .cmp(&right.timepoint)
            .then_with(|| left.observation_id.cmp(&right.observation_id))
    });
    let xs = ordered
        .iter()
        .map(|observation| observation.timepoint as i128)
        .collect::<Vec<_>>();
    let ys = ordered
        .iter()
        .map(|observation| observation.outcome_milli as i128)
        .collect::<Vec<_>>();
    let n = xs.len() as i128;
    let sum_x = xs.iter().sum::<i128>();
    let sum_y = ys.iter().sum::<i128>();
    let sum_xx = xs.iter().map(|value| value * value).sum::<i128>();
    let sum_xy = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum::<i128>();
    let denominator = n * sum_xx - sum_x * sum_x;
    let numerator = n * sum_xy - sum_x * sum_y;
    let slope = if denominator == 0 {
        0
    } else {
        (numerator / denominator).clamp(i64::MIN as i128, i64::MAX as i128) as i64
    };
    let intercept =
        ((sum_y - slope as i128 * sum_x) / n).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    let mut residuals = ordered
        .iter()
        .map(|observation| {
            let predicted = intercept as i128 + slope as i128 * observation.timepoint as i128;
            (observation.outcome_milli as i128 - predicted).unsigned_abs() as u64
        })
        .collect::<Vec<_>>();
    let residual_mad = median(&mut residuals);
    let slope_sign = slope.signum();
    let monotonicity_violations = ordered
        .windows(2)
        .filter(|pair| {
            let delta = pair[1].outcome_milli - pair[0].outcome_milli;
            (slope_sign > 0 && delta < 0)
                || (slope_sign < 0 && delta > 0)
                || (slope_sign == 0 && delta != 0)
        })
        .count()
        .min(u16::MAX as usize) as u16;
    let mut uncertainty = BTreeSet::new();
    if ordered.len() < request.min_timepoints_per_unit as usize {
        uncertainty.insert("unit-timepoint-floor-not-met".into());
    }
    if denominator == 0 {
        uncertainty.insert("unit-timepoints-have-no-span".into());
    }
    if residual_mad > request.max_residual_milli {
        uncertainty.insert("unit-residual-noise-exceeds-tolerance".into());
    }
    if monotonicity_violations > request.max_monotonicity_violations {
        uncertainty.insert("unit-monotonicity-violations-exceed-tolerance".into());
    }
    let disposition =
        if ordered.len() < request.min_timepoints_per_unit as usize || denominator == 0 {
            UnitTrajectoryDisposition::Unresolved
        } else {
            UnitTrajectoryDisposition::Eligible
        };
    UnitTrajectory {
        unit_id: ordered[0].unit_id.clone(),
        arm_id: ordered[0].arm_id.clone(),
        observation_order: ordered
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect(),
        timepoint_order: ordered
            .iter()
            .map(|observation| observation.timepoint)
            .collect(),
        slope_milli_per_tick: slope,
        intercept_milli: intercept,
        residual_mad_milli: residual_mad,
        monotonicity_violations,
        disposition,
        uncertainty: uncertainty.into_iter().collect(),
    }
}

fn arm_summary(arm_id: &str, trajectories: &[&UnitTrajectory]) -> TrajectoryArmSummary {
    let unit_order = trajectories
        .iter()
        .map(|trajectory| trajectory.unit_id.clone())
        .collect::<Vec<_>>();
    let eligible = trajectories
        .iter()
        .filter(|trajectory| trajectory.disposition == UnitTrajectoryDisposition::Eligible)
        .map(|trajectory| trajectory.unit_id.clone())
        .collect::<Vec<_>>();
    let timepoints = trajectories
        .iter()
        .flat_map(|trajectory| trajectory.timepoint_order.iter().copied())
        .collect::<BTreeSet<_>>();
    let eligible_trajectories = trajectories
        .iter()
        .filter(|trajectory| trajectory.disposition == UnitTrajectoryDisposition::Eligible)
        .collect::<Vec<_>>();
    let slopes = eligible_trajectories
        .iter()
        .map(|trajectory| trajectory.slope_milli_per_tick)
        .collect::<Vec<_>>();
    let residuals = eligible_trajectories
        .iter()
        .map(|trajectory| trajectory.residual_mad_milli)
        .collect::<Vec<_>>();
    let (mean_slope, min_slope, max_slope, mean_residual) = if slopes.is_empty() {
        (0, 0, 0, 0)
    } else {
        (
            (slopes.iter().map(|value| *value as i128).sum::<i128>() / slopes.len() as i128) as i64,
            *slopes.iter().min().unwrap(),
            *slopes.iter().max().unwrap(),
            (residuals.iter().sum::<u64>() / residuals.len() as u64),
        )
    };
    TrajectoryArmSummary {
        arm_id: arm_id.into(),
        unit_order,
        eligible_unit_order: eligible,
        timepoint_order: timepoints.into_iter().collect(),
        mean_slope_milli_per_tick: mean_slope,
        min_slope_milli_per_tick: min_slope,
        max_slope_milli_per_tick: max_slope,
        mean_residual_mad_milli: mean_residual,
    }
}

fn digest_input(analysis: &TrajectoryAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": analysis.feature_id,
        "output_schema": analysis.output_schema,
        "objective": analysis.objective,
        "control": analysis.control,
        "treatment": analysis.treatment,
        "unit_order": analysis.unit_order,
        "unresolved_unit_order": analysis.unresolved_unit_order,
        "slope_effect_milli_per_tick": analysis.slope_effect_milli_per_tick,
        "interval_low_milli_per_tick": analysis.interval_low_milli_per_tick,
        "interval_high_milli_per_tick": analysis.interval_high_milli_per_tick,
        "eligible_unit_count": analysis.eligible_unit_count,
        "slope_direction_concordance_milli": analysis.slope_direction_concordance_milli,
        "negative_evidence": analysis.negative_evidence,
        "uncertainty": analysis.uncertainty,
        "disposition": analysis.disposition,
    })
}

impl TrajectoryAnalysis {
    pub fn validate(&self) -> Result<(), TrajectoryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.unit_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .unresolved_unit_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.eligible_unit_count > self.unit_order.len()
            || self.slope_direction_concordance_milli > 1_000
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(TrajectoryError::InvalidOutput(
                "identity, unit ordering, counts, bounds, or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| TrajectoryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(TrajectoryError::InvalidOutput(
                "digest is not bound to the trajectory analysis".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze deterministic per-unit longitudinal slopes and the between-arm trajectory effect.
pub fn analyze_glioma_trajectories(
    request: &TrajectoryRequest,
    observations: &[TrajectoryObservation],
) -> Result<TrajectoryAnalysis, TrajectoryError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_timepoints_per_unit < 2
        || request.min_units_per_arm == 0
        || request.slope_threshold_milli_per_tick == 0
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(TrajectoryError::InvalidRequest(
            "objective, distinct arms, timepoint/unit floors, slope threshold, and bounded observations are required".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut grouped = BTreeMap::<(String, String), Vec<&TrajectoryObservation>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || (observation.arm_id != request.control_arm
                && observation.arm_id != request.treatment_arm)
            || observation.model_system != request.model_system
            || !observation_ids.insert(observation.observation_id.clone())
        {
            return Err(TrajectoryError::InvalidObservation(
                "observation identity, arm, model, batch, or uniqueness is invalid".into(),
            ));
        }
        grouped
            .entry((observation.arm_id.clone(), observation.unit_id.clone()))
            .or_default()
            .push(observation);
    }
    if grouped.len() > MAX_UNITS {
        return Err(TrajectoryError::InvalidObservation(
            "unit count exceeds bounded trajectory capacity".into(),
        ));
    }
    for unit in grouped.values() {
        let mut timepoints = BTreeSet::new();
        if unit
            .iter()
            .any(|observation| !timepoints.insert(observation.timepoint))
        {
            return Err(TrajectoryError::InvalidObservation(
                "each unit must have at most one observation per timepoint".into(),
            ));
        }
    }
    let mut trajectories = grouped
        .values()
        .map(|unit| fit_unit(unit, request))
        .collect::<Vec<_>>();
    trajectories.sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
    let unit_order = trajectories
        .iter()
        .map(|trajectory| trajectory.unit_id.clone())
        .collect::<Vec<_>>();
    let unresolved_unit_order = trajectories
        .iter()
        .filter(|trajectory| trajectory.disposition == UnitTrajectoryDisposition::Unresolved)
        .map(|trajectory| trajectory.unit_id.clone())
        .collect::<Vec<_>>();
    let control = trajectories
        .iter()
        .filter(|trajectory| trajectory.arm_id == request.control_arm)
        .collect::<Vec<_>>();
    let treatment = trajectories
        .iter()
        .filter(|trajectory| trajectory.arm_id == request.treatment_arm)
        .collect::<Vec<_>>();
    let control_summary = arm_summary(&request.control_arm, &control);
    let treatment_summary = arm_summary(&request.treatment_arm, &treatment);
    let balanced_timepoints = control_summary.timepoint_order == treatment_summary.timepoint_order;
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if control_summary.eligible_unit_order.len() < request.min_units_per_arm {
        negative.insert("control-unit-floor-not-met".into());
    }
    if treatment_summary.eligible_unit_order.len() < request.min_units_per_arm {
        negative.insert("treatment-unit-floor-not-met".into());
    }
    if !unresolved_unit_order.is_empty() {
        uncertainty.insert("one-or-more-units-are-unresolved".into());
    }
    if !balanced_timepoints {
        uncertainty.insert("control-and-treatment-time-grids-differ".into());
    }
    if !balanced_timepoints && request.require_balanced_timepoints {
        negative.insert("balanced-timepoint-requirement-not-met".into());
    }
    let enough_arms = control_summary.eligible_unit_order.len() >= request.min_units_per_arm
        && treatment_summary.eligible_unit_order.len() >= request.min_units_per_arm;
    let slope_effect = treatment_summary
        .mean_slope_milli_per_tick
        .saturating_sub(control_summary.mean_slope_milli_per_tick);
    let residual_uncertainty = control_summary
        .mean_residual_mad_milli
        .max(treatment_summary.mean_residual_mad_milli);
    let interval_low =
        slope_effect.saturating_sub(residual_uncertainty.min(i64::MAX as u64) as i64);
    let interval_high =
        slope_effect.saturating_add(residual_uncertainty.min(i64::MAX as u64) as i64);
    let eligible_unit_count =
        control_summary.eligible_unit_order.len() + treatment_summary.eligible_unit_order.len();
    let expected_sign = slope_effect.signum();
    let direction_matches = control
        .iter()
        .chain(treatment.iter())
        .filter(|trajectory| trajectory.disposition == UnitTrajectoryDisposition::Eligible)
        .filter(|trajectory| trajectory.slope_milli_per_tick.signum() == expected_sign)
        .count();
    let slope_direction_concordance_milli = (direction_matches * 1_000)
        .checked_div(eligible_unit_count)
        .unwrap_or_default()
        .min(1_000) as u16;
    let excessive_violations = trajectories
        .iter()
        .any(|trajectory| trajectory.monotonicity_violations > request.max_monotonicity_violations);
    if excessive_violations {
        negative.insert("trajectory-monotonicity-violation-exceeds-tolerance".into());
    }
    if residual_uncertainty > request.max_residual_milli {
        uncertainty.insert("arm-residual-noise-exceeds-tolerance".into());
    }
    let disposition = if !enough_arms
        || !unresolved_unit_order.is_empty()
        || request.require_balanced_timepoints && !balanced_timepoints
    {
        TrajectoryDisposition::Unresolved
    } else if slope_effect.unsigned_abs() < request.slope_threshold_milli_per_tick
        || excessive_violations
    {
        negative.insert("trajectory-effect-below-declared-threshold-or-noisy".into());
        TrajectoryDisposition::Negative
    } else {
        TrajectoryDisposition::Qualified
    };
    let mut analysis = TrajectoryAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        control: control_summary,
        treatment: treatment_summary,
        unit_order,
        unresolved_unit_order,
        slope_effect_milli_per_tick: slope_effect,
        interval_low_milli_per_tick: interval_low,
        interval_high_milli_per_tick: interval_high,
        eligible_unit_count,
        slope_direction_concordance_milli,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| TrajectoryError::Digest(error.to_string()))?,
    };
    analysis.digest = ContentHash::of_value(&digest_input(&analysis))
        .map_err(|error| TrajectoryError::Digest(error.to_string()))?;
    analysis.validate()?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observations(delta: i64) -> Vec<TrajectoryObservation> {
        let mut rows = Vec::new();
        for (arm, sign) in [("control", 1_i64), ("treated", 1_i64)] {
            for unit in 0..2 {
                for timepoint in 0..3 {
                    rows.push(TrajectoryObservation {
                        observation_id: format!("{arm}-{unit}-{timepoint}"),
                        unit_id: format!("{arm}-unit-{unit}"),
                        arm_id: arm.into(),
                        model_system: GliomaModelSystem::Organoid,
                        batch_id: format!("batch-{timepoint}"),
                        timepoint,
                        outcome_milli: 100
                            + sign * (timepoint as i64 * if arm == "treated" { delta } else { 1 }),
                    });
                }
            }
        }
        rows
    }

    fn request() -> TrajectoryRequest {
        TrajectoryRequest {
            objective: "compare glioma invasion trajectories".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            min_timepoints_per_unit: 3,
            min_units_per_arm: 2,
            slope_threshold_milli_per_tick: 2,
            max_residual_milli: 0,
            max_monotonicity_violations: 0,
            require_balanced_timepoints: true,
        }
    }

    #[test]
    fn deterministic_trajectory_effect_is_qualified_and_replay_stable() {
        let first = analyze_glioma_trajectories(&request(), &observations(10)).unwrap();
        let second = analyze_glioma_trajectories(&request(), &observations(10)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, TrajectoryDisposition::Qualified);
        assert_eq!(first.slope_effect_milli_per_tick, 9);
        first.validate().unwrap();
    }

    #[test]
    fn missing_timepoint_floor_is_unresolved() {
        let mut rows = observations(10);
        rows.retain(|row| row.observation_id != "treated-1-2");
        let output = analyze_glioma_trajectories(&request(), &rows).unwrap();
        assert_eq!(output.disposition, TrajectoryDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("unresolved")));
    }

    #[test]
    fn small_slope_is_published_as_negative() {
        let output = analyze_glioma_trajectories(&request(), &observations(1)).unwrap();
        assert_eq!(output.disposition, TrajectoryDisposition::Negative);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("below")));
    }
}
