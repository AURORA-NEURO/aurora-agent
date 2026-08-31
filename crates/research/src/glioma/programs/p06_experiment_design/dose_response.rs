//! Fixed-point dose-response analysis for preclinical glioma experiments.
//!
//! This feature turns multi-dose assay observations into a monotone response curve using a
//! weighted pool-adjacent-violators fit.  It exposes the raw means, the fitted curve, residual
//! uncertainty, observed monotonicity violations, and an interpolated half-maximal dose when the
//! curve identifies one.  The fit is deliberately transparent and bounded; it is not a clinical
//! dose recommendation and does not infer efficacy outside the declared preclinical model.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaDoseResponseAnalysis1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_DOSE_LEVELS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoseDirection {
    Increasing,
    Decreasing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseResponseObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub dose_milli: u32,
    pub outcome_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseResponseRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_dose_milli: u32,
    pub direction: DoseDirection,
    pub min_observations_per_dose: usize,
    pub min_dose_levels: usize,
    pub effect_threshold_milli: u64,
    pub max_residual_milli: u64,
    pub max_monotonicity_violations: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseResponsePoint {
    pub dose_milli: u32,
    pub observation_order: Vec<String>,
    pub observation_count: usize,
    pub observed_mean_milli: i64,
    pub fitted_mean_milli: i64,
    pub residual_mad_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoseResponseDisposition {
    Qualified,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseResponseAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub direction: DoseDirection,
    pub dose_order: Vec<u32>,
    pub curve: Vec<DoseResponsePoint>,
    pub control_dose_milli: u32,
    pub control_fitted_mean_milli: i64,
    pub terminal_dose_milli: u32,
    pub terminal_effect_milli: i64,
    pub half_maximal_dose_milli: Option<u32>,
    pub monotonicity_violations: u16,
    pub eligible_dose_count: usize,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: DoseResponseDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DoseResponseError {
    #[error("dose-response request is invalid: {0}")]
    InvalidRequest(String),
    #[error("dose-response observations are invalid: {0}")]
    InvalidObservation(String),
    #[error("dose-response output is invalid: {0}")]
    InvalidOutput(String),
    #[error("dose-response digest failed: {0}")]
    Digest(String),
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn isotonic_fit(means: &[(i64, u64)], direction: DoseDirection) -> Vec<i64> {
    #[derive(Clone, Copy)]
    struct Block {
        sum: i128,
        weight: u64,
    }
    let mut blocks = Vec::<Block>::new();
    let increasing = matches!(direction, DoseDirection::Increasing);
    for (mean, weight) in means {
        let transformed = if increasing { *mean } else { -*mean };
        blocks.push(Block {
            sum: transformed as i128 * *weight as i128,
            weight: *weight,
        });
        while blocks.len() >= 2 {
            let right = blocks[blocks.len() - 1];
            let left = blocks[blocks.len() - 2];
            if left.sum * right.weight as i128 <= right.sum * left.weight as i128 {
                break;
            }
            let merged = Block {
                sum: left.sum + right.sum,
                weight: left.weight + right.weight,
            };
            blocks.pop();
            blocks.pop();
            blocks.push(merged);
        }
    }
    let mut fitted = Vec::with_capacity(means.len());
    let mut block_index = 0;
    let mut remaining = blocks.first().map(|block| block.weight).unwrap_or(0);
    for (_, weight) in means {
        if *weight > remaining {
            block_index += 1;
            remaining = blocks[block_index].weight;
        }
        let block = blocks[block_index];
        let value =
            (block.sum / block.weight as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        fitted.push(if increasing { value } else { -value });
        remaining -= *weight;
    }
    fitted
}

fn digest_input(analysis: &DoseResponseAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": analysis.feature_id,
        "output_schema": analysis.output_schema,
        "objective": analysis.objective,
        "direction": analysis.direction,
        "dose_order": analysis.dose_order,
        "curve": analysis.curve,
        "control_dose_milli": analysis.control_dose_milli,
        "control_fitted_mean_milli": analysis.control_fitted_mean_milli,
        "terminal_dose_milli": analysis.terminal_dose_milli,
        "terminal_effect_milli": analysis.terminal_effect_milli,
        "half_maximal_dose_milli": analysis.half_maximal_dose_milli,
        "monotonicity_violations": analysis.monotonicity_violations,
        "eligible_dose_count": analysis.eligible_dose_count,
        "negative_evidence": analysis.negative_evidence,
        "uncertainty": analysis.uncertainty,
        "disposition": analysis.disposition,
    })
}

impl DoseResponseAnalysis {
    pub fn validate(&self) -> Result<(), DoseResponseError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.dose_order.len() != self.curve.len()
            || self.dose_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.curve.iter().any(|point| point.observation_count == 0)
            || self.eligible_dose_count > self.dose_order.len()
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(DoseResponseError::InvalidOutput(
                "identity, dose ordering, counts, or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DoseResponseError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DoseResponseError::InvalidOutput(
                "digest is not bound to the dose-response analysis".into(),
            ));
        }
        Ok(())
    }
}

/// Fit a bounded monotone dose-response curve and report a reproducible half-maximal dose.
pub fn analyze_glioma_dose_response(
    request: &DoseResponseRequest,
    observations: &[DoseResponseObservation],
) -> Result<DoseResponseAnalysis, DoseResponseError> {
    if request.objective.trim().is_empty()
        || request.min_observations_per_dose == 0
        || request.min_dose_levels < 2
        || request.min_dose_levels > MAX_DOSE_LEVELS
        || request.effect_threshold_milli == 0
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(DoseResponseError::InvalidRequest(
            "objective, dose/replicate floors, effect threshold, and bounded observations are required".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut grouped = BTreeMap::<u32, Vec<&DoseResponseObservation>>::new();
    let mut unit_dose = BTreeSet::<(String, u32)>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.model_system != request.model_system
            || !observation_ids.insert(observation.observation_id.clone())
            || !unit_dose.insert((observation.unit_id.clone(), observation.dose_milli))
        {
            return Err(DoseResponseError::InvalidObservation(
                "observation identity, model, batch, uniqueness, or unit-dose binding is invalid"
                    .into(),
            ));
        }
        grouped
            .entry(observation.dose_milli)
            .or_default()
            .push(observation);
    }
    if grouped.len() > MAX_DOSE_LEVELS {
        return Err(DoseResponseError::InvalidObservation(
            "dose-level count exceeds bounded curve capacity".into(),
        ));
    }
    if !grouped.contains_key(&request.control_dose_milli) {
        return Err(DoseResponseError::InvalidObservation(
            "declared control dose is absent".into(),
        ));
    }
    let dose_order = grouped.keys().copied().collect::<Vec<_>>();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if dose_order.len() < request.min_dose_levels {
        negative.insert("minimum-dose-level-floor-not-met".into());
    }
    let means = dose_order
        .iter()
        .map(|dose| {
            let values = &grouped[dose];
            let mean = values
                .iter()
                .map(|observation| observation.outcome_milli as i128)
                .sum::<i128>()
                / values.len() as i128;
            (mean as i64, values.len() as u64)
        })
        .collect::<Vec<_>>();
    let fitted = isotonic_fit(&means, request.direction);
    let observed_direction_violations = means
        .windows(2)
        .filter(|pair| match request.direction {
            DoseDirection::Increasing => pair[1].0 < pair[0].0,
            DoseDirection::Decreasing => pair[1].0 > pair[0].0,
        })
        .count()
        .min(u16::MAX as usize) as u16;
    if observed_direction_violations > request.max_monotonicity_violations {
        negative.insert("observed-dose-order-violations-exceed-tolerance".into());
    }
    let mut curve = Vec::with_capacity(dose_order.len());
    for (index, dose) in dose_order.iter().enumerate() {
        let observations_at_dose = &grouped[dose];
        let mut residuals = observations_at_dose
            .iter()
            .map(|observation| {
                (observation.outcome_milli as i128 - fitted[index] as i128).unsigned_abs() as u64
            })
            .collect::<Vec<_>>();
        let residual_mad = median(&mut residuals);
        if residual_mad > request.max_residual_milli {
            uncertainty.insert(format!("dose-{dose}-residual-noise-exceeds-tolerance"));
        }
        let mut observation_order = observations_at_dose
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect::<Vec<_>>();
        observation_order.sort();
        curve.push(DoseResponsePoint {
            dose_milli: *dose,
            observation_order,
            observation_count: observations_at_dose.len(),
            observed_mean_milli: means[index].0,
            fitted_mean_milli: fitted[index],
            residual_mad_milli: residual_mad,
        });
    }
    let insufficient_dose_order = curve
        .iter()
        .filter(|point| point.observation_count < request.min_observations_per_dose)
        .map(|point| point.dose_milli)
        .collect::<Vec<_>>();
    if !insufficient_dose_order.is_empty() {
        uncertainty.insert("one-or-more-dose-levels-below-replicate-floor".into());
    }
    let control_index = dose_order
        .iter()
        .position(|dose| *dose == request.control_dose_milli)
        .expect("control dose checked above");
    // The terminal dose is the highest declared dose in either monotonic direction.  The
    // direction changes the expected response ordering, not which physical dose is terminal.
    let terminal_index = dose_order.len() - 1;
    let control_fitted = fitted[control_index];
    let terminal_effect = fitted[terminal_index].saturating_sub(control_fitted);
    let terminal_dose = dose_order[terminal_index];
    let target = control_fitted as i128 + terminal_effect as i128 / 2;
    let mut half_maximal = None;
    for pair in curve.windows(2) {
        let first = pair[0].fitted_mean_milli as i128;
        let second = pair[1].fitted_mean_milli as i128;
        if (first <= target && target <= second) || (second <= target && target <= first) {
            let denominator = second - first;
            if denominator != 0 {
                let numerator = target - first;
                let dose_delta = pair[1].dose_milli as i128 - pair[0].dose_milli as i128;
                let interpolated =
                    pair[0].dose_milli as i128 + numerator * dose_delta / denominator;
                if interpolated >= 0 && interpolated <= u32::MAX as i128 {
                    half_maximal = Some(interpolated as u32);
                    break;
                }
            }
        }
    }
    if half_maximal.is_none() && terminal_effect != 0 {
        uncertainty.insert("half-maximal-dose-not-identified-on-declared-grid".into());
    }
    let disposition =
        if dose_order.len() < request.min_dose_levels || !insufficient_dose_order.is_empty() {
            DoseResponseDisposition::Unresolved
        } else if terminal_effect.unsigned_abs() < request.effect_threshold_milli
            || observed_direction_violations > request.max_monotonicity_violations
        {
            negative.insert("terminal-dose-effect-below-threshold-or-non-monotone".into());
            DoseResponseDisposition::Negative
        } else {
            DoseResponseDisposition::Qualified
        };
    let mut analysis = DoseResponseAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        direction: request.direction,
        dose_order,
        curve,
        control_dose_milli: request.control_dose_milli,
        control_fitted_mean_milli: control_fitted,
        terminal_dose_milli: terminal_dose,
        terminal_effect_milli: terminal_effect,
        half_maximal_dose_milli: half_maximal,
        monotonicity_violations: observed_direction_violations,
        eligible_dose_count: observations
            .iter()
            .map(|observation| observation.dose_milli)
            .collect::<BTreeSet<_>>()
            .len()
            .saturating_sub(insufficient_dose_order.len()),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| DoseResponseError::Digest(error.to_string()))?,
    };
    analysis.digest = ContentHash::of_value(&digest_input(&analysis))
        .map_err(|error| DoseResponseError::Digest(error.to_string()))?;
    analysis.validate()?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observations(values: &[(u32, i64)]) -> Vec<DoseResponseObservation> {
        values
            .iter()
            .enumerate()
            .map(|(index, (dose, outcome))| DoseResponseObservation {
                observation_id: format!("obs-{index}"),
                unit_id: format!("unit-{index}"),
                model_system: GliomaModelSystem::Organoid,
                batch_id: format!("batch-{index}"),
                dose_milli: *dose,
                outcome_milli: *outcome,
            })
            .collect()
    }

    fn request() -> DoseResponseRequest {
        DoseResponseRequest {
            objective: "map a glioma invasion dose response".into(),
            model_system: GliomaModelSystem::Organoid,
            control_dose_milli: 0,
            direction: DoseDirection::Increasing,
            min_observations_per_dose: 1,
            min_dose_levels: 3,
            effect_threshold_milli: 100,
            max_residual_milli: 0,
            max_monotonicity_violations: 0,
        }
    }

    #[test]
    fn monotone_curve_is_fit_and_half_maximal_dose_is_interpolated() {
        let output = analyze_glioma_dose_response(
            &request(),
            &observations(&[(0, 100), (10, 200), (20, 300)]),
        )
        .unwrap();
        assert_eq!(output.disposition, DoseResponseDisposition::Qualified);
        assert_eq!(output.half_maximal_dose_milli, Some(10));
        assert_eq!(output.terminal_effect_milli, 200);
        output.validate().unwrap();
    }

    #[test]
    fn non_monotone_curve_is_negative_not_silently_repaired() {
        let output = analyze_glioma_dose_response(
            &request(),
            &observations(&[(0, 100), (10, 300), (20, 150)]),
        )
        .unwrap();
        assert_eq!(output.disposition, DoseResponseDisposition::Negative);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("violations")));
        assert_eq!(output.curve[2].fitted_mean_milli, 225);
    }

    #[test]
    fn missing_replicate_floor_is_unresolved() {
        let mut request = request();
        request.min_observations_per_dose = 2;
        let output = analyze_glioma_dose_response(
            &request,
            &observations(&[(0, 100), (10, 200), (20, 300)]),
        )
        .unwrap();
        assert_eq!(output.disposition, DoseResponseDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("replicate-floor")));
    }

    #[test]
    fn decreasing_curve_uses_the_highest_declared_terminal_dose() {
        let mut request = request();
        request.direction = DoseDirection::Decreasing;
        let output = analyze_glioma_dose_response(
            &request,
            &observations(&[(0, 300), (10, 200), (20, 100)]),
        )
        .unwrap();
        assert_eq!(output.disposition, DoseResponseDisposition::Qualified);
        assert_eq!(output.terminal_dose_milli, 20);
        assert_eq!(output.terminal_effect_milli, -200);
        assert_eq!(output.half_maximal_dose_milli, Some(10));
    }
}
