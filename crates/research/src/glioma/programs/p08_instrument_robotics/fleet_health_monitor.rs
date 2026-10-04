//! Robust multi-instrument health monitoring for preclinical glioma acquisition.
//!
//! The monitor consumes site-local summaries, not raw sample data.  It compares a fixed baseline
//! window with a recent window using integer arithmetic, separates throughput drift from QC and
//! calibration instability, detects downtime clusters, and calibrates alert confidence from
//! effective observation count.  No single noisy metric can silently block an instrument: a
//! blocked disposition requires corroborated failure signals or an explicit calibration limit.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaFleetHealthAssessment1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_INSTRUMENTS: usize = 2_048;
pub const MAX_REASON_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentHealthObservation {
    pub instrument_id: String,
    pub site_id: String,
    pub tick: u64,
    pub throughput_milli: u16,
    pub qc_pass_milli: u16,
    pub downtime_milli: u16,
    pub calibration_error_milli: u16,
    pub run_count: u32,
    pub summary_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetHealthMonitorRequest {
    pub fleet_id: String,
    pub observation_order: Vec<InstrumentHealthObservation>,
    pub baseline_window_count: usize,
    pub recent_window_count: usize,
    pub minimum_observations: usize,
    pub drift_threshold_milli: u16,
    pub qc_failure_threshold_milli: u16,
    pub downtime_cluster_threshold: usize,
    pub max_alerts: usize,
    pub current_tick: u64,
    pub mask_site_identity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetHealthDisposition {
    Healthy,
    Watch,
    Investigate,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentHealthAssessment {
    pub instrument_id: String,
    pub site_label: Option<String>,
    pub disposition: FleetHealthDisposition,
    pub baseline_observations: usize,
    pub recent_observations: usize,
    pub effective_observations: usize,
    pub throughput_drop_milli: i32,
    pub qc_pass_drop_milli: i32,
    pub calibration_error_rise_milli: i32,
    pub downtime_cluster_max: usize,
    pub qc_failure_count: usize,
    pub confidence_milli: u16,
    pub reason_order: Vec<String>,
    pub investigation_task_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetHealthAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub fleet_id: String,
    pub instrument_order: Vec<String>,
    pub assessments: Vec<InstrumentHealthAssessment>,
    pub fleet_disposition: FleetHealthDisposition,
    pub alert_order: Vec<String>,
    pub aggregate_digest_order: Vec<ContentHash>,
    pub freshness_tick: u64,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FleetHealthError {
    #[error("fleet health request is invalid: {0}")]
    InvalidRequest(String),
    #[error("fleet health output is invalid: {0}")]
    InvalidOutput(String),
    #[error("fleet health digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn observation_body(observation: &InstrumentHealthObservation) -> serde_json::Value {
    serde_json::json!({
        "instrument_id": observation.instrument_id,
        "site_id": observation.site_id,
        "tick": observation.tick,
        "throughput_milli": observation.throughput_milli,
        "qc_pass_milli": observation.qc_pass_milli,
        "downtime_milli": observation.downtime_milli,
        "calibration_error_milli": observation.calibration_error_milli,
        "run_count": observation.run_count,
    })
}

fn digest_body(assessment: &FleetHealthAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": assessment.feature_id,
        "output_schema": assessment.output_schema,
        "fleet_id": assessment.fleet_id,
        "instrument_order": assessment.instrument_order,
        "assessments": assessment.assessments,
        "fleet_disposition": assessment.fleet_disposition,
        "alert_order": assessment.alert_order,
        "aggregate_digest_order": assessment.aggregate_digest_order,
        "freshness_tick": assessment.freshness_tick,
    })
}

fn mean(values: &[u64]) -> u64 {
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u64>() / values.len() as u64
    }
}

fn max_downtime_cluster(observations: &[&InstrumentHealthObservation]) -> usize {
    let mut best = 0;
    let mut run = 0;
    for observation in observations {
        if observation.downtime_milli > 0 {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

fn validate_request(request: &FleetHealthMonitorRequest) -> Result<(), FleetHealthError> {
    if !safe_text(&request.fleet_id, MAX_REASON_LEN)
        || request.observation_order.is_empty()
        || request.observation_order.len() > MAX_OBSERVATIONS
        || request.baseline_window_count == 0
        || request.recent_window_count == 0
        || request.minimum_observations < 2
        || request.minimum_observations
            > request.baseline_window_count + request.recent_window_count
        || request.drift_threshold_milli == 0
        || request.qc_failure_threshold_milli > 1_000
        || request.downtime_cluster_threshold == 0
        || request.max_alerts == 0
        || request.max_alerts > MAX_INSTRUMENTS
        || request.current_tick == 0
    {
        return Err(FleetHealthError::InvalidRequest(
            "bounded fleet, windows, thresholds, alert limit, and current tick are required".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for observation in &request.observation_order {
        if !safe_text(&observation.instrument_id, MAX_REASON_LEN)
            || !safe_text(&observation.site_id, MAX_REASON_LEN)
            || observation.tick == 0
            || observation.tick > request.current_tick
            || observation.throughput_milli > 1_000
            || observation.qc_pass_milli > 1_000
            || observation.downtime_milli > 1_000
            || observation.calibration_error_milli > 1_000
            || !valid_hash(&observation.summary_digest)
            || !keys.insert((observation.instrument_id.clone(), observation.tick))
        {
            return Err(FleetHealthError::InvalidRequest(
                "observations require unique instrument ticks, bounded metrics, and digests".into(),
            ));
        }
        let expected = ContentHash::of_value(&observation_body(observation))
            .map_err(|error| FleetHealthError::Digest(error.to_string()))?;
        if expected != observation.summary_digest {
            return Err(FleetHealthError::InvalidRequest(format!(
                "observation {} at tick {} has a tampered summary digest",
                observation.instrument_id, observation.tick
            )));
        }
    }
    Ok(())
}

impl FleetHealthAssessment {
    pub fn validate(&self) -> Result<(), FleetHealthError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.fleet_id, MAX_REASON_LEN)
            || self.instrument_order.len() != self.assessments.len()
            || !canonical(&self.instrument_order)
            || self.assessments.iter().any(|assessment| {
                !safe_text(&assessment.instrument_id, MAX_REASON_LEN)
                    || assessment.baseline_observations == 0
                    || assessment.recent_observations == 0
                    || assessment.effective_observations == 0
                    || assessment.confidence_milli > 1_000
                    || !canonical(&assessment.reason_order)
                    || !canonical(&assessment.investigation_task_order)
            })
            || !canonical(&self.alert_order)
            || self.aggregate_digest_order.len() != self.assessments.len()
            || self
                .aggregate_digest_order
                .iter()
                .any(|digest| !valid_hash(digest))
            || self.freshness_tick == 0
            || !valid_hash(&self.digest)
        {
            return Err(FleetHealthError::InvalidOutput(
                "fleet assessment identity, alignment, partitions, metrics, or digest is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| FleetHealthError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FleetHealthError::InvalidOutput(
                "fleet health digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Assess instrument drift and failure clusters from local, aggregate-only summaries.
pub fn monitor_glioma_instrument_fleet_health(
    request: &FleetHealthMonitorRequest,
) -> Result<FleetHealthAssessment, FleetHealthError> {
    validate_request(request)?;
    let mut grouped =
        std::collections::BTreeMap::<String, Vec<&InstrumentHealthObservation>>::new();
    for observation in &request.observation_order {
        grouped
            .entry(observation.instrument_id.clone())
            .or_default()
            .push(observation);
    }
    let mut assessments = Vec::new();
    let mut alert_order = Vec::new();
    let mut aggregate_digest_order = Vec::new();
    for (instrument_id, mut observations) in grouped {
        observations.sort_by_key(|observation| observation.tick);
        let baseline = observations
            .iter()
            .take(request.baseline_window_count)
            .copied()
            .collect::<Vec<_>>();
        let recent = observations
            .iter()
            .rev()
            .take(request.recent_window_count)
            .copied()
            .collect::<Vec<_>>();
        if baseline.len() + recent.len() < request.minimum_observations {
            continue;
        }
        let throughput_drop = mean(
            &baseline
                .iter()
                .map(|o| o.throughput_milli as u64)
                .collect::<Vec<_>>(),
        ) as i32
            - mean(
                &recent
                    .iter()
                    .map(|o| o.throughput_milli as u64)
                    .collect::<Vec<_>>(),
            ) as i32;
        let qc_pass_drop = mean(
            &baseline
                .iter()
                .map(|o| o.qc_pass_milli as u64)
                .collect::<Vec<_>>(),
        ) as i32
            - mean(
                &recent
                    .iter()
                    .map(|o| o.qc_pass_milli as u64)
                    .collect::<Vec<_>>(),
            ) as i32;
        let calibration_error_rise = mean(
            &recent
                .iter()
                .map(|o| o.calibration_error_milli as u64)
                .collect::<Vec<_>>(),
        ) as i32
            - mean(
                &baseline
                    .iter()
                    .map(|o| o.calibration_error_milli as u64)
                    .collect::<Vec<_>>(),
            ) as i32;
        let qc_failure_count = recent
            .iter()
            .filter(|observation| observation.qc_pass_milli < request.qc_failure_threshold_milli)
            .count();
        let downtime_cluster_max = max_downtime_cluster(&observations);
        let mut reasons = BTreeSet::new();
        if throughput_drop >= request.drift_threshold_milli as i32 {
            reasons.insert("throughput-drift".to_string());
        }
        if qc_pass_drop >= request.drift_threshold_milli as i32 {
            reasons.insert("qc-pass-drift".to_string());
        }
        if calibration_error_rise >= request.drift_threshold_milli as i32 {
            reasons.insert("calibration-instability".to_string());
        }
        if qc_failure_count >= 2 {
            reasons.insert("repeated-qc-failure".to_string());
        }
        if downtime_cluster_max >= request.downtime_cluster_threshold {
            reasons.insert("downtime-cluster".to_string());
        }
        let signal_count = reasons.len();
        let confidence =
            ((baseline.len().min(32) + recent.len().min(32)) * 1_000 / 64).min(1_000) as u16;
        let disposition = if calibration_error_rise >= request.drift_threshold_milli as i32
            && qc_failure_count >= 2
        {
            FleetHealthDisposition::Blocked
        } else if signal_count >= 2 {
            FleetHealthDisposition::Investigate
        } else if signal_count == 1 || confidence < 500 {
            FleetHealthDisposition::Watch
        } else {
            FleetHealthDisposition::Healthy
        };
        let reason_order = reasons.into_iter().collect::<Vec<_>>();
        let investigation_task_order = if matches!(
            disposition,
            FleetHealthDisposition::Investigate | FleetHealthDisposition::Blocked
        ) {
            reason_order
                .iter()
                .map(|reason| format!("{}:{reason}", instrument_id))
                .collect()
        } else {
            Vec::new()
        };
        if !reason_order.is_empty() && alert_order.len() < request.max_alerts {
            alert_order.push(instrument_id.clone());
        }
        let site_label = if request.mask_site_identity {
            None
        } else {
            observations
                .first()
                .map(|observation| observation.site_id.clone())
        };
        let assessment = InstrumentHealthAssessment {
            instrument_id,
            site_label,
            disposition,
            baseline_observations: baseline.len(),
            recent_observations: recent.len(),
            effective_observations: baseline.len() + recent.len(),
            throughput_drop_milli: throughput_drop,
            qc_pass_drop_milli: qc_pass_drop,
            calibration_error_rise_milli: calibration_error_rise,
            downtime_cluster_max,
            qc_failure_count,
            confidence_milli: confidence,
            reason_order,
            investigation_task_order,
        };
        let digest = ContentHash::of_value(&serde_json::json!({
            "instrument_id": assessment.instrument_id,
            "disposition": assessment.disposition,
            "throughput_drop_milli": assessment.throughput_drop_milli,
            "qc_pass_drop_milli": assessment.qc_pass_drop_milli,
            "calibration_error_rise_milli": assessment.calibration_error_rise_milli,
            "downtime_cluster_max": assessment.downtime_cluster_max,
            "confidence_milli": assessment.confidence_milli,
        }))
        .map_err(|error| FleetHealthError::Digest(error.to_string()))?;
        aggregate_digest_order.push(digest);
        assessments.push(assessment);
    }
    assessments.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
    aggregate_digest_order.truncate(assessments.len());
    let instrument_order = assessments
        .iter()
        .map(|assessment| assessment.instrument_id.clone())
        .collect::<Vec<_>>();
    alert_order.sort();
    let fleet_disposition = if assessments
        .iter()
        .any(|assessment| assessment.disposition == FleetHealthDisposition::Blocked)
    {
        FleetHealthDisposition::Blocked
    } else if assessments
        .iter()
        .any(|assessment| assessment.disposition == FleetHealthDisposition::Investigate)
    {
        FleetHealthDisposition::Investigate
    } else if assessments
        .iter()
        .any(|assessment| assessment.disposition == FleetHealthDisposition::Watch)
    {
        FleetHealthDisposition::Watch
    } else {
        FleetHealthDisposition::Healthy
    };
    let mut output = FleetHealthAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        fleet_id: request.fleet_id.clone(),
        instrument_order,
        assessments,
        fleet_disposition,
        alert_order,
        aggregate_digest_order,
        freshness_tick: request.current_tick,
        digest: ContentHash::of_bytes(b"unsealed-glioma-fleet-health"),
    };
    output.digest = ContentHash::of_value(&digest_body(&output))
        .map_err(|error| FleetHealthError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &serde_json::Value) -> ContentHash {
        ContentHash::of_value(value).unwrap()
    }

    fn observation(
        instrument_id: &str,
        tick: u64,
        throughput: u16,
        qc: u16,
        calibration: u16,
        downtime: u16,
    ) -> InstrumentHealthObservation {
        let mut observation = InstrumentHealthObservation {
            instrument_id: instrument_id.into(),
            site_id: "site-a".into(),
            tick,
            throughput_milli: throughput,
            qc_pass_milli: qc,
            downtime_milli: downtime,
            calibration_error_milli: calibration,
            run_count: 10,
            summary_digest: hash(&serde_json::json!({"unsealed": tick})),
        };
        observation.summary_digest =
            ContentHash::of_value(&observation_body(&observation)).unwrap();
        observation
    }

    fn request(observation_order: Vec<InstrumentHealthObservation>) -> FleetHealthMonitorRequest {
        FleetHealthMonitorRequest {
            fleet_id: "glioma-fleet".into(),
            observation_order,
            baseline_window_count: 2,
            recent_window_count: 2,
            minimum_observations: 4,
            drift_threshold_milli: 100,
            qc_failure_threshold_milli: 800,
            downtime_cluster_threshold: 2,
            max_alerts: 8,
            current_tick: 10,
            mask_site_identity: true,
        }
    }

    #[test]
    fn detects_correlated_drift_and_qc_failure() {
        let output = monitor_glioma_instrument_fleet_health(&request(vec![
            observation("scope-1", 1, 900, 950, 50, 0),
            observation("scope-1", 2, 900, 950, 50, 0),
            observation("scope-1", 3, 700, 700, 220, 1),
            observation("scope-1", 4, 650, 650, 250, 1),
        ]))
        .unwrap();
        assert_eq!(output.fleet_disposition, FleetHealthDisposition::Blocked);
        assert_eq!(output.assessments[0].instrument_id, "scope-1");
        assert!(output.assessments[0].investigation_task_order.len() >= 2);
    }

    #[test]
    fn separates_single_metric_watch_from_multi_signal_investigation() {
        let output = monitor_glioma_instrument_fleet_health(&request(vec![
            observation("scope-1", 1, 900, 950, 50, 0),
            observation("scope-1", 2, 900, 950, 50, 0),
            observation("scope-1", 3, 780, 950, 50, 0),
            observation("scope-1", 4, 780, 950, 50, 0),
        ]))
        .unwrap();
        assert_eq!(
            output.assessments[0].disposition,
            FleetHealthDisposition::Watch
        );
        assert_eq!(
            output.assessments[0].investigation_task_order,
            Vec::<String>::new()
        );
    }

    #[test]
    fn rejects_tampered_summary_and_duplicate_ticks() {
        let mut tampered = observation("scope-1", 1, 900, 950, 50, 0);
        tampered.throughput_milli = 1;
        assert!(matches!(
            monitor_glioma_instrument_fleet_health(&request(vec![tampered])),
            Err(FleetHealthError::InvalidRequest(_))
        ));
        let duplicate = observation("scope-1", 1, 900, 950, 50, 0);
        assert!(matches!(
            monitor_glioma_instrument_fleet_health(&request(vec![
                observation("scope-1", 1, 900, 950, 50, 0),
                duplicate
            ])),
            Err(FleetHealthError::InvalidRequest(_))
        ));
    }

    #[test]
    fn masks_site_identity_and_is_digest_stable() {
        let output = monitor_glioma_instrument_fleet_health(&request(vec![
            observation("scope-1", 1, 900, 950, 50, 0),
            observation("scope-1", 2, 900, 950, 50, 0),
            observation("scope-1", 3, 900, 950, 50, 0),
            observation("scope-1", 4, 900, 950, 50, 0),
        ]))
        .unwrap();
        assert!(output.assessments[0].site_label.is_none());
        output.validate().unwrap();
    }
}
