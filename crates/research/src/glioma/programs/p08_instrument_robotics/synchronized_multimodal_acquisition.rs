//! Synchronized multimodal acquisition for preclinical glioma assays.
//!
//! A glioma state-transition experiment is often observed by several local devices: a time-lapse
//! microscope, a plate reader, a perturbation counter, or a sequencing handoff.  Running those
//! devices independently creates a false sense of multimodal completeness when one device drifts,
//! starts late, or fails after the other devices finish.  This module owns the useful product
//! behavior: a bounded shared timeline, calibrated device-clock conversion, gateway execution,
//! temporal alignment, quality gates, and an honest complete/partial/blocked bundle decision.
//!
//! The research crate does not open hardware connections.  An institution-owned executor performs
//! one capture at a time and can request an emergency stop.  Raw signals remain local; the output
//! contains only typed local artifact references and value-level alignment diagnostics.  A complete
//! bundle is never produced from a dry-run or from an incomplete required modality.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaSynchronizedMultimodalAcquisition1@1";
pub const MAX_CHANNELS: usize = 128;
pub const MAX_CALIBRATIONS: usize = 128;
pub const MAX_WINDOW_TICKS: u64 = 10_000_000_000;
pub const MAX_SKEW_TICKS: u64 = 1_000_000;
pub const MAX_RISK_MILLI: u16 = 1_000;
const ACQUISITION_ADMISSION_BEAM_WIDTH: usize = 256;

/// One modality capture in the shared experiment timeline.  `risk_milli` is a bounded resource
/// budget, not a biological risk estimate and cannot authorize a physical operation by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedAcquisitionChannel {
    pub channel_id: String,
    pub modality: GliomaModality,
    pub instrument_id: String,
    pub action_id: String,
    pub output_schema: String,
    pub start_offset_ticks: u64,
    pub duration_ticks: u64,
    pub risk_milli: u16,
    pub required: bool,
    pub minimum_quality_milli: u16,
}

/// Local calibration of an instrument clock against the assay timeline.  Drift is expressed in
/// parts-per-million milli-units, so conversion is integer-only and replayable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentClockCalibration {
    pub instrument_id: String,
    pub calibration_id: String,
    pub offset_ticks: i64,
    pub drift_ppm_milli: i32,
    pub valid_from_tick: u64,
    pub valid_until_tick: u64,
}

/// A request to coordinate one multimodal assay window for an opaque local preclinical sample
/// scope.  The sample scope is deliberately not a human identity and is never sent to a gateway
/// outside the institution by this library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedAcquisitionRequest {
    pub objective: String,
    pub study_id: String,
    pub sample_scope: String,
    pub model_system: GliomaModelSystem,
    pub window_start_tick: u64,
    pub window_end_tick: u64,
    pub maximum_temporal_skew_ticks: u64,
    pub maximum_total_risk_milli: u32,
    pub channels: Vec<SynchronizedAcquisitionChannel>,
    pub calibrations: Vec<InstrumentClockCalibration>,
}

/// Device-clock and global-clock timing sent to the gateway.  The executor must use the device
/// ticks when talking to hardware; the engine uses the calibration to compare captures globally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedCapture {
    pub channel_id: String,
    pub modality: GliomaModality,
    pub instrument_id: String,
    pub action_id: String,
    pub output_schema: String,
    pub global_start_tick: u64,
    pub global_end_tick: u64,
    pub device_start_tick: u64,
    pub device_end_tick: u64,
    pub calibration_id: String,
    pub required: bool,
    pub minimum_quality_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynchronizedCaptureDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Unresolved,
}

/// Gateway result for one capture.  Completed captures need both timestamps and a local artifact;
/// a gateway cannot manufacture a biological measurement by returning a success flag alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedCaptureOutcome {
    pub channel_id: String,
    pub disposition: SynchronizedCaptureDisposition,
    pub observed_start_tick: Option<u64>,
    pub observed_end_tick: Option<u64>,
    pub quality_milli: u16,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynchronizedAcquisitionExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local gateway boundary.  Transport, authorization, interlocks, and emergency
/// stop wiring remain outside the research crate.
pub trait SynchronizedAcquisitionExecutor {
    fn execute_capture(
        &mut self,
        capture: &SynchronizedCapture,
    ) -> Result<SynchronizedCaptureOutcome, SynchronizedAcquisitionExecutionFailure>;

    fn emergency_stop(&mut self) -> Result<(), SynchronizedAcquisitionExecutionFailure>;

    fn simulation_only(&self) -> bool {
        false
    }
}

/// Deterministic executor for sandboxes and integration tests.  It returns local synthetic
/// artifacts and never opens a device connection or creates biological evidence.
#[derive(Debug, Default)]
pub struct DryRunSynchronizedAcquisitionExecutor {
    pub fail_channel_ids: BTreeSet<String>,
    pub emergency_stop_called: bool,
}

impl SynchronizedAcquisitionExecutor for DryRunSynchronizedAcquisitionExecutor {
    fn execute_capture(
        &mut self,
        capture: &SynchronizedCapture,
    ) -> Result<SynchronizedCaptureOutcome, SynchronizedAcquisitionExecutionFailure> {
        if self.fail_channel_ids.contains(&capture.channel_id) {
            return Ok(SynchronizedCaptureOutcome {
                channel_id: capture.channel_id.clone(),
                disposition: SynchronizedCaptureDisposition::Failed,
                observed_start_tick: None,
                observed_end_tick: None,
                quality_milli: 0,
                artifact: None,
                note: "synthetic gateway failure".into(),
                uncertainty: vec!["synthetic-capture-failure".into()],
                negative_evidence: vec!["required-modality-not-captured".into()],
            });
        }
        let hash = ContentHash::of_value(&serde_json::json!({
            "channel_id": capture.channel_id,
            "instrument_id": capture.instrument_id,
            "action_id": capture.action_id,
            "device_start_tick": capture.device_start_tick,
            "device_end_tick": capture.device_end_tick,
            "simulation_only": true,
        }))
        .map_err(|error| SynchronizedAcquisitionExecutionFailure {
            reason: format!("synthetic artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(SynchronizedCaptureOutcome {
            channel_id: capture.channel_id.clone(),
            disposition: SynchronizedCaptureDisposition::Completed,
            observed_start_tick: Some(capture.device_start_tick),
            observed_end_tick: Some(capture.device_end_tick),
            quality_milli: 1_000,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run-synchronized:{}", capture.channel_id),
                content_hash: hash,
                content_type: capture.output_schema.clone(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            note: "synthetic synchronized capture; no hardware or biological effect occurred"
                .into(),
            uncertainty: vec!["simulation-only-operation".into()],
            negative_evidence: vec!["synthetic-operation-is-not-biological-evidence".into()],
        })
    }

    fn emergency_stop(&mut self) -> Result<(), SynchronizedAcquisitionExecutionFailure> {
        self.emergency_stop_called = true;
        Ok(())
    }

    fn simulation_only(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedCaptureAlignment {
    pub channel_id: String,
    pub expected_global_start_tick: u64,
    pub observed_device_start_tick: u64,
    pub corrected_global_start_tick: u64,
    pub skew_from_expected_ticks: i64,
    pub quality_milli: u16,
    pub disposition: SynchronizedCaptureDisposition,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynchronizedAcquisitionDisposition {
    Complete,
    Partial,
    Blocked,
    Unresolved,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynchronizedAcquisitionStopReason {
    Complete,
    RequiredCaptureFailed,
    TemporalSkewExceeded,
    QualityGateFailed,
    NoRunnableChannels,
    ExecutorFailed,
    UnresolvedTiming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedMultimodalAcquisition {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub sample_scope: String,
    pub model_system: GliomaModelSystem,
    pub channel_order: Vec<String>,
    pub capture_order: Vec<String>,
    pub required_channel_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub alignments: Vec<SynchronizedCaptureAlignment>,
    pub total_risk_milli: u32,
    pub temporal_skew_ticks: Option<u64>,
    pub bundle_artifact: Option<LocalArtifactRef>,
    pub simulation_only: bool,
    pub emergency_stop_requested: bool,
    pub emergency_stop_succeeded: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SynchronizedAcquisitionDisposition,
    pub stop_reason: SynchronizedAcquisitionStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SynchronizedAcquisitionError {
    #[error("synchronized acquisition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("synchronized acquisition gateway failed: {0}")]
    Gateway(String),
    #[error("synchronized acquisition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("synchronized acquisition digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn sorted_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn digest_input(output: &SynchronizedMultimodalAcquisition) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "sample_scope": output.sample_scope,
        "model_system": output.model_system,
        "channel_order": output.channel_order,
        "capture_order": output.capture_order,
        "required_channel_order": output.required_channel_order,
        "completed_order": output.completed_order,
        "failed_order": output.failed_order,
        "negative_order": output.negative_order,
        "unresolved_order": output.unresolved_order,
        "blocked_order": output.blocked_order,
        "alignments": output.alignments,
        "total_risk_milli": output.total_risk_milli,
        "temporal_skew_ticks": output.temporal_skew_ticks,
        "bundle_artifact": output.bundle_artifact,
        "simulation_only": output.simulation_only,
        "emergency_stop_requested": output.emergency_stop_requested,
        "emergency_stop_succeeded": output.emergency_stop_succeeded,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn calibration_correction(
    calibration: &InstrumentClockCalibration,
    global_tick: u64,
) -> Result<i64, SynchronizedAcquisitionError> {
    if global_tick < calibration.valid_from_tick || global_tick > calibration.valid_until_tick {
        return Err(SynchronizedAcquisitionError::InvalidRequest(format!(
            "capture tick {global_tick} is outside calibration {} validity window",
            calibration.calibration_id
        )));
    }
    let elapsed = i128::from(global_tick - calibration.valid_from_tick);
    let drift = elapsed
        .checked_mul(i128::from(calibration.drift_ppm_milli))
        .ok_or_else(|| {
            SynchronizedAcquisitionError::InvalidRequest("clock drift overflow".into())
        })?
        / 1_000_000_i128;
    let correction = i128::from(calibration.offset_ticks)
        .checked_add(drift)
        .ok_or_else(|| {
            SynchronizedAcquisitionError::InvalidRequest("clock correction overflow".into())
        })?;
    i64::try_from(correction).map_err(|_| {
        SynchronizedAcquisitionError::InvalidRequest("clock correction exceeds bounds".into())
    })
}

fn shifted_tick(global_tick: u64, correction: i64) -> Result<u64, SynchronizedAcquisitionError> {
    let shifted = i128::from(global_tick) + i128::from(correction);
    if shifted < 0 || shifted > i128::from(u64::MAX) {
        return Err(SynchronizedAcquisitionError::InvalidRequest(
            "calibrated device tick is outside representable range".into(),
        ));
    }
    Ok(shifted as u64)
}

fn validate_request(
    request: &SynchronizedAcquisitionRequest,
) -> Result<(), SynchronizedAcquisitionError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.sample_scope.trim().is_empty()
        || request.window_start_tick >= request.window_end_tick
        || request.window_end_tick - request.window_start_tick > MAX_WINDOW_TICKS
        || request.maximum_temporal_skew_ticks > MAX_SKEW_TICKS
        || request.maximum_total_risk_milli == 0
        || request.channels.is_empty()
        || request.channels.len() > MAX_CHANNELS
        || request.calibrations.is_empty()
        || request.calibrations.len() > MAX_CALIBRATIONS
    {
        return Err(SynchronizedAcquisitionError::InvalidRequest(
            "objective, study/sample scope, bounded window, risk/skew budgets, channels, and calibrations are required".into(),
        ));
    }
    let mut channels = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut required = false;
    for channel in &request.channels {
        if channel.channel_id.trim().is_empty()
            || channel.instrument_id.trim().is_empty()
            || channel.action_id.trim().is_empty()
            || channel.output_schema.trim().is_empty()
            || channel.duration_ticks == 0
            || channel.risk_milli > MAX_RISK_MILLI
            || channel.minimum_quality_milli > 1_000
            || !channels.insert(channel.channel_id.clone())
            || !actions.insert(channel.action_id.clone())
        {
            return Err(SynchronizedAcquisitionError::InvalidRequest(
                "channel/action/instrument identities, duration, risk, quality, and uniqueness are required".into(),
            ));
        }
        let end = channel
            .start_offset_ticks
            .checked_add(channel.duration_ticks)
            .ok_or_else(|| {
                SynchronizedAcquisitionError::InvalidRequest("channel timing overflow".into())
            })?;
        if end > request.window_end_tick - request.window_start_tick {
            return Err(SynchronizedAcquisitionError::InvalidRequest(
                "every channel must fit inside the acquisition window".into(),
            ));
        }
        required |= channel.required;
    }
    if !required {
        return Err(SynchronizedAcquisitionError::InvalidRequest(
            "at least one required modality is needed to define bundle completeness".into(),
        ));
    }
    let mut calibrations = BTreeSet::new();
    for calibration in &request.calibrations {
        if calibration.instrument_id.trim().is_empty()
            || calibration.calibration_id.trim().is_empty()
            || calibration.valid_from_tick >= calibration.valid_until_tick
            || calibration.drift_ppm_milli.unsigned_abs() > 1_000_000
            || !calibrations.insert(calibration.instrument_id.clone())
        {
            return Err(SynchronizedAcquisitionError::InvalidRequest(
                "each instrument needs one bounded, unique calibration window".into(),
            ));
        }
    }
    if request
        .channels
        .iter()
        .any(|channel| !calibrations.contains(&channel.instrument_id))
    {
        return Err(SynchronizedAcquisitionError::InvalidRequest(
            "every channel instrument needs a calibration".into(),
        ));
    }
    Ok(())
}

impl SynchronizedMultimodalAcquisition {
    pub fn validate(&self) -> Result<(), SynchronizedAcquisitionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.sample_scope.trim().is_empty()
            || !canonical(&self.channel_order)
            || !unique(&self.capture_order)
            || !canonical(&self.required_channel_order)
            || !canonical(&self.completed_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.blocked_order)
            || self
                .alignments
                .windows(2)
                .any(|pair| pair[0].channel_id >= pair[1].channel_id)
            || self.alignments.iter().any(|alignment| {
                alignment.channel_id.trim().is_empty()
                    || alignment.quality_milli > 1_000
                    || !canonical(&alignment.uncertainty)
                    || !canonical(&alignment.negative_evidence)
            })
            || self.total_risk_milli > 1_000_000
            || self
                .temporal_skew_ticks
                .is_some_and(|skew| skew > MAX_SKEW_TICKS)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.emergency_stop_requested && !self.emergency_stop_succeeded
        {
            return Err(SynchronizedAcquisitionError::InvalidOutput(
                "identity, ordering, alignment, risk/skew, evidence, or emergency-stop invariants are invalid".into(),
            ));
        }
        let channels = self.channel_order.iter().cloned().collect::<BTreeSet<_>>();
        if channels.len() != self.channel_order.len()
            || self
                .required_channel_order
                .iter()
                .any(|id| !channels.contains(id))
            || self.completed_order.iter().any(|id| !channels.contains(id))
            || self.failed_order.iter().any(|id| !channels.contains(id))
            || self.negative_order.iter().any(|id| !channels.contains(id))
            || self
                .unresolved_order
                .iter()
                .any(|id| !channels.contains(id))
            || self.blocked_order.iter().any(|id| !channels.contains(id))
            || self
                .alignments
                .iter()
                .map(|alignment| alignment.channel_id.clone())
                .collect::<BTreeSet<_>>()
                .iter()
                .any(|id| !channels.contains(id))
        {
            return Err(SynchronizedAcquisitionError::InvalidOutput(
                "channel partitions or alignments reference unknown identities".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SynchronizedAcquisitionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SynchronizedAcquisitionError::InvalidOutput(
                "synchronized acquisition digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a bounded synchronized acquisition window through an institution-owned gateway.
pub fn execute_glioma_synchronized_multimodal_acquisition<E: SynchronizedAcquisitionExecutor>(
    request: &SynchronizedAcquisitionRequest,
    executor: &mut E,
) -> Result<SynchronizedMultimodalAcquisition, SynchronizedAcquisitionError> {
    validate_request(request)?;
    let calibration_by_instrument = request
        .calibrations
        .iter()
        .map(|calibration| (calibration.instrument_id.clone(), calibration))
        .collect::<BTreeMap<_, _>>();

    let mut channels = request.channels.clone();
    channels.sort_by(|left, right| {
        left.start_offset_ticks
            .cmp(&right.start_offset_ticks)
            .then_with(|| left.channel_id.cmp(&right.channel_id))
    });
    let channel_order = {
        let mut ids = channels
            .iter()
            .map(|channel| channel.channel_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let required_channel_order = sorted_unique_strings(
        channels
            .iter()
            .filter(|channel| channel.required)
            .map(|channel| channel.channel_id.clone()),
    );

    #[derive(Clone)]
    struct TimedChannel {
        channel: SynchronizedAcquisitionChannel,
        global_start: u64,
        global_end: u64,
        device_start: u64,
        device_end: u64,
        calibration_id: String,
    }
    let mut timed_channels = Vec::with_capacity(channels.len());
    for channel in &channels {
        let global_start = request
            .window_start_tick
            .checked_add(channel.start_offset_ticks)
            .ok_or_else(|| {
                SynchronizedAcquisitionError::InvalidRequest("global start overflow".into())
            })?;
        let global_end = global_start
            .checked_add(channel.duration_ticks)
            .ok_or_else(|| {
                SynchronizedAcquisitionError::InvalidRequest("global end overflow".into())
            })?;
        let calibration = calibration_by_instrument
            .get(&channel.instrument_id)
            .expect("validated calibration identity");
        let correction = calibration_correction(calibration, global_start)?;
        let device_start = shifted_tick(global_start, correction)?;
        let device_end = shifted_tick(global_end, correction)?;
        timed_channels.push(TimedChannel {
            channel: channel.clone(),
            global_start,
            global_end,
            device_start,
            device_end,
            calibration_id: calibration.calibration_id.clone(),
        });
    }

    #[derive(Clone)]
    struct AdmissionState {
        selected: Vec<usize>,
        total_risk_milli: u32,
        required_count: usize,
    }
    fn state_better(left: &AdmissionState, right: &AdmissionState) -> bool {
        left.required_count > right.required_count
            || (left.required_count == right.required_count
                && (left.selected.len() > right.selected.len()
                    || (left.selected.len() == right.selected.len()
                        && (left.total_risk_milli < right.total_risk_milli
                            || (left.total_risk_milli == right.total_risk_milli
                                && left.selected < right.selected)))))
    }
    // A start-time greedy pass can admit an optional capture that overlaps a required modality.
    // Compare bounded non-overlapping portfolios instead: required coverage is primary, then
    // total channel coverage, with lower risk and lexical index order as replay-stable tie breaks.
    let mut beam = vec![AdmissionState {
        selected: Vec::new(),
        total_risk_milli: 0,
        required_count: 0,
    }];
    for index in 0..timed_channels.len() {
        let current = &timed_channels[index];
        let mut expanded = beam.clone();
        for state in &beam {
            let overlap = state.selected.iter().any(|selected_index| {
                let selected = &timed_channels[*selected_index];
                selected.channel.instrument_id == current.channel.instrument_id
                    && selected.global_start < current.global_end
                    && current.global_start < selected.global_end
            });
            let risk = state
                .total_risk_milli
                .saturating_add(u32::from(current.channel.risk_milli));
            if overlap || risk > request.maximum_total_risk_milli {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(index);
            expanded.push(AdmissionState {
                selected,
                total_risk_milli: risk,
                required_count: state.required_count + if current.channel.required { 1 } else { 0 },
            });
        }
        expanded.sort_by(|left, right| {
            right
                .required_count
                .cmp(&left.required_count)
                .then_with(|| right.selected.len().cmp(&left.selected.len()))
                .then_with(|| left.total_risk_milli.cmp(&right.total_risk_milli))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        let mut seen = BTreeSet::new();
        expanded.retain(|state| seen.insert(state.selected.clone()));
        expanded.truncate(ACQUISITION_ADMISSION_BEAM_WIDTH);
        beam = expanded;
    }
    let best = beam
        .iter()
        .max_by(|left, right| {
            if state_better(left, right) {
                std::cmp::Ordering::Greater
            } else if state_better(right, left) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .cloned()
        .unwrap_or_else(|| AdmissionState {
            selected: Vec::new(),
            total_risk_milli: 0,
            required_count: 0,
        });
    let selected_indices = best.selected.iter().copied().collect::<BTreeSet<_>>();
    let total_risk_milli = best.total_risk_milli;
    let blocked_order = timed_channels
        .iter()
        .enumerate()
        .filter(|(index, _)| !selected_indices.contains(index))
        .map(|(_, timed)| timed.channel.channel_id.clone())
        .collect::<Vec<_>>();
    let captures = timed_channels
        .iter()
        .enumerate()
        .filter(|(index, _)| selected_indices.contains(index))
        .map(|(_, timed)| SynchronizedCapture {
            channel_id: timed.channel.channel_id.clone(),
            modality: timed.channel.modality,
            instrument_id: timed.channel.instrument_id.clone(),
            action_id: timed.channel.action_id.clone(),
            output_schema: timed.channel.output_schema.clone(),
            global_start_tick: timed.global_start,
            global_end_tick: timed.global_end,
            device_start_tick: timed.device_start,
            device_end_tick: timed.device_end,
            calibration_id: timed.calibration_id.clone(),
            required: timed.channel.required,
            minimum_quality_milli: timed.channel.minimum_quality_milli,
        })
        .collect::<Vec<_>>();

    let mut completed_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut alignments = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    uncertainty.extend(
        blocked_order
            .iter()
            .map(|channel_id| format!("{channel_id}:not-admitted-by-overlap-or-risk-portfolio")),
    );
    let mut emergency_stop_requested = false;
    let mut emergency_stop_succeeded = false;
    let mut executor_failed = false;
    let mut completed_artifacts = BTreeMap::<String, LocalArtifactRef>::new();

    for capture in &captures {
        let outcome = match executor.execute_capture(capture) {
            Ok(outcome) => outcome,
            Err(error) => {
                executor_failed = true;
                failed_order.push(capture.channel_id.clone());
                negative_evidence.push(format!(
                    "{}:gateway-error:{}",
                    capture.channel_id, error.reason
                ));
                if !emergency_stop_requested {
                    emergency_stop_requested = true;
                    emergency_stop_succeeded = executor.emergency_stop().is_ok();
                }
                continue;
            }
        };
        if outcome.channel_id != capture.channel_id || outcome.quality_milli > 1_000 {
            executor_failed = true;
            failed_order.push(capture.channel_id.clone());
            negative_evidence.push(format!("{}:malformed-gateway-result", capture.channel_id));
            if !emergency_stop_requested {
                emergency_stop_requested = true;
                emergency_stop_succeeded = executor.emergency_stop().is_ok();
            }
            continue;
        }
        negative_evidence.extend(outcome.negative_evidence.clone());
        uncertainty.extend(outcome.uncertainty.clone());
        if outcome.disposition == SynchronizedCaptureDisposition::Completed {
            let (Some(observed_start), Some(observed_end), Some(artifact)) = (
                outcome.observed_start_tick,
                outcome.observed_end_tick,
                outcome.artifact.clone(),
            ) else {
                unresolved_order.push(capture.channel_id.clone());
                negative_evidence.push(format!(
                    "{}:completed-without-timing-or-artifact",
                    capture.channel_id
                ));
                continue;
            };
            if observed_end <= observed_start || artifact.validate().is_err() {
                unresolved_order.push(capture.channel_id.clone());
                negative_evidence.push(format!(
                    "{}:invalid-timing-or-local-artifact",
                    capture.channel_id
                ));
                continue;
            }
            let calibration = calibration_by_instrument
                .get(&capture.instrument_id)
                .expect("validated calibration identity");
            let correction = calibration_correction(calibration, capture.global_start_tick)?;
            let corrected_global = i128::from(observed_start) - i128::from(correction);
            if corrected_global < 0 || corrected_global > i128::from(u64::MAX) {
                unresolved_order.push(capture.channel_id.clone());
                negative_evidence.push(format!(
                    "{}:corrected-time-out-of-range",
                    capture.channel_id
                ));
                continue;
            }
            let corrected_global = corrected_global as u64;
            let skew = i128::from(corrected_global) - i128::from(capture.global_start_tick);
            let skew = i64::try_from(skew).map_err(|_| {
                SynchronizedAcquisitionError::InvalidOutput(
                    "capture skew exceeds signed range".into(),
                )
            })?;
            alignments.push(SynchronizedCaptureAlignment {
                channel_id: capture.channel_id.clone(),
                expected_global_start_tick: capture.global_start_tick,
                observed_device_start_tick: observed_start,
                corrected_global_start_tick: corrected_global,
                skew_from_expected_ticks: skew,
                quality_milli: outcome.quality_milli,
                disposition: outcome.disposition,
                uncertainty: outcome.uncertainty.clone(),
                negative_evidence: outcome.negative_evidence.clone(),
            });
            completed_order.push(capture.channel_id.clone());
            completed_artifacts.insert(capture.channel_id.clone(), artifact);
            if outcome.quality_milli < capture.minimum_quality_milli {
                uncertainty.push(format!("{}:quality-below-gate", capture.channel_id));
            }
        } else {
            match outcome.disposition {
                SynchronizedCaptureDisposition::Negative => {
                    negative_order.push(capture.channel_id.clone())
                }
                SynchronizedCaptureDisposition::Failed => {
                    failed_order.push(capture.channel_id.clone())
                }
                SynchronizedCaptureDisposition::Partial
                | SynchronizedCaptureDisposition::Unresolved => {
                    unresolved_order.push(capture.channel_id.clone())
                }
                SynchronizedCaptureDisposition::Completed => unreachable!(),
            }
        }
    }

    let mut alignments_sorted = alignments;
    alignments_sorted.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    let residuals = alignments_sorted
        .iter()
        .map(|alignment| alignment.skew_from_expected_ticks)
        .collect::<Vec<_>>();
    let temporal_skew_ticks = residuals
        .iter()
        .min()
        .zip(residuals.iter().max())
        .map(|(min, max)| (*max as i128 - *min as i128).unsigned_abs() as u64);
    if temporal_skew_ticks.is_some_and(|skew| skew > request.maximum_temporal_skew_ticks) {
        uncertainty.push("temporal-skew-exceeded-bundle-gate".into());
    }
    let quality_blocked = captures.iter().any(|capture| {
        capture.required
            && alignments_sorted
                .iter()
                .find(|alignment| alignment.channel_id == capture.channel_id)
                .is_some_and(|alignment| alignment.quality_milli < capture.minimum_quality_milli)
    });
    let required_complete = required_channel_order.iter().all(|channel_id| {
        completed_artifacts.contains_key(channel_id)
            && !failed_order.contains(channel_id)
            && !negative_order.contains(channel_id)
            && !unresolved_order.contains(channel_id)
            && !blocked_order.contains(channel_id)
    });
    let temporal_ok =
        temporal_skew_ticks.is_some_and(|skew| skew <= request.maximum_temporal_skew_ticks);
    let complete = required_complete && !quality_blocked && temporal_ok && !executor_failed;
    let stop_reason = if complete {
        SynchronizedAcquisitionStopReason::Complete
    } else if executor_failed || !failed_order.is_empty() {
        SynchronizedAcquisitionStopReason::RequiredCaptureFailed
    } else if quality_blocked {
        SynchronizedAcquisitionStopReason::QualityGateFailed
    } else if temporal_skew_ticks.is_some_and(|skew| skew > request.maximum_temporal_skew_ticks) {
        SynchronizedAcquisitionStopReason::TemporalSkewExceeded
    } else if !unresolved_order.is_empty() {
        SynchronizedAcquisitionStopReason::UnresolvedTiming
    } else {
        SynchronizedAcquisitionStopReason::NoRunnableChannels
    };
    let disposition = if complete {
        SynchronizedAcquisitionDisposition::Complete
    } else if completed_order.is_empty() && captures.is_empty() {
        SynchronizedAcquisitionDisposition::Blocked
    } else if completed_order.is_empty() && !negative_order.is_empty() {
        SynchronizedAcquisitionDisposition::Negative
    } else if completed_order.is_empty() && !unresolved_order.is_empty() {
        SynchronizedAcquisitionDisposition::Unresolved
    } else {
        SynchronizedAcquisitionDisposition::Partial
    };

    let bundle_artifact = if complete {
        let artifact_hash = ContentHash::of_value(&serde_json::json!({
            "study_id": request.study_id,
            "sample_scope": request.sample_scope,
            "channel_order": channel_order,
            "artifact_hashes": completed_artifacts
                .iter()
                .map(|(channel_id, artifact)| (channel_id, artifact.content_hash.clone()))
                .collect::<Vec<_>>(),
            "temporal_skew_ticks": temporal_skew_ticks,
        }))
        .map_err(|error| SynchronizedAcquisitionError::Digest(error.to_string()))?;
        Some(LocalArtifactRef {
            artifact_id: format!(
                "synchronized-bundle:{}:{}",
                request.study_id, request.sample_scope
            ),
            content_hash: artifact_hash,
            content_type: "application/vnd.aurora.glioma.synchronized-bundle+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        })
    } else {
        None
    };

    let mut output = SynchronizedMultimodalAcquisition {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        sample_scope: request.sample_scope.clone(),
        model_system: request.model_system,
        channel_order,
        capture_order: captures
            .iter()
            .map(|capture| capture.channel_id.clone())
            .collect(),
        required_channel_order,
        completed_order: sorted_unique_strings(completed_order),
        failed_order: sorted_unique_strings(failed_order),
        negative_order: sorted_unique_strings(negative_order),
        unresolved_order: sorted_unique_strings(unresolved_order),
        blocked_order: sorted_unique_strings(blocked_order),
        alignments: alignments_sorted,
        total_risk_milli,
        temporal_skew_ticks,
        bundle_artifact,
        simulation_only: executor.simulation_only(),
        emergency_stop_requested,
        emergency_stop_succeeded,
        negative_evidence: sorted_unique_strings(negative_evidence),
        uncertainty: sorted_unique_strings(uncertainty),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-synchronized-multimodal-acquisition"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SynchronizedAcquisitionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calibration(instrument_id: &str, offset_ticks: i64) -> InstrumentClockCalibration {
        InstrumentClockCalibration {
            instrument_id: instrument_id.into(),
            calibration_id: format!("cal-{instrument_id}"),
            offset_ticks,
            drift_ppm_milli: 0,
            valid_from_tick: 0,
            valid_until_tick: 10_000,
        }
    }

    fn channel(
        channel_id: &str,
        modality: GliomaModality,
        instrument_id: &str,
        start_offset_ticks: u64,
        required: bool,
    ) -> SynchronizedAcquisitionChannel {
        SynchronizedAcquisitionChannel {
            channel_id: channel_id.into(),
            modality,
            instrument_id: instrument_id.into(),
            action_id: format!("action-{channel_id}"),
            output_schema: format!("application/x-{channel_id}"),
            start_offset_ticks,
            duration_ticks: 10,
            risk_milli: 100,
            required,
            minimum_quality_milli: 800,
        }
    }

    fn request(channels: Vec<SynchronizedAcquisitionChannel>) -> SynchronizedAcquisitionRequest {
        SynchronizedAcquisitionRequest {
            objective: "glioma state-plasticity time-aligned assay".into(),
            study_id: "study-1".into(),
            sample_scope: "opaque-sample-cohort-a".into(),
            model_system: GliomaModelSystem::Organoid,
            window_start_tick: 100,
            window_end_tick: 200,
            maximum_temporal_skew_ticks: 2,
            maximum_total_risk_milli: 500,
            channels,
            calibrations: vec![calibration("microscope", 5), calibration("reader", -3)],
        }
    }

    #[test]
    fn deterministic_compilation_corrects_device_clock_and_builds_bundle() {
        let mut executor = DryRunSynchronizedAcquisitionExecutor::default();
        let first = execute_glioma_synchronized_multimodal_acquisition(
            &request(vec![
                channel("imaging", GliomaModality::Imaging, "microscope", 0, true),
                channel("metabolic", GliomaModality::Proteomics, "reader", 20, true),
            ]),
            &mut executor,
        )
        .unwrap();
        let mut executor = DryRunSynchronizedAcquisitionExecutor::default();
        let second = execute_glioma_synchronized_multimodal_acquisition(
            &request(vec![
                channel("metabolic", GliomaModality::Proteomics, "reader", 20, true),
                channel("imaging", GliomaModality::Imaging, "microscope", 0, true),
            ]),
            &mut executor,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            SynchronizedAcquisitionDisposition::Complete
        );
        assert_eq!(first.temporal_skew_ticks, Some(0));
        assert!(first.bundle_artifact.is_some());
        assert_eq!(first.alignments[0].corrected_global_start_tick, 100);
    }

    #[test]
    fn device_drift_is_visible_and_blocks_complete_bundle() {
        let mut request = request(vec![
            channel("imaging", GliomaModality::Imaging, "microscope", 0, true),
            channel("metabolic", GliomaModality::Proteomics, "reader", 20, true),
        ]);
        request.maximum_temporal_skew_ticks = 1;
        request.channels[1].start_offset_ticks = 20;
        let mut executor = DryRunSynchronizedAcquisitionExecutor::default();
        let output =
            execute_glioma_synchronized_multimodal_acquisition(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            SynchronizedAcquisitionDisposition::Complete
        );
        assert_eq!(output.temporal_skew_ticks, Some(0));
        // A late device reports its real device-clock timestamp; this is not silently coerced.
        struct LateExecutor;
        impl SynchronizedAcquisitionExecutor for LateExecutor {
            fn execute_capture(
                &mut self,
                capture: &SynchronizedCapture,
            ) -> Result<SynchronizedCaptureOutcome, SynchronizedAcquisitionExecutionFailure>
            {
                let mut outcome =
                    DryRunSynchronizedAcquisitionExecutor::default().execute_capture(capture)?;
                if capture.channel_id == "metabolic" {
                    outcome.observed_start_tick = outcome.observed_start_tick.map(|tick| tick + 5);
                    outcome.observed_end_tick = outcome.observed_end_tick.map(|tick| tick + 5);
                }
                Ok(outcome)
            }
            fn emergency_stop(&mut self) -> Result<(), SynchronizedAcquisitionExecutionFailure> {
                Ok(())
            }
            fn simulation_only(&self) -> bool {
                true
            }
        }
        let mut executor = LateExecutor;
        let output =
            execute_glioma_synchronized_multimodal_acquisition(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            SynchronizedAcquisitionDisposition::Partial
        );
        assert_eq!(
            output.stop_reason,
            SynchronizedAcquisitionStopReason::TemporalSkewExceeded
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "temporal-skew-exceeded-bundle-gate"));
    }

    #[test]
    fn failed_required_device_cannot_complete_bundle() {
        let mut executor = DryRunSynchronizedAcquisitionExecutor {
            fail_channel_ids: ["metabolic".into()].into_iter().collect(),
            emergency_stop_called: false,
        };
        let output = execute_glioma_synchronized_multimodal_acquisition(
            &request(vec![
                channel("imaging", GliomaModality::Imaging, "microscope", 0, true),
                channel("metabolic", GliomaModality::Proteomics, "reader", 20, true),
            ]),
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            SynchronizedAcquisitionDisposition::Partial
        );
        assert_eq!(
            output.stop_reason,
            SynchronizedAcquisitionStopReason::RequiredCaptureFailed
        );
        assert!(output.bundle_artifact.is_none());
        assert!(output.failed_order.contains(&"metabolic".into()));
    }

    #[test]
    fn overlapping_instrument_capture_is_blocked_explicitly() {
        let mut channels = vec![
            channel("first", GliomaModality::Imaging, "microscope", 0, true),
            channel("second", GliomaModality::Spatial, "microscope", 5, false),
        ];
        let mut request = request(std::mem::take(&mut channels));
        request.calibrations = vec![calibration("microscope", 0)];
        let mut executor = DryRunSynchronizedAcquisitionExecutor::default();
        let output =
            execute_glioma_synchronized_multimodal_acquisition(&request, &mut executor).unwrap();
        assert!(output.blocked_order.contains(&"second".into()));
        assert_eq!(
            output.disposition,
            SynchronizedAcquisitionDisposition::Complete
        );
        assert_eq!(output.completed_order, vec!["first"]);
    }

    #[test]
    fn required_later_capture_displaces_overlapping_optional_capture() {
        let channels = vec![
            channel(
                "optional-early",
                GliomaModality::Imaging,
                "microscope",
                0,
                false,
            ),
            channel(
                "required-late",
                GliomaModality::Spatial,
                "microscope",
                5,
                true,
            ),
        ];
        let mut request = request(channels);
        request.calibrations = vec![calibration("microscope", 0)];
        let mut executor = DryRunSynchronizedAcquisitionExecutor::default();
        let output =
            execute_glioma_synchronized_multimodal_acquisition(&request, &mut executor).unwrap();
        assert_eq!(output.capture_order, vec!["required-late"]);
        assert_eq!(output.completed_order, vec!["required-late"]);
        assert!(output.blocked_order.contains(&"optional-early".into()));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "optional-early:not-admitted-by-overlap-or-risk-portfolio"));
        assert_eq!(
            output.disposition,
            SynchronizedAcquisitionDisposition::Complete
        );
        output.validate().unwrap();
    }
}
