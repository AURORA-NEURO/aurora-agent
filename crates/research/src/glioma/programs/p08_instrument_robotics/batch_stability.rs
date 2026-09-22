//! High-throughput cross-run endpoint stability for preclinical glioma instruments.
//!
//! Signal extraction produces a trustworthy result for one local run, but an autonomous research
//! engine must also know whether an endpoint survives repeated runs. This capability aligns typed
//! extraction summaries by channel, measures coverage, robust amplitude dispersion, noise, and
//! first-to-last drift, and admits only reproducible channels to downstream assay adjudication or
//! mechanism analysis. It consumes summaries rather than raw traces and never executes hardware.

use super::signal_extraction::{InstrumentSignalExtraction, SignalChannelDisposition};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentBatchStability1@1";
pub const MAX_RUNS: usize = 4_096;
pub const MAX_CHANNELS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalBatchStabilityRequest {
    pub objective: String,
    pub instrument_id: String,
    pub min_runs: usize,
    pub min_channel_coverage_milli: u16,
    pub max_noise_milli: u64,
    pub max_dispersion_milli: u64,
    pub max_run_drift_milli: u64,
    pub min_peak_score_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSignalRun {
    pub run_id: String,
    pub sequence_index: u32,
    pub extraction: InstrumentSignalExtraction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStabilityDisposition {
    Qualified,
    CoverageBlocked,
    NoiseBlocked,
    DispersionBlocked,
    DriftBlocked,
    SignalBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelStability {
    pub channel_id: String,
    pub run_order: Vec<String>,
    pub usable_run_order: Vec<String>,
    pub coverage_milli: u16,
    pub median_signal_milli: u64,
    pub dispersion_milli: u64,
    pub median_noise_milli: u64,
    pub run_drift_milli: i64,
    pub median_peak_score_milli: u64,
    pub disposition: ChannelStabilityDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStabilityDisposition {
    Qualified,
    Partial,
    NoStableChannels,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentBatchStability {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub run_order: Vec<String>,
    pub channel_order: Vec<String>,
    pub channels: Vec<ChannelStability>,
    pub qualified_channel_order: Vec<String>,
    pub blocked_channel_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: BatchStabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BatchStabilityError {
    #[error("instrument batch stability request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument batch stability run is invalid: {0}")]
    InvalidRun(String),
    #[error("instrument batch stability output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument batch stability digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn digest_input(output: &InstrumentBatchStability) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_id": output.instrument_id,
        "run_order": output.run_order,
        "channel_order": output.channel_order,
        "channels": output.channels,
        "qualified_channel_order": output.qualified_channel_order,
        "blocked_channel_order": output.blocked_channel_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &SignalBatchStabilityRequest) -> Result<(), BatchStabilityError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.min_runs < 2
        || request.min_runs > MAX_RUNS
        || request.min_channel_coverage_milli == 0
        || request.min_channel_coverage_milli > 1_000
        || request.max_noise_milli == 0
        || request.max_dispersion_milli == 0
        || request.max_run_drift_milli == 0
        || request.min_peak_score_milli == 0
    {
        return Err(BatchStabilityError::InvalidRequest(
            "objective, instrument, bounded run floor, positive coverage/noise/dispersion/drift, and peak gates are required".into(),
        ));
    }
    Ok(())
}

fn validate_runs(
    request: &SignalBatchStabilityRequest,
    runs: &[InstrumentSignalRun],
) -> Result<(), BatchStabilityError> {
    if runs.len() < request.min_runs || runs.len() > MAX_RUNS {
        return Err(BatchStabilityError::InvalidRun(
            "run count does not meet the bounded minimum or maximum".into(),
        ));
    }
    let mut run_ids = BTreeSet::new();
    let mut sequence_ids = BTreeSet::new();
    for run in runs {
        if run.run_id.trim().is_empty()
            || !run_ids.insert(run.run_id.clone())
            || !sequence_ids.insert(run.sequence_index)
            || run.extraction.instrument_id != request.instrument_id
        {
            return Err(BatchStabilityError::InvalidRun(
                "run identities, unique sequence indices, instrument binding, and ordering are required".into(),
            ));
        }
        run.extraction
            .validate()
            .map_err(|error| BatchStabilityError::InvalidRun(error.to_string()))?;
    }
    Ok(())
}

fn validate_output(output: &InstrumentBatchStability) -> Result<(), BatchStabilityError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.instrument_id.trim().is_empty()
        || !canonical(&output.run_order)
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
                || !canonical(&channel.run_order)
                || !canonical(&channel.usable_run_order)
                || channel.coverage_milli > 1_000
                || channel.run_drift_milli.unsigned_abs() > u64::MAX / 2
        })
    {
        return Err(BatchStabilityError::InvalidOutput(
            "identity, canonical ordering, coverage, drift, or channel invariants are invalid"
                .into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| BatchStabilityError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(BatchStabilityError::InvalidOutput(
            "digest is not bound to instrument batch stability".into(),
        ));
    }
    Ok(())
}

impl InstrumentBatchStability {
    pub fn validate(&self) -> Result<(), BatchStabilityError> {
        validate_output(self)
    }
}

fn channel_stability(
    request: &SignalBatchStabilityRequest,
    channel_id: &str,
    runs: &[&InstrumentSignalRun],
) -> ChannelStability {
    let mut run_order = runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    run_order.sort();
    let mut observations = Vec::new();
    let mut usable_run_order = Vec::new();
    for run in runs {
        if let Some(channel) = run
            .extraction
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
        {
            if channel.disposition == SignalChannelDisposition::Qualified {
                let peak_score = channel
                    .peaks
                    .iter()
                    .map(|peak| peak.score_milli)
                    .max()
                    .unwrap_or(0);
                observations.push((
                    run.run_id.clone(),
                    channel.max_abs_residual_milli,
                    channel.noise_milli,
                    channel.drift_milli,
                    peak_score,
                ));
                usable_run_order.push(run.run_id.clone());
            }
        }
    }
    usable_run_order.sort();
    let coverage_milli = ((observations.len() as u64 * 1_000) / runs.len() as u64) as u16;
    if observations.is_empty() {
        return ChannelStability {
            channel_id: channel_id.into(),
            run_order,
            usable_run_order,
            coverage_milli,
            median_signal_milli: 0,
            dispersion_milli: 0,
            median_noise_milli: 0,
            run_drift_milli: 0,
            median_peak_score_milli: 0,
            disposition: ChannelStabilityDisposition::SignalBlocked,
        };
    }
    let mut signals = observations
        .iter()
        .map(|(_, signal, _, _, _)| *signal)
        .collect::<Vec<_>>();
    let median_signal_milli = median(&mut signals);
    let mut deviations = observations
        .iter()
        .map(|(_, signal, _, _, _)| signal.abs_diff(median_signal_milli))
        .collect::<Vec<_>>();
    let dispersion_milli = median(&mut deviations);
    let mut noise = observations
        .iter()
        .map(|(_, _, noise, _, _)| *noise)
        .collect::<Vec<_>>();
    let median_noise_milli = median(&mut noise);
    let mut peak_scores = observations
        .iter()
        .map(|(_, _, _, _, score)| *score)
        .collect::<Vec<_>>();
    let median_peak_score_milli = median(&mut peak_scores);
    let run_drift_milli = observations
        .last()
        .map(|last| last.1 as i128)
        .unwrap_or(0)
        .saturating_sub(
            observations
                .first()
                .map(|first| first.1 as i128)
                .unwrap_or(0),
        )
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    let disposition = if coverage_milli < request.min_channel_coverage_milli {
        ChannelStabilityDisposition::CoverageBlocked
    } else if median_peak_score_milli < request.min_peak_score_milli {
        ChannelStabilityDisposition::SignalBlocked
    } else if median_noise_milli > request.max_noise_milli {
        ChannelStabilityDisposition::NoiseBlocked
    } else if dispersion_milli > request.max_dispersion_milli {
        ChannelStabilityDisposition::DispersionBlocked
    } else if run_drift_milli.unsigned_abs() > request.max_run_drift_milli {
        ChannelStabilityDisposition::DriftBlocked
    } else {
        ChannelStabilityDisposition::Qualified
    };
    ChannelStability {
        channel_id: channel_id.into(),
        run_order,
        usable_run_order,
        coverage_milli,
        median_signal_milli,
        dispersion_milli,
        median_noise_milli,
        run_drift_milli,
        median_peak_score_milli,
        disposition,
    }
}

/// Evaluate endpoint stability across a bounded prospective instrument run batch.
pub fn analyze_glioma_instrument_batch_stability(
    request: &SignalBatchStabilityRequest,
    runs: &[InstrumentSignalRun],
) -> Result<InstrumentBatchStability, BatchStabilityError> {
    validate_request(request)?;
    validate_runs(request, runs)?;
    let mut ordered_runs = runs.iter().collect::<Vec<_>>();
    ordered_runs.sort_by(|left, right| {
        (left.sequence_index, &left.run_id).cmp(&(right.sequence_index, &right.run_id))
    });
    let run_order = ordered_runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    let mut channel_ids = BTreeSet::new();
    for run in &ordered_runs {
        channel_ids.extend(
            run.extraction
                .channels
                .iter()
                .map(|channel| channel.channel_id.clone()),
        );
    }
    if channel_ids.len() > MAX_CHANNELS {
        return Err(BatchStabilityError::InvalidRun(
            "channel count exceeds the bounded stability surface".into(),
        ));
    }
    let channel_order = channel_ids.iter().cloned().collect::<Vec<_>>();
    let channels = channel_order
        .iter()
        .map(|channel_id| channel_stability(request, channel_id, &ordered_runs))
        .collect::<Vec<_>>();
    let qualified_channel_order = channels
        .iter()
        .filter(|channel| channel.disposition == ChannelStabilityDisposition::Qualified)
        .map(|channel| channel.channel_id.clone())
        .collect::<Vec<_>>();
    let blocked_channel_order = channels
        .iter()
        .filter(|channel| channel.disposition != ChannelStabilityDisposition::Qualified)
        .map(|channel| channel.channel_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = channels
        .iter()
        .filter(|channel| channel.disposition != ChannelStabilityDisposition::Qualified)
        .map(|channel| format!("{:?}:{}", channel.disposition, channel.channel_id).to_lowercase())
        .collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = Vec::new();
    if !blocked_channel_order.is_empty() {
        uncertainty.push("one-or-more-endpoints-failed-prospective-stability-gates".into());
    }
    if ordered_runs.len() < request.min_runs {
        uncertainty.push("run-batch-is-underpowered-for-configured-minimum".into());
    }
    uncertainty.sort();
    let disposition = if qualified_channel_order.len() == channel_order.len() {
        BatchStabilityDisposition::Qualified
    } else if !qualified_channel_order.is_empty() {
        BatchStabilityDisposition::Partial
    } else if channel_order.is_empty() {
        BatchStabilityDisposition::Unresolved
    } else {
        BatchStabilityDisposition::NoStableChannels
    };
    let mut output = InstrumentBatchStability {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        run_order,
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
        .map_err(|error| BatchStabilityError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::signal_extraction::{
        extract_glioma_instrument_signal, InstrumentSignalPoint, SignalExtractionRequest,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn extraction(run_offset: i64, peak_offset: i64) -> InstrumentSignalExtraction {
        let request = SignalExtractionRequest {
            objective: "endpoint".into(),
            instrument_id: "scope-a".into(),
            model_system: GliomaModelSystem::Organoid,
            baseline_window: 3,
            min_quality_milli: 700,
            peak_threshold_milli: 250,
            max_drift_milli: 100,
            min_peak_spacing: 2,
            max_peaks_per_channel: 2,
        };
        let points = (0..8)
            .map(|index| InstrumentSignalPoint {
                point_id: format!("p-{index}"),
                channel_id: "invasion".into(),
                sequence_index: index,
                value_milli: if index == 3 || index == 6 {
                    500 + peak_offset
                } else {
                    100 + run_offset
                },
                quality_milli: 900,
                local_only: true,
                contains_human_data: false,
                artifact: None,
            })
            .collect::<Vec<_>>();
        extract_glioma_instrument_signal(&request, &points).expect("extraction")
    }

    fn request() -> SignalBatchStabilityRequest {
        SignalBatchStabilityRequest {
            objective: "stability".into(),
            instrument_id: "scope-a".into(),
            min_runs: 3,
            min_channel_coverage_milli: 800,
            max_noise_milli: 250,
            max_dispersion_milli: 100,
            max_run_drift_milli: 100,
            min_peak_score_milli: 200,
        }
    }

    #[test]
    fn qualifies_replayable_stable_batch() {
        let runs = vec![
            InstrumentSignalRun {
                run_id: "r-1".into(),
                sequence_index: 1,
                extraction: extraction(0, 0),
            },
            InstrumentSignalRun {
                run_id: "r-2".into(),
                sequence_index: 2,
                extraction: extraction(10, 10),
            },
            InstrumentSignalRun {
                run_id: "r-3".into(),
                sequence_index: 3,
                extraction: extraction(5, 5),
            },
        ];
        let first =
            analyze_glioma_instrument_batch_stability(&request(), &runs).expect("stability");
        let second =
            analyze_glioma_instrument_batch_stability(&request(), &runs).expect("stability");
        assert_eq!(first, second);
        assert_eq!(first.disposition, BatchStabilityDisposition::Qualified);
        first.validate().expect("valid stability");
    }

    #[test]
    fn drift_is_negative_evidence() {
        let runs = vec![
            InstrumentSignalRun {
                run_id: "r-1".into(),
                sequence_index: 1,
                extraction: extraction(0, 0),
            },
            InstrumentSignalRun {
                run_id: "r-2".into(),
                sequence_index: 2,
                extraction: extraction(0, 0),
            },
            InstrumentSignalRun {
                run_id: "r-3".into(),
                sequence_index: 3,
                extraction: extraction(0, 300),
            },
        ];
        let output =
            analyze_glioma_instrument_batch_stability(&request(), &runs).expect("stability");
        assert_eq!(
            output.disposition,
            BatchStabilityDisposition::NoStableChannels
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("driftblocked")));
    }
}
