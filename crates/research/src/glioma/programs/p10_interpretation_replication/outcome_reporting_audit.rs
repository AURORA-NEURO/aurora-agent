//! Registered-outcome completeness and timing audit for local preclinical studies.
//!
//! The detailed P10-F04 blueprint is not configured in this checkout. This repository-defined
//! audit compares digest-bound registry plans with caller-reviewed result reports. It identifies
//! overdue missing primary outcomes, incomplete or ambiguous reports, retrospective registration,
//! and reported outcomes absent from the registry. It does not inspect effect values, infer why a
//! result is absent, or claim publication bias. All source artifacts remain local and de-identified.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = super::lineage_response_decomposition::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaRegisteredOutcomeReportingAudit1@1";
pub const MAX_STUDIES: usize = 512;
pub const MAX_OUTCOMES_PER_STUDY: usize = 64;
pub const MAX_REPORTED_OUTCOMES_PER_STUDY: usize = 128;
pub const MAX_TOTAL_OUTCOMES: usize = 32_768;
pub const MAX_EPOCH_DAY: u32 = 200_000;
pub const MAX_REPORTING_LAG_DAYS: u16 = 3_650;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisteredOutcomeRole {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisteredStudyStatus {
    Completed,
    Terminated,
    Ongoing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcome {
    pub outcome_id: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub role: RegisteredOutcomeRole,
    pub outcome_window_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportedOutcomeStatus {
    Reported,
    Incomplete,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportedOutcome {
    pub outcome_id: String,
    /// Reporting completeness only. Effect values, signs, and statistical significance are absent.
    pub status: ReportedOutcomeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyOutcomeReport {
    pub report_digest: ContentHash,
    pub report_day: u32,
    pub artifact: LocalArtifactRef,
    pub outcomes: Vec<ReportedOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcomeStudy {
    pub study_id: String,
    pub independence_group: String,
    pub registry_report_digest: ContentHash,
    pub protocol_artifact: LocalArtifactRef,
    pub registration_day: Option<u32>,
    pub first_enrollment_day: Option<u32>,
    pub status: RegisteredStudyStatus,
    pub completion_day: Option<u32>,
    pub registered_outcomes: Vec<RegisteredOutcome>,
    pub result_report: Option<StudyOutcomeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeReportingAuditRequest {
    pub objective: String,
    pub registry_report_order: Vec<ContentHash>,
    pub result_report_order: Vec<ContentHash>,
    pub evaluation_day: u32,
    pub max_reporting_lag_days: u16,
    pub min_studies: usize,
    pub min_independent_groups: usize,
    pub min_completed_studies: usize,
    pub min_primary_coverage_milli: u16,
    pub max_unreported_primary_groups: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationTiming {
    Prospective,
    Retrospective,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeAssessmentStatus {
    Reported,
    Incomplete,
    Ambiguous,
    Missing,
    NotDue,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeAssessment {
    pub study_id: String,
    pub outcome_id: String,
    pub role: Option<RegisteredOutcomeRole>,
    pub registered: bool,
    pub due: bool,
    pub status: OutcomeAssessmentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyReportingAssessment {
    pub study_id: String,
    pub independence_group: String,
    pub status: RegisteredStudyStatus,
    pub registration_timing: RegistrationTiming,
    pub registry_report_digest: ContentHash,
    pub result_report_digest: Option<ContentHash>,
    pub completed_study: bool,
    pub reporting_due: bool,
    pub registered_primary_count: usize,
    pub due_primary_count: usize,
    pub reported_primary_count: usize,
    pub overdue_missing_primary_count: usize,
    pub unregistered_reported_outcome_count: usize,
    pub outcomes: Vec<OutcomeAssessment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeReportingDisposition {
    InsufficientEvidence,
    InsufficientFollowUp,
    MaterialReportingGap,
    RegistrationAnomaly,
    Uncertain,
    CoverageThresholdMet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcomeReportingAudit {
    pub feature_id: String,
    pub output_schema: String,
    pub request: OutcomeReportingAuditRequest,
    pub studies: Vec<RegisteredOutcomeStudy>,
    pub input_digest: ContentHash,
    pub study_order: Vec<String>,
    pub study_assessments: Vec<StudyReportingAssessment>,
    pub outcome_order: Vec<OutcomeAssessment>,
    pub unreported_primary_group_order: Vec<String>,
    pub retrospective_registration_order: Vec<String>,
    pub unregistered_reported_outcome_order: Vec<String>,
    pub completed_study_count: usize,
    pub reporting_due_study_count: usize,
    pub due_primary_count: usize,
    pub reported_primary_count: usize,
    pub primary_coverage_milli: u16,
    pub incomplete_outcome_count: usize,
    pub ambiguous_outcome_count: usize,
    pub unknown_timing_study_order: Vec<String>,
    pub signals: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: OutcomeReportingDisposition,
    /// Audit only. No source is fetched, no study is contacted, and no research action is dispatched.
    pub dispatch: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutcomeReportingAuditError {
    #[error("outcome-reporting audit request is invalid: {0}")]
    InvalidRequest(String),
    #[error("registered outcome study rows are invalid: {0}")]
    InvalidStudy(String),
    #[error("outcome-reporting audit output is invalid: {0}")]
    InvalidOutput(String),
    #[error("outcome-reporting audit digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn validate_request(
    request: &OutcomeReportingAuditRequest,
) -> Result<(), OutcomeReportingAuditError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > 4_096
        || !(2..=MAX_STUDIES).contains(&request.min_studies)
        || !(2..=request.min_studies).contains(&request.min_independent_groups)
        || !(1..=request.min_studies).contains(&request.min_completed_studies)
        || request.registry_report_order.len() > MAX_STUDIES
        || !canonical(&request.registry_report_order)
        || request
            .registry_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || request.result_report_order.len() > MAX_STUDIES
        || !canonical(&request.result_report_order)
        || request
            .result_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || request.evaluation_day == 0
        || request.evaluation_day > MAX_EPOCH_DAY
        || request.max_reporting_lag_days > MAX_REPORTING_LAG_DAYS
        || request.min_primary_coverage_milli > 1_000
        || request.max_unreported_primary_groups > MAX_STUDIES
    {
        return Err(OutcomeReportingAuditError::InvalidRequest(
            "objective, canonical registry/result report digests, study floors, evaluation day, and bounded reporting thresholds are required".into(),
        ));
    }
    Ok(())
}

fn validate_day(day: Option<u32>, evaluation_day: u32) -> bool {
    day.is_none_or(|value| value > 0 && value <= evaluation_day && value <= MAX_EPOCH_DAY)
}

fn validate_studies(
    request: &OutcomeReportingAuditRequest,
    studies: &[RegisteredOutcomeStudy],
) -> Result<(), OutcomeReportingAuditError> {
    if studies.len() > MAX_STUDIES {
        return Err(OutcomeReportingAuditError::InvalidStudy(
            "study count exceeds the declared bound".into(),
        ));
    }
    let registry_reports = request
        .registry_report_order
        .iter()
        .collect::<BTreeSet<_>>();
    let result_reports = request.result_report_order.iter().collect::<BTreeSet<_>>();
    let mut used_registry_reports = BTreeSet::new();
    let mut used_result_reports = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    let mut independence_groups = BTreeSet::new();
    let mut total_outcomes = 0usize;
    for study in studies {
        let primary_count = study
            .registered_outcomes
            .iter()
            .filter(|outcome| outcome.role == RegisteredOutcomeRole::Primary)
            .count();
        let mut outcome_ids = BTreeSet::new();
        total_outcomes = total_outcomes.saturating_add(study.registered_outcomes.len());
        let outcomes_valid = !study.registered_outcomes.is_empty()
            && study.registered_outcomes.len() <= MAX_OUTCOMES_PER_STUDY
            && primary_count > 0
            && study.registered_outcomes.iter().all(|outcome| {
                !outcome.outcome_id.trim().is_empty()
                    && outcome.outcome_id.len() <= 128
                    && !outcome.estimand_id.trim().is_empty()
                    && outcome.estimand_id.len() <= 128
                    && !outcome.effect_unit.trim().is_empty()
                    && outcome.effect_unit.len() <= 128
                    && outcome.outcome_window_days > 0
                    && outcome.outcome_window_days <= MAX_EPOCH_DAY
                    && outcome_ids.insert(outcome.outcome_id.as_str())
            });
        let date_order_valid = match (study.registration_day, study.first_enrollment_day) {
            (Some(registration), Some(enrollment)) => {
                registration <= MAX_EPOCH_DAY
                    && enrollment <= MAX_EPOCH_DAY
                    && registration <= request.evaluation_day
                    && enrollment <= request.evaluation_day
            }
            (registration, enrollment) => {
                validate_day(registration, request.evaluation_day)
                    && validate_day(enrollment, request.evaluation_day)
            }
        };
        let completion_valid = match study.status {
            RegisteredStudyStatus::Completed | RegisteredStudyStatus::Terminated => {
                study.completion_day.is_some()
                    && validate_day(study.completion_day, request.evaluation_day)
                    && study.first_enrollment_day.is_none_or(|enrollment| {
                        study
                            .completion_day
                            .is_some_and(|completion| completion >= enrollment)
                    })
            }
            RegisteredStudyStatus::Ongoing | RegisteredStudyStatus::Unknown => {
                study.completion_day.is_none()
            }
        };
        let mut report_valid = true;
        if let Some(report) = &study.result_report {
            total_outcomes = total_outcomes.saturating_add(report.outcomes.len());
            let mut reported_ids = BTreeSet::new();
            report_valid = valid_hash(&report.report_digest)
                && result_reports.contains(&&report.report_digest)
                && used_result_reports.insert(&report.report_digest)
                && report.report_day > 0
                && report.report_day <= request.evaluation_day
                && report.report_day <= MAX_EPOCH_DAY
                && report.outcomes.len() <= MAX_REPORTED_OUTCOMES_PER_STUDY
                && report.outcomes.iter().all(|outcome| {
                    !outcome.outcome_id.trim().is_empty()
                        && outcome.outcome_id.len() <= 128
                        && reported_ids.insert(outcome.outcome_id.as_str())
                })
                && report.artifact.validate().is_ok()
                && report.artifact.local_only
                && !report.artifact.contains_human_data
                && !report.artifact.contains_direct_identifiers;
        }
        let registry_valid = valid_hash(&study.registry_report_digest)
            && registry_reports.contains(&&study.registry_report_digest)
            && used_registry_reports.insert(&study.registry_report_digest);
        if study.study_id.trim().is_empty()
            || study.study_id.len() > 128
            || study.independence_group.trim().is_empty()
            || study.independence_group.len() > 128
            || !study_ids.insert(study.study_id.as_str())
            || !independence_groups.insert(study.independence_group.as_str())
            || !registry_valid
            || study.protocol_artifact.validate().is_err()
            || !study.protocol_artifact.local_only
            || study.protocol_artifact.contains_human_data
            || study.protocol_artifact.contains_direct_identifiers
            || !date_order_valid
            || study.registration_day.is_some_and(|registration| {
                study
                    .first_enrollment_day
                    .is_some_and(|enrollment| registration > enrollment)
                    && registration > request.evaluation_day
            })
            || !completion_valid
            || !outcomes_valid
            || !report_valid
            || study.result_report.as_ref().is_some_and(|report| {
                matches!(
                    study.status,
                    RegisteredStudyStatus::Completed | RegisteredStudyStatus::Terminated
                ) && study.completion_day.is_some_and(|completion| {
                    report.report_day < study.first_enrollment_day.unwrap_or(completion)
                })
            })
        {
            return Err(OutcomeReportingAuditError::InvalidStudy(
                "each study must bind a unique independent group, registry protocol, valid dates, at least one primary registered outcome, and optional local de-identified result report".into(),
            ));
        }
    }
    if total_outcomes > MAX_TOTAL_OUTCOMES
        || used_registry_reports != registry_reports
        || used_result_reports != result_reports
    {
        return Err(OutcomeReportingAuditError::InvalidStudy(
            "outcome/report bounds or exact source-report coverage are invalid".into(),
        ));
    }
    Ok(())
}

fn input_digest(
    request: &OutcomeReportingAuditRequest,
    studies: &[RegisteredOutcomeStudy],
) -> Result<ContentHash, OutcomeReportingAuditError> {
    #[derive(Serialize)]
    struct Input<'a> {
        schema: &'static str,
        request: &'a OutcomeReportingAuditRequest,
        studies: &'a [RegisteredOutcomeStudy],
    }
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    ContentHash::of_serializable(&Input {
        schema: "GliomaRegisteredOutcomeReportingAuditInput1@1",
        request,
        studies: &ordered,
    })
    .map_err(|error| OutcomeReportingAuditError::Digest(error.to_string()))
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a OutcomeReportingAuditRequest,
    input_digest: &'a ContentHash,
    study_order: &'a [String],
    study_assessments: &'a [StudyReportingAssessment],
    outcome_order: &'a [OutcomeAssessment],
    unreported_primary_group_order: &'a [String],
    retrospective_registration_order: &'a [String],
    unregistered_reported_outcome_order: &'a [String],
    completed_study_count: usize,
    reporting_due_study_count: usize,
    due_primary_count: usize,
    reported_primary_count: usize,
    primary_coverage_milli: u16,
    incomplete_outcome_count: usize,
    ambiguous_outcome_count: usize,
    unknown_timing_study_order: &'a [String],
    signals: &'a [String],
    uncertainty: &'a [String],
    disposition: OutcomeReportingDisposition,
    dispatch: &'a str,
}

fn output_digest(
    output: &RegisteredOutcomeReportingAudit,
) -> Result<ContentHash, OutcomeReportingAuditError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &output.feature_id,
        output_schema: &output.output_schema,
        request: &output.request,
        input_digest: &output.input_digest,
        study_order: &output.study_order,
        study_assessments: &output.study_assessments,
        outcome_order: &output.outcome_order,
        unreported_primary_group_order: &output.unreported_primary_group_order,
        retrospective_registration_order: &output.retrospective_registration_order,
        unregistered_reported_outcome_order: &output.unregistered_reported_outcome_order,
        completed_study_count: output.completed_study_count,
        reporting_due_study_count: output.reporting_due_study_count,
        due_primary_count: output.due_primary_count,
        reported_primary_count: output.reported_primary_count,
        primary_coverage_milli: output.primary_coverage_milli,
        incomplete_outcome_count: output.incomplete_outcome_count,
        ambiguous_outcome_count: output.ambiguous_outcome_count,
        unknown_timing_study_order: &output.unknown_timing_study_order,
        signals: &output.signals,
        uncertainty: &output.uncertainty,
        disposition: output.disposition,
        dispatch: &output.dispatch,
    })
    .map_err(|error| OutcomeReportingAuditError::Digest(error.to_string()))
}

fn is_reporting_due(
    request: &OutcomeReportingAuditRequest,
    study: &RegisteredOutcomeStudy,
) -> bool {
    study.status == RegisteredStudyStatus::Completed
        && study.completion_day.is_some_and(|completion| {
            u64::from(request.evaluation_day)
                >= u64::from(completion) + u64::from(request.max_reporting_lag_days)
        })
}

fn registration_timing(study: &RegisteredOutcomeStudy) -> RegistrationTiming {
    match (study.registration_day, study.first_enrollment_day) {
        (Some(registration), Some(enrollment)) if registration <= enrollment => {
            RegistrationTiming::Prospective
        }
        (Some(_), Some(_)) => RegistrationTiming::Retrospective,
        _ => RegistrationTiming::Unknown,
    }
}

fn reported_status(status: ReportedOutcomeStatus) -> OutcomeAssessmentStatus {
    match status {
        ReportedOutcomeStatus::Reported => OutcomeAssessmentStatus::Reported,
        ReportedOutcomeStatus::Incomplete => OutcomeAssessmentStatus::Incomplete,
        ReportedOutcomeStatus::Ambiguous => OutcomeAssessmentStatus::Ambiguous,
    }
}

fn assess_study(
    request: &OutcomeReportingAuditRequest,
    study: &RegisteredOutcomeStudy,
) -> StudyReportingAssessment {
    let reporting_due = is_reporting_due(request, study);
    let timing = registration_timing(study);
    let report = study.result_report.as_ref();
    let mut outcomes = Vec::new();
    let mut due_primary_count = 0usize;
    let mut reported_primary_count = 0usize;
    let mut overdue_missing_primary_count = 0usize;
    for registered in &study.registered_outcomes {
        let due = reporting_due && registered.role == RegisteredOutcomeRole::Primary;
        if due {
            due_primary_count += 1;
        }
        let report_row = report.and_then(|value| {
            value
                .outcomes
                .iter()
                .find(|reported| reported.outcome_id == registered.outcome_id)
        });
        let status = if let Some(row) = report_row {
            reported_status(row.status)
        } else if due {
            OutcomeAssessmentStatus::Missing
        } else if study.status == RegisteredStudyStatus::Unknown
            || timing == RegistrationTiming::Unknown
        {
            OutcomeAssessmentStatus::Unknown
        } else {
            OutcomeAssessmentStatus::NotDue
        };
        if due && status == OutcomeAssessmentStatus::Reported {
            reported_primary_count += 1;
        }
        if due && status != OutcomeAssessmentStatus::Reported {
            overdue_missing_primary_count += 1;
        }
        outcomes.push(OutcomeAssessment {
            study_id: study.study_id.clone(),
            outcome_id: registered.outcome_id.clone(),
            role: Some(registered.role),
            registered: true,
            due,
            status,
        });
    }
    let mut unregistered_reported_outcome_count = 0usize;
    if let Some(report) = report {
        for reported in &report.outcomes {
            if !study
                .registered_outcomes
                .iter()
                .any(|registered| registered.outcome_id == reported.outcome_id)
            {
                unregistered_reported_outcome_count += 1;
                outcomes.push(OutcomeAssessment {
                    study_id: study.study_id.clone(),
                    outcome_id: reported.outcome_id.clone(),
                    role: None,
                    registered: false,
                    due: false,
                    status: reported_status(reported.status),
                });
            }
        }
    }
    outcomes.sort_by(|left, right| left.outcome_id.cmp(&right.outcome_id));
    StudyReportingAssessment {
        study_id: study.study_id.clone(),
        independence_group: study.independence_group.clone(),
        status: study.status,
        registration_timing: timing,
        registry_report_digest: study.registry_report_digest.clone(),
        result_report_digest: report.map(|value| value.report_digest.clone()),
        completed_study: study.status == RegisteredStudyStatus::Completed,
        reporting_due,
        registered_primary_count: study
            .registered_outcomes
            .iter()
            .filter(|outcome| outcome.role == RegisteredOutcomeRole::Primary)
            .count(),
        due_primary_count,
        reported_primary_count,
        overdue_missing_primary_count,
        unregistered_reported_outcome_count,
        outcomes,
    }
}

struct ReportingMetrics {
    study_count: usize,
    group_count: usize,
    completed_count: usize,
    reporting_due_count: usize,
    due_primary_count: usize,
    primary_coverage_milli: u16,
    unreported_group_count: usize,
    retrospective_count: usize,
    unregistered_count: usize,
    uncertain_count: usize,
}

fn derive_disposition(
    request: &OutcomeReportingAuditRequest,
    metrics: ReportingMetrics,
) -> OutcomeReportingDisposition {
    if metrics.study_count < request.min_studies
        || metrics.group_count < request.min_independent_groups
    {
        OutcomeReportingDisposition::InsufficientEvidence
    } else if metrics.completed_count < request.min_completed_studies
        || metrics.reporting_due_count < request.min_completed_studies
        || metrics.due_primary_count == 0
    {
        OutcomeReportingDisposition::InsufficientFollowUp
    } else if metrics.retrospective_count > 0 || metrics.unregistered_count > 0 {
        OutcomeReportingDisposition::RegistrationAnomaly
    } else if metrics.primary_coverage_milli < request.min_primary_coverage_milli
        || metrics.unreported_group_count > request.max_unreported_primary_groups
    {
        OutcomeReportingDisposition::MaterialReportingGap
    } else if metrics.uncertain_count > 0 {
        OutcomeReportingDisposition::Uncertain
    } else {
        OutcomeReportingDisposition::CoverageThresholdMet
    }
}

fn compile(
    request: &OutcomeReportingAuditRequest,
    studies: &[RegisteredOutcomeStudy],
) -> Result<RegisteredOutcomeReportingAudit, OutcomeReportingAuditError> {
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = ordered
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let study_assessments = ordered
        .iter()
        .map(|study| assess_study(request, study))
        .collect::<Vec<_>>();
    let mut outcome_order = study_assessments
        .iter()
        .flat_map(|study| study.outcomes.iter().cloned())
        .collect::<Vec<_>>();
    outcome_order.sort_by(|left, right| {
        (&left.study_id, &left.outcome_id).cmp(&(&right.study_id, &right.outcome_id))
    });
    let mut unreported_groups = study_assessments
        .iter()
        .filter(|study| study.overdue_missing_primary_count > 0)
        .map(|study| study.independence_group.clone())
        .collect::<Vec<_>>();
    unreported_groups.sort();
    let retrospective_registration_order = study_assessments
        .iter()
        .filter(|study| study.registration_timing == RegistrationTiming::Retrospective)
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let unregistered_reported_outcome_order = study_assessments
        .iter()
        .filter(|study| study.unregistered_reported_outcome_count > 0)
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let unknown_timing_study_order = study_assessments
        .iter()
        .filter(|study| study.registration_timing == RegistrationTiming::Unknown)
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let completed_study_count = study_assessments
        .iter()
        .filter(|study| study.completed_study)
        .count();
    let reporting_due_study_count = study_assessments
        .iter()
        .filter(|study| study.reporting_due)
        .count();
    let due_primary_count = study_assessments
        .iter()
        .map(|study| study.due_primary_count)
        .sum::<usize>();
    let reported_primary_count = study_assessments
        .iter()
        .map(|study| study.reported_primary_count)
        .sum::<usize>();
    let primary_coverage_milli = if due_primary_count == 0 {
        0
    } else {
        ((reported_primary_count as u128 * 1_000) / due_primary_count as u128) as u16
    };
    let incomplete_outcome_count = outcome_order
        .iter()
        .filter(|outcome| outcome.status == OutcomeAssessmentStatus::Incomplete)
        .count();
    let ambiguous_outcome_count = outcome_order
        .iter()
        .filter(|outcome| outcome.status == OutcomeAssessmentStatus::Ambiguous)
        .count();
    let unknown_count = outcome_order
        .iter()
        .filter(|outcome| outcome.status == OutcomeAssessmentStatus::Unknown)
        .count();
    let disposition = derive_disposition(
        request,
        ReportingMetrics {
            study_count: study_assessments.len(),
            group_count: study_assessments
                .iter()
                .map(|study| study.independence_group.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            completed_count: completed_study_count,
            reporting_due_count: reporting_due_study_count,
            due_primary_count,
            primary_coverage_milli,
            unreported_group_count: unreported_groups.len(),
            retrospective_count: retrospective_registration_order.len(),
            unregistered_count: unregistered_reported_outcome_order.len(),
            uncertain_count: incomplete_outcome_count + ambiguous_outcome_count + unknown_count,
        },
    );
    let mut signals = BTreeSet::new();
    if !unreported_groups.is_empty() {
        signals.insert("overdue_primary_outcome_reporting_gaps".to_string());
    }
    if !retrospective_registration_order.is_empty() {
        signals.insert("retrospective_registry_entries".to_string());
    }
    if !unregistered_reported_outcome_order.is_empty() {
        signals.insert("reported_outcome_absent_from_registry_plan".to_string());
    }
    if incomplete_outcome_count > 0 {
        signals.insert("incomplete_result_reporting".to_string());
    }
    if ambiguous_outcome_count > 0 {
        signals.insert("ambiguous_result_reporting".to_string());
    }
    let mut uncertainty = BTreeSet::new();
    if !unknown_timing_study_order.is_empty() {
        uncertainty.insert("registry_or_enrollment_timing_unknown".to_string());
    }
    if study_assessments
        .iter()
        .any(|study| study.status == RegisteredStudyStatus::Unknown)
    {
        uncertainty.insert("study_completion_status_unknown".to_string());
    }
    if incomplete_outcome_count > 0 || ambiguous_outcome_count > 0 {
        uncertainty.insert("reported_outcome_completeness_not_resolved".to_string());
    }
    if !unregistered_reported_outcome_order.is_empty() {
        uncertainty.insert("unregistered_outcomes_require_human_protocol_review".to_string());
    }
    if !unreported_groups.is_empty() {
        uncertainty
            .insert("absence_is_not_evidence_of_unpublished_or_negative_results".to_string());
    }
    if reporting_due_study_count == 0 {
        uncertainty.insert("no_completed_studies_have_reached_the_reporting_lag".to_string());
    }
    let mut output = RegisteredOutcomeReportingAudit {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        studies: ordered,
        input_digest: input_digest(request, studies)?,
        study_order,
        study_assessments,
        outcome_order,
        unreported_primary_group_order: unreported_groups,
        retrospective_registration_order,
        unregistered_reported_outcome_order,
        completed_study_count,
        reporting_due_study_count,
        due_primary_count,
        reported_primary_count,
        primary_coverage_milli,
        incomplete_outcome_count,
        ambiguous_outcome_count,
        unknown_timing_study_order,
        signals: signals.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        dispatch: "not_dispatched_audit_only".into(),
        digest: ContentHash::of_bytes(b"unsealed-registered-outcome-reporting-audit"),
    };
    output.digest = output_digest(&output)?;
    Ok(output)
}

impl RegisteredOutcomeReportingAudit {
    pub fn validate(&self) -> Result<(), OutcomeReportingAuditError> {
        self.validate_against(&self.request, &self.studies)
    }

    pub fn validate_against(
        &self,
        request: &OutcomeReportingAuditRequest,
        studies: &[RegisteredOutcomeStudy],
    ) -> Result<(), OutcomeReportingAuditError> {
        validate_request(request)?;
        validate_studies(request, studies)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.dispatch != "not_dispatched_audit_only"
            || &self.request != request
            || input_digest(request, studies)? != self.input_digest
        {
            return Err(OutcomeReportingAuditError::InvalidOutput(
                "identity, input binding, or analysis-only dispatch boundary is invalid".into(),
            ));
        }
        let expected = compile(request, studies)?;
        if self != &expected {
            return Err(OutcomeReportingAuditError::InvalidOutput(
                "reporting assessments, aggregate thresholds, or provenance do not derive from the retained registry and result reports".into(),
            ));
        }
        Ok(())
    }
}

pub fn audit_glioma_registered_outcome_reporting(
    request: &OutcomeReportingAuditRequest,
    studies: &[RegisteredOutcomeStudy],
) -> Result<RegisteredOutcomeReportingAudit, OutcomeReportingAuditError> {
    validate_request(request)?;
    validate_studies(request, studies)?;
    let output = compile(request, studies)?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn artifact(value: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: value.into(),
            content_hash: hash(value),
            content_type: "application/vnd.aurora.local-study-report+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> OutcomeReportingAuditRequest {
        let mut registry_report_order = vec![hash("registry-a"), hash("registry-b")];
        registry_report_order.sort();
        let mut result_report_order = vec![hash("result-a")];
        result_report_order.sort();
        OutcomeReportingAuditRequest {
            objective: "audit completeness of prospectively registered preclinical outcomes".into(),
            registry_report_order,
            result_report_order,
            evaluation_day: 900,
            max_reporting_lag_days: 90,
            min_studies: 2,
            min_independent_groups: 2,
            min_completed_studies: 1,
            min_primary_coverage_milli: 1_000,
            max_unreported_primary_groups: 0,
        }
    }

    fn study(id: &str, report: bool) -> RegisteredOutcomeStudy {
        let registry_report_digest = hash(&format!("registry-{id}"));
        let result_report = report.then(|| {
            let mut outcomes = vec![ReportedOutcome {
                outcome_id: "primary-viability".into(),
                status: ReportedOutcomeStatus::Reported,
            }];
            outcomes.sort_by(|left, right| left.outcome_id.cmp(&right.outcome_id));
            StudyOutcomeReport {
                report_digest: hash(&format!("result-{id}")),
                report_day: 850,
                artifact: artifact(&format!("report-{id}")),
                outcomes,
            }
        });
        RegisteredOutcomeStudy {
            study_id: id.into(),
            independence_group: format!("group-{id}"),
            registry_report_digest,
            protocol_artifact: artifact(&format!("protocol-{id}")),
            registration_day: Some(100),
            first_enrollment_day: Some(120),
            status: RegisteredStudyStatus::Completed,
            completion_day: Some(700),
            registered_outcomes: vec![
                RegisteredOutcome {
                    outcome_id: "primary-viability".into(),
                    estimand_id: "viability-change-at-24h".into(),
                    effect_unit: "normalized-signal-milli".into(),
                    role: RegisteredOutcomeRole::Primary,
                    outcome_window_days: 7,
                },
                RegisteredOutcome {
                    outcome_id: "secondary-migration".into(),
                    estimand_id: "migration-distance-at-24h".into(),
                    effect_unit: "micrometers-milli".into(),
                    role: RegisteredOutcomeRole::Secondary,
                    outcome_window_days: 7,
                },
            ],
            result_report,
        }
    }

    #[test]
    fn complete_reports_are_replayable_and_outcome_values_are_not_required() {
        let request = request();
        let studies = vec![study("a", true), study("b", true)];
        let mut request = request;
        request.result_report_order = studies
            .iter()
            .filter_map(|study| {
                study
                    .result_report
                    .as_ref()
                    .map(|report| report.report_digest.clone())
            })
            .collect();
        request.result_report_order.sort();
        let result = audit_glioma_registered_outcome_reporting(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeReportingDisposition::CoverageThresholdMet
        );
        assert_eq!(result.primary_coverage_milli, 1_000);
        assert_eq!(result.reported_primary_count, 2);
        assert_eq!(result.due_primary_count, 2);
        assert_eq!(result.dispatch, "not_dispatched_audit_only");
        result.validate_against(&request, &studies).unwrap();
        let replay = audit_glioma_registered_outcome_reporting(
            &request,
            &[studies[1].clone(), studies[0].clone()],
        )
        .unwrap();
        assert_eq!(result, replay);
    }

    #[test]
    fn overdue_missing_primary_outcome_is_not_silently_treated_as_negative() {
        let studies = vec![study("a", true), study("b", false)];
        let mut request = request();
        request.result_report_order = studies
            .iter()
            .filter_map(|study| {
                study
                    .result_report
                    .as_ref()
                    .map(|report| report.report_digest.clone())
            })
            .collect();
        request.result_report_order.sort();
        let result = audit_glioma_registered_outcome_reporting(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeReportingDisposition::MaterialReportingGap
        );
        assert_eq!(result.unreported_primary_group_order, vec!["group-b"]);
        assert!(
            result
                .uncertainty
                .iter()
                .any(|row| row.contains("absence_is_not_evidence"))
        );
        let missing = result
            .outcome_order
            .iter()
            .find(|row| row.study_id == "b" && row.outcome_id == "primary-viability")
            .unwrap();
        assert_eq!(missing.status, OutcomeAssessmentStatus::Missing);
        assert!(result.validate().is_ok());
    }

    #[test]
    fn retrospective_registration_and_unregistered_outcomes_are_separate_anomalies() {
        let mut studies = vec![study("a", true), study("b", true)];
        studies[0].registration_day = Some(130);
        if let Some(report) = &mut studies[1].result_report {
            report.outcomes.push(ReportedOutcome {
                outcome_id: "unregistered-survival".into(),
                status: ReportedOutcomeStatus::Reported,
            });
            report
                .outcomes
                .sort_by(|left, right| left.outcome_id.cmp(&right.outcome_id));
        }
        let mut request = request();
        request.result_report_order = studies
            .iter()
            .filter_map(|study| {
                study
                    .result_report
                    .as_ref()
                    .map(|report| report.report_digest.clone())
            })
            .collect();
        request.result_report_order.sort();
        let result = audit_glioma_registered_outcome_reporting(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeReportingDisposition::RegistrationAnomaly
        );
        assert_eq!(result.retrospective_registration_order, vec!["a"]);
        assert_eq!(result.unregistered_reported_outcome_order, vec!["b"]);
    }

    #[test]
    fn recent_completed_studies_are_not_penalized_before_reporting_lag() {
        let mut studies = vec![study("a", false), study("b", false)];
        studies[0].completion_day = Some(850);
        studies[1].completion_day = Some(850);
        let mut request = request();
        request.result_report_order.clear();
        request.evaluation_day = 900;
        let result = audit_glioma_registered_outcome_reporting(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeReportingDisposition::InsufficientFollowUp
        );
        assert_eq!(result.due_primary_count, 0);
        assert!(
            result
                .outcome_order
                .iter()
                .all(|row| row.status == OutcomeAssessmentStatus::NotDue)
        );
    }

    #[test]
    fn an_empty_registry_slice_is_a_valid_insufficient_evidence_audit() {
        let mut request = request();
        request.registry_report_order.clear();
        request.result_report_order.clear();
        let result = audit_glioma_registered_outcome_reporting(&request, &[]).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeReportingDisposition::InsufficientEvidence
        );
        assert_eq!(result.due_primary_count, 0);
        assert!(result.validate_against(&request, &[]).is_ok());
    }

    #[test]
    fn tampered_summary_and_nonlocal_artifacts_are_rejected() {
        let request = request();
        let mut studies = vec![study("a", true), study("b", true)];
        let mut request = request;
        request.result_report_order = studies
            .iter()
            .filter_map(|study| {
                study
                    .result_report
                    .as_ref()
                    .map(|report| report.report_digest.clone())
            })
            .collect();
        request.result_report_order.sort();
        let mut result = audit_glioma_registered_outcome_reporting(&request, &studies).unwrap();
        result.primary_coverage_milli = 999;
        result.digest = output_digest(&result).unwrap();
        assert!(matches!(
            result.validate(),
            Err(OutcomeReportingAuditError::InvalidOutput(_))
        ));
        studies[0].protocol_artifact.contains_human_data = true;
        assert!(matches!(
            audit_glioma_registered_outcome_reporting(&request, &studies),
            Err(OutcomeReportingAuditError::InvalidStudy(_))
        ));
    }

    #[test]
    fn outcome_inputs_reject_effect_values_instead_of_silently_ignoring_them() {
        let reported: Result<ReportedOutcome, _> = serde_json::from_value(serde_json::json!({
            "outcome_id": "primary-survival",
            "status": "reported",
            "effect_milli": 42
        }));
        assert!(reported.is_err());

        let registered: Result<RegisteredOutcome, _> = serde_json::from_value(serde_json::json!({
            "outcome_id": "primary-survival",
            "estimand_id": "survival-at-90d",
            "effect_unit": "days",
            "role": "primary",
            "outcome_window_days": 90,
            "p_value": 0.01
        }));
        assert!(registered.is_err());

        let artifact: Result<LocalArtifactRef, _> = serde_json::from_value(serde_json::json!({
            "artifact_id": "local-protocol",
            "content_hash": hash("protocol").as_str(),
            "content_type": "application/vnd.aurora.local-study-report+json",
            "local_only": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false,
            "effect_milli": 42
        }));
        assert!(artifact.is_err());
    }
}
