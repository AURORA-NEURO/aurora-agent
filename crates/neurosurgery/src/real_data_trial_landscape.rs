//! Compact, digest-bound ClinicalTrials.gov landscape metadata.
//!
//! This projection is intentionally descriptive. It counts registry rows and the aggregate
//! fields already present in a validated public snapshot; it does not rank trials, infer
//! eligibility, estimate efficacy/safety, or merge registry records into patient evidence.
//! Counts are explicitly marked partial when the caller's bounded query truncates the returned
//! rows, and missing registry fields become review reasons rather than guessed defaults.

use crate::{
    NeurosurgeryError, RealDataQuery, RealDataQueryHit, RealDataRecordKind, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_TRIAL_LANDSCAPE_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-trial-landscape/0.1";
const MAX_INTERVENTIONS: usize = 256;
const MAX_REVIEW_REASONS: usize = 32;

fn default_max_interventions() -> usize {
    128
}

/// Bounded query for a descriptive registry landscape. The nested query must select only
/// clinical trials when `record_kind` is provided; omitted `record_kind` is normalized to that
/// same trial-only scope during execution while remaining visible in the persisted query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataTrialLandscapeQuery {
    #[serde(default)]
    pub query: RealDataQuery,
    #[serde(default = "default_max_interventions")]
    pub max_interventions: usize,
}

impl Default for RealDataTrialLandscapeQuery {
    fn default() -> Self {
        Self {
            query: RealDataQuery::default(),
            max_interventions: default_max_interventions(),
        }
    }
}

/// A deterministic count bucket. Labels are copied registry metadata, not clinical categories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataTrialLandscapeCount {
    pub label: String,
    pub count: usize,
}

/// An intervention-name occurrence count copied from registry rows. It is not a recommendation,
/// treatment comparison, or efficacy signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataTrialLandscapeIntervention {
    pub name: String,
    pub count: usize,
}

/// One explicit metadata gap or boundedness condition for reviewer follow-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataTrialLandscapeReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Digest-bound registry reconnaissance over one validated local snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataTrialLandscapeReport {
    pub schema_version: String,
    pub landscape_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataTrialLandscapeQuery,
    pub total_matching_trials: usize,
    pub returned_trial_count: usize,
    pub omitted_trial_count: usize,
    pub truncated: bool,
    pub status_counts: Vec<RealDataTrialLandscapeCount>,
    pub phase_counts: Vec<RealDataTrialLandscapeCount>,
    /// Number of returned trials carrying one or more phase labels. A trial may contribute to
    /// multiple phase buckets, so this is kept separate from the bucket total.
    pub phase_annotated_trial_count: usize,
    pub study_type_counts: Vec<RealDataTrialLandscapeCount>,
    pub intervention_counts: Vec<RealDataTrialLandscapeIntervention>,
    pub distinct_intervention_count: usize,
    pub omitted_intervention_count: usize,
    pub intervention_truncated: bool,
    pub missing_phase_count: usize,
    pub missing_last_update_count: usize,
    pub missing_study_type_count: usize,
    pub missing_enrollment_count: usize,
    pub missing_intervention_count: usize,
    pub earliest_last_update: Option<String>,
    pub latest_last_update: Option<String>,
    pub source_ids: Vec<String>,
    pub review_reasons: Vec<RealDataTrialLandscapeReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataTrialLandscapeReport {
    /// Validate a persisted report without opening a source or performing network access.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_landscape_query(&self.query)?;
        if self.schema_version != REAL_DATA_TRIAL_LANDSCAPE_SCHEMA_VERSION
            || !is_sha256(&self.landscape_digest)
            || !is_sha256(&self.bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.omitted_trial_count
                != self
                    .total_matching_trials
                    .saturating_sub(self.returned_trial_count)
            || self.returned_trial_count > self.total_matching_trials
            || self.truncated != (self.omitted_trial_count > 0)
            || self.returned_trial_count > self.query.query.limit
            || self.intervention_counts.len() > self.query.max_interventions
            || self.distinct_intervention_count
                != self
                    .intervention_counts
                    .len()
                    .saturating_add(self.omitted_intervention_count)
            || self.intervention_truncated != (self.omitted_intervention_count > 0)
            || self.review_reasons.len() > MAX_REVIEW_REASONS
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(rejected("trial landscape envelope is invalid"));
        }
        if self
            .earliest_last_update
            .as_deref()
            .is_some_and(|value| !is_calendar_date(value))
            || self
                .latest_last_update
                .as_deref()
                .is_some_and(|value| !is_calendar_date(value))
            || self
                .earliest_last_update
                .as_ref()
                .zip(self.latest_last_update.as_ref())
                .is_some_and(|(earliest, latest)| earliest > latest)
            || !is_sorted_unique(&self.source_ids)
            || !canonical_counts(&self.status_counts)
            || !canonical_counts(&self.phase_counts)
            || !canonical_counts(&self.study_type_counts)
            || !canonical_interventions(&self.intervention_counts)
            || self
                .status_counts
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>()
                != self.returned_trial_count
            || self.phase_annotated_trial_count > self.returned_trial_count
            || self
                .missing_phase_count
                .saturating_add(self.phase_annotated_trial_count)
                != self.returned_trial_count
            || self
                .phase_counts
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>()
                > self.phase_annotated_trial_count.saturating_mul(16)
            || self.missing_study_type_count.saturating_add(
                self.study_type_counts
                    .iter()
                    .map(|bucket| bucket.count)
                    .sum::<usize>(),
            ) != self.returned_trial_count
            || self.missing_last_update_count > self.returned_trial_count
            || self.missing_enrollment_count > self.returned_trial_count
            || self.missing_intervention_count > self.returned_trial_count
        {
            return Err(rejected("trial landscape aggregation is invalid"));
        }
        let mut reason_keys = BTreeSet::new();
        for reason in &self.review_reasons {
            if reason.code.trim().is_empty()
                || reason.detail.trim().is_empty()
                || reason.count == 0
                || !reason_keys.insert(reason.code.as_str())
            {
                return Err(rejected("trial landscape review reasons are invalid"));
            }
        }
        if self.landscape_digest != digest_report(self)? {
            return Err(rejected(
                "trial landscape digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the landscape against the exact validated local snapshot.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.trial_landscape(&self.query)?;
        if &expected != self {
            return Err(rejected(
                "trial landscape does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Build a bounded descriptive registry landscape from the validated snapshot.
    pub fn trial_landscape(
        &self,
        query: &RealDataTrialLandscapeQuery,
    ) -> Result<RealDataTrialLandscapeReport, NeurosurgeryError> {
        validate_landscape_query(query)?;
        let mut trial_query = query.query.clone();
        trial_query.record_kind = Some(RealDataRecordKind::ClinicalTrial);
        let result = self.query(&trial_query)?;
        let mut status_counts = BTreeMap::new();
        let mut phase_counts = BTreeMap::new();
        let mut phase_annotated_trial_count = 0;
        let mut study_type_counts = BTreeMap::new();
        let mut intervention_counts = BTreeMap::new();
        let mut source_ids = BTreeSet::new();
        let mut missing_phase_count = 0;
        let mut missing_last_update_count = 0;
        let mut missing_study_type_count = 0;
        let mut missing_enrollment_count = 0;
        let mut missing_intervention_count = 0;
        let mut earliest_last_update: Option<String> = None;
        let mut latest_last_update: Option<String> = None;

        for hit in &result.hits {
            source_ids.insert(hit.source_id.clone());
            if let Some(status) = hit.status.as_deref() {
                increment(&mut status_counts, status);
            }
            if hit.phases.is_empty() {
                missing_phase_count += 1;
            } else {
                phase_annotated_trial_count += 1;
                for phase in &hit.phases {
                    increment(&mut phase_counts, phase);
                }
            }
            match hit.study_type.as_deref() {
                Some(study_type) => increment(&mut study_type_counts, study_type),
                None => missing_study_type_count += 1,
            }
            if let Some(last_update) = hit.last_update.as_deref() {
                earliest_last_update = Some(match earliest_last_update {
                    Some(current) if current.as_str() <= last_update => current,
                    _ => last_update.to_string(),
                });
                latest_last_update = Some(match latest_last_update {
                    Some(current) if current.as_str() >= last_update => current,
                    _ => last_update.to_string(),
                });
            } else {
                missing_last_update_count += 1;
            }
            if hit.enrollment_count.is_none() {
                missing_enrollment_count += 1;
            }
            if hit.intervention_names.is_empty() {
                missing_intervention_count += 1;
            } else {
                for intervention in &hit.intervention_names {
                    increment(&mut intervention_counts, intervention);
                }
            }
        }

        let distinct_intervention_count = intervention_counts.len();
        let omitted_intervention_count = distinct_intervention_count
            .saturating_sub(query.max_interventions.min(intervention_counts.len()));
        let intervention_counts = intervention_counts
            .into_iter()
            .take(query.max_interventions)
            .map(|(name, count)| RealDataTrialLandscapeIntervention { name, count })
            .collect::<Vec<_>>();
        let mut review_reasons = Vec::new();
        if result.truncated {
            review_reasons.push(RealDataTrialLandscapeReviewReason {
                code: "trial_rows_truncated".to_string(),
                count: result.total_matches.saturating_sub(result.returned_matches),
                detail: "the bounded registry query omitted matching trial rows; aggregate buckets describe returned rows only".to_string(),
            });
        }
        for (code, count, detail) in [
            (
                "missing_phase",
                missing_phase_count,
                "returned registry rows lack phase metadata",
            ),
            (
                "missing_last_update",
                missing_last_update_count,
                "returned registry rows lack last-update metadata",
            ),
            (
                "missing_study_type",
                missing_study_type_count,
                "returned registry rows lack study-design metadata",
            ),
            (
                "missing_enrollment",
                missing_enrollment_count,
                "returned registry rows lack aggregate enrollment metadata",
            ),
            (
                "missing_interventions",
                missing_intervention_count,
                "returned registry rows lack intervention metadata",
            ),
        ] {
            if count > 0 {
                review_reasons.push(RealDataTrialLandscapeReviewReason {
                    code: code.to_string(),
                    count,
                    detail: detail.to_string(),
                });
            }
        }
        if omitted_intervention_count > 0 {
            review_reasons.push(RealDataTrialLandscapeReviewReason {
                code: "interventions_truncated".to_string(),
                count: omitted_intervention_count,
                detail: "the distinct intervention-name inventory exceeded its explicit bound; omitted names require a larger reviewer-owned bound".to_string(),
            });
        }

        let mut report = RealDataTrialLandscapeReport {
            schema_version: REAL_DATA_TRIAL_LANDSCAPE_SCHEMA_VERSION.to_string(),
            landscape_digest: String::new(),
            bundle_digest: self.summary()?.bundle_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            total_matching_trials: result.total_matches,
            returned_trial_count: result.returned_matches,
            omitted_trial_count: result.total_matches.saturating_sub(result.returned_matches),
            truncated: result.truncated,
            status_counts: to_counts(status_counts),
            phase_counts: to_counts(phase_counts),
            phase_annotated_trial_count,
            study_type_counts: to_counts(study_type_counts),
            intervention_counts,
            distinct_intervention_count,
            omitted_intervention_count,
            intervention_truncated: omitted_intervention_count > 0,
            missing_phase_count,
            missing_last_update_count,
            missing_study_type_count,
            missing_enrollment_count,
            missing_intervention_count,
            earliest_last_update,
            latest_last_update,
            source_ids: source_ids.into_iter().collect(),
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the landscape counts only metadata rows already present in the caller-supplied validated snapshot".to_string(),
                "status, phase, design, enrollment, intervention, and update-date fields are descriptive registry metadata, not eligibility, quality, efficacy, safety, or outcome claims".to_string(),
                "bounded or missing fields remain explicit review obligations; no values are inferred and no source is fetched".to_string(),
                "population registry records remain separate from caller observations and cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.landscape_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_landscape_query(query: &RealDataTrialLandscapeQuery) -> Result<(), NeurosurgeryError> {
    crate::real_data::validate_query_shape(&query.query)?;
    if query
        .query
        .record_kind
        .is_some_and(|kind| kind != RealDataRecordKind::ClinicalTrial)
    {
        return Err(rejected(
            "trial landscape query record_kind must be clinical_trial",
        ));
    }
    if query.max_interventions == 0 || query.max_interventions > MAX_INTERVENTIONS {
        return Err(NeurosurgeryError::TooMany {
            field: "trial_landscape.max_interventions",
            found: query.max_interventions,
            max: MAX_INTERVENTIONS,
        });
    }
    Ok(())
}

fn increment(counts: &mut BTreeMap<String, usize>, label: &str) {
    *counts.entry(label.to_string()).or_default() += 1;
}

fn to_counts(counts: BTreeMap<String, usize>) -> Vec<RealDataTrialLandscapeCount> {
    counts
        .into_iter()
        .map(|(label, count)| RealDataTrialLandscapeCount { label, count })
        .collect()
}

fn canonical_counts(counts: &[RealDataTrialLandscapeCount]) -> bool {
    let mut previous = None;
    for bucket in counts {
        if bucket.label.trim().is_empty()
            || bucket.count == 0
            || previous.is_some_and(|previous: &str| previous >= bucket.label.as_str())
            || bucket.label.chars().any(char::is_control)
        {
            return false;
        }
        previous = Some(bucket.label.as_str());
    }
    true
}

fn canonical_interventions(counts: &[RealDataTrialLandscapeIntervention]) -> bool {
    let mut previous = None;
    counts.iter().all(|bucket| {
        let valid = !bucket.name.trim().is_empty()
            && bucket.count > 0
            && !bucket.name.chars().any(char::is_control)
            && previous.is_none_or(|previous: &str| previous < bucket.name.as_str());
        if valid {
            previous = Some(bucket.name.as_str());
        }
        valid
    })
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || ![0usize, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    day >= 1 && day <= days_in_month
}

fn digest_report(report: &RealDataTrialLandscapeReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.landscape_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

// Keep this import visibly tied to the report projection while avoiding a public re-export of
// query-hit internals. It also documents that the landscape is built from query-hit metadata.
#[allow(dead_code)]
fn _metadata_only(_hit: &RealDataQueryHit) {}
