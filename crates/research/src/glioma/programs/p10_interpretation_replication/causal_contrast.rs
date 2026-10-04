//! Exact, bounded difference-in-differences interpretation for preclinical glioma studies.
//!
//! This feature estimates whether a treatment-associated change differs from a control change
//! across a declared intervention boundary.  It uses integer means, an exact treatment-label
//! permutation test when the unit count is bounded, and leave-one-unit-out effect bounds.  The
//! output is a research interpretation object: it does not infer patient benefit, select a dose,
//! or make a clinical decision.

use super::trajectory::TrajectoryObservation;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaCausalContrast1@1";
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_UNITS: usize = 4_096;
pub const MAX_EXACT_PERMUTATION_UNITS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalContrastRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub intervention_timepoint: u32,
    pub min_units_per_arm: usize,
    pub effect_threshold_milli: u64,
    pub alpha_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitContrast {
    pub unit_id: String,
    pub arm_id: String,
    pub baseline_mean_milli: i64,
    pub post_mean_milli: i64,
    pub change_milli: i64,
    pub baseline_timepoint_order: Vec<u32>,
    pub post_timepoint_order: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalContrastDisposition {
    Qualified,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalContrastAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub intervention_timepoint: u32,
    pub unit_order: Vec<String>,
    pub contrasts: Vec<UnitContrast>,
    pub unresolved_unit_order: Vec<String>,
    pub control_change_milli: i64,
    pub treatment_change_milli: i64,
    pub difference_in_differences_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub permutation_p_milli: Option<u16>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CausalContrastDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CausalContrastError {
    #[error("causal-contrast request is invalid: {0}")]
    InvalidRequest(String),
    #[error("causal-contrast observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("causal-contrast output is invalid: {0}")]
    InvalidOutput(String),
    #[error("causal-contrast digest failed: {0}")]
    Digest(String),
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn mean(values: &[i64]) -> i64 {
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| *value as i128).sum::<i128>() / values.len() as i128) as i64
    }
}

fn digest_input(output: &CausalContrastAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "intervention_timepoint": output.intervention_timepoint,
        "unit_order": output.unit_order,
        "contrasts": output.contrasts,
        "unresolved_unit_order": output.unresolved_unit_order,
        "control_change_milli": output.control_change_milli,
        "treatment_change_milli": output.treatment_change_milli,
        "difference_in_differences_milli": output.difference_in_differences_milli,
        "interval_low_milli": output.interval_low_milli,
        "interval_high_milli": output.interval_high_milli,
        "permutation_p_milli": output.permutation_p_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl CausalContrastAnalysis {
    pub fn validate(&self) -> Result<(), CausalContrastError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || !ordered_unique(&self.unit_order)
            || !ordered_unique(&self.unresolved_unit_order)
            || !ordered_unique(&self.negative_evidence)
            || !ordered_unique(&self.uncertainty)
            || self.contrasts.iter().any(|contrast| {
                contrast.unit_id.trim().is_empty()
                    || contrast.arm_id != self.control_arm && contrast.arm_id != self.treatment_arm
                    || contrast.baseline_timepoint_order.is_empty()
                    || contrast.post_timepoint_order.is_empty()
                    || !ordered_unique(&contrast.baseline_timepoint_order)
                    || !ordered_unique(&contrast.post_timepoint_order)
            })
            || self.interval_low_milli > self.interval_high_milli
            || self.permutation_p_milli.is_some_and(|value| value > 1_000)
        {
            return Err(CausalContrastError::InvalidOutput(
                "identity, unit partition, ordering, interval, or p-value bounds are invalid"
                    .into(),
            ));
        }
        let contrast_ids = self
            .contrasts
            .iter()
            .map(|contrast| contrast.unit_id.clone())
            .collect::<BTreeSet<_>>();
        let unresolved_ids = self
            .unresolved_unit_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let all_ids = self.unit_order.iter().cloned().collect::<BTreeSet<_>>();
        if contrast_ids.len() != self.contrasts.len()
            || contrast_ids.intersection(&unresolved_ids).next().is_some()
            || contrast_ids
                .union(&unresolved_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != all_ids
        {
            return Err(CausalContrastError::InvalidOutput(
                "eligible and unresolved units do not partition unit_order".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CausalContrastError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CausalContrastError::InvalidOutput(
                "digest is not bound to causal contrast".into(),
            ));
        }
        Ok(())
    }
}

fn exact_permutation_p_milli(
    changes: &[(String, i64)],
    treatment_count: usize,
    observed: i64,
) -> u16 {
    let n = changes.len();
    if n == 0 || n > MAX_EXACT_PERMUTATION_UNITS || treatment_count == 0 || treatment_count == n {
        return 1_000;
    }
    let total = changes
        .iter()
        .map(|(_, value)| *value as i128)
        .sum::<i128>();
    let mut combinations = 0_u64;
    let mut extreme = 0_u64;
    let limit = 1_u64 << n;
    for mask in 0..limit {
        if mask.count_ones() as usize != treatment_count {
            continue;
        }
        let treatment_sum = changes
            .iter()
            .enumerate()
            .filter(|(index, _)| (mask & (1_u64 << index)) != 0)
            .map(|(_, (_, value))| *value as i128)
            .sum::<i128>();
        let control_sum = total - treatment_sum;
        let statistic = (treatment_sum / treatment_count as i128
            - control_sum / (n - treatment_count) as i128)
            .abs() as i64;
        combinations += 1;
        if statistic >= observed.abs() {
            extreme += 1;
        }
    }
    if combinations == 0 {
        1_000
    } else {
        (extreme
            .saturating_mul(1_000)
            .checked_div(combinations)
            .unwrap_or(1_000)
            .min(1_000)) as u16
    }
}

pub fn analyze_glioma_causal_contrast(
    request: &CausalContrastRequest,
    observations: &[TrajectoryObservation],
) -> Result<CausalContrastAnalysis, CausalContrastError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_units_per_arm == 0
        || request.alpha_milli > 1_000
        || request.effect_threshold_milli > i64::MAX as u64
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(CausalContrastError::InvalidRequest(
            "objective, distinct arms, unit floor, alpha, effect threshold, or observation bound is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut units = BTreeMap::<String, Vec<&TrajectoryObservation>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.arm_id != request.control_arm
                && observation.arm_id != request.treatment_arm
            || observation.model_system != request.model_system
            || observation.batch_id.trim().is_empty()
            || observation.outcome_milli.abs() > 1_000_000_000_000
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(CausalContrastError::InvalidObservation(
                "observation identity, arm/model binding, outcome bound, or uniqueness is invalid"
                    .into(),
            ));
        }
        units
            .entry(observation.unit_id.clone())
            .or_default()
            .push(observation);
    }
    if units.len() > MAX_UNITS {
        return Err(CausalContrastError::InvalidObservation(
            "unit bound exceeded".into(),
        ));
    }
    let unit_order = units.keys().cloned().collect::<Vec<_>>();
    let mut contrasts = Vec::new();
    let mut unresolved_unit_order = BTreeSet::new();
    for unit_id in &unit_order {
        let values = &units[unit_id];
        let mut timepoints = BTreeSet::new();
        for observation in values {
            if !timepoints.insert(observation.timepoint) {
                return Err(CausalContrastError::InvalidObservation(
                    "a unit contains duplicate timepoints".into(),
                ));
            }
        }
        let baseline = values
            .iter()
            .filter(|observation| observation.timepoint < request.intervention_timepoint)
            .collect::<Vec<_>>();
        let post = values
            .iter()
            .filter(|observation| observation.timepoint >= request.intervention_timepoint)
            .collect::<Vec<_>>();
        if baseline.is_empty() || post.is_empty() {
            unresolved_unit_order.insert(unit_id.clone());
            continue;
        }
        let baseline_values = baseline
            .iter()
            .map(|observation| observation.outcome_milli)
            .collect::<Vec<_>>();
        let post_values = post
            .iter()
            .map(|observation| observation.outcome_milli)
            .collect::<Vec<_>>();
        let baseline_mean_milli = mean(&baseline_values);
        let post_mean_milli = mean(&post_values);
        contrasts.push(UnitContrast {
            unit_id: unit_id.clone(),
            arm_id: values[0].arm_id.clone(),
            baseline_mean_milli,
            post_mean_milli,
            change_milli: post_mean_milli - baseline_mean_milli,
            baseline_timepoint_order: baseline
                .iter()
                .map(|observation| observation.timepoint)
                .collect(),
            post_timepoint_order: post
                .iter()
                .map(|observation| observation.timepoint)
                .collect(),
        });
    }
    let unresolved_unit_order = unresolved_unit_order.into_iter().collect::<Vec<_>>();
    let control = contrasts
        .iter()
        .filter(|contrast| contrast.arm_id == request.control_arm)
        .collect::<Vec<_>>();
    let treatment = contrasts
        .iter()
        .filter(|contrast| contrast.arm_id == request.treatment_arm)
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    if control.len() < request.min_units_per_arm {
        uncertainty.insert("control-unit-floor-not-met".into());
    }
    if treatment.len() < request.min_units_per_arm {
        uncertainty.insert("treatment-unit-floor-not-met".into());
    }
    if !unresolved_unit_order.is_empty() {
        uncertainty.insert("unit-missing-baseline-or-post-window".into());
    }
    let control_change_milli = mean(
        &control
            .iter()
            .map(|contrast| contrast.change_milli)
            .collect::<Vec<_>>(),
    );
    let treatment_change_milli = mean(
        &treatment
            .iter()
            .map(|contrast| contrast.change_milli)
            .collect::<Vec<_>>(),
    );
    let difference_in_differences_milli = treatment_change_milli - control_change_milli;
    let eligible =
        control.len() >= request.min_units_per_arm && treatment.len() >= request.min_units_per_arm;
    let mut interval_low_milli = difference_in_differences_milli;
    let mut interval_high_milli = difference_in_differences_milli;
    if eligible && control.len() + treatment.len() > 2 {
        let mut jackknife = Vec::new();
        for dropped in &contrasts {
            let control_values = control
                .iter()
                .filter(|contrast| contrast.unit_id != dropped.unit_id)
                .map(|contrast| contrast.change_milli)
                .collect::<Vec<_>>();
            let treatment_values = treatment
                .iter()
                .filter(|contrast| contrast.unit_id != dropped.unit_id)
                .map(|contrast| contrast.change_milli)
                .collect::<Vec<_>>();
            if !control_values.is_empty() && !treatment_values.is_empty() {
                jackknife.push(mean(&treatment_values) - mean(&control_values));
            }
        }
        if let Some(minimum) = jackknife.iter().min() {
            interval_low_milli = *minimum;
        }
        if let Some(maximum) = jackknife.iter().max() {
            interval_high_milli = *maximum;
        }
    }
    let permutation_p_milli = if eligible {
        let changes = contrasts
            .iter()
            .map(|contrast| (contrast.unit_id.clone(), contrast.change_milli))
            .collect::<Vec<_>>();
        if changes.len() <= MAX_EXACT_PERMUTATION_UNITS {
            Some(exact_permutation_p_milli(
                &changes,
                treatment.len(),
                difference_in_differences_milli,
            ))
        } else {
            uncertainty.insert("exact-permutation-unit-cap-exceeded".into());
            None
        }
    } else {
        None
    };
    let mut negative_evidence = BTreeSet::new();
    if difference_in_differences_milli.unsigned_abs() < request.effect_threshold_milli {
        negative_evidence.insert("difference-in-differences-below-threshold".into());
    }
    if let Some(p_value) = permutation_p_milli {
        if p_value > request.alpha_milli {
            negative_evidence.insert("permutation-p-above-alpha".into());
        }
    }
    let disposition = if !eligible || permutation_p_milli.is_none() {
        CausalContrastDisposition::Unresolved
    } else if !negative_evidence.is_empty() {
        CausalContrastDisposition::Negative
    } else {
        CausalContrastDisposition::Qualified
    };
    let mut output = CausalContrastAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        intervention_timepoint: request.intervention_timepoint,
        unit_order,
        contrasts,
        unresolved_unit_order,
        control_change_milli,
        treatment_change_milli,
        difference_in_differences_milli,
        interval_low_milli,
        interval_high_milli,
        permutation_p_milli,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| CausalContrastError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CausalContrastError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        id: &str,
        unit: &str,
        arm: &str,
        timepoint: u32,
        outcome: i64,
    ) -> TrajectoryObservation {
        TrajectoryObservation {
            observation_id: id.into(),
            unit_id: unit.into(),
            arm_id: arm.into(),
            model_system: GliomaModelSystem::Organoid,
            batch_id: format!("batch-{id}"),
            timepoint,
            outcome_milli: outcome,
        }
    }

    fn request() -> CausalContrastRequest {
        CausalContrastRequest {
            objective: "estimate treatment-associated invasion change".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            intervention_timepoint: 1,
            min_units_per_arm: 2,
            effect_threshold_milli: 5,
            alpha_milli: 500,
        }
    }

    #[test]
    fn difference_in_differences_is_significant_and_replay_stable() {
        let observations = vec![
            observation("c0-pre", "c0", "control", 0, 100),
            observation("c0-post", "c0", "control", 1, 101),
            observation("c1-pre", "c1", "control", 0, 100),
            observation("c1-post", "c1", "control", 1, 101),
            observation("t0-pre", "t0", "treated", 0, 100),
            observation("t0-post", "t0", "treated", 1, 120),
            observation("t1-pre", "t1", "treated", 0, 100),
            observation("t1-post", "t1", "treated", 1, 120),
        ];
        let first = analyze_glioma_causal_contrast(&request(), &observations).unwrap();
        let second = analyze_glioma_causal_contrast(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.difference_in_differences_milli, 19);
        assert_eq!(first.disposition, CausalContrastDisposition::Qualified);
        assert!(first.permutation_p_milli.unwrap() <= 500);
    }

    #[test]
    fn missing_window_is_unresolved_not_imputed() {
        let mut observations = vec![
            observation("c0-pre", "c0", "control", 0, 100),
            observation("c0-post", "c0", "control", 1, 101),
            observation("c1-pre", "c1", "control", 0, 100),
            observation("c1-post", "c1", "control", 1, 101),
            observation("t0-pre", "t0", "treated", 0, 100),
            observation("t1-pre", "t1", "treated", 0, 100),
        ];
        let output = analyze_glioma_causal_contrast(&request(), &observations).unwrap();
        assert_eq!(output.disposition, CausalContrastDisposition::Unresolved);
        assert_eq!(output.permutation_p_milli, None);
        assert_eq!(output.unresolved_unit_order, vec!["t0", "t1"]);
        observations.push(observation("duplicate", "c0", "control", 0, 100));
        assert!(analyze_glioma_causal_contrast(&request(), &observations).is_err());
    }
}
