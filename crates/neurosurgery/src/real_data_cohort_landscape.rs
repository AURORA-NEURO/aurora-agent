//! Digest-bound comparative inventory of public genomic cohorts.
//!
//! This projection makes the aggregate GDC metadata already present in a validated glioma
//! snapshot useful for research planning: it compares projects, released-case inventory, and
//! file-type availability without opening files or pretending that projects are interchangeable
//! cohorts. Every row is source-linked, bounded, and explicit about missing data-type metadata.

use crate::{
    GenomicProjectDataTypeCount, NeurosurgeryError, RealDataQuery, RealDataQueryHit,
    RealDataRecordKind, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_COHORT_LANDSCAPE_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-cohort-landscape/0.1";
const MAX_PROJECTS: usize = 128;
const MAX_REVIEW_REASONS: usize = 16;

fn default_max_projects() -> usize {
    32
}

/// Bounded query over aggregate genomic-project metadata already present in a validated bundle.
/// The nested query is normalized to `genomic_project` scope during execution while the caller's
/// original facets remain visible in the persisted report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCohortLandscapeQuery {
    #[serde(default)]
    pub query: RealDataQuery,
    #[serde(default = "default_max_projects")]
    pub max_projects: usize,
}

impl Default for RealDataCohortLandscapeQuery {
    fn default() -> Self {
        Self {
            query: RealDataQuery::default(),
            max_projects: default_max_projects(),
        }
    }
}

/// One public genomic project row. `case_count` is the source-reported released-case inventory;
/// it is not a patient-level value for a caller and cannot establish cohort comparability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCohortProjectRow {
    pub project_id: String,
    pub source_id: String,
    pub source_uri: String,
    pub name: String,
    pub primary_site: Vec<String>,
    pub disease_types: Vec<String>,
    pub case_count: usize,
    pub data_type_metadata_present: bool,
    pub data_type_counts: Vec<GenomicProjectDataTypeCount>,
    pub total_file_count: usize,
}

/// Aggregate availability for one data type across the returned project rows. This is a file
/// inventory, not a statement that every project contains equivalent assays or observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCohortDataTypeCoverage {
    pub data_type: String,
    pub project_count: usize,
    pub total_file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCohortLandscapeReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Digest-bound comparative catalogue of public genomic projects in one validated snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCohortLandscapeReport {
    pub schema_version: String,
    pub landscape_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataCohortLandscapeQuery,
    pub total_matching_projects: usize,
    pub returned_project_count: usize,
    pub omitted_project_count: usize,
    pub truncated: bool,
    pub project_rows: Vec<RealDataCohortProjectRow>,
    pub total_released_case_inventory: usize,
    pub data_type_coverage: Vec<RealDataCohortDataTypeCoverage>,
    pub shared_data_type_count: usize,
    pub shared_data_types: Vec<String>,
    pub projects_with_data_type_metadata: usize,
    pub projects_without_data_type_metadata: usize,
    pub source_ids: Vec<String>,
    pub review_reasons: Vec<RealDataCohortLandscapeReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataCohortLandscapeReport {
    /// Validate a persisted report without opening a source or performing network access.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        if self.schema_version != REAL_DATA_COHORT_LANDSCAPE_SCHEMA_VERSION
            || !is_sha256(&self.landscape_digest)
            || !is_sha256(&self.bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.returned_project_count != self.project_rows.len()
            || self.omitted_project_count
                != self
                    .total_matching_projects
                    .saturating_sub(self.returned_project_count)
            || self.returned_project_count > self.total_matching_projects
            || self.truncated != (self.omitted_project_count > 0)
            || self.returned_project_count > self.query.query.limit
            || self.returned_project_count > self.query.max_projects
            || self
                .projects_with_data_type_metadata
                .saturating_add(self.projects_without_data_type_metadata)
                != self.returned_project_count
            || self.shared_data_type_count != self.shared_data_types.len()
            || self.review_reasons.len() > MAX_REVIEW_REASONS
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(rejected("cohort landscape envelope is invalid"));
        }

        if self
            .project_rows
            .windows(2)
            .any(|window| window[0].project_id >= window[1].project_id)
            || self.project_rows.iter().any(|row| !valid_project_row(row))
            || self
                .data_type_coverage
                .windows(2)
                .any(|window| window[0].data_type >= window[1].data_type)
            || self.data_type_coverage.iter().any(|row| {
                row.data_type.trim().is_empty()
                    || row.data_type.chars().any(char::is_control)
                    || row.project_count == 0
                    || row.project_count > self.returned_project_count
                    || row.total_file_count == 0
            })
            || self
                .shared_data_types
                .windows(2)
                .any(|window| window[0] >= window[1])
            || self.shared_data_types.iter().any(|data_type| {
                data_type.trim().is_empty() || data_type.chars().any(char::is_control)
            })
            || self.total_released_case_inventory
                != self
                    .project_rows
                    .iter()
                    .map(|row| row.case_count)
                    .sum::<usize>()
            || self.projects_with_data_type_metadata
                != self
                    .project_rows
                    .iter()
                    .filter(|row| row.data_type_metadata_present)
                    .count()
            || self.shared_data_types.iter().any(|data_type| {
                self.data_type_coverage
                    .iter()
                    .find(|row| row.data_type == *data_type)
                    .is_none_or(|row| row.project_count != self.returned_project_count)
            })
            || !is_sorted_unique(&self.source_ids)
        {
            return Err(rejected("cohort landscape aggregation is invalid"));
        }

        let mut reason_codes = BTreeSet::new();
        for reason in &self.review_reasons {
            if reason.code.trim().is_empty()
                || reason.detail.trim().is_empty()
                || reason.count == 0
                || !reason_codes.insert(reason.code.as_str())
            {
                return Err(rejected("cohort landscape review reasons are invalid"));
            }
        }
        for (code, expected) in [
            (
                "project_rows_truncated",
                self.truncated.then_some(self.omitted_project_count),
            ),
            (
                "missing_data_type_metadata",
                (self.projects_without_data_type_metadata > 0)
                    .then_some(self.projects_without_data_type_metadata),
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
                        "cohort landscape review reasons omit an explicit gap",
                    ));
                }
            }
        }
        if self.landscape_digest != digest_report(self)? {
            return Err(rejected(
                "cohort landscape digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the landscape against the exact validated snapshot and query bounds.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.cohort_landscape(&self.query)?;
        if &expected != self {
            return Err(rejected(
                "cohort landscape does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Build a bounded comparative inventory from local aggregate genomic metadata only.
    pub fn cohort_landscape(
        &self,
        query: &RealDataCohortLandscapeQuery,
    ) -> Result<RealDataCohortLandscapeReport, NeurosurgeryError> {
        validate_query(query)?;
        let mut project_query = query.query.clone();
        project_query.record_kind = Some(RealDataRecordKind::GenomicProject);
        let result = self.query(&project_query)?;
        let project_by_id = self
            .genomic_projects
            .iter()
            .map(|project| (project.project_id.as_str(), project))
            .collect::<BTreeMap<_, _>>();
        let mut rows = Vec::with_capacity(result.hits.len().min(query.max_projects));
        for hit in result.hits.iter().take(query.max_projects) {
            let project = project_by_id.get(hit.record_id.as_str()).ok_or_else(|| {
                rejected("genomic-project query returned a project absent from the bundle")
            })?;
            let mut data_type_counts = hit.genomic_data_type_counts.clone();
            data_type_counts.sort_by(|left, right| left.data_type.cmp(&right.data_type));
            let total_file_count = data_type_counts.iter().map(|facet| facet.file_count).sum();
            rows.push(RealDataCohortProjectRow {
                project_id: project.project_id.clone(),
                source_id: project.source_id.clone(),
                source_uri: hit.source_uri.clone(),
                name: project.name.clone(),
                primary_site: project.primary_site.clone(),
                disease_types: project.disease_types.clone(),
                case_count: project.case_count,
                data_type_metadata_present: !data_type_counts.is_empty(),
                data_type_counts,
                total_file_count,
            });
        }
        rows.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        let returned_project_count = rows.len();
        let total_matching_projects = result.total_matches;
        let omitted_project_count = total_matching_projects.saturating_sub(returned_project_count);

        let mut data_types = BTreeMap::<String, (usize, usize)>::new();
        for row in &rows {
            for facet in &row.data_type_counts {
                let entry = data_types.entry(facet.data_type.clone()).or_default();
                entry.0 += 1;
                entry.1 = entry.1.saturating_add(facet.file_count);
            }
        }
        let data_type_coverage = data_types
            .into_iter()
            .map(
                |(data_type, (project_count, total_file_count))| RealDataCohortDataTypeCoverage {
                    data_type,
                    project_count,
                    total_file_count,
                },
            )
            .collect::<Vec<_>>();
        let shared_data_types = data_type_coverage
            .iter()
            .filter(|row| row.project_count == returned_project_count && returned_project_count > 0)
            .map(|row| row.data_type.clone())
            .collect::<Vec<_>>();
        let projects_with_data_type_metadata = rows
            .iter()
            .filter(|row| row.data_type_metadata_present)
            .count();
        let projects_without_data_type_metadata =
            returned_project_count.saturating_sub(projects_with_data_type_metadata);
        let source_ids = rows
            .iter()
            .map(|row| row.source_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut review_reasons = Vec::new();
        if omitted_project_count > 0 {
            review_reasons.push(RealDataCohortLandscapeReviewReason {
                code: "project_rows_truncated".to_string(),
                count: omitted_project_count,
                detail: "the bounded genomic-project query omitted matching project rows; aggregates describe returned projects only".to_string(),
            });
        }
        if projects_without_data_type_metadata > 0 {
            review_reasons.push(RealDataCohortLandscapeReviewReason {
                code: "missing_data_type_metadata".to_string(),
                count: projects_without_data_type_metadata,
                detail: "returned genomic projects lack aggregate GDC data-type facets; assay/file availability is unknown for those projects".to_string(),
            });
        }
        let mut report = RealDataCohortLandscapeReport {
            schema_version: REAL_DATA_COHORT_LANDSCAPE_SCHEMA_VERSION.to_string(),
            landscape_digest: String::new(),
            bundle_digest: self.summary()?.bundle_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            total_matching_projects,
            returned_project_count,
            omitted_project_count,
            truncated: omitted_project_count > 0,
            project_rows: rows,
            total_released_case_inventory: 0,
            data_type_coverage,
            shared_data_type_count: shared_data_types.len(),
            shared_data_types,
            projects_with_data_type_metadata,
            projects_without_data_type_metadata,
            source_ids,
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "case_count is an aggregate released-case inventory copied from public GDC metadata, not a patient-level count or a denominator for clinical inference".to_string(),
                "data-type and file counts describe public availability metadata; they do not expose files, samples, molecular values, assay quality, or cohort comparability".to_string(),
                "shared data types are shared only across returned projects with observed metadata; missing facets remain unknown and are never treated as absence".to_string(),
                "the report never fetches URLs, invokes a model, opens credentials or patient files, merges cohorts, ranks projects, or emits diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.total_released_case_inventory =
            report.project_rows.iter().map(|row| row.case_count).sum();
        report.landscape_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &RealDataCohortLandscapeQuery) -> Result<(), NeurosurgeryError> {
    crate::real_data::validate_query_shape(&query.query)?;
    if query
        .query
        .record_kind
        .is_some_and(|kind| kind != RealDataRecordKind::GenomicProject)
    {
        return Err(rejected(
            "cohort landscape query record_kind must be genomic_project",
        ));
    }
    if query.max_projects == 0 || query.max_projects > MAX_PROJECTS {
        return Err(NeurosurgeryError::TooMany {
            field: "cohort_landscape.max_projects",
            found: query.max_projects,
            max: MAX_PROJECTS,
        });
    }
    Ok(())
}

fn valid_project_row(row: &RealDataCohortProjectRow) -> bool {
    !row.project_id.trim().is_empty()
        && !row.source_id.trim().is_empty()
        && crate::real_data::is_allow_listed_uri(&row.source_uri)
        && !row.name.trim().is_empty()
        && !row.primary_site.is_empty()
        && !row.disease_types.is_empty()
        && row.case_count > 0
        && row.data_type_metadata_present == !row.data_type_counts.is_empty()
        && row
            .data_type_counts
            .windows(2)
            .all(|window| window[0].data_type < window[1].data_type)
        && row.data_type_counts.iter().all(|facet| {
            !facet.data_type.trim().is_empty()
                && !facet.data_type.chars().any(char::is_control)
                && facet.file_count > 0
        })
        && row.total_file_count
            == row
                .data_type_counts
                .iter()
                .map(|facet| facet.file_count)
                .sum::<usize>()
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn digest_report(report: &RealDataCohortLandscapeReport) -> Result<String, NeurosurgeryError> {
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

// Keep query-hit provenance visible in the implementation contract without exposing internals.
#[allow(dead_code)]
fn _metadata_only(_hit: &RealDataQueryHit) {}
