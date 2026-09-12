//! Deterministic causal-mediation analysis for preclinical glioma studies.
//!
//! The analyzer decomposes a declared treatment/control contrast into mediator, total, direct,
//! and indirect components using integer covariance and slope estimates. It is deliberately a
//! bounded interpretation tool: units are never allowed to cross arms, repeated observations are
//! rejected to avoid pseudoreplication, leave-one-unit-out fragility is exposed, and a result is
//! never a patient, diagnostic, or treatment decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaCausalMediation1@1";
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_UNITS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediationRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub min_units_per_arm: usize,
    pub effect_threshold_milli: u64,
    pub min_signal_to_noise_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

/// One independent unit contributes one mediator/outcome pair. The artifact is local and
/// de-identified; the analyzer never reads its payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediationObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub arm_id: String,
    pub mediator_milli: i64,
    pub outcome_milli: i64,
    pub uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediationDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub unit_order: Vec<String>,
    pub control_unit_order: Vec<String>,
    pub treatment_unit_order: Vec<String>,
    pub control_mediator_mean_milli: i64,
    pub treatment_mediator_mean_milli: i64,
    pub control_outcome_mean_milli: i64,
    pub treatment_outcome_mean_milli: i64,
    pub mediator_effect_milli: i64,
    pub total_effect_milli: i64,
    pub mediator_outcome_slope_milli: i64,
    pub indirect_effect_milli: i64,
    pub direct_effect_milli: i64,
    pub indirect_share_milli: u16,
    pub mediator_variance_milli: u64,
    pub residual_noise_milli: u64,
    pub total_signal_to_noise_milli: u64,
    pub indirect_signal_to_noise_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub leave_one_out_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MediationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MediationError {
    #[error("causal-mediation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("causal-mediation observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("causal-mediation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("causal-mediation digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, Copy)]
struct Estimate {
    control_mediator_mean: i64,
    treatment_mediator_mean: i64,
    control_outcome_mean: i64,
    treatment_outcome_mean: i64,
    mediator_effect: i64,
    total_effect: i64,
    slope: i64,
    indirect_effect: i64,
    direct_effect: i64,
    mediator_variance: u64,
    residual_noise: u64,
}

#[derive(Debug, Clone, Copy)]
struct Row<'a> {
    observation: &'a MediationObservation,
}

fn mean(values: &[i64]) -> i64 {
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| i128::from(*value)).sum::<i128>() / values.len() as i128) as i64
    }
}

fn signed_div(numerator: i128, denominator: i128) -> i64 {
    if denominator == 0 {
        0
    } else {
        numerator
            .checked_div(denominator)
            .unwrap_or_else(|| {
                if numerator.is_negative() {
                    i128::from(i64::MIN)
                } else {
                    i128::from(i64::MAX)
                }
            })
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
}

fn abs_i128(value: i128) -> u128 {
    value.unsigned_abs()
}

fn estimate(rows: &[Row<'_>], control_arm: &str, treatment_arm: &str) -> Option<Estimate> {
    let control = rows
        .iter()
        .filter(|row| row.observation.arm_id == control_arm)
        .map(|row| row.observation)
        .collect::<Vec<_>>();
    let treatment = rows
        .iter()
        .filter(|row| row.observation.arm_id == treatment_arm)
        .map(|row| row.observation)
        .collect::<Vec<_>>();
    if control.is_empty() || treatment.is_empty() {
        return None;
    }
    let control_mediator_mean = mean(
        &control
            .iter()
            .map(|observation| observation.mediator_milli)
            .collect::<Vec<_>>(),
    );
    let treatment_mediator_mean = mean(
        &treatment
            .iter()
            .map(|observation| observation.mediator_milli)
            .collect::<Vec<_>>(),
    );
    let control_outcome_mean = mean(
        &control
            .iter()
            .map(|observation| observation.outcome_milli)
            .collect::<Vec<_>>(),
    );
    let treatment_outcome_mean = mean(
        &treatment
            .iter()
            .map(|observation| observation.outcome_milli)
            .collect::<Vec<_>>(),
    );
    let mediator_effect = treatment_mediator_mean.saturating_sub(control_mediator_mean);
    let total_effect = treatment_outcome_mean.saturating_sub(control_outcome_mean);
    let pooled_mediator_mean = mean(
        &rows
            .iter()
            .map(|row| row.observation.mediator_milli)
            .collect::<Vec<_>>(),
    );
    let pooled_outcome_mean = mean(
        &rows
            .iter()
            .map(|row| row.observation.outcome_milli)
            .collect::<Vec<_>>(),
    );
    let mut variance_sum = 0_i128;
    let mut covariance_sum = 0_i128;
    let mut residual_sum = 0_u128;
    let uncertainty_sum = rows
        .iter()
        .map(|row| u128::from(row.observation.uncertainty_milli))
        .sum::<u128>();
    for row in rows {
        let mediator_delta = i128::from(row.observation.mediator_milli)
            .saturating_sub(i128::from(pooled_mediator_mean));
        let outcome_delta = i128::from(row.observation.outcome_milli)
            .saturating_sub(i128::from(pooled_outcome_mean));
        variance_sum = variance_sum.saturating_add(mediator_delta.saturating_mul(mediator_delta));
        covariance_sum =
            covariance_sum.saturating_add(mediator_delta.saturating_mul(outcome_delta));
    }
    if variance_sum == 0 {
        return None;
    }
    let slope = signed_div(covariance_sum.saturating_mul(1_000), variance_sum);
    let indirect_effect = signed_div(
        i128::from(mediator_effect).saturating_mul(i128::from(slope)),
        1_000,
    );
    let direct_effect = total_effect.saturating_sub(indirect_effect);
    for row in rows {
        let fitted = i128::from(pooled_outcome_mean).saturating_add(
            i128::from(slope)
                .saturating_mul(
                    i128::from(row.observation.mediator_milli)
                        .saturating_sub(i128::from(pooled_mediator_mean)),
                )
                .saturating_div(1_000),
        );
        residual_sum = residual_sum
            .saturating_add(abs_i128(i128::from(row.observation.outcome_milli) - fitted));
    }
    let residual_noise = ((residual_sum / rows.len() as u128)
        .max(uncertainty_sum / rows.len() as u128)
        .min(u128::from(u64::MAX))) as u64;
    Some(Estimate {
        control_mediator_mean,
        treatment_mediator_mean,
        control_outcome_mean,
        treatment_outcome_mean,
        mediator_effect,
        total_effect,
        slope,
        indirect_effect,
        direct_effect,
        mediator_variance: (variance_sum / rows.len() as i128).min(i128::from(u64::MAX)) as u64,
        residual_noise,
    })
}

fn signal_to_noise(effect: i64, noise: u64) -> u64 {
    if noise == 0 {
        if effect == 0 {
            0
        } else {
            u64::MAX
        }
    } else {
        (u128::from(effect.unsigned_abs())
            .saturating_mul(1_000)
            .checked_div(u128::from(noise))
            .unwrap_or(0)
            .min(u128::from(u64::MAX))) as u64
    }
}

fn digest_input(output: &MediationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "model_system": output.model_system,
        "unit_order": output.unit_order,
        "control_unit_order": output.control_unit_order,
        "treatment_unit_order": output.treatment_unit_order,
        "control_mediator_mean_milli": output.control_mediator_mean_milli,
        "treatment_mediator_mean_milli": output.treatment_mediator_mean_milli,
        "control_outcome_mean_milli": output.control_outcome_mean_milli,
        "treatment_outcome_mean_milli": output.treatment_outcome_mean_milli,
        "mediator_effect_milli": output.mediator_effect_milli,
        "total_effect_milli": output.total_effect_milli,
        "mediator_outcome_slope_milli": output.mediator_outcome_slope_milli,
        "indirect_effect_milli": output.indirect_effect_milli,
        "direct_effect_milli": output.direct_effect_milli,
        "indirect_share_milli": output.indirect_share_milli,
        "mediator_variance_milli": output.mediator_variance_milli,
        "residual_noise_milli": output.residual_noise_milli,
        "total_signal_to_noise_milli": output.total_signal_to_noise_milli,
        "indirect_signal_to_noise_milli": output.indirect_signal_to_noise_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "leave_one_out_order": output.leave_one_out_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MediationAnalysis {
    pub fn validate(&self) -> Result<(), MediationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || self.unit_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .control_unit_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .treatment_unit_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .leave_one_out_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.indirect_share_milli > 1_000
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.digest.as_str().len() != 64
        {
            return Err(MediationError::InvalidOutput(
                "identity, arm, canonical ordering, effect bounds, or digest invariants are invalid".into(),
            ));
        }
        let units = self.unit_order.iter().collect::<BTreeSet<_>>();
        let control = self.control_unit_order.iter().collect::<BTreeSet<_>>();
        let treatment = self.treatment_unit_order.iter().collect::<BTreeSet<_>>();
        if units.len() != self.unit_order.len()
            || control.len() != self.control_unit_order.len()
            || treatment.len() != self.treatment_unit_order.len()
            || control.intersection(&treatment).next().is_some()
            || control.union(&treatment).copied().collect::<BTreeSet<_>>() != units
        {
            return Err(MediationError::InvalidOutput(
                "unit and arm partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MediationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MediationError::InvalidOutput(
                "causal-mediation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze a preclinical mediator/outcome decomposition with deterministic influence bounds.
pub fn analyze_glioma_mediation(
    request: &MediationRequest,
    observations: &[MediationObservation],
) -> Result<MediationAnalysis, MediationError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_units_per_arm == 0
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
        || request.min_units_per_arm > MAX_UNITS
    {
        return Err(MediationError::InvalidRequest(
            "non-empty distinct arms/objective, bounded observations, and a positive unit floor are required".into(),
        ));
    }
    let mut seen_observation_ids = BTreeSet::new();
    let mut seen_units = BTreeMap::<String, String>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.arm_id != request.control_arm
                && observation.arm_id != request.treatment_arm
            || observation.uncertainty_milli == 0
            || !seen_observation_ids.insert(observation.observation_id.clone())
            || seen_units
                .insert(observation.unit_id.clone(), observation.arm_id.clone())
                .is_some()
        {
            return Err(MediationError::InvalidObservation(
                "observation ids and unit ids must be unique, arms must match the request, and uncertainty must be positive".into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| MediationError::InvalidObservation(error.to_string()))?;
    }
    let mut rows = observations
        .iter()
        .map(|observation| Row { observation })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.observation.unit_id.cmp(&right.observation.unit_id));
    let mut control_unit_order = rows
        .iter()
        .filter(|row| row.observation.arm_id == request.control_arm)
        .map(|row| row.observation.unit_id.clone())
        .collect::<Vec<_>>();
    let mut treatment_unit_order = rows
        .iter()
        .filter(|row| row.observation.arm_id == request.treatment_arm)
        .map(|row| row.observation.unit_id.clone())
        .collect::<Vec<_>>();
    control_unit_order.sort();
    treatment_unit_order.sort();
    let mut unit_order = control_unit_order
        .iter()
        .cloned()
        .chain(treatment_unit_order.iter().cloned())
        .collect::<Vec<_>>();
    unit_order.sort();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    if control_unit_order.len() < request.min_units_per_arm
        || treatment_unit_order.len() < request.min_units_per_arm
    {
        uncertainty.insert("unit-floor-not-met-for-mediation".into());
    }
    let full = estimate(&rows, &request.control_arm, &request.treatment_arm);
    let full_estimate = full.unwrap_or(Estimate {
        control_mediator_mean: 0,
        treatment_mediator_mean: 0,
        control_outcome_mean: 0,
        treatment_outcome_mean: 0,
        mediator_effect: 0,
        total_effect: 0,
        slope: 0,
        indirect_effect: 0,
        direct_effect: 0,
        mediator_variance: 0,
        residual_noise: 0,
    });
    if full.is_none() {
        uncertainty.insert("mediator-variance-is-zero-or-effect-is-not-estimable".into());
    }
    let total_signal_to_noise =
        signal_to_noise(full_estimate.total_effect, full_estimate.residual_noise);
    let indirect_signal_to_noise =
        signal_to_noise(full_estimate.indirect_effect, full_estimate.residual_noise);
    let indirect_share = (u128::from(full_estimate.indirect_effect.unsigned_abs())
        .saturating_mul(1_000)
        .checked_div(u128::from(full_estimate.total_effect.unsigned_abs().max(1)))
        .unwrap_or(0)
        .min(1_000)) as u16;
    let mut leave_one_out_order = Vec::new();
    let mut max_leave_one_out_shift = 0_u64;
    for (index, row) in rows.iter().enumerate() {
        let mut omitted = rows.clone();
        omitted.remove(index);
        let control_count = omitted
            .iter()
            .filter(|item| item.observation.arm_id == request.control_arm)
            .count();
        let treatment_count = omitted
            .iter()
            .filter(|item| item.observation.arm_id == request.treatment_arm)
            .count();
        // Influence diagnostics remain useful when removing one unit leaves an arm below the
        // qualification floor; only an empty arm makes the decomposition mathematically invalid.
        if control_count == 0 || treatment_count == 0 {
            continue;
        }
        if let Some(leave_one_out) =
            estimate(&omitted, &request.control_arm, &request.treatment_arm)
        {
            leave_one_out_order.push(row.observation.unit_id.clone());
            let shift = [
                i128::from(leave_one_out.total_effect) - i128::from(full_estimate.total_effect),
                i128::from(leave_one_out.indirect_effect)
                    - i128::from(full_estimate.indirect_effect),
                i128::from(leave_one_out.direct_effect) - i128::from(full_estimate.direct_effect),
            ]
            .iter()
            .map(|value| abs_i128(*value))
            .max()
            .unwrap_or(0)
            .min(u128::from(u64::MAX)) as u64;
            max_leave_one_out_shift = max_leave_one_out_shift.max(shift);
        }
    }
    leave_one_out_order.sort();
    if leave_one_out_order.is_empty() {
        uncertainty.insert("leave-one-unit-out-floor-not-met".into());
    }
    if full_estimate.total_effect.unsigned_abs() <= request.effect_threshold_milli {
        negative_evidence.insert("total-effect-below-declared-threshold".into());
    }
    if full_estimate.indirect_effect.unsigned_abs() <= request.effect_threshold_milli {
        negative_evidence.insert("indirect-effect-below-declared-threshold".into());
    }
    if max_leave_one_out_shift > request.max_leave_one_out_shift_milli {
        uncertainty.insert("mediation-is-influenced-by-single-unit-omission".into());
    }
    let disposition = if full.is_none() || uncertainty.contains("unit-floor-not-met-for-mediation")
    {
        MediationDisposition::Unresolved
    } else if negative_evidence.contains("total-effect-below-declared-threshold")
        || negative_evidence.contains("indirect-effect-below-declared-threshold")
    {
        MediationDisposition::Negative
    } else if total_signal_to_noise < request.min_signal_to_noise_milli
        || indirect_signal_to_noise < request.min_signal_to_noise_milli
        || max_leave_one_out_shift > request.max_leave_one_out_shift_milli
        || leave_one_out_order.is_empty()
    {
        MediationDisposition::Partial
    } else {
        MediationDisposition::Qualified
    };
    let mut output = MediationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        model_system: request.model_system,
        unit_order,
        control_unit_order,
        treatment_unit_order,
        control_mediator_mean_milli: full_estimate.control_mediator_mean,
        treatment_mediator_mean_milli: full_estimate.treatment_mediator_mean,
        control_outcome_mean_milli: full_estimate.control_outcome_mean,
        treatment_outcome_mean_milli: full_estimate.treatment_outcome_mean,
        mediator_effect_milli: full_estimate.mediator_effect,
        total_effect_milli: full_estimate.total_effect,
        mediator_outcome_slope_milli: full_estimate.slope,
        indirect_effect_milli: full_estimate.indirect_effect,
        direct_effect_milli: full_estimate.direct_effect,
        indirect_share_milli: indirect_share,
        mediator_variance_milli: full_estimate.mediator_variance,
        residual_noise_milli: full_estimate.residual_noise,
        total_signal_to_noise_milli: total_signal_to_noise,
        indirect_signal_to_noise_milli: indirect_signal_to_noise,
        max_leave_one_out_shift_milli: max_leave_one_out_shift,
        leave_one_out_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-causal-mediation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MediationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> MediationRequest {
        MediationRequest {
            objective: "preclinical invasion mediation".into(),
            control_arm: "control".into(),
            treatment_arm: "treatment".into(),
            model_system: GliomaModelSystem::Organoid,
            min_units_per_arm: 2,
            effect_threshold_milli: 20,
            min_signal_to_noise_milli: 100,
            max_leave_one_out_shift_milli: 200,
        }
    }

    fn observation(
        id: &str,
        unit: &str,
        arm: &str,
        mediator: i64,
        outcome: i64,
    ) -> MediationObservation {
        MediationObservation {
            observation_id: id.into(),
            unit_id: unit.into(),
            arm_id: arm.into(),
            mediator_milli: mediator,
            outcome_milli: outcome,
            uncertainty_milli: 5,
            artifact: LocalArtifactRef {
                artifact_id: format!("local:{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-mediation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    #[test]
    fn mediation_decomposes_effect_and_replays_stably() {
        let observations = vec![
            observation("c1", "u1", "control", 100, 120),
            observation("c2", "u2", "control", 110, 130),
            observation("t1", "u3", "treatment", 200, 250),
            observation("t2", "u4", "treatment", 210, 260),
        ];
        let first = analyze_glioma_mediation(&request(), &observations).unwrap();
        let second = analyze_glioma_mediation(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert!(first.indirect_effect_milli > 0);
        assert_eq!(first.disposition, MediationDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn low_unit_floor_is_unresolved_and_duplicate_units_are_refused() {
        let mut request = request();
        request.min_units_per_arm = 3;
        let observations = vec![
            observation("c1", "u1", "control", 100, 120),
            observation("c2", "u2", "control", 110, 130),
            observation("t1", "u3", "treatment", 200, 250),
            observation("t2", "u3", "treatment", 210, 260),
        ];
        assert!(matches!(
            analyze_glioma_mediation(&request, &observations),
            Err(MediationError::InvalidObservation(_))
        ));
        let observations = vec![
            observation("c1", "u1", "control", 100, 120),
            observation("c2", "u2", "control", 110, 130),
            observation("t1", "u3", "treatment", 200, 250),
            observation("t2", "u4", "treatment", 210, 260),
        ];
        let output = analyze_glioma_mediation(&request, &observations).unwrap();
        assert_eq!(output.disposition, MediationDisposition::Unresolved);
        assert!(output
            .uncertainty
            .contains(&"unit-floor-not-met-for-mediation".into()));
    }

    #[test]
    fn null_effect_is_published_as_negative() {
        let observations = vec![
            observation("c1", "u1", "control", 100, 120),
            observation("c2", "u2", "control", 110, 130),
            observation("t1", "u3", "treatment", 101, 121),
            observation("t2", "u4", "treatment", 109, 129),
        ];
        let output = analyze_glioma_mediation(&request(), &observations).unwrap();
        assert_eq!(output.disposition, MediationDisposition::Negative);
        assert!(!output.negative_evidence.is_empty());
    }
}
