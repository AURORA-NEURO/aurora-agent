//! Digest-bound inventory of public cBioPortal molecular-assay and GDC availability metadata.
//!
//! This projection answers a narrow but important handoff question: which studies and assay
//! representations are actually present in the caller-supplied snapshot? It never exposes
//! mutation calls, expression values, methylation values, sample identifiers, or patient-level
//! observations. Missing descriptions and bounded rows remain explicit review obligations.

use crate::{
    NeurosurgeryError, RealDataQuery, RealDataQueryHit, RealDataRecordKind,
    RealGenomicProjectDataTypeCount, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_MOLECULAR_COVERAGE_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-molecular-coverage/0.1";
const MAX_STUDY_ROWS: usize = 256;
const MAX_REVIEW_REASONS: usize = 32;
const MAX_GENOMIC_FACET_ROWS: usize = 4096;

fn default_max_studies() -> usize {
    128
}

/// Bounded query over cBioPortal assay/profile metadata already present in a validated bundle.
/// The nested query is normalized to `portal_molecular_profile` scope during execution while the
/// caller's original bounds remain visible in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataMolecularCoverageQuery {
    #[serde(default)]
    pub query: RealDataQuery,
    #[serde(default = "default_max_studies")]
    pub max_studies: usize,
}

impl Default for RealDataMolecularCoverageQuery {
    fn default() -> Self {
        Self {
            query: RealDataQuery::default(),
            max_studies: default_max_studies(),
        }
    }
}

/// Deterministic count bucket for an assay alteration type or datatype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataMolecularCoverageCount {
    pub label: String,
    pub count: usize,
}

/// Per-study inventory over returned public profile metadata. Counts describe available profile
/// rows only; they do not establish that an assay was run for a caller's specimen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataMolecularStudyCoverage {
    pub study_id: String,
    pub profile_count: usize,
    pub patient_level_profile_count: usize,
    pub analysis_visible_profile_count: usize,
    pub description_present_count: usize,
    #[serde(default)]
    pub missing_alteration_type_count: usize,
    #[serde(default)]
    pub missing_datatype_count: usize,
    pub alteration_type_counts: Vec<RealDataMolecularCoverageCount>,
    pub datatype_counts: Vec<RealDataMolecularCoverageCount>,
}

/// One explicit metadata gap or boundedness condition for reviewer follow-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataMolecularCoverageReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Replayable public molecular-assay and genomic-file availability ledger. It is intentionally
/// metadata-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataMolecularCoverageReport {
    pub schema_version: String,
    pub coverage_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataMolecularCoverageQuery,
    pub total_matching_profile_count: usize,
    pub returned_profile_count: usize,
    pub omitted_profile_count: usize,
    pub truncated: bool,
    pub distinct_returned_study_count: usize,
    pub emitted_study_count: usize,
    pub omitted_study_count: usize,
    pub study_rows_truncated: bool,
    pub emitted_profile_count: usize,
    pub study_rows: Vec<RealDataMolecularStudyCoverage>,
    pub alteration_type_counts: Vec<RealDataMolecularCoverageCount>,
    pub datatype_counts: Vec<RealDataMolecularCoverageCount>,
    pub patient_level_profile_count: usize,
    pub analysis_visible_profile_count: usize,
    pub description_present_count: usize,
    pub missing_description_count: usize,
    #[serde(default)]
    pub missing_alteration_type_count: usize,
    #[serde(default)]
    pub missing_datatype_count: usize,
    #[serde(default)]
    pub missing_study_link_count: usize,
    /// Number of genomic projects represented by the validated snapshot. This is catalogue
    /// coverage metadata, not a patient or cohort count.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub genomic_project_count: usize,
    /// Sum of aggregate GDC file counts across all project/data-type facets in the snapshot.
    /// This never exposes file contents, sample identifiers, or patient-level values.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub genomic_project_file_count: usize,
    /// Aggregate GDC data-type facets copied from the snapshot and sorted by project/data type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genomic_project_data_type_counts: Vec<RealGenomicProjectDataTypeCount>,
    pub source_ids: Vec<String>,
    pub review_reasons: Vec<RealDataMolecularCoverageReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataMolecularCoverageReport {
    /// Validate a persisted inventory without opening a source or performing network access.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        if self.schema_version != REAL_DATA_MOLECULAR_COVERAGE_SCHEMA_VERSION
            || !is_sha256(&self.coverage_digest)
            || !is_sha256(&self.bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.omitted_profile_count
                != self
                    .total_matching_profile_count
                    .saturating_sub(self.returned_profile_count)
            || self.returned_profile_count > self.total_matching_profile_count
            || self.truncated != (self.omitted_profile_count > 0)
            || self.returned_profile_count > self.query.query.limit
            || self.emitted_study_count != self.study_rows.len()
            || self.omitted_study_count
                != self
                    .distinct_returned_study_count
                    .saturating_sub(self.emitted_study_count)
            || self.study_rows_truncated != (self.omitted_study_count > 0)
            || self.emitted_study_count > self.query.max_studies
            || self.emitted_profile_count > self.returned_profile_count
            || self
                .study_rows
                .iter()
                .map(|row| row.profile_count)
                .sum::<usize>()
                != self.emitted_profile_count
            || self
                .alteration_type_counts
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>()
                .saturating_add(self.missing_alteration_type_count)
                != self.emitted_profile_count
            || self
                .datatype_counts
                .iter()
                .map(|bucket| bucket.count)
                .sum::<usize>()
                .saturating_add(self.missing_datatype_count)
                != self.emitted_profile_count
            || self.patient_level_profile_count > self.emitted_profile_count
            || self.analysis_visible_profile_count > self.emitted_profile_count
            || self.description_present_count > self.emitted_profile_count
            || self.missing_alteration_type_count > self.emitted_profile_count
            || self.missing_datatype_count > self.emitted_profile_count
            || self
                .missing_description_count
                .saturating_add(self.description_present_count)
                != self.emitted_profile_count
            || self.missing_study_link_count > self.returned_profile_count
            || self
                .emitted_profile_count
                .saturating_add(self.missing_study_link_count)
                > self.returned_profile_count
            || self.review_reasons.len() > MAX_REVIEW_REASONS
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
            || !is_sorted_unique(&self.source_ids)
            || !canonical_counts(&self.alteration_type_counts)
            || !canonical_counts(&self.datatype_counts)
            || self.genomic_project_data_type_counts.len() > MAX_GENOMIC_FACET_ROWS
            || self
                .genomic_project_data_type_counts
                .iter()
                .fold(0usize, |total, row| total.saturating_add(row.file_count))
                != self.genomic_project_file_count
            || !self
                .genomic_project_data_type_counts
                .windows(2)
                .all(|window| {
                    (window[0].project_id.as_str(), window[0].data_type.as_str())
                        < (window[1].project_id.as_str(), window[1].data_type.as_str())
                })
            || self.genomic_project_data_type_counts.iter().any(|row| {
                row.project_id.trim().is_empty()
                    || row.data_type.trim().is_empty()
                    || row.file_count == 0
            })
            || self
                .genomic_project_data_type_counts
                .iter()
                .map(|row| row.project_id.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                > self.genomic_project_count
        {
            return Err(rejected("molecular coverage envelope is invalid"));
        }
        let mut study_ids = BTreeSet::new();
        for row in &self.study_rows {
            if row.study_id.trim().is_empty()
                || row.profile_count == 0
                || row.patient_level_profile_count > row.profile_count
                || row.analysis_visible_profile_count > row.profile_count
                || row.description_present_count > row.profile_count
                || row.missing_alteration_type_count > row.profile_count
                || row.missing_datatype_count > row.profile_count
                || !study_ids.insert(row.study_id.as_str())
                || !canonical_counts(&row.alteration_type_counts)
                || !canonical_counts(&row.datatype_counts)
                || row
                    .alteration_type_counts
                    .iter()
                    .map(|bucket| bucket.count)
                    .sum::<usize>()
                    .saturating_add(row.missing_alteration_type_count)
                    != row.profile_count
                || row
                    .datatype_counts
                    .iter()
                    .map(|bucket| bucket.count)
                    .sum::<usize>()
                    .saturating_add(row.missing_datatype_count)
                    != row.profile_count
            {
                return Err(rejected("molecular coverage study rows are invalid"));
            }
        }
        let mut reason_codes = BTreeSet::new();
        for reason in &self.review_reasons {
            if reason.code.trim().is_empty()
                || reason.count == 0
                || reason.detail.trim().is_empty()
                || !reason_codes.insert(reason.code.as_str())
            {
                return Err(rejected("molecular coverage review reasons are invalid"));
            }
        }
        for (code, expected) in [
            (
                "profile_rows_truncated",
                self.truncated.then_some(self.omitted_profile_count),
            ),
            (
                "study_rows_truncated",
                self.study_rows_truncated
                    .then_some(self.omitted_study_count),
            ),
            (
                "missing_profile_description",
                (self.missing_description_count > 0).then_some(self.missing_description_count),
            ),
            (
                "missing_alteration_type",
                (self.missing_alteration_type_count > 0)
                    .then_some(self.missing_alteration_type_count),
            ),
            (
                "missing_datatype",
                (self.missing_datatype_count > 0).then_some(self.missing_datatype_count),
            ),
            (
                "missing_study_link",
                (self.missing_study_link_count > 0).then_some(self.missing_study_link_count),
            ),
            (
                "missing_gdc_data_type_facets",
                (self.genomic_project_count > 0
                    && self
                        .genomic_project_data_type_counts
                        .iter()
                        .map(|row| row.project_id.as_str())
                        .collect::<BTreeSet<_>>()
                        .len()
                        < self.genomic_project_count)
                    .then_some(
                        self.genomic_project_count.saturating_sub(
                            self.genomic_project_data_type_counts
                                .iter()
                                .map(|row| row.project_id.as_str())
                                .collect::<BTreeSet<_>>()
                                .len(),
                        ),
                    ),
            ),
        ] {
            if let Some(expected) = expected {
                if self
                    .review_reasons
                    .iter()
                    .find(|reason| reason.code == code)
                    .map(|reason| reason.count)
                    != Some(expected)
                {
                    return Err(rejected(
                        "molecular coverage review reasons do not account for explicit gaps",
                    ));
                }
            }
        }
        if self.coverage_digest != digest_report(self)? {
            return Err(rejected(
                "molecular coverage digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the inventory against the exact validated snapshot and query bounds.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.molecular_coverage(&self.query)?;
        if &expected != self {
            return Err(rejected(
                "molecular coverage does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Build a bounded molecular-assay availability ledger from local public metadata only.
    pub fn molecular_coverage(
        &self,
        query: &RealDataMolecularCoverageQuery,
    ) -> Result<RealDataMolecularCoverageReport, NeurosurgeryError> {
        validate_query(query)?;
        let mut profile_query = query.query.clone();
        profile_query.record_kind = Some(RealDataRecordKind::PortalMolecularProfile);
        let result = self.query(&profile_query)?;
        let mut by_study = BTreeMap::<String, Vec<&RealDataQueryHit>>::new();
        let mut source_ids = BTreeSet::new();
        let mut missing_study_link_count = 0;
        for hit in &result.hits {
            source_ids.insert(hit.source_id.clone());
            let Some(study_id) = hit
                .related_records
                .iter()
                .find(|related| related.relation == crate::RealDataRelation::ProfileOfStudy)
                .map(|related| related.record_id.clone())
            else {
                missing_study_link_count += 1;
                continue;
            };
            by_study.entry(study_id).or_default().push(hit);
        }
        let distinct_returned_study_count = by_study.len();
        let emitted_study_count = distinct_returned_study_count.min(query.max_studies);
        let omitted_study_count = distinct_returned_study_count.saturating_sub(emitted_study_count);
        let mut study_rows = Vec::with_capacity(emitted_study_count);
        for (study_id, hits) in by_study.into_iter().take(query.max_studies) {
            study_rows.push(study_row(study_id, &hits));
        }
        let emitted_profile_count: usize = study_rows.iter().map(|row| row.profile_count).sum();
        let mut alteration_type_counts = BTreeMap::new();
        let mut datatype_counts = BTreeMap::new();
        let mut patient_level_profile_count = 0;
        let mut analysis_visible_profile_count = 0;
        let mut description_present_count = 0;
        let mut missing_alteration_type_count = 0;
        let mut missing_datatype_count = 0;
        for row in &study_rows {
            merge_counts(&mut alteration_type_counts, &row.alteration_type_counts);
            merge_counts(&mut datatype_counts, &row.datatype_counts);
            patient_level_profile_count += row.patient_level_profile_count;
            analysis_visible_profile_count += row.analysis_visible_profile_count;
            description_present_count += row.description_present_count;
            missing_alteration_type_count += row.missing_alteration_type_count;
            missing_datatype_count += row.missing_datatype_count;
        }
        let missing_description_count =
            emitted_profile_count.saturating_sub(description_present_count);
        let genomic_project_count = self.genomic_projects.len();
        let mut genomic_project_data_type_counts = self
            .genomic_projects
            .iter()
            .flat_map(|project| {
                source_ids.insert(project.source_id.clone());
                project
                    .data_type_counts
                    .iter()
                    .map(|facet| RealGenomicProjectDataTypeCount {
                        project_id: project.project_id.clone(),
                        data_type: facet.data_type.clone(),
                        file_count: facet.file_count,
                    })
            })
            .collect::<Vec<_>>();
        genomic_project_data_type_counts.sort_by(|left, right| {
            left.project_id
                .cmp(&right.project_id)
                .then_with(|| left.data_type.cmp(&right.data_type))
        });
        let genomic_project_file_count = genomic_project_data_type_counts
            .iter()
            .fold(0usize, |total, row| total.saturating_add(row.file_count));
        let projects_with_facets = genomic_project_data_type_counts
            .iter()
            .map(|row| row.project_id.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let missing_gdc_data_type_facets =
            genomic_project_count.saturating_sub(projects_with_facets);
        let mut review_reasons = Vec::new();
        if result.truncated {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "profile_rows_truncated".to_string(),
                count: result.total_matches.saturating_sub(result.returned_matches),
                detail: "the bounded molecular-profile query omitted matching rows; aggregate buckets describe returned rows only".to_string(),
            });
        }
        if omitted_study_count > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "study_rows_truncated".to_string(),
                count: omitted_study_count,
                detail: "the distinct-study inventory exceeded its explicit bound; omitted studies require a larger reviewer-owned bound".to_string(),
            });
        }
        if missing_description_count > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "missing_profile_description".to_string(),
                count: missing_description_count,
                detail: "returned public profile rows lack an optional description; assay semantics require reviewer inspection".to_string(),
            });
        }
        if missing_alteration_type_count > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "missing_alteration_type".to_string(),
                count: missing_alteration_type_count,
                detail: "returned public profile rows lack an alteration-type label; modality semantics require reviewer inspection".to_string(),
            });
        }
        if missing_datatype_count > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "missing_datatype".to_string(),
                count: missing_datatype_count,
                detail: "returned public profile rows lack a datatype label; modality semantics require reviewer inspection".to_string(),
            });
        }
        if missing_study_link_count > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "missing_study_link".to_string(),
                count: missing_study_link_count,
                detail: "returned public profile rows lack an explicit study crosswalk and are excluded from per-study aggregation".to_string(),
            });
        }
        if missing_gdc_data_type_facets > 0 {
            review_reasons.push(RealDataMolecularCoverageReviewReason {
                code: "missing_gdc_data_type_facets".to_string(),
                count: missing_gdc_data_type_facets,
                detail: "one or more genomic projects lack aggregate GDC data-type facets in the supplied snapshot; modality availability remains incomplete and requires reviewer follow-up".to_string(),
            });
        }
        let mut report = RealDataMolecularCoverageReport {
            schema_version: REAL_DATA_MOLECULAR_COVERAGE_SCHEMA_VERSION.to_string(),
            coverage_digest: String::new(),
            bundle_digest: self.summary()?.bundle_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            total_matching_profile_count: result.total_matches,
            returned_profile_count: result.returned_matches,
            omitted_profile_count: result.total_matches.saturating_sub(result.returned_matches),
            truncated: result.truncated,
            distinct_returned_study_count,
            emitted_study_count,
            omitted_study_count,
            study_rows_truncated: omitted_study_count > 0,
            emitted_profile_count,
            study_rows,
            alteration_type_counts: to_counts(alteration_type_counts),
            datatype_counts: to_counts(datatype_counts),
            patient_level_profile_count,
            analysis_visible_profile_count,
            description_present_count,
            missing_description_count,
            missing_alteration_type_count,
            missing_datatype_count,
            missing_study_link_count,
            genomic_project_count,
            genomic_project_file_count,
            genomic_project_data_type_counts,
            source_ids: source_ids.into_iter().collect(),
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the ledger inventories only cBioPortal profile metadata already present in the caller-supplied validated snapshot".to_string(),
                "aggregate GDC data-type facets are availability metadata only; they do not expose file contents, sample identifiers, molecular values, or patient-level observations".to_string(),
                "profile, datatype, patient-level, analysis-visible, and description counts are availability metadata, not molecular values or evidence of an assay result for a specimen".to_string(),
                "bounded rows and missing descriptions remain explicit review obligations; no assay meaning, cohort identity, diagnosis, prognosis, treatment, or operative action is inferred".to_string(),
                "the ledger never fetches URLs, invokes a provider, opens credentials, exposes sample identifiers, or performs an external effect".to_string(),
            ],
        };
        report.coverage_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &RealDataMolecularCoverageQuery) -> Result<(), NeurosurgeryError> {
    crate::real_data::validate_query_shape(&query.query)?;
    if query
        .query
        .record_kind
        .is_some_and(|kind| kind != RealDataRecordKind::PortalMolecularProfile)
    {
        return Err(rejected(
            "molecular coverage query record_kind must be portal_molecular_profile",
        ));
    }
    if query.query.genomic_data_type.is_some() {
        return Err(rejected(
            "molecular coverage query cannot carry genomic_data_type; query GDC facets with real_data_query",
        ));
    }
    if query.max_studies == 0 || query.max_studies > MAX_STUDY_ROWS {
        return Err(NeurosurgeryError::TooMany {
            field: "molecular_coverage.max_studies",
            found: query.max_studies,
            max: MAX_STUDY_ROWS,
        });
    }
    Ok(())
}

fn study_row(study_id: String, hits: &[&RealDataQueryHit]) -> RealDataMolecularStudyCoverage {
    let mut alteration_type_counts = BTreeMap::new();
    let mut datatype_counts = BTreeMap::new();
    let mut patient_level_profile_count = 0;
    let mut analysis_visible_profile_count = 0;
    let mut description_present_count = 0;
    let mut missing_alteration_type_count = 0;
    let mut missing_datatype_count = 0;
    for hit in hits {
        if let Some(alteration_type) = hit.molecular_alteration_type.as_deref() {
            increment(&mut alteration_type_counts, alteration_type);
        } else {
            missing_alteration_type_count += 1;
        }
        if let Some(datatype) = hit.datatype.as_deref() {
            increment(&mut datatype_counts, datatype);
        } else {
            missing_datatype_count += 1;
        }
        if hit.molecular_patient_level == Some(true) {
            patient_level_profile_count += 1;
        }
        if hit.molecular_show_in_analysis == Some(true) {
            analysis_visible_profile_count += 1;
        }
        if hit.molecular_description.is_some() {
            description_present_count += 1;
        }
    }
    RealDataMolecularStudyCoverage {
        study_id,
        profile_count: hits.len(),
        patient_level_profile_count,
        analysis_visible_profile_count,
        description_present_count,
        missing_alteration_type_count,
        missing_datatype_count,
        alteration_type_counts: to_counts(alteration_type_counts),
        datatype_counts: to_counts(datatype_counts),
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, label: &str) {
    *counts.entry(label.to_ascii_uppercase()).or_default() += 1;
}

fn merge_counts(target: &mut BTreeMap<String, usize>, source: &[RealDataMolecularCoverageCount]) {
    for bucket in source {
        *target.entry(bucket.label.clone()).or_default() += bucket.count;
    }
}

fn to_counts(counts: BTreeMap<String, usize>) -> Vec<RealDataMolecularCoverageCount> {
    counts
        .into_iter()
        .map(|(label, count)| RealDataMolecularCoverageCount { label, count })
        .collect()
}

fn canonical_counts(counts: &[RealDataMolecularCoverageCount]) -> bool {
    let mut previous = None;
    counts.iter().all(|bucket| {
        let valid = !bucket.label.trim().is_empty()
            && bucket.count > 0
            && !bucket.label.chars().any(char::is_control)
            && previous.is_none_or(|previous: &str| previous < bucket.label.as_str());
        if valid {
            previous = Some(bucket.label.as_str());
        }
        valid
    })
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn digest_report(report: &RealDataMolecularCoverageReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.coverage_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}
