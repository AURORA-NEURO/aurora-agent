//! Robust instrument-control calibration and drift detection for preclinical glioma workflows.
//!
//! The instrument gateway contributes only de-identified control measurements and local artifact
//! references.  A robust reference median/MAD and Theil-Sen drift slope are calculated before a
//! physical run can be admitted.  Drift, noisy controls, and insufficient calibration history
//! are hard workflow states; this module never executes hardware or makes a clinical decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentCalibration1@1";
pub const MAX_RUNS: usize = 512;
pub const MAX_VALUE_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationRequest {
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub metric_name: String,
    pub minimum_runs: usize,
    pub reference_run_count: usize,
    pub max_reference_mad_milli: u64,
    pub max_drift_milli: u64,
    pub max_slope_milli_per_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationRun {
    pub run_id: String,
    pub sequence_index: u32,
    pub batch_id: String,
    pub instrument_id: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub observed_milli: i64,
    pub expected_milli: i64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationPoint {
    pub run_id: String,
    pub sequence_index: u32,
    pub observed_milli: i64,
    pub expected_milli: i64,
    pub residual_milli: i64,
    pub drift_from_reference_milli: i64,
    pub robust_z_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationDisposition {
    Qualified,
    Drifting,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCalibration {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub metric_name: String,
    pub run_order: Vec<String>,
    pub reference_order: Vec<String>,
    pub points: Vec<CalibrationPoint>,
    pub reference_residual_median_milli: i64,
    pub reference_mad_milli: u64,
    pub final_drift_milli: i64,
    pub max_abs_drift_milli: u64,
    pub slope_milli_per_tick: i64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CalibrationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CalibrationError {
    #[error("instrument calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument calibration run is invalid: {0}")]
    InvalidRun(String),
    #[error("instrument calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument calibration digest failed: {0}")]
    Digest(String),
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn digest_input(output: &InstrumentCalibration) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_id": output.instrument_id,
        "model_system": output.model_system,
        "metric_name": output.metric_name,
        "run_order": output.run_order,
        "reference_order": output.reference_order,
        "points": output.points,
        "reference_residual_median_milli": output.reference_residual_median_milli,
        "reference_mad_milli": output.reference_mad_milli,
        "final_drift_milli": output.final_drift_milli,
        "max_abs_drift_milli": output.max_abs_drift_milli,
        "slope_milli_per_tick": output.slope_milli_per_tick,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl InstrumentCalibration {
    pub fn validate(&self) -> Result<(), CalibrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.metric_name.trim().is_empty()
            || self.run_order.len() != self.points.len()
            || self.run_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.reference_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.points.windows(2).any(|pair| {
                (pair[0].sequence_index, &pair[0].run_id)
                    >= (pair[1].sequence_index, &pair[1].run_id)
            })
            || self.points.iter().any(|point| {
                point.run_id.trim().is_empty()
                    || point.residual_milli.unsigned_abs() > MAX_VALUE_MILLI
                    || point.drift_from_reference_milli.unsigned_abs() > MAX_VALUE_MILLI
                    || point.robust_z_milli > u64::MAX / 2
            })
            || self.negative_evidence.windows(2).any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CalibrationError::InvalidOutput(
                "identity, bounds, point order, or canonical ordering is invalid".into(),
            ));
        }
        let runs = self.run_order.iter().cloned().collect::<BTreeSet<_>>();
        let points = self
            .points
            .iter()
            .map(|point| point.run_id.clone())
            .collect::<BTreeSet<_>>();
        let references = self.reference_order.iter().cloned().collect::<BTreeSet<_>>();
        if runs != points || !references.is_subset(&runs) {
            return Err(CalibrationError::InvalidOutput(
                "run, point, and reference partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CalibrationError::InvalidOutput(
                "digest is not bound to instrument calibration".into(),
            ));
        }
        Ok(())
    }
}

fn theil_sen_slope(points: &[CalibrationPoint]) -> i64 {
    let mut slopes = Vec::new();
    for (index, left) in points.iter().enumerate() {
        for right in points.iter().skip(index + 1) {
            let delta_tick = i64::from(right.sequence_index) - i64::from(left.sequence_index);
            if delta_tick != 0 {
                slopes.push(
                    (i128::from(right.residual_milli) - i128::from(left.residual_milli))
                        .checked_div(i128::from(delta_tick))
                        .unwrap_or(0)
                        .clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                        as i64,
                );
            }
        }
    }
    if slopes.is_empty() {
        0
    } else {
        median(&mut slopes)
    }
}

pub fn analyze_instrument_calibration(
    request: &CalibrationRequest,
    runs: &[CalibrationRun],
) -> Result<InstrumentCalibration, CalibrationError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.metric_name.trim().is_empty()
        || request.minimum_runs == 0
        || request.reference_run_count == 0
        || request.max_drift_milli == 0
        || request.max_slope_milli_per_tick == 0
        || runs.is_empty()
        || runs.len() > MAX_RUNS
        || request.reference_run_count > runs.len()
    {
        return Err(CalibrationError::InvalidRequest(
            "instrument/metric identity, run/reference floors, drift bounds, and bounded runs are required".into(),
        ));
    }
    let mut run_ids = BTreeSet::new();
    let mut sequence_ids = BTreeSet::new();
    let mut ordered = runs.to_vec();
    for run in &ordered {
        run.artifact
            .validate()
            .map_err(|error| CalibrationError::InvalidRun(error.to_string()))?;
        if run.run_id.trim().is_empty()
            || run.batch_id.trim().is_empty()
            || run.instrument_id != request.instrument_id
            || run.metric_name != request.metric_name
            || run.model_system != request.model_system
            || run.observed_milli.unsigned_abs() > MAX_VALUE_MILLI
            || run.expected_milli.unsigned_abs() > MAX_VALUE_MILLI
            || !run_ids.insert(run.run_id.clone())
            || !sequence_ids.insert(run.sequence_index)
        {
            return Err(CalibrationError::InvalidRun(
                "run identity, instrument/metric/model binding, score bounds, or sequence uniqueness is invalid".into(),
            ));
        }
    }
    ordered.sort_by(|left, right| {
        left.sequence_index
            .cmp(&right.sequence_index)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    let reference = &ordered[..request.reference_run_count];
    let mut reference_residuals = reference
        .iter()
        .map(|run| run.observed_milli.saturating_sub(run.expected_milli))
        .collect::<Vec<_>>();
    let reference_median = median(&mut reference_residuals);
    let mut deviations = reference
        .iter()
        .map(|run| {
            run.observed_milli
                .saturating_sub(run.expected_milli)
                .saturating_sub(reference_median)
                .unsigned_abs() as i64
        })
        .collect::<Vec<_>>();
    let reference_mad = median(&mut deviations).unsigned_abs();
    let scale = reference_mad.max(1);
    let mut points = ordered
        .iter()
        .map(|run| {
            let residual = run.observed_milli.saturating_sub(run.expected_milli);
            let drift = residual.saturating_sub(reference_median);
            CalibrationPoint {
                run_id: run.run_id.clone(),
                sequence_index: run.sequence_index,
                observed_milli: run.observed_milli,
                expected_milli: run.expected_milli,
                residual_milli: residual,
                drift_from_reference_milli: drift,
                robust_z_milli: drift.unsigned_abs().saturating_mul(1_000) / scale,
            }
        })
        .collect::<Vec<_>>();
    let slope = theil_sen_slope(&points);
    let max_abs_drift = points
        .iter()
        .map(|point| point.drift_from_reference_milli.unsigned_abs())
        .max()
        .unwrap_or(0);
    let final_drift = points
        .last()
        .map(|point| point.drift_from_reference_milli)
        .unwrap_or(0);
    let run_order = points.iter().map(|point| point.run_id.clone()).collect::<Vec<_>>();
    let reference_order = reference.iter().map(|run| run.run_id.clone()).collect::<Vec<_>>();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if runs.len() < request.minimum_runs {
        uncertainty.insert("minimum-calibration-run-floor-not-met".into());
    }
    if reference_mad > request.max_reference_mad_milli {
        negative.insert("reference-control-mad-exceeds-tolerance".into());
    }
    if max_abs_drift > request.max_drift_milli {
        negative.insert("instrument-drift-exceeds-tolerance".into());
    }
    if slope.unsigned_abs() > request.max_slope_milli_per_tick {
        negative.insert("instrument-drift-slope-exceeds-tolerance".into());
    }
    let disposition = if runs.len() < request.minimum_runs {
        CalibrationDisposition::Unresolved
    } else if max_abs_drift > request.max_drift_milli
        || slope.unsigned_abs() > request.max_slope_milli_per_tick
    {
        CalibrationDisposition::Drifting
    } else if reference_mad > request.max_reference_mad_milli {
        CalibrationDisposition::Negative
    } else {
        CalibrationDisposition::Qualified
    };
    let mut output = InstrumentCalibration {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        model_system: request.model_system,
        metric_name: request.metric_name.clone(),
        run_order,
        reference_order,
        points: std::mem::take(&mut points),
        reference_residual_median_milli: reference_median,
        reference_mad_milli: reference_mad,
        final_drift_milli: final_drift,
        max_abs_drift_milli: max_abs_drift,
        slope_milli_per_tick: slope,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| CalibrationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CalibrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn run(id: &str, sequence: u32, observed: i64) -> CalibrationRun {
        CalibrationRun {
            run_id: id.into(),
            sequence_index: sequence,
            batch_id: format!("batch-{id}"),
            instrument_id: "imager-1".into(),
            metric_name: "control_intensity".into(),
            model_system: GliomaModelSystem::Organoid,
            observed_milli: observed,
            expected_milli: 500,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-control+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> CalibrationRequest {
        CalibrationRequest {
            objective: "qualify imaging control before invasion assay".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            metric_name: "control_intensity".into(),
            minimum_runs: 3,
            reference_run_count: 2,
            max_reference_mad_milli: 5,
            max_drift_milli: 20,
            max_slope_milli_per_tick: 10,
        }
    }

    #[test]
    fn stable_controls_are_qualified_and_replay_stable() {
        let runs = vec![run("r1", 1, 500), run("r2", 2, 502), run("r3", 3, 504)];
        let first = analyze_instrument_calibration(&request(), &runs).unwrap();
        let second = analyze_instrument_calibration(&request(), &runs).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, CalibrationDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn monotonic_drift_is_blocked_and_explained() {
        let runs = vec![run("r1", 1, 500), run("r2", 2, 510), run("r3", 3, 550)];
        let output = analyze_instrument_calibration(&request(), &runs).unwrap();
        assert_eq!(output.disposition, CalibrationDisposition::Drifting);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("drift-exceeds")));
    }

    #[test]
    fn insufficient_history_is_unresolved() {
        let mut request = request();
        request.minimum_runs = 4;
        let output = analyze_instrument_calibration(&request, &[run("r1", 1, 500), run("r2", 2, 500), run("r3", 3, 500)]).unwrap();
        assert_eq!(output.disposition, CalibrationDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("minimum-calibration")));
    }
}
