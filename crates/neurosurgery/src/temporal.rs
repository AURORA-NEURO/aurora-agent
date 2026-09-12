//! Descriptive longitudinal alignment for de-identified neurosurgical intake.
//!
//! This is deliberately a temporal *audit*, not a trajectory model. It orders only caller-
//! supplied UTC timestamps, keeps undated and labelled-only observations visible, and reports
//! which specialty intake classes cannot yet be aligned. No interval is interpreted as growth,
//! progression, response, deterioration, or a recommendation.

use crate::evidence_audit::required_observation_kinds;
use crate::{CaseRequest, NeurosurgeryError, ObservationKind, ObservationStatus, Specialty};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the temporal-alignment contract.
pub const TEMPORAL_ALIGNMENT_SCHEMA_VERSION: &str = "bioprism-neurosurgery-temporal-alignment/0.1";

/// State of one observation class' date coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalCoverageState {
    Complete,
    Partial,
    Missing,
    NotObserved,
}

/// A source-preserving projection of one caller observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalObservation {
    pub observation_index: usize,
    pub observation_kind: ObservationKind,
    pub label: String,
    pub status: ObservationStatus,
    pub source_id: Option<String>,
    pub observed_at: Option<String>,
    pub timepoint: Option<String>,
}

/// Coverage of explicit acquisition/assessment dates for one observation class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalKindCoverage {
    pub observation_kind: ObservationKind,
    pub observed_count: usize,
    pub timestamped_count: usize,
    pub untimestamped_count: usize,
    pub earliest_observed_at: Option<String>,
    pub latest_observed_at: Option<String>,
    pub state: TemporalCoverageState,
}

/// All observations sharing one exact caller-supplied UTC timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalTimepoint {
    pub observed_at: String,
    pub observation_indices: Vec<usize>,
    pub observation_kinds: Vec<ObservationKind>,
    pub labels: Vec<String>,
}

/// A concrete, reviewable temporal issue. An issue is metadata about alignment, not an inference
/// about what changed between two observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalFinding {
    pub code: String,
    pub detail: String,
    pub observation_indices: Vec<usize>,
}

/// Digest-bound, deterministic temporal coverage report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalAlignmentReport {
    pub schema_version: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub observation_count: usize,
    pub timestamped_observation_count: usize,
    pub untimestamped_observation_count: usize,
    pub labelled_without_timestamp_count: usize,
    pub distinct_timestamp_count: usize,
    /// Number of observations after the first whose timestamp is earlier than the preceding
    /// timestamp in caller order. This does not reorder or discard any input.
    pub input_order_inversion_count: usize,
    /// Number of observations beyond the first sharing an exact timestamp.
    pub duplicate_timestamp_count: usize,
    pub required_time_aligned_kinds: Vec<ObservationKind>,
    pub missing_time_aligned_kinds: Vec<ObservationKind>,
    pub kind_coverage: Vec<TemporalKindCoverage>,
    pub timepoints: Vec<TemporalTimepoint>,
    pub observations: Vec<TemporalObservation>,
    pub status: TemporalAlignmentStatus,
    pub coverage_complete: bool,
    pub findings: Vec<TemporalFinding>,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

/// Overall temporal posture. `Complete` means only that every supplied observation and every
/// required intake class has an explicit timestamp and caller order is chronological; it is not
/// a claim of clinical sufficiency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalAlignmentStatus {
    Complete,
    Partial,
    Unavailable,
    RequiresReview,
}

/// Build the temporal report after request safety/bounds validation.
pub fn audit_temporal(request: &CaseRequest) -> Result<TemporalAlignmentReport, NeurosurgeryError> {
    let required = required_observation_kinds(request.specialty).to_vec();
    let mut timestamp_groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut timestamp_kinds: BTreeMap<String, BTreeSet<ObservationKind>> = BTreeMap::new();
    let mut timestamp_labels: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut kind_rows: BTreeMap<ObservationKind, Vec<(usize, Option<&str>)>> = BTreeMap::new();
    let mut observations = Vec::with_capacity(request.observations.len());
    let mut timestamped_observation_count = 0usize;
    let mut labelled_without_timestamp_count = 0usize;
    let mut input_order_inversion_count = 0usize;
    let mut previous_timestamp: Option<&str> = None;

    for (index, observation) in request.observations.iter().enumerate() {
        if let Some(timestamp) = observation.observed_at.as_deref() {
            if !is_utc_timestamp(timestamp) {
                return Err(NeurosurgeryError::TemporalRejected {
                    reason: format!(
                        "observation {index} observed_at must be a UTC RFC3339 timestamp"
                    ),
                });
            }
            if previous_timestamp.is_some_and(|previous| timestamp < previous) {
                input_order_inversion_count = input_order_inversion_count.saturating_add(1);
            }
            previous_timestamp = Some(timestamp);
            timestamped_observation_count = timestamped_observation_count.saturating_add(1);
            timestamp_groups
                .entry(timestamp.to_string())
                .or_default()
                .push(index);
            timestamp_kinds
                .entry(timestamp.to_string())
                .or_default()
                .insert(observation.kind);
            if let Some(label) = observation.timepoint.as_deref() {
                timestamp_labels
                    .entry(timestamp.to_string())
                    .or_default()
                    .insert(label.to_string());
            }
        } else if observation.timepoint.is_some() {
            labelled_without_timestamp_count = labelled_without_timestamp_count.saturating_add(1);
        }
        kind_rows
            .entry(observation.kind)
            .or_default()
            .push((index, observation.observed_at.as_deref()));
        observations.push(TemporalObservation {
            observation_index: index,
            observation_kind: observation.kind,
            label: observation.label.clone(),
            status: observation.status,
            source_id: observation.source_id.clone(),
            observed_at: observation.observed_at.clone(),
            timepoint: observation.timepoint.clone(),
        });
    }

    let mut all_kinds = required.iter().copied().collect::<BTreeSet<_>>();
    all_kinds.extend(kind_rows.keys().copied());
    let mut missing_time_aligned_kinds = Vec::new();
    let mut kind_coverage = Vec::with_capacity(all_kinds.len());
    for kind in all_kinds {
        let rows = kind_rows.get(&kind).cloned().unwrap_or_default();
        let observed_count = rows.len();
        let timestamped_count = rows
            .iter()
            .filter(|(_, timestamp)| timestamp.is_some())
            .count();
        let untimestamped_count = observed_count.saturating_sub(timestamped_count);
        let timestamps = rows
            .iter()
            .filter_map(|(_, timestamp)| *timestamp)
            .collect::<Vec<_>>();
        let earliest_observed_at = timestamps.iter().copied().min().map(str::to_string);
        let latest_observed_at = timestamps.iter().copied().max().map(str::to_string);
        let state = if observed_count == 0 {
            TemporalCoverageState::NotObserved
        } else if timestamped_count == 0 {
            TemporalCoverageState::Missing
        } else if untimestamped_count > 0 {
            TemporalCoverageState::Partial
        } else {
            TemporalCoverageState::Complete
        };
        if required.contains(&kind) && state != TemporalCoverageState::Complete {
            missing_time_aligned_kinds.push(kind);
        }
        kind_coverage.push(TemporalKindCoverage {
            observation_kind: kind,
            observed_count,
            timestamped_count,
            untimestamped_count,
            earliest_observed_at,
            latest_observed_at,
            state,
        });
    }

    let timepoints = timestamp_groups
        .iter()
        .map(|(observed_at, indices)| TemporalTimepoint {
            observed_at: observed_at.clone(),
            observation_indices: indices.clone(),
            observation_kinds: timestamp_kinds
                .get(observed_at)
                .into_iter()
                .flat_map(|kinds| kinds.iter().copied())
                .collect(),
            labels: timestamp_labels
                .get(observed_at)
                .into_iter()
                .flat_map(|labels| labels.iter().cloned())
                .collect(),
        })
        .collect::<Vec<_>>();
    let duplicate_timestamp_count = timestamp_groups
        .values()
        .map(|indices| indices.len().saturating_sub(1))
        .sum();

    let mut findings = Vec::new();
    if request.observations.is_empty() {
        findings.push(TemporalFinding {
            code: "no_observations".to_string(),
            detail: "no observations were supplied, so longitudinal alignment is unavailable"
                .to_string(),
            observation_indices: Vec::new(),
        });
    } else if timestamped_observation_count == 0 {
        findings.push(TemporalFinding {
            code: "no_observed_at".to_string(),
            detail: "no caller-supplied observation has an explicit observed_at timestamp"
                .to_string(),
            observation_indices: (0..request.observations.len()).collect(),
        });
    }
    for kind in &missing_time_aligned_kinds {
        let indices = kind_rows
            .get(kind)
            .map(|rows| rows.iter().map(|(index, _)| *index).collect())
            .unwrap_or_default();
        let coverage = kind_coverage
            .iter()
            .find(|coverage| coverage.observation_kind == *kind)
            .expect("coverage was built for every required kind");
        findings.push(TemporalFinding {
            code: if coverage.state == TemporalCoverageState::NotObserved {
                "required_kind_not_observed".to_string()
            } else if coverage.state == TemporalCoverageState::Missing {
                "required_kind_undated".to_string()
            } else {
                "required_kind_partially_dated".to_string()
            },
            detail: format!(
                "{} has {} observed record(s), {} timestamped; no temporal sufficiency is inferred",
                observation_kind_label(*kind),
                coverage.observed_count,
                coverage.timestamped_count
            ),
            observation_indices: indices,
        });
    }
    if labelled_without_timestamp_count > 0 {
        let indices = observations
            .iter()
            .filter(|observation| {
                observation.timepoint.is_some() && observation.observed_at.is_none()
            })
            .map(|observation| observation.observation_index)
            .collect();
        findings.push(TemporalFinding {
            code: "timepoint_label_without_timestamp".to_string(),
            detail: format!(
                "{} observation(s) carry a caller label but no UTC observed_at; date ordering remains unknown",
                labelled_without_timestamp_count
            ),
            observation_indices: indices,
        });
    }
    if input_order_inversion_count > 0 {
        findings.push(TemporalFinding {
            code: "input_order_not_chronological".to_string(),
            detail: format!(
                "{} timestamp ordering inversion(s) were supplied; input order was preserved",
                input_order_inversion_count
            ),
            observation_indices: Vec::new(),
        });
    }
    if duplicate_timestamp_count > 0 {
        let indices = timestamp_groups
            .values()
            .filter(|indices| indices.len() > 1)
            .flat_map(|indices| indices.iter().copied())
            .collect();
        findings.push(TemporalFinding {
            code: "duplicate_observed_at".to_string(),
            detail: format!(
                "{} observation(s) share a timestamp with another observation; same-time records are retained",
                duplicate_timestamp_count
            ),
            observation_indices: indices,
        });
    }

    let status = if request.observations.is_empty() {
        TemporalAlignmentStatus::Unavailable
    } else if input_order_inversion_count > 0 {
        TemporalAlignmentStatus::RequiresReview
    } else if timestamped_observation_count < request.observations.len()
        || !missing_time_aligned_kinds.is_empty()
    {
        TemporalAlignmentStatus::Partial
    } else {
        TemporalAlignmentStatus::Complete
    };
    let coverage_complete = status == TemporalAlignmentStatus::Complete;
    let request_digest = digest_request(request)?;
    Ok(TemporalAlignmentReport {
        schema_version: TEMPORAL_ALIGNMENT_SCHEMA_VERSION.to_string(),
        request_digest,
        specialty: request.specialty,
        observation_count: request.observations.len(),
        timestamped_observation_count,
        untimestamped_observation_count: request
            .observations
            .len()
            .saturating_sub(timestamped_observation_count),
        labelled_without_timestamp_count,
        distinct_timestamp_count: timepoints.len(),
        input_order_inversion_count,
        duplicate_timestamp_count,
        required_time_aligned_kinds: required,
        missing_time_aligned_kinds,
        kind_coverage,
        timepoints,
        observations,
        status,
        coverage_complete,
        findings,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "timestamps and labels are caller-supplied metadata; no date is inferred from free text".to_string(),
            "alignment does not establish progression, response, prognosis, diagnosis, treatment, triage, or operative suitability".to_string(),
            "complete temporal coverage is not clinical sufficiency and still requires qualified human review".to_string(),
        ],
    })
}

fn digest_request(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn observation_kind_label(kind: ObservationKind) -> &'static str {
    match kind {
        ObservationKind::Imaging => "imaging",
        ObservationKind::Histology => "histology",
        ObservationKind::Molecular => "molecular",
        ObservationKind::Neuroanatomy => "neuroanatomy",
        ObservationKind::NeurologicFunction => "neurologic function",
        ObservationKind::DevelopmentalTrajectory => "developmental trajectory",
        ObservationKind::SpinalDysraphism => "spinal dysraphism",
        ObservationKind::CraniocervicalJunction => "craniocervical junction",
        ObservationKind::SurgicalHistory => "surgical history",
        ObservationKind::LongitudinalOutcome => "longitudinal outcome",
    }
}

/// The bounded timestamp grammar used by the neurosurgery contracts. UTC-only values make
/// lexicographic ordering deterministic across Rust, Python, and TypeScript callers.
pub(crate) fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.len() != 20
        || ![0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let hour = (bytes[11] - b'0') * 10 + (bytes[12] - b'0');
    let minute = (bytes[14] - b'0') * 10 + (bytes[15] - b'0');
    let second = (bytes[17] - b'0') * 10 + (bytes[18] - b'0');
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    month >= 1 && day >= 1 && day <= days_in_month && hour < 24 && minute < 60 && second < 60
}
