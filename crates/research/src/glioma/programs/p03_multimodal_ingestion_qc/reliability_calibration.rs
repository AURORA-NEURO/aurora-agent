//! Replicate-aware modality reliability calibration for preclinical glioma studies.
//!
//! Presence is not reliability. This feature uses repeated local observations to quantify
//! within-sample spread, leave-one-replicate-out instability, quality-floor attrition, and
//! replicate debt for every modality. It emits a deterministic admission gate and reacquisition
//! order for downstream multimodal workflows without inventing observations or making a clinical
//! decision.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalReliabilityCalibration1@1";
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_MODALITIES: usize = 64;
pub const MAX_OBSERVATIONS: usize = 262_144;
pub const MAX_REPLICATES: u16 = 128;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliabilityObservation {
    pub sample_id: String,
    pub modality: GliomaModality,
    pub replicate_index: u16,
    pub value_milli: i64,
    pub quality_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliabilityCalibrationRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_ids: Vec<String>,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<ReliabilityObservation>,
    pub min_replicates_per_sample: u16,
    pub max_replicates_per_sample: u16,
    pub min_quality_milli: u16,
    pub max_within_sample_mad_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub min_reliability_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliabilityModalitySummary {
    pub modality: GliomaModality,
    pub sample_count: u32,
    pub eligible_sample_count: u32,
    pub replicate_count: u32,
    pub replicate_debt: u32,
    pub quality_failed_replicate_count: u32,
    pub median_within_sample_mad_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub median_quality_milli: u16,
    pub reliability_milli: u16,
    pub disposition: ReliabilityDisposition,
    pub acquisition_priority_milli: u16,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliabilityCalibration {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub modality_summaries: Vec<ReliabilityModalitySummary>,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReliabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReliabilityCalibrationError {
    #[error("reliability calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("reliability calibration input is invalid: {0}")]
    InvalidInput(String),
    #[error("reliability calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("reliability calibration digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_u64(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn fraction_milli(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn digest_input(output: &ReliabilityCalibration) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_order": output.sample_order,
        "modality_order": output.modality_order,
        "modality_summaries": output.modality_summaries,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &ReliabilityCalibrationRequest,
) -> Result<(), ReliabilityCalibrationError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.sample_ids.is_empty()
        || request.sample_ids.len() > MAX_SAMPLES
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_replicates_per_sample == 0
        || request.min_replicates_per_sample > request.max_replicates_per_sample
        || request.max_replicates_per_sample > MAX_REPLICATES
        || request.min_quality_milli > 1_000
        || request.max_within_sample_mad_milli > MAX_ABS_VALUE_MILLI
        || request.max_leave_one_out_shift_milli > MAX_ABS_VALUE_MILLI
        || request.min_reliability_milli > 1_000
    {
        return Err(ReliabilityCalibrationError::InvalidRequest(
            "objective, study, bounded samples/modalities/observations, replicate floors, and reliability gates are required".into(),
        ));
    }
    let samples = request.sample_ids.iter().collect::<BTreeSet<_>>();
    if samples.len() != request.sample_ids.len()
        || !canonical(&request.sample_ids)
        || !canonical(&request.required_modalities)
    {
        return Err(ReliabilityCalibrationError::InvalidRequest(
            "sample and modality identifiers must be unique and canonical".into(),
        ));
    }
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut keys = BTreeSet::new();
    for observation in &request.observations {
        if !samples.contains(&observation.sample_id)
            || !modalities.contains(&observation.modality)
            || observation.replicate_index >= request.max_replicates_per_sample
            || observation.quality_milli > 1_000
            || observation.value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || !keys.insert((
                observation.sample_id.clone(),
                observation.modality,
                observation.replicate_index,
            ))
        {
            return Err(ReliabilityCalibrationError::InvalidInput(
                "observations require known sample/modality keys, bounded replicate indices, finite values, quality, and unique replicate identity".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &ReliabilityCalibration) -> Result<(), ReliabilityCalibrationError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.sample_order)
        || !canonical(&output.modality_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.modality_summaries.len() != output.modality_order.len()
        || output
            .modality_summaries
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || output.modality_summaries.iter().any(|summary| {
            summary.sample_count == 0
                || summary.eligible_sample_count > summary.sample_count
                || summary.replicate_debt
                    > summary
                        .sample_count
                        .saturating_mul(u32::from(MAX_REPLICATES))
                || summary.quality_failed_replicate_count > summary.replicate_count
                || summary.median_quality_milli > 1_000
                || summary.reliability_milli > 1_000
                || summary.acquisition_priority_milli > 1_000
                || summary.next_action.trim().is_empty()
        })
    {
        return Err(ReliabilityCalibrationError::InvalidOutput(
            "identity, ordering, summary cardinality, bounded metrics, or action invariants are invalid".into(),
        ));
    }
    if output
        .acquisition_order
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != output.acquisition_order.len()
    {
        return Err(ReliabilityCalibrationError::InvalidOutput(
            "acquisition order contains duplicate modalities".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ReliabilityCalibrationError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ReliabilityCalibrationError::InvalidOutput(
            "digest is not bound to reliability-calibration output".into(),
        ));
    }
    Ok(())
}

impl ReliabilityCalibration {
    pub fn validate(&self) -> Result<(), ReliabilityCalibrationError> {
        validate_output(self)
    }
}

fn sample_metrics(
    observations: &[&ReliabilityObservation],
    min_quality_milli: u16,
) -> (Option<u64>, Option<u64>, Option<u16>, u32) {
    let eligible = observations
        .iter()
        .filter(|observation| observation.quality_milli >= min_quality_milli)
        .copied()
        .collect::<Vec<_>>();
    let quality_failed = observations
        .iter()
        .filter(|observation| observation.quality_milli < min_quality_milli)
        .count() as u32;
    if eligible.is_empty() {
        return (None, None, None, quality_failed);
    }
    let mut values = eligible
        .iter()
        .map(|observation| observation.value_milli)
        .collect::<Vec<_>>();
    let centre = median(&mut values);
    let mut deviations = eligible
        .iter()
        .map(|observation| {
            observation
                .value_milli
                .saturating_sub(centre)
                .unsigned_abs()
        })
        .collect::<Vec<_>>();
    let spread = median_u64(&mut deviations);
    let mut leave_one_out_shift = 0_u64;
    if eligible.len() > 1 {
        for index in 0..eligible.len() {
            let mut leave_one_out = eligible
                .iter()
                .enumerate()
                .filter_map(|(candidate, observation)| {
                    (candidate != index).then_some(observation.value_milli)
                })
                .collect::<Vec<_>>();
            let reduced_centre = median(&mut leave_one_out);
            leave_one_out_shift =
                leave_one_out_shift.max(reduced_centre.saturating_sub(centre).unsigned_abs());
        }
    }
    let mut qualities = eligible
        .iter()
        .map(|observation| observation.quality_milli)
        .collect::<Vec<_>>();
    qualities.sort_unstable();
    let median_quality = qualities[qualities.len() / 2];
    (
        Some(spread),
        Some(leave_one_out_shift),
        Some(median_quality),
        quality_failed,
    )
}

/// Calibrate replicate-level modality reliability for a local preclinical glioma study.
pub fn calibrate_glioma_multimodal_reliability(
    request: &ReliabilityCalibrationRequest,
) -> Result<ReliabilityCalibration, ReliabilityCalibrationError> {
    validate_request(request)?;
    let observations = request.observations.iter().collect::<Vec<_>>();
    let mut by_key = BTreeMap::<(String, GliomaModality), Vec<&ReliabilityObservation>>::new();
    for observation in observations {
        by_key
            .entry((observation.sample_id.clone(), observation.modality))
            .or_default()
            .push(observation);
    }

    let mut summaries = Vec::new();
    let mut acquisition_scores = BTreeMap::<GliomaModality, (u16, u32, u32)>::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &request.required_modalities {
        let mut eligible_sample_count = 0_u32;
        let mut replicate_count = 0_u32;
        let mut replicate_debt = 0_u32;
        let mut quality_failed_replicate_count = 0_u32;
        let mut spreads = Vec::new();
        let mut leave_one_out_shifts = Vec::new();
        let mut qualities = Vec::new();
        for sample_id in &request.sample_ids {
            let sample_observations = by_key
                .get(&(sample_id.clone(), *modality))
                .cloned()
                .unwrap_or_default();
            replicate_count = replicate_count.saturating_add(sample_observations.len() as u32);
            let (spread, leave_one_out, quality, quality_failed) =
                sample_metrics(&sample_observations, request.min_quality_milli);
            quality_failed_replicate_count =
                quality_failed_replicate_count.saturating_add(quality_failed);
            let eligible_count = sample_observations
                .iter()
                .filter(|observation| observation.quality_milli >= request.min_quality_milli)
                .count();
            if eligible_count < request.min_replicates_per_sample as usize {
                replicate_debt = replicate_debt.saturating_add(
                    request
                        .min_replicates_per_sample
                        .saturating_sub(eligible_count as u16) as u32,
                );
                if sample_observations.is_empty() {
                    uncertainty.insert(format!("{:?}:{sample_id}:missing-replicates", modality));
                } else {
                    uncertainty
                        .insert(format!("{:?}:{sample_id}:quality-replicate-debt", modality));
                }
            } else if let (Some(spread), Some(leave_one_out), Some(quality)) =
                (spread, leave_one_out, quality)
            {
                eligible_sample_count += 1;
                spreads.push(spread);
                leave_one_out_shifts.push(leave_one_out);
                qualities.push(quality);
            }
        }
        let mut median_spread = spreads;
        let median_within_sample_mad_milli = if median_spread.is_empty() {
            0
        } else {
            median_u64(&mut median_spread)
        };
        let max_leave_one_out_shift_milli = leave_one_out_shifts.into_iter().max().unwrap_or(0);
        let median_quality_milli = if qualities.is_empty() {
            0
        } else {
            qualities.sort_unstable();
            qualities[qualities.len() / 2]
        };
        let spread_penalty = if request.max_within_sample_mad_milli == 0 {
            u64::from(median_within_sample_mad_milli > 0) * 1_000
        } else {
            (median_within_sample_mad_milli.saturating_mul(700)
                / request.max_within_sample_mad_milli)
                .min(700)
        };
        let instability_penalty = if request.max_leave_one_out_shift_milli == 0 {
            u64::from(max_leave_one_out_shift_milli > 0) * 200
        } else {
            (max_leave_one_out_shift_milli.saturating_mul(200)
                / request.max_leave_one_out_shift_milli)
                .min(200)
        };
        let quality_penalty = u64::from(1_000_u16.saturating_sub(median_quality_milli)).min(100);
        let debt_penalty = fraction_milli(
            u64::from(replicate_debt),
            u64::from(request.min_replicates_per_sample)
                .saturating_mul(request.sample_ids.len() as u64),
        ) as u64;
        let reliability_milli = 1_000_u64
            .saturating_sub(spread_penalty + instability_penalty + quality_penalty + debt_penalty)
            .min(1_000) as u16;
        let blocked = eligible_sample_count == 0
            || reliability_milli < request.min_reliability_milli
            || max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli;
        let conditional = replicate_debt > 0
            || median_within_sample_mad_milli > request.max_within_sample_mad_milli
            || max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli;
        let disposition = if blocked {
            ReliabilityDisposition::Blocked
        } else if conditional {
            ReliabilityDisposition::Conditional
        } else {
            ReliabilityDisposition::Ready
        };
        if blocked {
            negative_evidence.insert(format!(
                "{:?}:reliability-below-gate={reliability_milli}",
                modality
            ));
        }
        if median_within_sample_mad_milli > request.max_within_sample_mad_milli {
            uncertainty.insert(format!(
                "{:?}:within-sample-spread={median_within_sample_mad_milli}",
                modality
            ));
        }
        let acquisition_priority_milli = fraction_milli(
            u64::from(replicate_debt).saturating_mul(600)
                + (1_000_u64.saturating_sub(u64::from(reliability_milli))).saturating_mul(400),
            u64::from(request.min_replicates_per_sample)
                .saturating_mul(request.sample_ids.len() as u64)
                .max(1),
        );
        let next_action = match disposition {
            ReliabilityDisposition::Ready => {
                format!(
                    "{:?} is reproducible at the declared reliability gate",
                    modality
                )
            }
            ReliabilityDisposition::Conditional => format!(
                "add or repeat replicates for {:?} before high-confidence interpretation",
                modality
            ),
            ReliabilityDisposition::Blocked => format!(
                "hold {:?} downstream use and acquire quality-controlled replicates",
                modality
            ),
            ReliabilityDisposition::Unresolved => {
                format!(
                    "resolve {:?} reliability evidence before interpretation",
                    modality
                )
            }
        };
        acquisition_scores.insert(
            *modality,
            (
                acquisition_priority_milli,
                replicate_debt,
                u32::from(1_000_u16.saturating_sub(reliability_milli)),
            ),
        );
        summaries.push(ReliabilityModalitySummary {
            modality: *modality,
            sample_count: request.sample_ids.len() as u32,
            eligible_sample_count,
            replicate_count,
            replicate_debt,
            quality_failed_replicate_count,
            median_within_sample_mad_milli,
            max_leave_one_out_shift_milli,
            median_quality_milli,
            reliability_milli,
            disposition,
            acquisition_priority_milli,
            next_action,
        });
    }
    let mut acquisition_order = request
        .required_modalities
        .iter()
        .copied()
        .filter(|modality| acquisition_scores[modality].0 > 0)
        .collect::<Vec<_>>();
    acquisition_order.sort_by(|left, right| {
        acquisition_scores[right]
            .cmp(&acquisition_scores[left])
            .then_with(|| left.cmp(right))
    });
    let disposition = if summaries
        .iter()
        .any(|summary| summary.disposition == ReliabilityDisposition::Blocked)
    {
        ReliabilityDisposition::Blocked
    } else if summaries
        .iter()
        .any(|summary| summary.disposition == ReliabilityDisposition::Conditional)
    {
        ReliabilityDisposition::Conditional
    } else {
        ReliabilityDisposition::Ready
    };
    let mut output = ReliabilityCalibration {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_order: request.sample_ids.clone(),
        modality_order: request.required_modalities.clone(),
        modality_summaries: summaries,
        acquisition_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-reliability-calibration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReliabilityCalibrationError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(observations: Vec<ReliabilityObservation>) -> ReliabilityCalibrationRequest {
        ReliabilityCalibrationRequest {
            objective: "calibrate modality reliability before mechanism analysis".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            sample_ids: vec!["sample-a".into(), "sample-b".into()],
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            min_replicates_per_sample: 2,
            max_replicates_per_sample: 4,
            min_quality_milli: 700,
            max_within_sample_mad_milli: 20,
            max_leave_one_out_shift_milli: 20,
            min_reliability_milli: 700,
        }
    }

    #[test]
    fn reliability_calibration_releases_reproducible_modalities() {
        let mut observations = Vec::new();
        for sample_id in ["sample-a", "sample-b"] {
            for modality in [GliomaModality::Genomics, GliomaModality::Imaging] {
                for replicate_index in 0..2 {
                    observations.push(ReliabilityObservation {
                        sample_id: sample_id.into(),
                        modality,
                        replicate_index,
                        value_milli: 100 + i64::from(replicate_index),
                        quality_milli: 900,
                    });
                }
            }
        }
        let output = calibrate_glioma_multimodal_reliability(&request(observations)).unwrap();
        assert_eq!(output.disposition, ReliabilityDisposition::Ready);
        assert!(output
            .modality_summaries
            .iter()
            .all(|summary| summary.reliability_milli >= 700));
        output.validate().unwrap();
    }

    #[test]
    fn reliability_calibration_blocks_replicate_debt_and_instability() {
        let output = calibrate_glioma_multimodal_reliability(&request(vec![
            ReliabilityObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Genomics,
                replicate_index: 0,
                value_milli: 0,
                quality_milli: 900,
            },
            ReliabilityObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Genomics,
                replicate_index: 1,
                value_milli: 100,
                quality_milli: 900,
            },
            ReliabilityObservation {
                sample_id: "sample-b".into(),
                modality: GliomaModality::Genomics,
                replicate_index: 0,
                value_milli: 0,
                quality_milli: 900,
            },
            ReliabilityObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Imaging,
                replicate_index: 0,
                value_milli: 100,
                quality_milli: 900,
            },
        ]))
        .unwrap();
        assert_eq!(output.disposition, ReliabilityDisposition::Blocked);
        assert_eq!(
            output.acquisition_order,
            vec![GliomaModality::Imaging, GliomaModality::Genomics]
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("reliability-below-gate")));
    }
}
