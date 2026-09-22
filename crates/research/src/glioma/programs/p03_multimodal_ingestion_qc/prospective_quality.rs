//! Prospective multimodal quality forecasting for autonomous preclinical glioma workflows.
//!
//! Historical drift alerts are not enough for a high-throughput research engine: the engine must
//! know which modality is likely to fail before it schedules the next endpoint analysis. This
//! feature estimates a bounded quality trend from ordered QC metadata, forecasts the next horizon,
//! and emits a preventive preflight/reacquisition order. It never forecasts biology, imputes a
//! missing assay, or dispatches an instrument.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalProspectiveQuality1@1";
pub const MAX_EPOCHS: usize = 4_096;
pub const MAX_MODALITIES: usize = 64;
pub const MAX_OBSERVATIONS: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityForecastObservation {
    pub epoch_id: String,
    pub epoch_index: u32,
    pub modality: GliomaModality,
    pub observed: bool,
    pub quality_milli: u16,
    pub expected_feature_count: u32,
    pub observed_feature_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveQualityRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<QualityForecastObservation>,
    pub history_epochs: usize,
    pub forecast_horizon: u32,
    pub min_history_points: usize,
    pub quality_floor_milli: u16,
    pub max_negative_slope_milli_per_epoch: u64,
    pub minimum_forecast_quality_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityForecastDisposition {
    Stable,
    AtRisk,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityQualityForecast {
    pub modality: GliomaModality,
    pub history_epoch_order: Vec<String>,
    pub eligible_epoch_order: Vec<String>,
    pub missing_epoch_order: Vec<String>,
    pub quality_failed_epoch_order: Vec<String>,
    pub baseline_quality_milli: Option<u16>,
    pub terminal_quality_milli: Option<u16>,
    pub signed_slope_milli_per_epoch: i64,
    pub forecast_quality_milli: Option<u16>,
    pub forecast_missing: bool,
    pub missing_fraction_milli: u16,
    pub quality_risk_milli: u16,
    pub disposition: QualityForecastDisposition,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveQualityForecast {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub summaries: Vec<ModalityQualityForecast>,
    pub forecast_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QualityForecastDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProspectiveQualityError {
    #[error("prospective quality request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective quality input is invalid: {0}")]
    InvalidInput(String),
    #[error("prospective quality output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective quality digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [u16]) -> u16 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn fraction_milli(numerator: usize, denominator: usize) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn digest_input(output: &ProspectiveQualityForecast) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "epoch_order": output.epoch_order,
        "modality_order": output.modality_order,
        "summaries": output.summaries,
        "forecast_order": output.forecast_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &ProspectiveQualityRequest) -> Result<(), ProspectiveQualityError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.epoch_order.len() < 2
        || request.epoch_order.len() > MAX_EPOCHS
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || !canonical(&request.required_modalities)
        || request.observations.len() > MAX_OBSERVATIONS
        || request.history_epochs == 0
        || request.history_epochs > request.epoch_order.len()
        || request.forecast_horizon == 0
        || request.min_history_points == 0
        || request.min_history_points > request.history_epochs
        || request.quality_floor_milli > 1_000
        || request.max_negative_slope_milli_per_epoch > 1_000
        || request.minimum_forecast_quality_milli > 1_000
    {
        return Err(ProspectiveQualityError::InvalidRequest(
            "objective, ordered epochs/modalities, bounded history/horizon, and quality gates are required".into(),
        ));
    }
    if !canonical(&request.epoch_order)
        || request
            .epoch_order
            .iter()
            .any(|epoch| epoch.trim().is_empty())
    {
        return Err(ProspectiveQualityError::InvalidRequest(
            "epoch identifiers must be unique, non-empty, and canonical".into(),
        ));
    }
    let epochs = request.epoch_order.iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &request.observations {
        if !epochs.contains(&observation.epoch_id)
            || observation.epoch_id.trim().is_empty()
            || !seen.insert((observation.epoch_id.clone(), observation.modality))
            || observation.quality_milli > 1_000
            || observation.expected_feature_count == 0
            || (observation.observed && observation.observed_feature_count == 0)
            || observation.observed_feature_count > observation.expected_feature_count
        {
            return Err(ProspectiveQualityError::InvalidInput(
                "observations require known epochs, unique epoch/modality cells, bounded quality, and valid feature coverage".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &ProspectiveQualityForecast) -> Result<(), ProspectiveQualityError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.epoch_order)
        || !canonical(&output.modality_order)
        || output.summaries.len() != output.modality_order.len()
        || output
            .summaries
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.summaries.iter().any(|summary| {
            summary
                .history_epoch_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        })
    {
        return Err(ProspectiveQualityError::InvalidOutput(
            "identity, canonical ordering, summary cardinality, or epoch ordering invariants are invalid".into(),
        ));
    }
    let mut seen_forecast = BTreeSet::new();
    if output
        .forecast_order
        .iter()
        .any(|modality| !seen_forecast.insert(*modality))
    {
        return Err(ProspectiveQualityError::InvalidOutput(
            "forecast order must contain unique modalities".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ProspectiveQualityError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ProspectiveQualityError::InvalidOutput(
            "digest is not bound to prospective quality output".into(),
        ));
    }
    Ok(())
}

impl ProspectiveQualityForecast {
    pub fn validate(&self) -> Result<(), ProspectiveQualityError> {
        validate_output(self)
    }
}

/// Forecast the next quality state of each required modality from local ordered QC metadata.
pub fn forecast_glioma_multimodal_quality(
    request: &ProspectiveQualityRequest,
) -> Result<ProspectiveQualityForecast, ProspectiveQualityError> {
    validate_request(request)?;
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let epoch_position = request
        .epoch_order
        .iter()
        .enumerate()
        .map(|(position, epoch)| (epoch.as_str(), position as i64))
        .collect::<HashMap<_, _>>();
    let mut observations = HashMap::new();
    for observation in &request.observations {
        observations.insert(
            (observation.epoch_id.as_str(), observation.modality),
            observation,
        );
    }
    let mut summaries = Vec::with_capacity(required.len());
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut risk_rank = Vec::new();
    for modality in &required {
        let history_epoch_order = request.epoch_order[..request.history_epochs].to_vec();
        let mut eligible = Vec::new();
        let mut missing = Vec::new();
        let mut quality_failed = Vec::new();
        for epoch in &history_epoch_order {
            match observations.get(&(epoch.as_str(), *modality)) {
                Some(observation)
                    if observation.observed
                        && observation.observed_feature_count > 0
                        && observation.quality_milli >= request.quality_floor_milli =>
                {
                    eligible.push((
                        *epoch_position.get(epoch.as_str()).unwrap_or(&0),
                        observation.quality_milli,
                    ));
                }
                Some(observation) if observation.observed => quality_failed.push(epoch.clone()),
                _ => missing.push(epoch.clone()),
            }
        }
        let eligible_epoch_order = eligible
            .iter()
            .filter_map(|(position, _)| request.epoch_order.get(*position as usize).cloned())
            .collect::<Vec<_>>();
        let baseline = if eligible.is_empty() {
            None
        } else {
            let mut values = eligible
                .iter()
                .map(|(_, quality)| *quality)
                .collect::<Vec<_>>();
            Some(median(&mut values))
        };
        let terminal = eligible.last().map(|(_, quality)| *quality);
        let slope = if eligible.len() < 2 {
            0
        } else {
            let (first_position, first_quality) = eligible[0];
            let (last_position, last_quality) =
                *eligible.last().unwrap_or(&(first_position, first_quality));
            let span = (last_position - first_position).max(1);
            (i64::from(last_quality) - i64::from(first_quality)) / span
        };
        let forecast_quality = terminal.map(|quality| {
            i64::from(quality)
                .saturating_add(slope.saturating_mul(i64::from(request.forecast_horizon)))
                .clamp(0, 1_000) as u16
        });
        let forecast_missing = eligible.len() < request.min_history_points
            || forecast_quality.is_none()
            || forecast_quality
                .is_some_and(|quality| quality < request.minimum_forecast_quality_milli);
        let missing_fraction = fraction_milli(
            missing.len() + quality_failed.len(),
            history_epoch_order.len(),
        );
        let slope_risk = if slope < 0 {
            (slope
                .unsigned_abs()
                .saturating_mul(u64::from(request.forecast_horizon))
                .min(1_000)) as u16
        } else {
            0
        };
        let forecast_gap = forecast_quality.map_or(1_000, |quality| {
            u16::from(
                request
                    .minimum_forecast_quality_milli
                    .saturating_sub(quality),
            )
        });
        let quality_risk = ((u32::from(missing_fraction) * 400 / 1_000)
            .saturating_add(u32::from(slope_risk) * 400 / 1_000)
            .saturating_add(u32::from(forecast_gap) * 200 / 1_000)
            .min(1_000)) as u16;
        let disposition = if eligible.len() < request.min_history_points {
            negative.insert(format!("insufficient-history:{modality:?}"));
            QualityForecastDisposition::Blocked
        } else if forecast_missing
            || (slope < 0 && slope.unsigned_abs() > request.max_negative_slope_milli_per_epoch)
        {
            uncertainty.insert(format!("future-quality-at-risk:{modality:?}"));
            risk_rank.push((*modality, quality_risk));
            QualityForecastDisposition::AtRisk
        } else {
            QualityForecastDisposition::Stable
        };
        if !missing.is_empty() {
            uncertainty.insert(format!("missing-history-epochs:{modality:?}"));
        }
        let next_action = match disposition {
            QualityForecastDisposition::Stable => "retain modality in next acquisition batch",
            QualityForecastDisposition::AtRisk => {
                "preflight modality and schedule preventive reacquisition"
            }
            QualityForecastDisposition::Blocked => {
                "collect additional QC history before forecasting"
            }
            QualityForecastDisposition::Unresolved => {
                "review modality QC provenance before scheduling"
            }
        }
        .into();
        summaries.push(ModalityQualityForecast {
            modality: *modality,
            history_epoch_order,
            eligible_epoch_order,
            missing_epoch_order: missing,
            quality_failed_epoch_order: quality_failed,
            baseline_quality_milli: baseline,
            terminal_quality_milli: terminal,
            signed_slope_milli_per_epoch: slope,
            forecast_quality_milli: forecast_quality,
            forecast_missing,
            missing_fraction_milli: missing_fraction,
            quality_risk_milli: quality_risk,
            disposition,
            next_action,
        });
    }
    summaries.sort_by(|left, right| left.modality.cmp(&right.modality));
    risk_rank.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let forecast_order = risk_rank
        .iter()
        .map(|(modality, _)| *modality)
        .collect::<Vec<_>>();
    let modality_order = required.iter().copied().collect::<Vec<_>>();
    let disposition = if summaries
        .iter()
        .all(|summary| summary.disposition == QualityForecastDisposition::Stable)
    {
        QualityForecastDisposition::Stable
    } else if summaries
        .iter()
        .any(|summary| summary.disposition == QualityForecastDisposition::Blocked)
    {
        QualityForecastDisposition::Blocked
    } else if summaries
        .iter()
        .any(|summary| summary.disposition == QualityForecastDisposition::AtRisk)
    {
        QualityForecastDisposition::AtRisk
    } else {
        QualityForecastDisposition::Unresolved
    };
    let next_action = match disposition {
        QualityForecastDisposition::Stable => "continue scheduled multimodal acquisition",
        QualityForecastDisposition::AtRisk => {
            "preflight and reacquire at-risk modalities before endpoint fusion"
        }
        QualityForecastDisposition::Blocked => {
            "collect the missing QC history before autonomous scheduling"
        }
        QualityForecastDisposition::Unresolved => "hold acquisition planning for QC adjudication",
    }
    .into();
    let mut output = ProspectiveQualityForecast {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        epoch_order: request.epoch_order.clone(),
        modality_order,
        summaries,
        forecast_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| ProspectiveQualityError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProspectiveQualityError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        epoch_id: &str,
        epoch_index: u32,
        modality: GliomaModality,
        quality_milli: u16,
    ) -> QualityForecastObservation {
        QualityForecastObservation {
            epoch_id: epoch_id.into(),
            epoch_index,
            modality,
            observed: true,
            quality_milli,
            expected_feature_count: 100,
            observed_feature_count: 100,
        }
    }

    fn request(observations: Vec<QualityForecastObservation>) -> ProspectiveQualityRequest {
        ProspectiveQualityRequest {
            objective: "forecast multimodal QC failure before autonomous acquisition".into(),
            study_id: "prospective-quality-study".into(),
            model_system: GliomaModelSystem::Organoid,
            epoch_order: vec!["epoch-0".into(), "epoch-1".into(), "epoch-2".into()],
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            history_epochs: 3,
            forecast_horizon: 2,
            min_history_points: 2,
            quality_floor_milli: 700,
            max_negative_slope_milli_per_epoch: 100,
            minimum_forecast_quality_milli: 700,
        }
    }

    #[test]
    fn forecasts_at_risk_modality_and_binds_digest() {
        let output = forecast_glioma_multimodal_quality(&request(vec![
            observation("epoch-0", 0, GliomaModality::Genomics, 950),
            observation("epoch-1", 1, GliomaModality::Genomics, 850),
            observation("epoch-2", 2, GliomaModality::Genomics, 750),
            observation("epoch-0", 0, GliomaModality::Imaging, 900),
            observation("epoch-1", 1, GliomaModality::Imaging, 900),
            observation("epoch-2", 2, GliomaModality::Imaging, 900),
        ]))
        .expect("quality forecast");
        assert_eq!(output.disposition, QualityForecastDisposition::AtRisk);
        assert_eq!(output.forecast_order[0], GliomaModality::Genomics);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn missing_history_blocks_without_forecasting_zero() {
        let output = forecast_glioma_multimodal_quality(&request(vec![
            observation("epoch-0", 0, GliomaModality::Genomics, 900),
            observation("epoch-1", 1, GliomaModality::Genomics, 900),
            observation("epoch-2", 2, GliomaModality::Genomics, 900),
        ]))
        .expect("blocked forecast");
        let imaging = output
            .summaries
            .iter()
            .find(|summary| summary.modality == GliomaModality::Imaging)
            .expect("imaging summary");
        assert_eq!(imaging.disposition, QualityForecastDisposition::Blocked);
        assert_eq!(imaging.forecast_quality_milli, None);
    }

    #[test]
    fn duplicate_epoch_modality_is_rejected() {
        let error = forecast_glioma_multimodal_quality(&request(vec![
            observation("epoch-0", 0, GliomaModality::Genomics, 900),
            observation("epoch-0", 0, GliomaModality::Genomics, 800),
        ]))
        .expect_err("duplicate cell");
        assert!(matches!(error, ProspectiveQualityError::InvalidInput(_)));
    }
}
