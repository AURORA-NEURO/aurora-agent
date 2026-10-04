//! Continual drift and calibration monitoring for federated preclinical glioma benchmarks.
//!
//! Each immutable window contributes only a bounded aggregate effect, uncertainty, site count,
//! and source digest. The monitor compares adjacent windows with integer arithmetic, exposes
//! change points and under-observed windows, and recommends recalibration or rerun without
//! rewriting historical snapshots or pretending missing coverage is stability.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaContinualBenchmarkAssessment1@1";
pub const MAX_WINDOWS: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWindow {
    pub window_id: String,
    pub epoch: u64,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub aggregate_effect_milli: i64,
    pub uncertainty_milli: u64,
    pub site_count: usize,
    pub immutable_snapshot: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualBenchmarkMonitorRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub minimum_windows: usize,
    pub maximum_uncertainty_milli: u64,
    pub maximum_step_drift_milli: u64,
    pub maximum_change_point_milli: u64,
    pub maximum_epoch_gap: u64,
    pub windows: Vec<FederatedBenchmarkWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualWindowSummary {
    pub window_id: String,
    pub epoch: u64,
    pub aggregate_effect_milli: i64,
    pub uncertainty_milli: u64,
    pub site_count: usize,
    pub step_drift_milli: u64,
    pub change_point: bool,
    pub adequate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkCalibrationState {
    Calibrated,
    RecalibrationRequired,
    UnderObserved,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinualBenchmarkDisposition {
    Stable,
    Drifted,
    Insufficient,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinualBenchmarkAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub window_order: Vec<String>,
    pub excluded_window_order: Vec<String>,
    pub summaries: Vec<ContinualWindowSummary>,
    pub change_point_order: Vec<String>,
    pub maximum_step_drift_milli: u64,
    pub median_step_drift_milli: u64,
    pub calibration_state: BenchmarkCalibrationState,
    pub disposition: ContinualBenchmarkDisposition,
    pub rerun_required: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContinualBenchmarkError {
    #[error("continual benchmark monitor request is invalid: {0}")]
    InvalidRequest(String),
    #[error("continual benchmark monitor output is invalid: {0}")]
    InvalidOutput(String),
    #[error("continual benchmark monitor digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value)) && values.windows(2).all(|pair| pair[0] != pair[1])
}

fn output_body(assessment: &ContinualBenchmarkAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": assessment.feature_id,
        "output_schema": assessment.output_schema,
        "objective": assessment.objective,
        "capability_id": assessment.capability_id,
        "benchmark_world": assessment.benchmark_world,
        "metric_name": assessment.metric_name,
        "window_order": assessment.window_order,
        "excluded_window_order": assessment.excluded_window_order,
        "summaries": assessment.summaries,
        "change_point_order": assessment.change_point_order,
        "maximum_step_drift_milli": assessment.maximum_step_drift_milli,
        "median_step_drift_milli": assessment.median_step_drift_milli,
        "calibration_state": assessment.calibration_state,
        "disposition": assessment.disposition,
        "rerun_required": assessment.rerun_required,
        "negative_evidence": assessment.negative_evidence,
        "uncertainty": assessment.uncertainty,
    })
}

impl ContinualBenchmarkAssessment {
    pub fn validate(&self) -> Result<(), ContinualBenchmarkError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !safe_text(&self.capability_id)
            || !safe_text(&self.benchmark_world)
            || !safe_text(&self.metric_name)
            || !canonical(&self.window_order)
            || !canonical(&self.excluded_window_order)
            || !canonical(&self.change_point_order)
            || self.summaries.iter().any(|summary| {
                !safe_text(&summary.window_id)
                    || summary.site_count == 0
                    || (summary.change_point && summary.step_drift_milli == 0)
            })
        {
            return Err(ContinualBenchmarkError::InvalidOutput(
                "monitor identity, immutable window partitions, or summary invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| ContinualBenchmarkError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ContinualBenchmarkError::InvalidOutput(
                "continual assessment digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ContinualBenchmarkMonitorRequest,
) -> Result<(), ContinualBenchmarkError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.capability_id)
        || !safe_text(&request.benchmark_world)
        || !safe_text(&request.metric_name)
        || request.minimum_sites == 0
        || request.minimum_windows == 0
        || request.maximum_uncertainty_milli == 0
        || request.maximum_step_drift_milli == 0
        || request.maximum_change_point_milli == 0
        || request.maximum_epoch_gap == 0
        || request.windows.is_empty()
        || request.windows.len() > MAX_WINDOWS
    {
        return Err(ContinualBenchmarkError::InvalidRequest(
            "binding, window/site floors, uncertainty, drift, epoch-gap, and bounded window inputs are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut epochs = BTreeSet::new();
    for window in &request.windows {
        if !safe_text(&window.window_id)
            || !ids.insert(window.window_id.clone())
            || !epochs.insert(window.epoch)
            || !safe_text(&window.capability_id)
            || !safe_text(&window.benchmark_world)
            || !safe_text(&window.metric_name)
            || window.uncertainty_milli == 0
            || window.site_count == 0
            || !window.immutable_snapshot
            || !window.local_only
            || window.contains_human_data
            || window.contains_direct_identifiers
        {
            return Err(ContinualBenchmarkError::InvalidRequest(format!(
                "window {} is non-immutable, duplicate, non-local, under-observed, or protected",
                window.window_id
            )));
        }
    }
    Ok(())
}

pub fn monitor_glioma_federated_benchmark_continuity(
    request: &ContinualBenchmarkMonitorRequest,
) -> Result<ContinualBenchmarkAssessment, ContinualBenchmarkError> {
    validate_request(request)?;
    let mut candidate = request
        .windows
        .iter()
        .filter(|window| {
            window.capability_id == request.capability_id
                && window.benchmark_world == request.benchmark_world
                && window.metric_name == request.metric_name
                && window.model_system == request.model_system
        })
        .collect::<Vec<_>>();
    let mut excluded = request
        .windows
        .iter()
        .filter(|window| {
            !candidate
                .iter()
                .any(|item| item.window_id == window.window_id)
        })
        .map(|window| window.window_id.clone())
        .collect::<Vec<_>>();
    candidate.sort_by(|left, right| {
        left.epoch
            .cmp(&right.epoch)
            .then_with(|| left.window_id.cmp(&right.window_id))
    });
    excluded.sort();
    let window_order = candidate
        .iter()
        .map(|window| window.window_id.clone())
        .collect::<Vec<_>>();
    let mut summaries = Vec::with_capacity(candidate.len());
    let mut change_points = Vec::new();
    let mut drifts = Vec::new();
    let mut negative = Vec::new();
    let mut uncertainty = Vec::new();
    let mut previous: Option<&FederatedBenchmarkWindow> = None;
    for window in &candidate {
        let (step_drift, gap_ok) = previous
            .map(|prior| {
                (
                    window
                        .aggregate_effect_milli
                        .abs_diff(prior.aggregate_effect_milli),
                    window.epoch.saturating_sub(prior.epoch) <= request.maximum_epoch_gap,
                )
            })
            .unwrap_or((0, true));
        let adequate = window.site_count >= request.minimum_sites
            && window.uncertainty_milli <= request.maximum_uncertainty_milli
            && gap_ok;
        let change_point = step_drift > request.maximum_change_point_milli;
        if change_point {
            change_points.push(window.window_id.clone());
        }
        if !adequate {
            negative.push(format!(
                "{} is under-observed or has an epoch/uncertainty gap and cannot establish drift-free continuity",
                window.window_id
            ));
        }
        if window.uncertainty_milli > request.maximum_uncertainty_milli {
            uncertainty.push(format!(
                "{} uncertainty exceeds the declared calibration ceiling",
                window.window_id
            ));
        }
        drifts.push(step_drift);
        summaries.push(ContinualWindowSummary {
            window_id: window.window_id.clone(),
            epoch: window.epoch,
            aggregate_effect_milli: window.aggregate_effect_milli,
            uncertainty_milli: window.uncertainty_milli,
            site_count: window.site_count,
            step_drift_milli: step_drift,
            change_point,
            adequate,
        });
        previous = Some(window);
    }
    if candidate.len() < request.minimum_windows {
        negative.push("immutable window count is below the temporal calibration floor".into());
    }
    if !excluded.is_empty() {
        uncertainty.push(
            "binding-mismatched windows were excluded without requesting raw observations".into(),
        );
    }
    drifts.sort_unstable();
    let maximum_step_drift = drifts.iter().copied().max().unwrap_or(0);
    let median_step_drift = drifts.get(drifts.len() / 2).copied().unwrap_or(0);
    if maximum_step_drift > request.maximum_step_drift_milli {
        negative
            .push("observed temporal step drift exceeds the declared comparison envelope".into());
    }
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let has_inadequate = summaries.iter().any(|summary| !summary.adequate);
    let disposition = if candidate.len() < request.minimum_windows {
        ContinualBenchmarkDisposition::Insufficient
    } else if has_inadequate {
        ContinualBenchmarkDisposition::Unresolved
    } else if !change_points.is_empty() || maximum_step_drift > request.maximum_step_drift_milli {
        ContinualBenchmarkDisposition::Drifted
    } else {
        ContinualBenchmarkDisposition::Stable
    };
    let calibration_state = if candidate.len() < request.minimum_windows {
        BenchmarkCalibrationState::UnderObserved
    } else if has_inadequate {
        BenchmarkCalibrationState::Unknown
    } else if disposition == ContinualBenchmarkDisposition::Drifted {
        BenchmarkCalibrationState::RecalibrationRequired
    } else {
        BenchmarkCalibrationState::Calibrated
    };
    let mut assessment = ContinualBenchmarkAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        metric_name: request.metric_name.clone(),
        window_order,
        excluded_window_order: excluded,
        summaries,
        change_point_order: change_points,
        maximum_step_drift_milli: maximum_step_drift,
        median_step_drift_milli: median_step_drift,
        calibration_state,
        disposition,
        rerun_required: disposition != ContinualBenchmarkDisposition::Stable,
        negative_evidence: negative,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-continual-benchmark"),
    };
    assessment.digest = ContentHash::of_value(&output_body(&assessment))
        .map_err(|error| ContinualBenchmarkError::Digest(error.to_string()))?;
    assessment.validate()?;
    Ok(assessment)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: &str, epoch: u64, effect: i64) -> FederatedBenchmarkWindow {
        FederatedBenchmarkWindow {
            window_id: id.into(),
            epoch,
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            aggregate_effect_milli: effect,
            uncertainty_milli: 20,
            site_count: 4,
            immutable_snapshot: true,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            source_digest: ContentHash::of_bytes(id.as_bytes()),
        }
    }

    fn request(windows: Vec<FederatedBenchmarkWindow>) -> ContinualBenchmarkMonitorRequest {
        ContinualBenchmarkMonitorRequest {
            objective: "monitor a glioma benchmark over time".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "dice".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 3,
            minimum_windows: 3,
            maximum_uncertainty_milli: 100,
            maximum_step_drift_milli: 100,
            maximum_change_point_milli: 150,
            maximum_epoch_gap: 2,
            windows,
        }
    }

    #[test]
    fn stable_windows_remain_calibrated() {
        let assessment = monitor_glioma_federated_benchmark_continuity(&request(vec![
            window("w1", 1, 500),
            window("w2", 2, 510),
            window("w3", 3, 505),
        ]))
        .unwrap();
        assert_eq!(
            assessment.disposition,
            ContinualBenchmarkDisposition::Stable
        );
        assert_eq!(
            assessment.calibration_state,
            BenchmarkCalibrationState::Calibrated
        );
        assert!(!assessment.rerun_required);
        assert!(assessment.validate().is_ok());
    }

    #[test]
    fn temporal_change_point_requires_recalibration() {
        let assessment = monitor_glioma_federated_benchmark_continuity(&request(vec![
            window("w1", 1, 500),
            window("w2", 2, 510),
            window("w3", 3, 800),
        ]))
        .unwrap();
        assert_eq!(
            assessment.disposition,
            ContinualBenchmarkDisposition::Drifted
        );
        assert_eq!(
            assessment.calibration_state,
            BenchmarkCalibrationState::RecalibrationRequired
        );
        assert!(assessment.change_point_order.contains(&"w3".into()));
    }

    #[test]
    fn under_observed_window_is_not_labeled_stable() {
        let mut sparse = window("w2", 2, 505);
        sparse.site_count = 1;
        let assessment = monitor_glioma_federated_benchmark_continuity(&request(vec![
            window("w1", 1, 500),
            sparse,
            window("w3", 3, 505),
        ]))
        .unwrap();
        assert_eq!(
            assessment.disposition,
            ContinualBenchmarkDisposition::Unresolved
        );
        assert!(assessment.rerun_required);
        assert!(assessment
            .negative_evidence
            .iter()
            .any(|item| item.contains("under-observed")));
    }

    #[test]
    fn binding_mismatch_is_excluded() {
        let mut mismatch = window("wrong", 4, 500);
        mismatch.metric_name = "wrong".into();
        let assessment = monitor_glioma_federated_benchmark_continuity(&request(vec![
            window("w1", 1, 500),
            window("w2", 2, 510),
            window("w3", 3, 505),
            mismatch,
        ]))
        .unwrap();
        assert_eq!(assessment.excluded_window_order, vec!["wrong"]);
    }

    #[test]
    fn mutable_or_protected_window_is_rejected() {
        let mut mutable = window("w2", 2, 500);
        mutable.immutable_snapshot = false;
        assert!(monitor_glioma_federated_benchmark_continuity(&request(vec![
            window("w1", 1, 500),
            mutable,
            window("w3", 3, 500),
        ]))
        .is_err());
    }
}
