//! Unmeasured-confounding sensitivity bounds for preclinical glioma effects.
//!
//! This feature does not claim that a confounder exists. It quantifies the declared hidden-bias
//! budget that would be needed to erase an observed arm contrast. A normalized confounder score is
//! compared between arms, the resulting worst-case bias is swept over a deterministic strength
//! grid, and leave-one-unit-out shifts are reported alongside the tipping point. The output is an
//! interpretation gate for a research claim, never a clinical effect or treatment recommendation.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaCausalSensitivity1@1";
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_POINTS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityDirection {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivityRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub expected_direction: SensitivityDirection,
    pub min_units_per_arm: usize,
    pub effect_threshold_milli: u64,
    pub max_confounder_strength_milli: u64,
    pub strength_step_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivityObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub outcome_milli: i64,
    pub confounder_score_milli: i64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivityPoint {
    pub strength_milli: u64,
    pub bias_bound_milli: u64,
    pub adjusted_low_milli: i64,
    pub adjusted_high_milli: i64,
    pub threshold_holds: bool,
    pub sign_holds: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalSensitivityAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub expected_direction: SensitivityDirection,
    pub control_unit_order: Vec<String>,
    pub treatment_unit_order: Vec<String>,
    pub control_mean_milli: i64,
    pub treatment_mean_milli: i64,
    pub observed_effect_milli: i64,
    pub confounder_imbalance_milli: i64,
    pub leave_one_out_low_milli: i64,
    pub leave_one_out_high_milli: i64,
    pub leave_one_out_shift_milli: u64,
    pub point_order: Vec<u64>,
    pub points: Vec<SensitivityPoint>,
    pub tipping_strength_milli: Option<u64>,
    pub max_robust_strength_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SensitivityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SensitivityError {
    #[error("causal sensitivity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("causal sensitivity observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("causal sensitivity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("causal sensitivity digest failed: {0}")]
    Digest(String),
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn mean(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    (values.iter().map(|value| i128::from(*value)).sum::<i128>()
        / i128::try_from(values.len()).unwrap_or(1))
    .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn diff_mean(values: &[(String, i64)]) -> i64 {
    mean(&values.iter().map(|(_, value)| *value).collect::<Vec<_>>())
}

fn signed_abs_diff(left: i64, right: i64) -> u64 {
    i128::from(left)
        .saturating_sub(i128::from(right))
        .unsigned_abs()
        .min(u128::from(u64::MAX)) as u64
}

fn bias_bound(strength: u64, imbalance: i64) -> u64 {
    u128::from(strength)
        .saturating_mul(u128::from(imbalance.unsigned_abs()))
        .checked_div(1_000)
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64
}

fn digest_input(output: &CausalSensitivityAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "model_system": output.model_system,
        "expected_direction": output.expected_direction,
        "control_unit_order": output.control_unit_order,
        "treatment_unit_order": output.treatment_unit_order,
        "control_mean_milli": output.control_mean_milli,
        "treatment_mean_milli": output.treatment_mean_milli,
        "observed_effect_milli": output.observed_effect_milli,
        "confounder_imbalance_milli": output.confounder_imbalance_milli,
        "leave_one_out_low_milli": output.leave_one_out_low_milli,
        "leave_one_out_high_milli": output.leave_one_out_high_milli,
        "leave_one_out_shift_milli": output.leave_one_out_shift_milli,
        "point_order": output.point_order,
        "points": output.points,
        "tipping_strength_milli": output.tipping_strength_milli,
        "max_robust_strength_milli": output.max_robust_strength_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl CausalSensitivityAnalysis {
    pub fn validate(&self) -> Result<(), SensitivityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || !ordered_unique(&self.control_unit_order)
            || !ordered_unique(&self.treatment_unit_order)
            || !ordered_unique(&self.point_order)
            || !ordered_unique(&self.negative_evidence)
            || !ordered_unique(&self.uncertainty)
            || self.points.len() != self.point_order.len()
            || self.points.windows(2).any(|pair| {
                pair[0].strength_milli >= pair[1].strength_milli
                    || pair[0].adjusted_low_milli > pair[0].adjusted_high_milli
                    || pair[0].bias_bound_milli > u64::MAX / 2
            })
            || self.points.iter().any(|point| {
                point.adjusted_low_milli > point.adjusted_high_milli
                    || point.strength_milli > 1_000_000_000
            })
            || self.leave_one_out_low_milli > self.leave_one_out_high_milli
        {
            return Err(SensitivityError::InvalidOutput(
                "identity, arm/unit ordering, sensitivity grid, or interval bounds are invalid"
                    .into(),
            ));
        }
        let expected_points = self
            .points
            .iter()
            .map(|point| point.strength_milli)
            .collect::<Vec<_>>();
        if expected_points != self.point_order
            || self
                .points
                .first()
                .is_none_or(|point| point.strength_milli != 0)
        {
            return Err(SensitivityError::InvalidOutput(
                "sensitivity point order does not reconcile from zero strength".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SensitivityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SensitivityError::InvalidOutput(
                "digest is not bound to causal sensitivity analysis".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_causal_sensitivity(
    request: &SensitivityRequest,
    observations: &[SensitivityObservation],
) -> Result<CausalSensitivityAnalysis, SensitivityError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_units_per_arm < 2
        || request.strength_step_milli == 0
        || request.max_confounder_strength_milli > 1_000_000_000
        || request.strength_step_milli > request.max_confounder_strength_milli
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(SensitivityError::InvalidRequest(
            "objective, distinct arms, unit floor, bounded confounder grid, and observations are required".into(),
        ));
    }
    let mut seen_observations = BTreeSet::new();
    let mut seen_units = BTreeSet::new();
    let mut arms = BTreeMap::<String, Vec<(String, i64, i64)>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.arm_id.trim().is_empty()
            || observation.model_system != request.model_system
            || observation.artifact.validate().is_err()
            || !observation.artifact.local_only
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
            || observation.confounder_score_milli.unsigned_abs() > 1_000
            || !seen_observations.insert(observation.observation_id.clone())
            || !seen_units.insert(observation.unit_id.clone())
        {
            return Err(SensitivityError::InvalidObservation(
                "observation identity, arm/model binding, local privacy posture, unit uniqueness, or normalized confounder bounds are invalid".into(),
            ));
        }
        arms.entry(observation.arm_id.clone()).or_default().push((
            observation.unit_id.clone(),
            observation.outcome_milli,
            observation.confounder_score_milli,
        ));
    }
    let control = arms.remove(&request.control_arm).unwrap_or_default();
    let treatment = arms.remove(&request.treatment_arm).unwrap_or_default();
    let control_unit_order = control
        .iter()
        .map(|(unit, _, _)| unit.clone())
        .collect::<Vec<_>>();
    let treatment_unit_order = treatment
        .iter()
        .map(|(unit, _, _)| unit.clone())
        .collect::<Vec<_>>();
    let control_mean = diff_mean(
        &control
            .iter()
            .map(|(unit, outcome, _)| (unit.clone(), *outcome))
            .collect::<Vec<_>>(),
    );
    let treatment_mean = diff_mean(
        &treatment
            .iter()
            .map(|(unit, outcome, _)| (unit.clone(), *outcome))
            .collect::<Vec<_>>(),
    );
    let observed_effect = treatment_mean.saturating_sub(control_mean);
    let control_confounder = diff_mean(
        &control
            .iter()
            .map(|(unit, _, score)| (unit.clone(), *score))
            .collect::<Vec<_>>(),
    );
    let treatment_confounder = diff_mean(
        &treatment
            .iter()
            .map(|(unit, _, score)| (unit.clone(), *score))
            .collect::<Vec<_>>(),
    );
    let confounder_imbalance = treatment_confounder.saturating_sub(control_confounder);
    let mut loo_effects = Vec::new();
    for (index, _) in control.iter().enumerate() {
        if control.len().saturating_sub(1) < request.min_units_per_arm {
            break;
        }
        let remaining_control = control
            .iter()
            .enumerate()
            .filter(|(candidate, _)| *candidate != index)
            .map(|(_, (_, outcome, _))| *outcome)
            .collect::<Vec<_>>();
        loo_effects.push(treatment_mean.saturating_sub(mean(&remaining_control)));
    }
    for (index, _) in treatment.iter().enumerate() {
        if treatment.len().saturating_sub(1) < request.min_units_per_arm {
            break;
        }
        let remaining_treatment = treatment
            .iter()
            .enumerate()
            .filter(|(candidate, _)| *candidate != index)
            .map(|(_, (_, outcome, _))| *outcome)
            .collect::<Vec<_>>();
        loo_effects.push(mean(&remaining_treatment).saturating_sub(control_mean));
    }
    let mut uncertainty = BTreeSet::new();
    if loo_effects.is_empty() {
        uncertainty.insert("leave-one-unit-out-floor-not-met".into());
    }
    let leave_one_out_low = loo_effects.iter().copied().min().unwrap_or(observed_effect);
    let leave_one_out_high = loo_effects.iter().copied().max().unwrap_or(observed_effect);
    let leave_one_out_shift = loo_effects
        .iter()
        .map(|effect| signed_abs_diff(*effect, observed_effect))
        .max()
        .unwrap_or(0);
    if leave_one_out_shift > request.max_leave_one_out_shift_milli {
        uncertainty.insert("leave-one-unit-out-shift-exceeds-declared-bound".into());
    }
    let mut points = Vec::new();
    let mut strength = 0_u64;
    loop {
        let bias = bias_bound(strength, confounder_imbalance);
        let bias_i64 = i64::try_from(bias).unwrap_or(i64::MAX);
        let low = observed_effect.saturating_sub(bias_i64);
        let high = observed_effect.saturating_add(bias_i64);
        let threshold_holds = match request.expected_direction {
            SensitivityDirection::Positive => low >= request.effect_threshold_milli as i64,
            SensitivityDirection::Negative => high <= -(request.effect_threshold_milli as i64),
        };
        let sign_holds = match request.expected_direction {
            SensitivityDirection::Positive => low > 0,
            SensitivityDirection::Negative => high < 0,
        };
        points.push(SensitivityPoint {
            strength_milli: strength,
            bias_bound_milli: bias,
            adjusted_low_milli: low,
            adjusted_high_milli: high,
            threshold_holds,
            sign_holds,
        });
        if strength >= request.max_confounder_strength_milli {
            break;
        }
        strength = strength
            .saturating_add(request.strength_step_milli)
            .min(request.max_confounder_strength_milli);
        if points.len() >= MAX_POINTS {
            return Err(SensitivityError::InvalidRequest(
                "confounder sensitivity grid exceeds point limit".into(),
            ));
        }
    }
    let tipping_strength = points
        .iter()
        .find(|point| !point.threshold_holds)
        .map(|point| point.strength_milli);
    let max_robust_strength = points
        .iter()
        .filter(|point| point.threshold_holds)
        .map(|point| point.strength_milli)
        .max()
        .unwrap_or(0);
    let observed_holds = points.first().is_some_and(|point| point.threshold_holds);
    let mut negative_evidence = BTreeSet::new();
    if !observed_holds {
        negative_evidence.insert("observed-effect-does-not-meet-declared-threshold".into());
    }
    if points.iter().any(|point| !point.sign_holds) {
        negative_evidence.insert("effect-sign-is-not-robust-to-declared-confounding".into());
    }
    if tipping_strength.is_some() {
        negative_evidence.insert("threshold-tipping-point-reached-within-declared-budget".into());
    }
    let unit_floor_met =
        control.len() >= request.min_units_per_arm && treatment.len() >= request.min_units_per_arm;
    if !unit_floor_met {
        uncertainty.insert("minimum-units-per-arm-not-met".into());
    }
    let disposition = if !unit_floor_met {
        SensitivityDisposition::Unresolved
    } else if !observed_holds {
        SensitivityDisposition::Negative
    } else if tipping_strength.is_some() || !uncertainty.is_empty() {
        SensitivityDisposition::Partial
    } else {
        SensitivityDisposition::Qualified
    };
    let mut output = CausalSensitivityAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        model_system: request.model_system,
        expected_direction: request.expected_direction,
        control_unit_order,
        treatment_unit_order,
        control_mean_milli: control_mean,
        treatment_mean_milli: treatment_mean,
        observed_effect_milli: observed_effect,
        confounder_imbalance_milli: confounder_imbalance,
        leave_one_out_low_milli: leave_one_out_low,
        leave_one_out_high_milli: leave_one_out_high,
        leave_one_out_shift_milli: leave_one_out_shift,
        point_order: points.iter().map(|point| point.strength_milli).collect(),
        points,
        tipping_strength_milli: tipping_strength,
        max_robust_strength_milli: max_robust_strength,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| SensitivityError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SensitivityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("sensitivity-artifact-{id}"),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-sensitivity+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn observation(
        id: &str,
        unit: &str,
        arm: &str,
        outcome: i64,
        score: i64,
    ) -> SensitivityObservation {
        SensitivityObservation {
            observation_id: id.into(),
            unit_id: unit.into(),
            arm_id: arm.into(),
            model_system: GliomaModelSystem::Organoid,
            outcome_milli: outcome,
            confounder_score_milli: score,
            artifact: artifact(id),
        }
    }

    fn request() -> SensitivityRequest {
        SensitivityRequest {
            objective: "stress an invasion mechanism effect".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            expected_direction: SensitivityDirection::Positive,
            min_units_per_arm: 2,
            effect_threshold_milli: 100,
            max_confounder_strength_milli: 400,
            strength_step_milli: 100,
            max_leave_one_out_shift_milli: 100,
        }
    }

    #[test]
    fn sensitivity_sweeps_tipping_point_and_is_replay_stable() {
        let observations = vec![
            observation("c1", "c1", "control", 100, -500),
            observation("c2", "c2", "control", 110, -400),
            observation("t1", "t1", "treated", 300, 400),
            observation("t2", "t2", "treated", 310, 500),
        ];
        let first = analyze_causal_sensitivity(&request(), &observations).unwrap();
        let second = analyze_causal_sensitivity(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.observed_effect_milli, 200);
        assert_eq!(first.tipping_strength_milli, Some(200));
        assert_eq!(first.max_robust_strength_milli, 100);
        assert_eq!(first.disposition, SensitivityDisposition::Partial);
    }

    #[test]
    fn insufficient_arm_coverage_is_unresolved() {
        let observations = vec![
            observation("c1", "c1", "control", 100, 0),
            observation("t1", "t1", "treated", 300, 0),
        ];
        let output = analyze_causal_sensitivity(&request(), &observations).unwrap();
        assert_eq!(output.disposition, SensitivityDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|reason| reason == "minimum-units-per-arm-not-met"));
    }

    #[test]
    fn negative_effect_remains_negative_for_negative_direction() {
        let mut request = request();
        request.expected_direction = SensitivityDirection::Negative;
        request.effect_threshold_milli = 50;
        request.max_confounder_strength_milli = 100;
        let observations = vec![
            observation("c1", "c1", "control", 300, 0),
            observation("c2", "c2", "control", 310, 0),
            observation("c3", "c3", "control", 305, 0),
            observation("t1", "t1", "treated", 100, 0),
            observation("t2", "t2", "treated", 110, 0),
            observation("t3", "t3", "treated", 105, 0),
        ];
        let output = analyze_causal_sensitivity(&request, &observations).unwrap();
        assert_eq!(output.disposition, SensitivityDisposition::Qualified);
        assert!(output.points.iter().all(|point| point.threshold_holds));
    }
}
