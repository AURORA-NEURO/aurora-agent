//! Robust local instrument-signal extraction for preclinical glioma workflows.
//!
//! This capability converts value-only instrument points into reproducible endpoint candidates.
//! It uses local median baselines, residual/noise estimates, quality gates, drift detection, and
//! spacing-constrained peak selection. Raw traces stay behind the institution-local gateway; the
//! returned artifact is a bounded analysis result for assay adjudication and downstream planning.
//! It never executes hardware or makes a clinical decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentSignalExtraction1@1";
pub const MAX_POINTS: usize = 32_768;
pub const MAX_CHANNELS: usize = 256;
pub const MAX_PEAKS: usize = 1_024;
pub const MAX_VALUE_MILLI: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalExtractionRequest {
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub baseline_window: usize,
    pub min_quality_milli: u16,
    pub peak_threshold_milli: u64,
    pub max_drift_milli: u64,
    pub min_peak_spacing: u32,
    pub max_peaks_per_channel: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSignalPoint {
    pub point_id: String,
    pub channel_id: String,
    pub sequence_index: u32,
    pub value_milli: i64,
    pub quality_milli: u16,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub artifact: Option<LocalArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSignalPeak {
    pub point_id: String,
    pub sequence_index: u32,
    pub signed_residual_milli: i64,
    pub score_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalChannelDisposition {
    Qualified,
    DriftBlocked,
    QualityBlocked,
    NoSignal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSignalChannel {
    pub channel_id: String,
    pub point_order: Vec<String>,
    pub usable_point_order: Vec<String>,
    pub rejected_point_order: Vec<String>,
    pub baseline_milli: i64,
    pub noise_milli: u64,
    pub max_abs_residual_milli: u64,
    pub drift_milli: i64,
    pub peaks: Vec<InstrumentSignalPeak>,
    pub quality_milli: u16,
    pub disposition: SignalChannelDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalExtractionDisposition {
    Qualified,
    Partial,
    NoUsableChannels,
    DriftBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSignalExtraction {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub channel_order: Vec<String>,
    pub channels: Vec<InstrumentSignalChannel>,
    pub qualified_channel_order: Vec<String>,
    pub blocked_channel_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SignalExtractionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignalExtractionError {
    #[error("instrument signal extraction request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument signal point is invalid: {0}")]
    InvalidPoint(String),
    #[error("instrument signal extraction output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument signal extraction digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn digest_input(output: &InstrumentSignalExtraction) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_id": output.instrument_id,
        "model_system": output.model_system,
        "channel_order": output.channel_order,
        "channels": output.channels,
        "qualified_channel_order": output.qualified_channel_order,
        "blocked_channel_order": output.blocked_channel_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &SignalExtractionRequest) -> Result<(), SignalExtractionError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.baseline_window == 0
        || request.baseline_window > MAX_POINTS
        || request.min_quality_milli > 1_000
        || request.peak_threshold_milli == 0
        || request.max_drift_milli == 0
        || request.max_peaks_per_channel == 0
        || request.max_peaks_per_channel > MAX_PEAKS
    {
        return Err(SignalExtractionError::InvalidRequest(
            "objective, instrument, bounded baseline window, quality, peak, drift, and peak-count gates are required".into(),
        ));
    }
    Ok(())
}

fn validate_points(points: &[InstrumentSignalPoint]) -> Result<(), SignalExtractionError> {
    if points.is_empty() || points.len() > MAX_POINTS {
        return Err(SignalExtractionError::InvalidPoint(
            "a non-empty bounded point set is required".into(),
        ));
    }
    let mut point_ids = BTreeSet::new();
    let mut channel_sequence = BTreeSet::new();
    let mut channels = BTreeSet::new();
    for point in points {
        if point.point_id.trim().is_empty()
            || point.channel_id.trim().is_empty()
            || !point_ids.insert(point.point_id.clone())
            || !channel_sequence.insert((point.channel_id.clone(), point.sequence_index))
            || !point.local_only
            || point.contains_human_data
            || point.quality_milli > 1_000
            || point.value_milli.unsigned_abs() > MAX_VALUE_MILLI as u64
        {
            return Err(SignalExtractionError::InvalidPoint(
                "point identity, channel/sequence uniqueness, local-only boundary, quality, and value bounds are required".into(),
            ));
        }
        if let Some(artifact) = &point.artifact {
            if !artifact.local_only
                || artifact.contains_human_data
                || artifact.contains_direct_identifiers
                || artifact.artifact_id.trim().is_empty()
            {
                return Err(SignalExtractionError::InvalidPoint(
                    "optional artifacts must remain local, de-identified, and content-addressed"
                        .into(),
                ));
            }
        }
        channels.insert(point.channel_id.clone());
    }
    if channels.len() > MAX_CHANNELS {
        return Err(SignalExtractionError::InvalidPoint(
            "channel count exceeds the bounded extraction surface".into(),
        ));
    }
    Ok(())
}

fn validate_output(output: &InstrumentSignalExtraction) -> Result<(), SignalExtractionError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.instrument_id.trim().is_empty()
        || !canonical(&output.channel_order)
        || !canonical(&output.qualified_channel_order)
        || !canonical(&output.blocked_channel_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .channels
            .windows(2)
            .any(|pair| pair[0].channel_id >= pair[1].channel_id)
        || output.channels.iter().any(|channel| {
            channel.channel_id.trim().is_empty()
                || !canonical(&channel.point_order)
                || !canonical(&channel.usable_point_order)
                || !canonical(&channel.rejected_point_order)
                || channel.quality_milli > 1_000
                || channel.peaks.windows(2).any(|pair| {
                    (pair[0].sequence_index, &pair[0].point_id)
                        >= (pair[1].sequence_index, &pair[1].point_id)
                })
                || channel.peaks.iter().any(|peak| peak.score_milli == 0)
        })
    {
        return Err(SignalExtractionError::InvalidOutput(
            "identity, canonical ordering, channel bounds, or peak ordering is invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| SignalExtractionError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(SignalExtractionError::InvalidOutput(
            "digest is not bound to instrument signal extraction".into(),
        ));
    }
    Ok(())
}

impl InstrumentSignalExtraction {
    pub fn validate(&self) -> Result<(), SignalExtractionError> {
        validate_output(self)
    }
}

fn local_baseline(values: &[i64], index: usize, window: usize) -> i64 {
    let half = window / 2;
    let start = index.saturating_sub(half);
    let end = (index + half + 1).min(values.len());
    let mut slice = values[start..end].to_vec();
    median(&mut slice)
}

fn quality_average(points: &[&InstrumentSignalPoint]) -> u16 {
    if points.is_empty() {
        0
    } else {
        (points
            .iter()
            .map(|point| u32::from(point.quality_milli))
            .sum::<u32>()
            / points.len() as u32) as u16
    }
}

fn channel_analysis(
    request: &SignalExtractionRequest,
    channel_id: &str,
    points: &[&InstrumentSignalPoint],
) -> InstrumentSignalChannel {
    let mut ordered = points.to_vec();
    ordered.sort_by(|left, right| {
        (left.sequence_index, &left.point_id).cmp(&(right.sequence_index, &right.point_id))
    });
    let point_order = ordered
        .iter()
        .map(|point| point.point_id.clone())
        .collect::<Vec<_>>();
    let usable = ordered
        .iter()
        .copied()
        .filter(|point| point.quality_milli >= request.min_quality_milli)
        .collect::<Vec<_>>();
    let usable_point_order = usable
        .iter()
        .map(|point| point.point_id.clone())
        .collect::<Vec<_>>();
    let rejected_point_order = ordered
        .iter()
        .filter(|point| point.quality_milli < request.min_quality_milli)
        .map(|point| point.point_id.clone())
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return InstrumentSignalChannel {
            channel_id: channel_id.into(),
            point_order,
            usable_point_order,
            rejected_point_order,
            baseline_milli: 0,
            noise_milli: 0,
            max_abs_residual_milli: 0,
            drift_milli: 0,
            peaks: Vec::new(),
            quality_milli: 0,
            disposition: SignalChannelDisposition::QualityBlocked,
        };
    }
    let values = usable
        .iter()
        .map(|point| point.value_milli)
        .collect::<Vec<_>>();
    let baselines = values
        .iter()
        .enumerate()
        .map(|(index, _)| local_baseline(&values, index, request.baseline_window))
        .collect::<Vec<_>>();
    let residuals = usable
        .iter()
        .enumerate()
        .map(|(index, point)| point.value_milli.saturating_sub(baselines[index]))
        .collect::<Vec<_>>();
    let baseline_milli = {
        let mut copy = baselines.clone();
        median(&mut copy)
    };
    let mut absolute_residuals = residuals
        .iter()
        .map(|value| value.unsigned_abs())
        .collect::<Vec<_>>();
    absolute_residuals.sort_unstable();
    let noise_milli = absolute_residuals[absolute_residuals.len() / 2];
    let max_abs_residual_milli = absolute_residuals.iter().copied().max().unwrap_or(0);
    let edge_window = request.baseline_window.min(baselines.len()).max(1);
    let first_edge = {
        let mut copy = baselines[..edge_window].to_vec();
        median(&mut copy)
    };
    let last_edge = {
        let mut copy = baselines[baselines.len() - edge_window..].to_vec();
        median(&mut copy)
    };
    let drift_milli = last_edge.saturating_sub(first_edge);
    let mut candidate_peaks = usable
        .iter()
        .enumerate()
        .filter_map(|(index, point)| {
            let residual = residuals[index];
            let magnitude = residual.unsigned_abs();
            if magnitude < request.peak_threshold_milli {
                return None;
            }
            let score_milli = magnitude.saturating_mul(u64::from(point.quality_milli)) / 1_000;
            Some(InstrumentSignalPeak {
                point_id: point.point_id.clone(),
                sequence_index: point.sequence_index,
                signed_residual_milli: residual,
                score_milli,
            })
        })
        .collect::<Vec<_>>();
    candidate_peaks.sort_by(|left, right| {
        right
            .score_milli
            .cmp(&left.score_milli)
            .then_with(|| left.sequence_index.cmp(&right.sequence_index))
            .then_with(|| left.point_id.cmp(&right.point_id))
    });
    let mut peaks = Vec::new();
    for candidate in candidate_peaks {
        if peaks.iter().all(|selected: &InstrumentSignalPeak| {
            selected.sequence_index.abs_diff(candidate.sequence_index) >= request.min_peak_spacing
        }) {
            peaks.push(candidate);
        }
        if peaks.len() >= request.max_peaks_per_channel {
            break;
        }
    }
    peaks.sort_by(|left, right| {
        (left.sequence_index, &left.point_id).cmp(&(right.sequence_index, &right.point_id))
    });
    let disposition = if drift_milli.unsigned_abs() > request.max_drift_milli {
        SignalChannelDisposition::DriftBlocked
    } else if peaks.is_empty() {
        SignalChannelDisposition::NoSignal
    } else {
        SignalChannelDisposition::Qualified
    };
    InstrumentSignalChannel {
        channel_id: channel_id.into(),
        point_order,
        usable_point_order,
        rejected_point_order,
        baseline_milli,
        noise_milli,
        max_abs_residual_milli,
        drift_milli,
        peaks,
        quality_milli: quality_average(&usable),
        disposition,
    }
}

/// Extract bounded, quality- and drift-gated signal endpoints from local instrument points.
pub fn extract_glioma_instrument_signal(
    request: &SignalExtractionRequest,
    points: &[InstrumentSignalPoint],
) -> Result<InstrumentSignalExtraction, SignalExtractionError> {
    validate_request(request)?;
    validate_points(points)?;
    let mut by_channel: BTreeMap<String, Vec<&InstrumentSignalPoint>> = BTreeMap::new();
    for point in points {
        by_channel
            .entry(point.channel_id.clone())
            .or_default()
            .push(point);
    }
    let channel_order = by_channel.keys().cloned().collect::<Vec<_>>();
    let channels = by_channel
        .iter()
        .map(|(channel_id, points)| channel_analysis(request, channel_id, points))
        .collect::<Vec<_>>();
    let qualified_channel_order = channels
        .iter()
        .filter(|channel| channel.disposition == SignalChannelDisposition::Qualified)
        .map(|channel| channel.channel_id.clone())
        .collect::<Vec<_>>();
    let blocked_channel_order = channels
        .iter()
        .filter(|channel| channel.disposition != SignalChannelDisposition::Qualified)
        .map(|channel| channel.channel_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = channels
        .iter()
        .flat_map(|channel| {
            let mut evidence = channel
                .rejected_point_order
                .iter()
                .map(|point_id| format!("quality-gate-blocked:{point_id}"))
                .collect::<Vec<_>>();
            match channel.disposition {
                SignalChannelDisposition::DriftBlocked => {
                    evidence.push(format!("drift-gate-blocked:{}", channel.channel_id));
                }
                SignalChannelDisposition::QualityBlocked => {
                    evidence.push(format!(
                        "quality-gate-blocked-channel:{}",
                        channel.channel_id
                    ));
                }
                SignalChannelDisposition::NoSignal => {
                    evidence.push(format!("no-signal:{}", channel.channel_id));
                }
                SignalChannelDisposition::Qualified => {}
            }
            evidence
        })
        .collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = Vec::new();
    if !blocked_channel_order.is_empty() {
        uncertainty
            .push("one-or-more-instrument-channels-failed-quality-drift-or-signal-gates".into());
    }
    if channels
        .iter()
        .any(|channel| channel.noise_milli > request.peak_threshold_milli)
    {
        uncertainty.push("robust-noise-is-large-relative-to-peak-threshold".into());
    }
    uncertainty.sort();
    let disposition = if qualified_channel_order.len() == channels.len() {
        SignalExtractionDisposition::Qualified
    } else if qualified_channel_order.is_empty()
        && channels
            .iter()
            .all(|channel| channel.disposition == SignalChannelDisposition::QualityBlocked)
    {
        SignalExtractionDisposition::NoUsableChannels
    } else if qualified_channel_order.is_empty()
        && channels
            .iter()
            .any(|channel| channel.disposition == SignalChannelDisposition::DriftBlocked)
    {
        SignalExtractionDisposition::DriftBlocked
    } else if !qualified_channel_order.is_empty() {
        SignalExtractionDisposition::Partial
    } else {
        SignalExtractionDisposition::Unresolved
    };
    let mut output = InstrumentSignalExtraction {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        model_system: request.model_system,
        channel_order,
        channels,
        qualified_channel_order,
        blocked_channel_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SignalExtractionError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(
        channel: &str,
        sequence_index: u32,
        value_milli: i64,
        quality_milli: u16,
    ) -> InstrumentSignalPoint {
        InstrumentSignalPoint {
            point_id: format!("{channel}-{sequence_index}"),
            channel_id: channel.into(),
            sequence_index,
            value_milli,
            quality_milli,
            local_only: true,
            contains_human_data: false,
            artifact: None,
        }
    }

    fn request() -> SignalExtractionRequest {
        SignalExtractionRequest {
            objective: "extract invasion assay endpoints".into(),
            instrument_id: "microscope-a".into(),
            model_system: GliomaModelSystem::Organoid,
            baseline_window: 3,
            min_quality_milli: 700,
            peak_threshold_milli: 250,
            max_drift_milli: 100,
            min_peak_spacing: 2,
            max_peaks_per_channel: 4,
        }
    }

    #[test]
    fn extracts_replayable_quality_gated_peaks() {
        let points = vec![
            point("signal", 0, 100, 900),
            point("signal", 1, 105, 900),
            point("signal", 2, 500, 900),
            point("signal", 3, 108, 900),
            point("signal", 4, 100, 900),
            point("signal", 5, 110, 900),
            point("signal", 6, 470, 900),
            point("signal", 7, 108, 900),
            point("signal", 8, 105, 900),
            point("signal", 9, 110, 900),
        ];
        let first = extract_glioma_instrument_signal(&request(), &points).expect("extraction");
        let second = extract_glioma_instrument_signal(&request(), &points).expect("extraction");
        assert_eq!(first, second);
        assert_eq!(first.disposition, SignalExtractionDisposition::Qualified);
        assert_eq!(first.channels[0].peaks.len(), 2);
        first.validate().expect("valid extraction");
    }

    #[test]
    fn drift_blocks_channel_without_promoting_signal() {
        let mut points = Vec::new();
        for index in 0..8 {
            points.push(point("drifting", index, i64::from(index) * 80, 900));
        }
        let output = extract_glioma_instrument_signal(&request(), &points).expect("extraction");
        assert_eq!(
            output.disposition,
            SignalExtractionDisposition::DriftBlocked
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "drift-gate-blocked:drifting"));
    }

    #[test]
    fn human_data_is_rejected_at_gateway_boundary() {
        let mut points = vec![point("signal", 0, 100, 900), point("signal", 1, 110, 900)];
        points[0].contains_human_data = true;
        assert!(matches!(
            extract_glioma_instrument_signal(&request(), &points),
            Err(SignalExtractionError::InvalidPoint(_))
        ));
    }
}
