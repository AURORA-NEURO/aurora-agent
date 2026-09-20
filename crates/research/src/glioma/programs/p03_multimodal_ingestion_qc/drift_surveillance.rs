//! Continuous modality/metric drift surveillance for preclinical glioma QC.
//!
//! This feature turns repeated QC epochs into an operational scientific signal. It estimates
//! robust baseline and terminal values, maximum excursion, signed drift slope, quality attrition,
//! and a bounded recalibration queue. It never rewrites observations, imputes missing epochs, or
//! treats a drift alert as a biological conclusion.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalDriftSurveillance1@1";
pub const MAX_EPOCHS: usize = 4096;
pub const MAX_METRICS: usize = 256;
pub const MAX_OBSERVATIONS: usize = 262_144;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftObservation {
    pub epoch_id: String,
    pub epoch_index: u32,
    pub modality: GliomaModality,
    pub metric_id: String,
    pub value_milli: i64,
    pub expected_center_milli: i64,
    pub expected_spread_milli: u64,
    pub quality_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftSurveillanceRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub required_modalities: Vec<GliomaModality>,
    pub metric_order: Vec<String>,
    pub observations: Vec<DriftObservation>,
    pub baseline_epoch_count: usize,
    pub min_eligible_epochs: usize,
    pub min_quality_milli: u16,
    pub max_allowed_drift_milli: u64,
    pub max_allowed_slope_milli_per_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDisposition {
    Stable,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftMetricSummary {
    pub metric_key: String,
    pub modality: GliomaModality,
    pub metric_id: String,
    pub eligible_epoch_order: Vec<String>,
    pub baseline_median_milli: Option<i64>,
    pub terminal_median_milli: Option<i64>,
    pub max_absolute_drift_milli: u64,
    pub signed_slope_milli_per_epoch: i64,
    pub quality_failed_epoch_count: u32,
    pub missing_epoch_count: u32,
    pub drift_score_milli: u16,
    pub disposition: DriftDisposition,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftSurveillance {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub metric_order: Vec<String>,
    pub summaries: Vec<DriftMetricSummary>,
    pub recalibration_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: DriftDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DriftSurveillanceError {
    #[error("drift surveillance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("drift surveillance input is invalid: {0}")]
    InvalidInput(String),
    #[error("drift surveillance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("drift surveillance digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn fraction_milli(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn key(modality: GliomaModality, metric_id: &str) -> String {
    format!("{modality:?}::{metric_id}")
}

fn digest_input(output: &DriftSurveillance) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "epoch_order": output.epoch_order,
        "modality_order": output.modality_order,
        "metric_order": output.metric_order,
        "summaries": output.summaries,
        "recalibration_order": output.recalibration_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &DriftSurveillanceRequest) -> Result<(), DriftSurveillanceError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.epoch_order.len() < 2
        || request.epoch_order.len() > MAX_EPOCHS
        || request.required_modalities.is_empty()
        || !canonical(&request.required_modalities)
        || request.metric_order.is_empty()
        || request.metric_order.len() > MAX_METRICS
        || !canonical(&request.metric_order)
        || request.observations.len() > MAX_OBSERVATIONS
        || request.baseline_epoch_count == 0
        || request.baseline_epoch_count > request.epoch_order.len()
        || request.min_eligible_epochs == 0
        || request.min_eligible_epochs > request.epoch_order.len()
        || request.min_quality_milli > 1_000
        || request.max_allowed_drift_milli > MAX_ABS_VALUE_MILLI
        || request.max_allowed_slope_milli_per_epoch > MAX_ABS_VALUE_MILLI
    {
        return Err(DriftSurveillanceError::InvalidRequest(
            "objective, ordered epochs/modalities/metrics, bounded observations, baseline/eligibility, quality, and drift gates are required".into(),
        ));
    }
    if !canonical(&request.epoch_order)
        || request
            .epoch_order
            .iter()
            .any(|epoch| epoch.trim().is_empty())
        || request
            .metric_order
            .iter()
            .any(|metric| metric.trim().is_empty())
    {
        return Err(DriftSurveillanceError::InvalidRequest(
            "epoch and metric identifiers must be unique, non-empty, and canonical".into(),
        ));
    }
    let epochs = request.epoch_order.iter().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let metrics = request.metric_order.iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &request.observations {
        if !epochs.contains(&observation.epoch_id)
            || !modalities.contains(&observation.modality)
            || !metrics.contains(&observation.metric_id)
            || observation.epoch_index >= request.epoch_order.len() as u32
            || request.epoch_order[observation.epoch_index as usize] != observation.epoch_id
            || observation.expected_spread_milli == 0
            || observation.expected_spread_milli > MAX_ABS_VALUE_MILLI
            || observation.value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || observation.expected_center_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || observation.quality_milli > 1_000
            || !seen.insert((
                observation.epoch_id.clone(),
                observation.modality,
                observation.metric_id.clone(),
            ))
        {
            return Err(DriftSurveillanceError::InvalidInput(
                "observations require known epoch/modality/metric bindings, exact epoch indices, bounded values/spread, finite quality, and unique cells".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &DriftSurveillance) -> Result<(), DriftSurveillanceError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.epoch_order)
        || !canonical(&output.modality_order)
        || !canonical(&output.metric_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.summaries.windows(2).any(|pair| {
            (pair[0].modality, pair[0].metric_id.clone())
                >= (pair[1].modality, pair[1].metric_id.clone())
        })
        || output.summaries.iter().any(|summary| {
            summary.metric_key != key(summary.modality, &summary.metric_id)
                || !canonical(&summary.eligible_epoch_order)
                || summary.drift_score_milli > 1_000
                || summary.quality_failed_epoch_count + summary.missing_epoch_count
                    > output.epoch_order.len() as u32
                || summary.next_action.trim().is_empty()
        })
    {
        return Err(DriftSurveillanceError::InvalidOutput(
            "identity, ordering, bounded metrics, epoch accounting, or action invariants are invalid".into(),
        ));
    }
    if output
        .recalibration_order
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != output.recalibration_order.len()
    {
        return Err(DriftSurveillanceError::InvalidOutput(
            "recalibration order contains duplicate metric keys".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| DriftSurveillanceError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(DriftSurveillanceError::InvalidOutput(
            "digest is not bound to drift-surveillance output".into(),
        ));
    }
    Ok(())
}

impl DriftSurveillance {
    pub fn validate(&self) -> Result<(), DriftSurveillanceError> {
        validate_output(self)
    }
}

/// Detect sustained multimodal QC drift across ordered local preclinical epochs.
pub fn surveil_glioma_multimodal_drift(
    request: &DriftSurveillanceRequest,
) -> Result<DriftSurveillance, DriftSurveillanceError> {
    validate_request(request)?;
    let grouped = request
        .observations
        .iter()
        .map(|observation| {
            (
                (observation.modality, observation.metric_id.clone()),
                observation,
            )
        })
        .fold(
            BTreeMap::<(GliomaModality, String), Vec<&DriftObservation>>::new(),
            |mut grouped, (group, observation)| {
                grouped.entry(group).or_default().push(observation);
                grouped
            },
        );
    let mut summaries = Vec::new();
    let mut recalibration_scores = BTreeMap::<String, (u16, GliomaModality, String)>::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &request.required_modalities {
        for metric_id in &request.metric_order {
            let key_value = (*modality, metric_id.clone());
            let observations = grouped.get(&key_value).cloned().unwrap_or_default();
            let mut by_epoch = BTreeMap::<u32, &DriftObservation>::new();
            let mut quality_failed_epoch_count = 0_u32;
            for observation in observations {
                if observation.quality_milli < request.min_quality_milli {
                    quality_failed_epoch_count += 1;
                } else {
                    by_epoch.insert(observation.epoch_index, observation);
                }
            }
            let eligible_epoch_order = by_epoch
                .values()
                .map(|observation| observation.epoch_id.clone())
                .collect::<Vec<_>>();
            let missing_epoch_count = request
                .epoch_order
                .len()
                .saturating_sub(by_epoch.len() + quality_failed_epoch_count as usize)
                as u32;
            let eligible_values = by_epoch
                .iter()
                .map(|(epoch_index, observation)| (*epoch_index, observation.value_milli))
                .collect::<Vec<_>>();
            let eligible_count = eligible_values.len();
            let baseline_indices = eligible_values
                .iter()
                .filter(|(index, _)| (*index as usize) < request.baseline_epoch_count)
                .map(|(_, value)| *value)
                .collect::<Vec<_>>();
            let terminal_value = eligible_values.last().map(|(_, value)| *value);
            let baseline_median_milli = if baseline_indices.is_empty() {
                None
            } else {
                let mut baseline = baseline_indices;
                Some(median(&mut baseline))
            };
            let terminal_median_milli = terminal_value;
            let (max_absolute_drift_milli, signed_slope_milli_per_epoch) = if let Some(baseline) =
                baseline_median_milli
            {
                let max_drift = eligible_values
                    .iter()
                    .map(|(_, value)| value.saturating_sub(baseline).unsigned_abs())
                    .max()
                    .unwrap_or(0);
                let (first_index, _) = eligible_values.first().copied().unwrap_or((0, baseline));
                let (last_index, last_value) = eligible_values
                    .last()
                    .copied()
                    .unwrap_or((first_index, baseline));
                let delta = last_value.saturating_sub(baseline);
                let span = i64::from(last_index.saturating_sub(first_index).max(1));
                (max_drift, delta / span)
            } else {
                (0, 0)
            };
            let expected_spread = grouped
                .get(&key_value)
                .and_then(|observations| observations.first())
                .map(|observation| observation.expected_spread_milli)
                .unwrap_or(1);
            let expected_center_deviation = grouped
                .get(&key_value)
                .map(|observations| {
                    observations
                        .iter()
                        .filter(|observation| {
                            observation.quality_milli >= request.min_quality_milli
                        })
                        .map(|observation| {
                            observation
                                .value_milli
                                .saturating_sub(observation.expected_center_milli)
                                .unsigned_abs()
                        })
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            let effective_drift = max_absolute_drift_milli.max(expected_center_deviation);
            let spread_penalty = fraction_milli(
                effective_drift.min(request.max_allowed_drift_milli.saturating_mul(2)),
                request.max_allowed_drift_milli.max(1),
            ) as u64
                * 600
                / 1_000;
            let slope_penalty = fraction_milli(
                signed_slope_milli_per_epoch.unsigned_abs(),
                request.max_allowed_slope_milli_per_epoch.max(1),
            ) as u64
                * 250
                / 1_000;
            let quality_penalty = fraction_milli(
                u64::from(quality_failed_epoch_count),
                request.epoch_order.len() as u64,
            ) as u64
                * 150
                / 1_000;
            let drift_score_milli = 1_000_u64
                .saturating_sub(spread_penalty + slope_penalty + quality_penalty)
                .min(1_000) as u16;
            let disposition = if eligible_count < request.min_eligible_epochs {
                uncertainty.insert(format!("{key_value:?}:insufficient-eligible-epochs"));
                DriftDisposition::Unresolved
            } else if effective_drift > request.max_allowed_drift_milli
                || signed_slope_milli_per_epoch.unsigned_abs()
                    > request.max_allowed_slope_milli_per_epoch
            {
                negative_evidence.insert(format!(
                    "{key_value:?}:drift-gate-exceeded={effective_drift}"
                ));
                DriftDisposition::Blocked
            } else if quality_failed_epoch_count > 0
                || effective_drift > request.max_allowed_drift_milli / 2
            {
                uncertainty.insert(format!("{key_value:?}:drift-or-quality-warning"));
                DriftDisposition::Conditional
            } else {
                DriftDisposition::Stable
            };
            if expected_center_deviation > expected_spread {
                uncertainty.insert(format!("{key_value:?}:outside-expected-center-spread"));
            }
            let next_action = match disposition {
                DriftDisposition::Stable => {
                    "continue surveillance at the current QC calibration".into()
                }
                DriftDisposition::Conditional => {
                    "increase QC sampling and review calibration before downstream release".into()
                }
                DriftDisposition::Blocked => {
                    "hold downstream use and recalibrate or replace the drifting modality metric"
                        .into()
                }
                DriftDisposition::Unresolved => {
                    "collect enough quality-controlled epochs before interpreting drift".into()
                }
            };
            let priority = 1_000_u16.saturating_sub(drift_score_milli);
            let metric_key = key(*modality, metric_id);
            recalibration_scores
                .insert(metric_key.clone(), (priority, *modality, metric_id.clone()));
            summaries.push(DriftMetricSummary {
                metric_key,
                modality: *modality,
                metric_id: metric_id.clone(),
                eligible_epoch_order,
                baseline_median_milli,
                terminal_median_milli,
                max_absolute_drift_milli: effective_drift,
                signed_slope_milli_per_epoch,
                quality_failed_epoch_count,
                missing_epoch_count,
                drift_score_milli,
                disposition,
                next_action,
            });
        }
    }
    summaries.sort_by_key(|summary| (summary.modality, summary.metric_id.clone()));
    let mut recalibration_order = recalibration_scores
        .values()
        .filter(|(priority, _, _)| *priority > 0)
        .map(|(_, modality, metric_id)| key(*modality, metric_id))
        .collect::<Vec<_>>();
    recalibration_order.sort_by(|left, right| {
        recalibration_scores[right]
            .0
            .cmp(&recalibration_scores[left].0)
            .then_with(|| left.cmp(right))
    });
    let disposition = if summaries
        .iter()
        .any(|summary| summary.disposition == DriftDisposition::Blocked)
    {
        DriftDisposition::Blocked
    } else if summaries
        .iter()
        .any(|summary| summary.disposition == DriftDisposition::Unresolved)
    {
        DriftDisposition::Unresolved
    } else if summaries
        .iter()
        .any(|summary| summary.disposition == DriftDisposition::Conditional)
    {
        DriftDisposition::Conditional
    } else {
        DriftDisposition::Stable
    };
    let mut output = DriftSurveillance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        epoch_order: request.epoch_order.clone(),
        modality_order: request.required_modalities.clone(),
        metric_order: request.metric_order.clone(),
        summaries,
        recalibration_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-drift-surveillance"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DriftSurveillanceError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(observations: Vec<DriftObservation>) -> DriftSurveillanceRequest {
        DriftSurveillanceRequest {
            objective: "surveil multimodal QC drift before mechanism analysis".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            epoch_order: vec!["epoch-0".into(), "epoch-1".into(), "epoch-2".into()],
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            metric_order: vec!["signal".into()],
            observations,
            baseline_epoch_count: 1,
            min_eligible_epochs: 3,
            min_quality_milli: 700,
            max_allowed_drift_milli: 20,
            max_allowed_slope_milli_per_epoch: 20,
        }
    }

    fn observation(
        epoch_index: u32,
        modality: GliomaModality,
        value_milli: i64,
        quality_milli: u16,
    ) -> DriftObservation {
        DriftObservation {
            epoch_id: format!("epoch-{epoch_index}"),
            epoch_index,
            modality,
            metric_id: "signal".into(),
            value_milli,
            expected_center_milli: 100,
            expected_spread_milli: 20,
            quality_milli,
        }
    }

    #[test]
    fn drift_surveillance_releases_stable_metrics() {
        let mut observations = Vec::new();
        for modality in [GliomaModality::Genomics, GliomaModality::Imaging] {
            for epoch_index in 0..3 {
                observations.push(observation(
                    epoch_index,
                    modality,
                    100 + epoch_index as i64,
                    900,
                ));
            }
        }
        let output = surveil_glioma_multimodal_drift(&request(observations)).unwrap();
        assert_eq!(output.disposition, DriftDisposition::Stable);
        assert!(output
            .summaries
            .iter()
            .all(|summary| { summary.disposition == DriftDisposition::Stable }));
        output.validate().unwrap();
    }

    #[test]
    fn drift_surveillance_blocks_sustained_metric_shift() {
        let mut observations = Vec::new();
        for modality in [GliomaModality::Genomics, GliomaModality::Imaging] {
            observations.push(observation(0, modality, 100, 900));
            observations.push(observation(1, modality, 110, 900));
            observations.push(observation(2, modality, 150, 900));
        }
        let output = surveil_glioma_multimodal_drift(&request(observations)).unwrap();
        assert_eq!(output.disposition, DriftDisposition::Blocked);
        assert_eq!(
            output.recalibration_order,
            vec![
                "Genomics::signal".to_string(),
                "Imaging::signal".to_string()
            ]
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("drift-gate-exceeded")));
    }
}
