//! Provenance-bound public glioma data.
//!
//! This module deliberately stores compact, de-identified records rather than pretending that
//! a checked-in fixture is a live medical knowledge base. A bundle is accepted only when every
//! record points at an allow-listed public authority and the per-source SHA-256 digest matches
//! the canonical records embedded in the bundle. Refreshing a bundle is an explicit, auditable
//! ingestion step outside the Rust core; the core never performs network access. Registry rows
//! may carry aggregate study-design metadata (type, enrollment target, interventions) when the
//! public endpoint supplied it; missing fields remain optional and are never inferred.

use crate::{EvidenceRecord, EvidenceTier, NeurosurgeryError, RealDataSummary, ToolCapability};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the real-data bundle contract (separate from the case request schema).
pub const REAL_DATA_SCHEMA_VERSION: &str = "bioprism-neurosurgery-real/0.1";
const MAX_REAL_SOURCES: usize = 32;
const MAX_REAL_RECORDS: usize = 4096;
const MAX_QUERY_TEXT_BYTES: usize = 512;
const MAX_QUERY_HITS: usize = 128;
pub(crate) const MAX_QUERY_HITS_PUBLIC: usize = MAX_QUERY_HITS;
const MAX_ABSTRACT_BYTES: usize = 12_000;
const MAX_ABSTRACT_EXCERPT_CHARS: usize = 4_000;
const MAX_LITERATURE_TAGS: usize = 64;
const MAX_TRIAL_ENROLLMENT: usize = 10_000_000;
const MAX_TRIAL_INTERVENTIONS: usize = 128;
const MAX_GENOMIC_DATA_TYPES: usize = 256;
const MAX_GENOMIC_FILES_PER_TYPE: usize = 100_000_000;
const MAX_GENOMIC_DATA_TYPE_ROWS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealSourceKind {
    ClinicalTrialsRegistry,
    GenomicCommons,
    StudyPortal,
    Guideline,
    LiteratureIndex,
}

/// Public source metadata. `content_sha256` is computed over the canonical records for this
/// source, not over a mutable URL response, so an offline reviewer can reproduce the check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataSource {
    pub source_id: String,
    pub kind: RealSourceKind,
    pub authority: String,
    pub uri: String,
    pub retrieved_at: String,
    pub content_sha256: String,
    pub record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClinicalTrialRecord {
    pub source_id: String,
    pub nct_id: String,
    pub title: String,
    pub overall_status: String,
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(default)]
    pub last_update: Option<String>,
    /// Registry study design metadata copied from ClinicalTrials.gov. It is optional because
    /// older snapshots may not have retrieved the field; absence stays explicit and is never
    /// converted into a study type or an eligibility conclusion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_type: Option<String>,
    /// Aggregate enrollment target reported by the registry, not an enrolled-patient count and
    /// never a case-level or eligibility signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_count: Option<usize>,
    /// Intervention names as reported by the public registry. Names are source metadata only;
    /// this agent does not rank, recommend, or infer treatment from them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intervention_names: Vec<String>,
}

/// Deterministic count of registry statuses in the caller-supplied public snapshot. This is a
/// descriptive registry measure, not a trial-quality, eligibility, or clinical-outcome score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealTrialStatusCount {
    pub status: String,
    pub count: usize,
}

/// Aggregate released-case count for one public genomic project. This is a provenance and
/// coverage inventory only; it is never a patient-level count or an eligibility signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealGenomicProjectCaseCount {
    pub project_id: String,
    pub case_count: usize,
}

/// Aggregate public GDC file-count facet for one project/data type. This is an availability
/// inventory only; it never carries file contents, sample identifiers, or patient-level values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealGenomicProjectDataTypeCount {
    pub project_id: String,
    pub data_type: String,
    pub file_count: usize,
}

/// Per-project facet row retained inside the canonical genomic-project record. The project ID
/// is supplied by the enclosing record; this compact shape keeps source hashes stable and avoids
/// duplicating it in every stored facet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicProjectDataTypeCount {
    pub data_type: String,
    pub file_count: usize,
}

/// Deterministic count of cBioPortal assay modalities represented by profile metadata. This is
/// an availability inventory, not a claim that any one modality was run for a patient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealMolecularProfileTypeCount {
    pub alteration_type: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicProjectRecord {
    pub source_id: String,
    pub project_id: String,
    pub name: String,
    pub primary_site: Vec<String>,
    pub disease_types: Vec<String>,
    pub case_count: usize,
    /// Aggregate GDC file-type facets. Older snapshots may omit this optional metadata; absence
    /// stays explicit and is never treated as assay absence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_type_counts: Vec<GenomicProjectDataTypeCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalStudyRecord {
    pub source_id: String,
    pub study_id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub sample_count: Option<usize>,
    #[serde(default)]
    pub pmid: Option<String>,
    pub public_study: bool,
}

/// Public cBioPortal molecular-profile metadata. This describes available assay modalities and
/// granularity; it does not embed mutation calls, expression values, or patient-level records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalMolecularProfileRecord {
    pub source_id: String,
    pub study_id: String,
    pub profile_id: String,
    pub name: String,
    pub molecular_alteration_type: String,
    pub datatype: String,
    #[serde(default)]
    pub description: Option<String>,
    pub show_in_analysis: bool,
    pub patient_level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuidelineReference {
    pub source_id: String,
    pub reference_id: String,
    pub title: String,
    pub uri: String,
    pub publisher: String,
}

/// Compact PubMed citation, abstract, and indexing metadata. An indexed citation or abstract is
/// not treated as evidence of applicability, study quality, or a patient-level finding; those
/// judgments remain reviewer work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureRecord {
    pub source_id: String,
    pub pmid: String,
    pub title: String,
    pub journal: String,
    #[serde(default)]
    pub publication_date: Option<String>,
    #[serde(default)]
    pub doi: Option<String>,
    /// Normalized PubMed abstract text, when the public record exposes one. This is source text
    /// for reviewer inspection, not a generated conclusion or patient-level assertion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,
    /// True when the upstream abstract exceeded the ingestion bound and was explicitly clipped.
    #[serde(default, skip_serializing_if = "is_false")]
    pub abstract_truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_terms: Vec<String>,
}

/// Compact, real-data snapshot for population-level glioma research.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealGliomaBundle {
    pub schema_version: String,
    pub generated_at: String,
    /// This must remain false. Synthetic data belongs in `fixtures/`, never in this bundle.
    pub synthetic_data: bool,
    pub sources: Vec<RealDataSource>,
    #[serde(default)]
    pub clinical_trials: Vec<ClinicalTrialRecord>,
    #[serde(default)]
    pub genomic_projects: Vec<GenomicProjectRecord>,
    #[serde(default)]
    pub portal_studies: Vec<PortalStudyRecord>,
    #[serde(default)]
    pub portal_molecular_profiles: Vec<PortalMolecularProfileRecord>,
    #[serde(default)]
    pub references: Vec<GuidelineReference>,
    #[serde(default)]
    pub literature: Vec<LiteratureRecord>,
}

/// Bounded query over the public records already present in a validated bundle. Queries never
/// fetch a URL and never expose sample-level or patient-level records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataQuery {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    /// Optional case-insensitive exact phase facet for registry trials. A missing phase never
    /// matches this filter; the query therefore preserves registry missingness rather than
    /// treating an absent phase as an exclusion or a negative finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_phase: Option<String>,
    /// Optional case-insensitive exact study-design facet for registry trials. This is copied
    /// metadata only and does not establish eligibility, quality, or clinical applicability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_study_type: Option<String>,
    /// Inclusive lower bound over the registry's last-update date. Trials without that date do
    /// not match a date-bounded query because recency cannot be inferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_updated_from: Option<String>,
    /// Inclusive upper bound over the registry's last-update date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trial_updated_to: Option<String>,
    /// Optional case-insensitive exact cBioPortal molecular-assay facet. A missing alteration
    /// type never matches this filter; the query therefore preserves assay metadata
    /// missingness rather than treating it as a negative molecular finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_alteration_type: Option<String>,
    /// Optional case-insensitive exact cBioPortal datatype facet. This is metadata about the
    /// available assay representation, not a patient-level molecular value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_datatype: Option<String>,
    /// Optional case-insensitive exact GDC file data-type facet for genomic projects. This is
    /// aggregate availability metadata only; it never selects or exposes file contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genomic_data_type: Option<String>,
    /// Optional case-insensitive partial match over PubMed publication-type labels. This is
    /// indexing metadata only; it does not establish study quality or applicability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_type: Option<String>,
    /// Optional case-insensitive partial match over PubMed MeSH descriptor labels. This is
    /// indexing metadata only; it does not establish a molecular or clinical finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_term: Option<String>,
    /// Inclusive lower bound over PubMed publication dates. Articles without a normalized
    /// publication date do not match a bounded query because chronology cannot be inferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_date_from: Option<String>,
    /// Inclusive upper bound over PubMed publication dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_date_to: Option<String>,
    /// Optional record-kind facet. Facets narrow the already validated local bundle; they never
    /// trigger a network fetch or reinterpret a record.
    #[serde(default)]
    pub record_kind: Option<RealDataRecordKind>,
    /// Optional exact source-id facet for provenance-focused review.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Optional exact related-record facet. For example, a PMID finds its linked cBioPortal study
    /// and a study id finds its linked PubMed article/profile edges.
    #[serde(default)]
    pub related_record_id: Option<String>,
    #[serde(default = "default_query_limit")]
    pub limit: usize,
}

fn default_query_limit() -> usize {
    32
}

impl Default for RealDataQuery {
    fn default() -> Self {
        Self {
            text: None,
            status: None,
            trial_phase: None,
            trial_study_type: None,
            trial_updated_from: None,
            trial_updated_to: None,
            molecular_alteration_type: None,
            molecular_datatype: None,
            genomic_data_type: None,
            publication_type: None,
            mesh_term: None,
            publication_date_from: None,
            publication_date_to: None,
            record_kind: None,
            source_id: None,
            related_record_id: None,
            limit: default_query_limit(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataRecordKind {
    ClinicalTrial,
    GenomicProject,
    PortalStudy,
    PortalMolecularProfile,
    GuidelineReference,
    LiteratureArticle,
}

impl RealDataRecordKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ClinicalTrial => "clinical_trial",
            Self::GenomicProject => "genomic_project",
            Self::PortalStudy => "portal_study",
            Self::PortalMolecularProfile => "portal_molecular_profile",
            Self::GuidelineReference => "guideline_reference",
            Self::LiteratureArticle => "literature_article",
        }
    }
}

/// Directional relationship exposed when a public record has an explicit stable crosswalk to
/// another record in the same bundle. These are metadata edges, not scientific conclusions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataRelation {
    PublishedAs,
    DescribesStudy,
    HasProfile,
    ProfileOfStudy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataRelatedRecord {
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    pub relation: RealDataRelation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataQueryHit {
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
    pub source_id: String,
    pub source_uri: String,
    /// Explicit, stable relationships to records already present in this validated bundle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_records: Vec<RealDataRelatedRecord>,
    /// Bounded source-text excerpt returned only for literature hits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_alteration_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datatype: Option<String>,
    /// Optional cBioPortal profile description copied from the public metadata row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_description: Option<String>,
    /// Optional cBioPortal analysis-visibility flag. This remains metadata-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_show_in_analysis: Option<bool>,
    /// Optional cBioPortal patient-level granularity flag. This never carries a patient value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_patient_level: Option<bool>,
    /// Optional registry metadata carried through a trial hit when the snapshot contains it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intervention_names: Vec<String>,
    /// Aggregate public-study sample count, when the portal exposed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<usize>,
    /// Publication date copied from PubMed metadata, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>,
    /// Aggregate GDC file-type facets copied only for genomic-project hits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genomic_data_type_counts: Vec<GenomicProjectDataTypeCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataQueryResult {
    pub schema_version: String,
    pub bundle_digest: String,
    pub query: RealDataQuery,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    pub hits: Vec<RealDataQueryHit>,
    #[serde(default)]
    pub relationship_count: usize,
    #[serde(default)]
    pub portal_molecular_profile_count: usize,
    #[serde(default)]
    pub literature_abstract_count: usize,
    #[serde(default)]
    pub literature_abstract_truncated_count: usize,
    #[serde(default)]
    pub portal_literature_linked_count: usize,
    #[serde(default)]
    pub portal_literature_unlinked_count: usize,
    #[serde(default)]
    pub literature_without_portal_count: usize,
    #[serde(default)]
    pub portal_study_without_pmid_count: usize,
}

impl RealDataQueryResult {
    /// Validate a persisted query result without reopening the source bundle. This is a
    /// structural gate only; `validate_for_inputs` performs the exact local replay.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_SCHEMA_VERSION
            || !is_sha256(&self.bundle_digest)
            || self.query.limit == 0
            || self.query.limit > MAX_QUERY_HITS
            || self.returned_matches > self.total_matches
            || self.hits.len() != self.returned_matches
            || self.returned_matches > self.query.limit
            || self.truncated != (self.returned_matches < self.total_matches)
            || self.hits.windows(2).any(|window| {
                (window[0].record_kind, window[0].record_id.as_str())
                    > (window[1].record_kind, window[1].record_id.as_str())
            })
            || self.hits.iter().any(|hit| {
                hit.record_id.trim().is_empty()
                    || hit.title.trim().is_empty()
                    || hit.source_id.trim().is_empty()
                    || !hit.source_uri.starts_with("https://")
                    || hit
                        .abstract_excerpt
                        .as_deref()
                        .is_some_and(|excerpt| excerpt.len() > MAX_ABSTRACT_EXCERPT_CHARS)
                    || hit.phases.len() > 16
                    || hit.intervention_names.len() > MAX_TRIAL_INTERVENTIONS
                    || hit
                        .phases
                        .iter()
                        .any(|phase| phase.trim().is_empty() || phase.chars().any(char::is_control))
                    || hit
                        .intervention_names
                        .iter()
                        .any(|name| name.trim().is_empty() || name.chars().any(char::is_control))
                    || hit.study_type.as_deref().is_some_and(|value| {
                        value.trim().is_empty() || value.chars().any(char::is_control)
                    })
                    || hit
                        .last_update
                        .as_deref()
                        .is_some_and(|date| !is_calendar_date(date))
                    || hit
                        .publication_date
                        .as_deref()
                        .is_some_and(|date| !is_calendar_date(date))
                    || hit
                        .enrollment_count
                        .is_some_and(|count| count > MAX_TRIAL_ENROLLMENT)
                    || hit
                        .molecular_description
                        .as_deref()
                        .is_some_and(|description| {
                            description.trim().is_empty()
                                || description.chars().any(char::is_control)
                        })
                    || (hit.record_kind != RealDataRecordKind::PortalMolecularProfile
                        && (hit.molecular_alteration_type.is_some()
                            || hit.datatype.is_some()
                            || hit.molecular_description.is_some()
                            || hit.molecular_show_in_analysis.is_some()
                            || hit.molecular_patient_level.is_some()))
                    || (hit.record_kind != RealDataRecordKind::ClinicalTrial
                        && (!hit.phases.is_empty()
                            || hit.last_update.is_some()
                            || hit.study_type.is_some()
                            || hit.enrollment_count.is_some()
                            || !hit.intervention_names.is_empty()))
                    || (hit.record_kind != RealDataRecordKind::PortalStudy
                        && hit.sample_count.is_some())
                    || (hit.record_kind != RealDataRecordKind::LiteratureArticle
                        && hit.publication_date.is_some())
                    || (hit.record_kind != RealDataRecordKind::GenomicProject
                        && !hit.genomic_data_type_counts.is_empty())
                    || !valid_genomic_data_type_counts(&hit.genomic_data_type_counts)
                    || hit.related_records.iter().any(|related| {
                        related.record_id.trim().is_empty()
                            || related.record_kind == hit.record_kind
                                && related.record_id == hit.record_id
                    })
                    || hit.related_records.windows(2).any(|window| {
                        (window[0].record_kind, window[0].record_id.as_str())
                            >= (window[1].record_kind, window[1].record_id.as_str())
                    })
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data query result envelope is invalid".to_string(),
            });
        }
        validate_query_shape(&self.query)?;
        Ok(())
    }

    /// Replay this result against the exact validated bundle and refuse any changed query,
    /// source digest, hit ordering, or count projection.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.query(&self.query)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data query result does not replay to the supplied bundle".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct SourceContent {
    clinical_trials: Vec<ClinicalTrialRecord>,
    genomic_projects: Vec<GenomicProjectRecord>,
    portal_studies: Vec<PortalStudyRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    portal_molecular_profiles: Vec<PortalMolecularProfileRecord>,
    references: Vec<GuidelineReference>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    literature: Vec<LiteratureRecord>,
}

impl RealGliomaBundle {
    /// Validates provenance, source authority, record linkage, and all embedded content hashes.
    pub fn validate(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_SCHEMA_VERSION {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "unsupported bundle schema {:?}; expected {:?}",
                    self.schema_version, REAL_DATA_SCHEMA_VERSION
                ),
            });
        }
        if self.synthetic_data {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "synthetic_data=true is never accepted for a real-data run".to_string(),
            });
        }
        if self.sources.iter().any(|source| {
            [
                source.source_id.as_str(),
                source.authority.as_str(),
                source.uri.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
        }) || self.clinical_trials.iter().any(|trial| {
            [
                trial.source_id.as_str(),
                trial.nct_id.as_str(),
                trial.title.as_str(),
                trial.overall_status.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || trial
                    .phases
                    .iter()
                    .any(|phase| contains_synthetic_marker(phase))
                || trial
                    .last_update
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
                || trial
                    .study_type
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
                || trial
                    .intervention_names
                    .iter()
                    .any(|name| contains_synthetic_marker(name))
        }) || self.genomic_projects.iter().any(|project| {
            [
                project.source_id.as_str(),
                project.project_id.as_str(),
                project.name.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || project
                    .primary_site
                    .iter()
                    .any(|site| contains_synthetic_marker(site))
                || project
                    .disease_types
                    .iter()
                    .any(|disease| contains_synthetic_marker(disease))
                || project
                    .data_type_counts
                    .iter()
                    .any(|facet| contains_synthetic_marker(&facet.data_type))
        }) || self.portal_studies.iter().any(|study| {
            [
                study.source_id.as_str(),
                study.study_id.as_str(),
                study.name.as_str(),
                study.description.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || study.pmid.as_deref().is_some_and(contains_synthetic_marker)
        }) || self.portal_molecular_profiles.iter().any(|profile| {
            [
                profile.source_id.as_str(),
                profile.study_id.as_str(),
                profile.profile_id.as_str(),
                profile.name.as_str(),
                profile.molecular_alteration_type.as_str(),
                profile.datatype.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || profile
                    .description
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
        }) || self.references.iter().any(|reference| {
            [
                reference.source_id.as_str(),
                reference.reference_id.as_str(),
                reference.title.as_str(),
                reference.uri.as_str(),
                reference.publisher.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
        }) || self.literature.iter().any(|article| {
            [
                article.source_id.as_str(),
                article.pmid.as_str(),
                article.title.as_str(),
                article.journal.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || article
                    .publication_date
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
                || article
                    .doi
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
                || article
                    .abstract_text
                    .as_deref()
                    .is_some_and(contains_synthetic_marker)
                || article
                    .publication_types
                    .iter()
                    .any(|value| contains_synthetic_marker(value))
                || article
                    .mesh_terms
                    .iter()
                    .any(|value| contains_synthetic_marker(value))
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "synthetic marker found in real-data provenance or record metadata"
                    .to_string(),
            });
        }
        validate_text(&self.generated_at, "generated_at")?;
        if !is_utc_timestamp(&self.generated_at) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "generated_at must be a UTC RFC3339 timestamp".to_string(),
            });
        }
        if self.sources.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "bundle has no provenance sources".to_string(),
            });
        }
        let total_records = self.clinical_trials.len()
            + self.genomic_projects.len()
            + self.portal_studies.len()
            + self.portal_molecular_profiles.len()
            + self.references.len()
            + self.literature.len();
        if self.sources.len() > MAX_REAL_SOURCES || total_records > MAX_REAL_RECORDS {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "bundle exceeds safety bounds ({} sources, {} records)",
                    self.sources.len(),
                    total_records
                ),
            });
        }
        if self.clinical_trials.is_empty()
            || self.genomic_projects.is_empty()
            || self.portal_studies.is_empty()
            || self.references.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "bundle must contain registry, genomic, portal, and guideline records"
                    .to_string(),
            });
        }

        let mut source_ids = BTreeSet::new();
        let mut source_kinds = BTreeMap::new();
        for source in &self.sources {
            validate_text(&source.source_id, "source_id")?;
            validate_text(&source.authority, "authority")?;
            validate_text(&source.uri, "source_uri")?;
            validate_text(&source.retrieved_at, "retrieved_at")?;
            if !source.uri.starts_with("https://")
                || !is_allow_listed_uri(&source.uri)
                || !source_kind_matches_uri(source.kind, &source.uri)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "source {} is not an allow-listed HTTPS authority",
                        source.source_id
                    ),
                });
            }
            if !is_utc_timestamp(&source.retrieved_at) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "source {} retrieved_at must be a UTC RFC3339 timestamp",
                        source.source_id
                    ),
                });
            }
            // The wire format is fixed-width UTC, so byte ordering is chronological. A source
            // cannot be retrieved after the bundle was generated; accepting that would make a
            // provenance snapshot internally impossible to reproduce.
            if source.retrieved_at > self.generated_at {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "source {} retrieved_at is later than bundle generated_at",
                        source.source_id
                    ),
                });
            }
            if source.record_count == 0 {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("source {} declares zero records", source.source_id),
                });
            }
            if !is_sha256(&source.content_sha256) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("source {} has an invalid content_sha256", source.source_id),
                });
            }
            if !source_ids.insert(source.source_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "duplicate source_id".to_string(),
                });
            }
            source_kinds.insert(source.source_id.clone(), source.kind);
        }

        let source_set = &source_ids;
        for record_source in self
            .clinical_trials
            .iter()
            .map(|record| &record.source_id)
            .chain(self.genomic_projects.iter().map(|record| &record.source_id))
            .chain(self.portal_studies.iter().map(|record| &record.source_id))
            .chain(
                self.portal_molecular_profiles
                    .iter()
                    .map(|record| &record.source_id),
            )
            .chain(self.references.iter().map(|record| &record.source_id))
            .chain(self.literature.iter().map(|record| &record.source_id))
        {
            if !source_set.contains(record_source) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "record references an unknown source_id".to_string(),
                });
            }
        }

        let source_kind = |source_id: &str| source_kinds.get(source_id).copied();
        for record in &self.clinical_trials {
            if source_kind(&record.source_id) != Some(RealSourceKind::ClinicalTrialsRegistry) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "clinical trial {} is linked to a non-registry source",
                        record.nct_id
                    ),
                });
            }
        }
        for record in &self.genomic_projects {
            if source_kind(&record.source_id) != Some(RealSourceKind::GenomicCommons) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "genomic project {} is linked to a non-genomic source",
                        record.project_id
                    ),
                });
            }
        }
        for record in &self.portal_studies {
            if source_kind(&record.source_id) != Some(RealSourceKind::StudyPortal) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "portal study {} is linked to a non-study-portal source",
                        record.study_id
                    ),
                });
            }
        }
        for record in &self.portal_molecular_profiles {
            if source_kind(&record.source_id) != Some(RealSourceKind::StudyPortal) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "molecular profile {} is linked to a non-study-portal source",
                        record.profile_id
                    ),
                });
            }
        }
        for reference in &self.references {
            if source_kind(&reference.source_id) != Some(RealSourceKind::Guideline) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "guideline reference {} is linked to a non-guideline source",
                        reference.reference_id
                    ),
                });
            }
        }
        for article in &self.literature {
            if source_kind(&article.source_id) != Some(RealSourceKind::LiteratureIndex) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "literature article {} is linked to a non-literature source",
                        article.pmid
                    ),
                });
            }
        }

        let mut trial_ids = BTreeSet::new();
        for record in &self.clinical_trials {
            validate_text(&record.nct_id, "nct_id")?;
            validate_text(&record.title, "trial.title")?;
            validate_text(&record.overall_status, "trial.overall_status")?;
            if record.phases.len() > 16 || record.intervention_names.len() > MAX_TRIAL_INTERVENTIONS
            {
                return Err(NeurosurgeryError::TooMany {
                    field: "clinical_trial.metadata",
                    found: record.phases.len().max(record.intervention_names.len()),
                    max: MAX_TRIAL_INTERVENTIONS,
                });
            }
            for phase in &record.phases {
                validate_text(phase, "trial.phase")?;
            }
            if let Some(last_update) = &record.last_update {
                validate_text(last_update, "trial.last_update")?;
                if !is_calendar_date(last_update) {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "clinical trial {} has an invalid last_update date",
                            record.nct_id
                        ),
                    });
                }
            }
            if let Some(study_type) = &record.study_type {
                validate_text(study_type, "trial.study_type")?;
            }
            for intervention in &record.intervention_names {
                validate_text(intervention, "trial.intervention_name")?;
            }
            if record
                .enrollment_count
                .is_some_and(|count| count > MAX_TRIAL_ENROLLMENT)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "clinical trial {} enrollment metadata exceeds the safety bound",
                        record.nct_id
                    ),
                });
            }
            if !record.nct_id.starts_with("NCT") {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "clinical trial identifier is not an NCT public identifier".to_string(),
                });
            }
            if !trial_ids.insert(record.nct_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate clinical trial identifier {}", record.nct_id),
                });
            }
        }
        let mut project_ids = BTreeSet::new();
        let genomic_data_type_rows = self
            .genomic_projects
            .iter()
            .map(|project| project.data_type_counts.len())
            .sum::<usize>();
        if genomic_data_type_rows > MAX_GENOMIC_DATA_TYPE_ROWS {
            return Err(NeurosurgeryError::TooMany {
                field: "genomic_project.data_type_counts",
                found: genomic_data_type_rows,
                max: MAX_GENOMIC_DATA_TYPE_ROWS,
            });
        }
        for record in &self.genomic_projects {
            validate_text(&record.project_id, "project_id")?;
            validate_text(&record.name, "project.name")?;
            if record.case_count == 0
                || record.primary_site.is_empty()
                || record.disease_types.is_empty()
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "genomic project {} is missing aggregate metadata",
                        record.project_id
                    ),
                });
            }
            if !project_ids.insert(record.project_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate genomic project identifier {}", record.project_id),
                });
            }
            if record.data_type_counts.len() > MAX_GENOMIC_DATA_TYPES {
                return Err(NeurosurgeryError::TooMany {
                    field: "genomic_project.data_type_counts",
                    found: record.data_type_counts.len(),
                    max: MAX_GENOMIC_DATA_TYPES,
                });
            }
            let mut data_types = BTreeSet::new();
            for facet in &record.data_type_counts {
                validate_text(&facet.data_type, "genomic_project.data_type")?;
                if facet.file_count == 0 || facet.file_count > MAX_GENOMIC_FILES_PER_TYPE {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "genomic project {} has an invalid {} file-count facet",
                            record.project_id, facet.data_type
                        ),
                    });
                }
                if !data_types.insert(facet.data_type.clone()) {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "genomic project {} repeats data-type {}",
                            record.project_id, facet.data_type
                        ),
                    });
                }
            }
        }
        let mut study_ids = BTreeSet::new();
        for record in &self.portal_studies {
            validate_text(&record.study_id, "study_id")?;
            validate_text(&record.name, "portal_study.name")?;
            validate_text(&record.description, "portal_study.description")?;
            if !record.public_study {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("portal study {} is not public", record.study_id),
                });
            }
            if !study_ids.insert(record.study_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate public study identifier {}", record.study_id),
                });
            }
            if let Some(pmid) = &record.pmid {
                if pmid.trim().is_empty()
                    || pmid.len() > 32
                    || !pmid.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!("portal study {} has an invalid PMID", record.study_id),
                    });
                }
            }
        }
        let mut profile_ids = BTreeSet::new();
        for record in &self.portal_molecular_profiles {
            validate_text(&record.study_id, "molecular_profile.study_id")?;
            validate_text(&record.profile_id, "molecular_profile.profile_id")?;
            validate_text(&record.name, "molecular_profile.name")?;
            validate_text(
                &record.molecular_alteration_type,
                "molecular_profile.molecular_alteration_type",
            )?;
            validate_text(&record.datatype, "molecular_profile.datatype")?;
            if let Some(description) = &record.description {
                validate_text(description, "molecular_profile.description")?;
            }
            if !study_ids.contains(&record.study_id) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "molecular profile {} references an unknown public study {}",
                        record.profile_id, record.study_id
                    ),
                });
            }
            if !profile_ids.insert((record.study_id.clone(), record.profile_id.clone())) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "duplicate molecular profile {} for study {}",
                        record.profile_id, record.study_id
                    ),
                });
            }
        }
        let mut reference_ids = BTreeSet::new();
        for reference in &self.references {
            validate_text(&reference.reference_id, "reference_id")?;
            validate_text(&reference.title, "reference.title")?;
            validate_text(&reference.publisher, "reference.publisher")?;
            if !reference.uri.starts_with("https://") || !is_allow_listed_uri(&reference.uri) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "guideline reference is not an allow-listed HTTPS authority"
                        .to_string(),
                });
            }
            if !reference_ids.insert(reference.reference_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "duplicate guideline reference identifier {}",
                        reference.reference_id
                    ),
                });
            }
        }
        let mut pmids = BTreeSet::new();
        for article in &self.literature {
            validate_text(&article.pmid, "literature.pmid")?;
            validate_text(&article.title, "literature.title")?;
            validate_text(&article.journal, "literature.journal")?;
            if article.pmid.len() > 32 || !article.pmid.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("literature article {} has an invalid PMID", article.pmid),
                });
            }
            if let Some(date) = &article.publication_date {
                validate_text(date, "literature.publication_date")?;
                if !is_calendar_date(date) {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "literature article {} has an invalid publication_date",
                            article.pmid
                        ),
                    });
                }
            }
            if let Some(doi) = &article.doi {
                validate_text(doi, "literature.doi")?;
                if doi.len() > 512 || !doi.starts_with("10.") {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!("literature article {} has an invalid DOI", article.pmid),
                    });
                }
            }
            if let Some(abstract_text) = &article.abstract_text {
                validate_text(abstract_text, "literature.abstract_text")?;
                if abstract_text.len() > MAX_ABSTRACT_BYTES {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "literature article {} abstract exceeds {} bytes",
                            article.pmid, MAX_ABSTRACT_BYTES
                        ),
                    });
                }
            } else if article.abstract_truncated {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "literature article {} marks a missing abstract as truncated",
                        article.pmid
                    ),
                });
            }
            if article.publication_types.len() > MAX_LITERATURE_TAGS {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "literature article {} has too many publication types",
                        article.pmid
                    ),
                });
            }
            for publication_type in &article.publication_types {
                validate_text(publication_type, "literature.publication_type")?;
            }
            if article.mesh_terms.len() > MAX_LITERATURE_TAGS {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "literature article {} has too many MeSH terms",
                        article.pmid
                    ),
                });
            }
            for mesh_term in &article.mesh_terms {
                validate_text(mesh_term, "literature.mesh_term")?;
            }
            if !pmids.insert(article.pmid.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate literature PMID {}", article.pmid),
                });
            }
        }

        let by_source = self.canonical_source_content();
        for source in &self.sources {
            let content = by_source.get(&source.source_id).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!("source {} has no linked records", source.source_id),
                }
            })?;
            let bytes = serde_json::to_vec(content)
                .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
            let digest = sha256_hex(&bytes);
            if digest != source.content_sha256.to_ascii_lowercase() {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("content hash mismatch for source {}", source.source_id),
                });
            }
            let count = content.clinical_trials.len()
                + content.genomic_projects.len()
                + content.portal_studies.len()
                + content.portal_molecular_profiles.len()
                + content.references.len()
                + content.literature.len();
            if count != source.record_count {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("record count mismatch for source {}", source.source_id),
                });
            }
        }
        Ok(())
    }

    /// Summarizes the bundle after validation. The digest binds every field, including hashes.
    pub fn summary(&self) -> Result<RealDataSummary, NeurosurgeryError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
        let recruiting_trial_count = self
            .clinical_trials
            .iter()
            .filter(|trial| trial.overall_status.eq_ignore_ascii_case("recruiting"))
            .count();
        let completed_trial_count = self
            .clinical_trials
            .iter()
            .filter(|trial| trial.overall_status.eq_ignore_ascii_case("completed"))
            .count();
        let mut status_counts = BTreeMap::<String, usize>::new();
        for trial in &self.clinical_trials {
            *status_counts
                .entry(trial.overall_status.to_ascii_uppercase())
                .or_default() += 1;
        }
        let trial_status_counts = status_counts
            .into_iter()
            .map(|(status, count)| RealTrialStatusCount { status, count })
            .collect::<Vec<_>>();
        let mut profile_type_counts = BTreeMap::<String, usize>::new();
        for profile in &self.portal_molecular_profiles {
            *profile_type_counts
                .entry(profile.molecular_alteration_type.to_ascii_uppercase())
                .or_default() += 1;
        }
        let profile_type_counts = profile_type_counts
            .into_iter()
            .map(|(alteration_type, count)| RealMolecularProfileTypeCount {
                alteration_type,
                count,
            })
            .collect::<Vec<_>>();
        let literature_abstract_count = self
            .literature
            .iter()
            .filter(|article| article.abstract_text.is_some())
            .count();
        let literature_abstract_truncated_count = self
            .literature
            .iter()
            .filter(|article| article.abstract_truncated)
            .count();
        let latest_trial_update = self
            .clinical_trials
            .iter()
            .filter_map(|trial| trial.last_update.as_ref())
            .max()
            .cloned();
        let trial_study_type_count = self
            .clinical_trials
            .iter()
            .filter(|trial| trial.study_type.is_some())
            .count();
        let trial_enrollment_count = self
            .clinical_trials
            .iter()
            .filter(|trial| trial.enrollment_count.is_some())
            .count();
        let trial_intervention_count = self
            .clinical_trials
            .iter()
            .filter(|trial| !trial.intervention_names.is_empty())
            .count();
        let genomic_project_case_counts = self
            .genomic_projects
            .iter()
            .map(|project| RealGenomicProjectCaseCount {
                project_id: project.project_id.clone(),
                case_count: project.case_count,
            })
            .collect::<Vec<_>>();
        let mut genomic_project_data_type_counts = self
            .genomic_projects
            .iter()
            .flat_map(|project| {
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
        let (
            portal_literature_linked_count,
            portal_literature_unlinked_count,
            literature_without_portal_count,
            portal_study_without_pmid_count,
        ) = self.literature_crosswalk_counts();
        Ok(RealDataSummary {
            bundle_schema_version: self.schema_version.clone(),
            bundle_digest: sha256_hex(&bytes),
            source_count: self.sources.len(),
            record_count: self.clinical_trials.len()
                + self.genomic_projects.len()
                + self.portal_studies.len()
                + self.portal_molecular_profiles.len()
                + self.references.len()
                + self.literature.len(),
            clinical_trial_count: self.clinical_trials.len(),
            recruiting_trial_count,
            completed_trial_count,
            genomic_project_count: self.genomic_projects.len(),
            genomic_case_count: self
                .genomic_projects
                .iter()
                .map(|project| project.case_count)
                .sum(),
            genomic_project_case_counts,
            genomic_project_data_type_counts,
            portal_study_count: self.portal_studies.len(),
            portal_molecular_profile_count: self.portal_molecular_profiles.len(),
            relationship_count: self.relationship_count(),
            portal_sample_count: self
                .portal_studies
                .iter()
                .filter_map(|study| study.sample_count)
                .sum(),
            public_pmid_count: self
                .portal_studies
                .iter()
                .filter(|study| study.pmid.is_some())
                .count(),
            reference_count: self.references.len(),
            literature_article_count: self.literature.len(),
            literature_abstract_count,
            literature_abstract_truncated_count,
            portal_literature_linked_count,
            portal_literature_unlinked_count,
            literature_without_portal_count,
            portal_study_without_pmid_count,
            trial_status_counts,
            portal_profile_type_counts: profile_type_counts,
            latest_trial_update,
            trial_study_type_count,
            trial_enrollment_count,
            trial_intervention_count,
            provenance_bound: true,
            synthetic_data: false,
        })
    }

    /// Queries validated public records deterministically by stable identifiers/titles/status.
    pub fn query(&self, query: &RealDataQuery) -> Result<RealDataQueryResult, NeurosurgeryError> {
        self.validate()?;
        validate_query_shape(query)?;
        if let Some(source_id) = &query.source_id {
            if !self
                .sources
                .iter()
                .any(|source| source.source_id == *source_id)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("real-data query source_id {source_id:?} is not in the bundle"),
                });
            }
        }
        let text = query.text.as_deref().map(str::to_ascii_lowercase);
        let status = query.status.as_deref().map(str::to_ascii_lowercase);
        let mut hits = Vec::new();
        for record in &self.clinical_trials {
            let related_records = Vec::new();
            if !facet_matches(
                query,
                RealDataRecordKind::ClinicalTrial,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            let phase_text = record.phases.join(" ");
            let intervention_text = record.intervention_names.join(" ");
            if trial_facets_match(record, query)
                && matches_query(
                    text.as_deref(),
                    status.as_deref(),
                    [
                        record.nct_id.as_str(),
                        record.title.as_str(),
                        record.overall_status.as_str(),
                        phase_text.as_str(),
                        record.last_update.as_deref().map_or("", |value| value),
                        record.study_type.as_deref().map_or("", |value| value),
                        intervention_text.as_str(),
                    ],
                    Some(&record.overall_status),
                )
            {
                let mut hit = self.hit(
                    RealDataRecordKind::ClinicalTrial,
                    &record.nct_id,
                    &record.title,
                    Some(record.overall_status.clone()),
                    &record.source_id,
                    related_records,
                )?;
                hit.phases = record.phases.clone();
                hit.last_update = record.last_update.clone();
                hit.study_type = record.study_type.clone();
                hit.enrollment_count = record.enrollment_count;
                hit.intervention_names = record.intervention_names.clone();
                hits.push(hit);
            }
        }
        for record in &self.genomic_projects {
            let related_records = Vec::new();
            if !facet_matches(
                query,
                RealDataRecordKind::GenomicProject,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            let primary_site_text = record.primary_site.join(" ");
            let disease_type_text = record.disease_types.join(" ");
            let data_type_text = record
                .data_type_counts
                .iter()
                .map(|facet| facet.data_type.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if genomic_facets_match(record, query)
                && matches_query(
                    text.as_deref(),
                    status.as_deref(),
                    [
                        record.project_id.as_str(),
                        record.name.as_str(),
                        primary_site_text.as_str(),
                        disease_type_text.as_str(),
                        data_type_text.as_str(),
                    ],
                    None,
                )
            {
                let mut hit = self.hit(
                    RealDataRecordKind::GenomicProject,
                    &record.project_id,
                    &record.name,
                    None,
                    &record.source_id,
                    related_records,
                )?;
                hit.genomic_data_type_counts = record.data_type_counts.clone();
                hits.push(hit);
            }
        }
        for record in &self.portal_studies {
            let pmid = record.pmid.as_deref().unwrap_or_default();
            let related_records =
                self.related_records_for_study(&record.study_id, record.pmid.as_deref());
            if !facet_matches(
                query,
                RealDataRecordKind::PortalStudy,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            if matches_query(
                text.as_deref(),
                status.as_deref(),
                [
                    record.study_id.as_str(),
                    record.name.as_str(),
                    record.description.as_str(),
                    pmid,
                ],
                None,
            ) {
                let mut hit = self.hit(
                    RealDataRecordKind::PortalStudy,
                    &record.study_id,
                    &record.name,
                    None,
                    &record.source_id,
                    related_records,
                )?;
                hit.sample_count = record.sample_count;
                hits.push(hit);
            }
        }
        for record in &self.portal_molecular_profiles {
            let related_records = self.related_records_for_profile(&record.study_id);
            if !facet_matches(
                query,
                RealDataRecordKind::PortalMolecularProfile,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            if molecular_facets_match(record, query)
                && matches_query(
                    text.as_deref(),
                    status.as_deref(),
                    [
                        record.study_id.as_str(),
                        record.profile_id.as_str(),
                        record.name.as_str(),
                        record.molecular_alteration_type.as_str(),
                        record.datatype.as_str(),
                        record.description.as_deref().unwrap_or_default(),
                    ],
                    None,
                )
            {
                let mut hit = self.hit(
                    RealDataRecordKind::PortalMolecularProfile,
                    &record.profile_id,
                    &record.name,
                    None,
                    &record.source_id,
                    related_records,
                )?;
                hit.molecular_alteration_type = Some(record.molecular_alteration_type.clone());
                hit.datatype = Some(record.datatype.clone());
                hit.molecular_description = record.description.clone();
                hit.molecular_show_in_analysis = Some(record.show_in_analysis);
                hit.molecular_patient_level = Some(record.patient_level);
                hits.push(hit);
            }
        }
        for record in &self.references {
            let related_records = Vec::new();
            if !facet_matches(
                query,
                RealDataRecordKind::GuidelineReference,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            if matches_query(
                text.as_deref(),
                status.as_deref(),
                [
                    record.reference_id.as_str(),
                    record.title.as_str(),
                    record.publisher.as_str(),
                ],
                None,
            ) {
                hits.push(self.hit(
                    RealDataRecordKind::GuidelineReference,
                    &record.reference_id,
                    &record.title,
                    None,
                    &record.source_id,
                    related_records,
                )?);
            }
        }
        for record in &self.literature {
            let indexed_terms = record
                .publication_types
                .iter()
                .chain(record.mesh_terms.iter())
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            let related_records = self.related_records_for_literature(&record.pmid);
            if !facet_matches(
                query,
                RealDataRecordKind::LiteratureArticle,
                &record.source_id,
                &related_records,
            ) {
                continue;
            }
            if literature_facets_match(record, query)
                && matches_query(
                    text.as_deref(),
                    status.as_deref(),
                    [
                        record.pmid.as_str(),
                        record.title.as_str(),
                        record.journal.as_str(),
                        record.doi.as_deref().unwrap_or_default(),
                        record.abstract_text.as_deref().unwrap_or_default(),
                        indexed_terms.as_str(),
                    ],
                    None,
                )
            {
                let mut hit = self.hit(
                    RealDataRecordKind::LiteratureArticle,
                    &record.pmid,
                    &record.title,
                    None,
                    &record.source_id,
                    related_records,
                )?;
                hit.abstract_excerpt = record
                    .abstract_text
                    .as_deref()
                    .map(bounded_abstract_excerpt);
                hit.publication_types = record.publication_types.clone();
                hit.mesh_terms = record.mesh_terms.clone();
                hit.publication_date = record.publication_date.clone();
                hits.push(hit);
            }
        }
        hits.sort_by(|left, right| {
            left.record_kind
                .cmp(&right.record_kind)
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        let total_matches = hits.len();
        hits.truncate(query.limit);
        let (
            portal_literature_linked_count,
            portal_literature_unlinked_count,
            literature_without_portal_count,
            portal_study_without_pmid_count,
        ) = self.literature_crosswalk_counts();
        Ok(RealDataQueryResult {
            schema_version: REAL_DATA_SCHEMA_VERSION.to_string(),
            bundle_digest: self.summary()?.bundle_digest,
            query: query.clone(),
            total_matches,
            returned_matches: hits.len(),
            truncated: total_matches > hits.len(),
            hits,
            portal_molecular_profile_count: self.portal_molecular_profiles.len(),
            relationship_count: self.relationship_count(),
            literature_abstract_count: self
                .literature
                .iter()
                .filter(|article| article.abstract_text.is_some())
                .count(),
            literature_abstract_truncated_count: self
                .literature
                .iter()
                .filter(|article| article.abstract_truncated)
                .count(),
            portal_literature_linked_count,
            portal_literature_unlinked_count,
            literature_without_portal_count,
            portal_study_without_pmid_count,
        })
    }

    /// Converts public guideline and indexed citation metadata into evidence records without
    /// asserting applicability, study quality, or patient-level meaning.
    pub fn evidence_records(&self) -> Vec<EvidenceRecord> {
        let guideline_records = self.references.iter().map(|reference| {
            let source = self
                .sources
                .iter()
                .find(|source| source.source_id == reference.source_id);
            let citation = source.map_or_else(
                || reference.uri.clone(),
                |source| {
                    format!(
                        "{}; source_id={}; sha256={}",
                        reference.uri, source.source_id, source.content_sha256
                    )
                },
            );
            EvidenceRecord {
                id: reference.reference_id.clone(),
                title: reference.title.clone(),
                citation,
                tier: EvidenceTier::Guideline,
                population: Some("public CNS/glioma research guidance".to_string()),
                year: None,
                supports: vec![
                    ToolCapability::EvidenceSynthesis,
                    ToolCapability::ImagingReview,
                    ToolCapability::MolecularContext,
                    ToolCapability::DifferentialMatrix,
                ],
            }
        });
        let literature_records = self.literature.iter().map(|article| {
            let source = self
                .sources
                .iter()
                .find(|source| source.source_id == article.source_id);
            let citation = source.map_or_else(
                || format!("PubMed:{}", article.pmid),
                |source| {
                    format!(
                        "PubMed:{}; source_id={}; sha256={}",
                        article.pmid, source.source_id, source.content_sha256
                    )
                },
            );
            EvidenceRecord {
                id: format!("PMID-{}", article.pmid),
                title: article.title.clone(),
                citation,
                tier: EvidenceTier::Unverified,
                population: Some(format!(
                    "PubMed indexed citation; journal={}",
                    article.journal
                )),
                year: article
                    .publication_date
                    .as_deref()
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse::<u16>().ok()),
                supports: vec![ToolCapability::EvidenceSynthesis],
            }
        });
        guideline_records.chain(literature_records).collect()
    }

    fn canonical_source_content(&self) -> BTreeMap<String, SourceContent> {
        let mut ids = self
            .sources
            .iter()
            .map(|source| source.source_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.into_iter()
            .map(|source_id| {
                let mut clinical_trials = self
                    .clinical_trials
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut genomic_projects = self
                    .genomic_projects
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut portal_studies = self
                    .portal_studies
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut portal_molecular_profiles = self
                    .portal_molecular_profiles
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut references = self
                    .references
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut literature = self
                    .literature
                    .iter()
                    .filter(|record| record.source_id == source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                clinical_trials.sort_by(|a, b| a.nct_id.cmp(&b.nct_id));
                genomic_projects.sort_by(|a, b| a.project_id.cmp(&b.project_id));
                portal_studies.sort_by(|a, b| a.study_id.cmp(&b.study_id));
                portal_molecular_profiles.sort_by(|a, b| {
                    a.study_id
                        .cmp(&b.study_id)
                        .then_with(|| a.profile_id.cmp(&b.profile_id))
                });
                references.sort_by(|a, b| a.reference_id.cmp(&b.reference_id));
                literature.sort_by(|a, b| a.pmid.cmp(&b.pmid));
                (
                    source_id,
                    SourceContent {
                        clinical_trials,
                        genomic_projects,
                        portal_studies,
                        portal_molecular_profiles,
                        references,
                        literature,
                    },
                )
            })
            .collect()
    }

    fn hit(
        &self,
        record_kind: RealDataRecordKind,
        record_id: &str,
        title: &str,
        status: Option<String>,
        source_id: &str,
        related_records: Vec<RealDataRelatedRecord>,
    ) -> Result<RealDataQueryHit, NeurosurgeryError> {
        let source_uri = self
            .sources
            .iter()
            .find(|source| source.source_id == source_id)
            .map(|source| source.uri.clone())
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: format!("query hit references unknown source {source_id}"),
            })?;
        Ok(RealDataQueryHit {
            record_kind,
            record_id: record_id.to_string(),
            title: title.to_string(),
            status,
            source_id: source_id.to_string(),
            source_uri,
            related_records,
            abstract_excerpt: None,
            publication_types: Vec::new(),
            mesh_terms: Vec::new(),
            molecular_alteration_type: None,
            datatype: None,
            molecular_description: None,
            molecular_show_in_analysis: None,
            molecular_patient_level: None,
            phases: Vec::new(),
            last_update: None,
            study_type: None,
            enrollment_count: None,
            intervention_names: Vec::new(),
            sample_count: None,
            publication_date: None,
            genomic_data_type_counts: Vec::new(),
        })
    }

    fn related_records_for_study(
        &self,
        study_id: &str,
        pmid: Option<&str>,
    ) -> Vec<RealDataRelatedRecord> {
        let mut related = self
            .portal_molecular_profiles
            .iter()
            .filter(|profile| profile.study_id == study_id)
            .map(|profile| RealDataRelatedRecord {
                record_kind: RealDataRecordKind::PortalMolecularProfile,
                record_id: profile.profile_id.clone(),
                relation: RealDataRelation::HasProfile,
            })
            .collect::<Vec<_>>();
        if let Some(pmid) = pmid {
            if self.literature.iter().any(|article| article.pmid == pmid) {
                related.push(RealDataRelatedRecord {
                    record_kind: RealDataRecordKind::LiteratureArticle,
                    record_id: pmid.to_string(),
                    relation: RealDataRelation::PublishedAs,
                });
            }
        }
        related.sort_by(|left, right| {
            left.record_kind
                .cmp(&right.record_kind)
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        related
    }

    fn related_records_for_profile(&self, study_id: &str) -> Vec<RealDataRelatedRecord> {
        if self
            .portal_studies
            .iter()
            .any(|study| study.study_id == study_id)
        {
            vec![RealDataRelatedRecord {
                record_kind: RealDataRecordKind::PortalStudy,
                record_id: study_id.to_string(),
                relation: RealDataRelation::ProfileOfStudy,
            }]
        } else {
            Vec::new()
        }
    }

    fn related_records_for_literature(&self, pmid: &str) -> Vec<RealDataRelatedRecord> {
        self.portal_studies
            .iter()
            .filter(|study| study.pmid.as_deref() == Some(pmid))
            .map(|study| RealDataRelatedRecord {
                record_kind: RealDataRecordKind::PortalStudy,
                record_id: study.study_id.clone(),
                relation: RealDataRelation::DescribesStudy,
            })
            .collect()
    }

    fn relationship_count(&self) -> usize {
        self.portal_molecular_profiles.len()
            + self
                .portal_studies
                .iter()
                .filter(|study| {
                    study.pmid.as_deref().is_some_and(|pmid| {
                        self.literature.iter().any(|article| article.pmid == pmid)
                    })
                })
                .count()
    }

    /// Returns the canonical per-source payloads to aid an ingestion tool when computing hashes.
    pub fn canonical_source_payloads(&self) -> Result<BTreeMap<String, Value>, NeurosurgeryError> {
        self.canonical_source_content()
            .into_iter()
            .map(|(source_id, content)| {
                serde_json::to_value(content)
                    .map(|value| (source_id, value))
                    .map_err(|error| NeurosurgeryError::Json(error.to_string()))
            })
            .collect()
    }

    /// Computes the hashes that an ingestion job should place in `sources[*].content_sha256`.
    /// This helper does not accept or fetch URLs; it only hashes the records already in memory.
    pub fn canonical_source_hashes(&self) -> Result<BTreeMap<String, String>, NeurosurgeryError> {
        self.canonical_source_content()
            .into_iter()
            .map(|(source_id, content)| {
                serde_json::to_vec(&content)
                    .map(|bytes| (source_id, sha256_hex(&bytes)))
                    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))
            })
            .collect()
    }

    fn literature_crosswalk_counts(&self) -> (usize, usize, usize, usize) {
        let literature_pmids = self
            .literature
            .iter()
            .map(|article| article.pmid.as_str())
            .collect::<BTreeSet<_>>();
        let portal_pmids = self
            .portal_studies
            .iter()
            .filter_map(|study| study.pmid.as_deref())
            .collect::<BTreeSet<_>>();
        let linked = portal_pmids.intersection(&literature_pmids).count();
        (
            linked,
            portal_pmids.len() - linked,
            literature_pmids.len() - linked,
            self.portal_studies.len() - portal_pmids.len(),
        )
    }
}

fn matches_query<const N: usize>(
    text: Option<&str>,
    status_filter: Option<&str>,
    fields: [&str; N],
    status: Option<&str>,
) -> bool {
    let text_matches = text.is_none_or(|needle| {
        fields
            .iter()
            .any(|field| field.to_ascii_lowercase().contains(needle))
    });
    let status_matches = status_filter
        .is_none_or(|needle| status.is_some_and(|value| value.to_ascii_lowercase() == needle));
    text_matches && status_matches
}

fn facet_matches(
    query: &RealDataQuery,
    record_kind: RealDataRecordKind,
    source_id: &str,
    related_records: &[RealDataRelatedRecord],
) -> bool {
    (!has_trial_filters(query) || record_kind == RealDataRecordKind::ClinicalTrial)
        && (!has_molecular_filters(query)
            || record_kind == RealDataRecordKind::PortalMolecularProfile)
        && (!has_genomic_filters(query) || record_kind == RealDataRecordKind::GenomicProject)
        && (!has_literature_filters(query) || record_kind == RealDataRecordKind::LiteratureArticle)
        && query
            .record_kind
            .is_none_or(|expected| expected == record_kind)
        && query
            .source_id
            .as_deref()
            .is_none_or(|expected| expected == source_id)
        && query.related_record_id.as_deref().is_none_or(|expected| {
            related_records
                .iter()
                .any(|related| related.record_id == expected)
        })
}

fn has_trial_filters(query: &RealDataQuery) -> bool {
    query.trial_phase.is_some()
        || query.trial_study_type.is_some()
        || query.trial_updated_from.is_some()
        || query.trial_updated_to.is_some()
}

fn has_molecular_filters(query: &RealDataQuery) -> bool {
    query.molecular_alteration_type.is_some() || query.molecular_datatype.is_some()
}

fn has_genomic_filters(query: &RealDataQuery) -> bool {
    query.genomic_data_type.is_some()
}

fn has_literature_filters(query: &RealDataQuery) -> bool {
    query.publication_type.is_some()
        || query.mesh_term.is_some()
        || query.publication_date_from.is_some()
        || query.publication_date_to.is_some()
}

fn molecular_facets_match(record: &PortalMolecularProfileRecord, query: &RealDataQuery) -> bool {
    query
        .molecular_alteration_type
        .as_deref()
        .is_none_or(|expected| {
            record
                .molecular_alteration_type
                .eq_ignore_ascii_case(expected)
        })
        && query
            .molecular_datatype
            .as_deref()
            .is_none_or(|expected| record.datatype.eq_ignore_ascii_case(expected))
}

fn genomic_facets_match(record: &GenomicProjectRecord, query: &RealDataQuery) -> bool {
    query.genomic_data_type.as_deref().is_none_or(|expected| {
        record
            .data_type_counts
            .iter()
            .any(|facet| facet.data_type.eq_ignore_ascii_case(expected))
    })
}

fn literature_facets_match(record: &LiteratureRecord, query: &RealDataQuery) -> bool {
    query.publication_type.as_deref().is_none_or(|expected| {
        let expected = expected.to_ascii_lowercase();
        record
            .publication_types
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(&expected))
    }) && query.mesh_term.as_deref().is_none_or(|expected| {
        let expected = expected.to_ascii_lowercase();
        record
            .mesh_terms
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(&expected))
    }) && query.publication_date_from.as_deref().is_none_or(|from| {
        record
            .publication_date
            .as_deref()
            .is_some_and(|date| date >= from)
    }) && query.publication_date_to.as_deref().is_none_or(|to| {
        record
            .publication_date
            .as_deref()
            .is_some_and(|date| date <= to)
    })
}

fn trial_facets_match(record: &ClinicalTrialRecord, query: &RealDataQuery) -> bool {
    let phase_matches = query.trial_phase.as_deref().is_none_or(|expected| {
        record
            .phases
            .iter()
            .any(|phase| phase.eq_ignore_ascii_case(expected))
    });
    let study_type_matches = query.trial_study_type.as_deref().is_none_or(|expected| {
        record
            .study_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
    });
    let from_matches = query.trial_updated_from.as_deref().is_none_or(|from| {
        record
            .last_update
            .as_deref()
            .is_some_and(|date| date >= from)
    });
    let to_matches = query
        .trial_updated_to
        .as_deref()
        .is_none_or(|to| record.last_update.as_deref().is_some_and(|date| date <= to));
    phase_matches && study_type_matches && from_matches && to_matches
}

pub(crate) fn validate_query_shape(query: &RealDataQuery) -> Result<(), NeurosurgeryError> {
    if query.limit == 0 || query.limit > MAX_QUERY_HITS {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("real-data query limit must be between 1 and {MAX_QUERY_HITS}"),
        });
    }
    if let Some(text) = &query.text {
        if text.len() > MAX_QUERY_TEXT_BYTES || text.chars().any(char::is_control) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data query text exceeds its safety bound".to_string(),
            });
        }
    }
    if let Some(status) = &query.status {
        validate_query_filter(status, "real-data query status")?;
    }
    if let Some(phase) = &query.trial_phase {
        validate_query_filter(phase, "real-data query trial_phase")?;
    }
    if let Some(study_type) = &query.trial_study_type {
        validate_query_filter(study_type, "real-data query trial_study_type")?;
    }
    if let Some(alteration_type) = &query.molecular_alteration_type {
        validate_query_filter(alteration_type, "real-data query molecular_alteration_type")?;
    }
    if let Some(datatype) = &query.molecular_datatype {
        validate_query_filter(datatype, "real-data query molecular_datatype")?;
    }
    if let Some(data_type) = &query.genomic_data_type {
        validate_query_filter(data_type, "real-data query genomic_data_type")?;
    }
    if let Some(publication_type) = &query.publication_type {
        validate_query_filter(publication_type, "real-data query publication_type")?;
    }
    if let Some(mesh_term) = &query.mesh_term {
        validate_query_filter(mesh_term, "real-data query mesh_term")?;
    }
    for (value, field) in [
        (
            &query.publication_date_from,
            "real-data query publication_date_from",
        ),
        (
            &query.publication_date_to,
            "real-data query publication_date_to",
        ),
    ] {
        if let Some(date) = value {
            validate_query_filter(date, field)?;
            if !is_calendar_date(date) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("{field} must be a valid YYYY-MM-DD date"),
                });
            }
        }
    }
    if query
        .publication_date_from
        .as_ref()
        .zip(query.publication_date_to.as_ref())
        .is_some_and(|(from, to)| from > to)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real-data query publication date bounds are reversed".to_string(),
        });
    }
    for (value, field) in [
        (
            &query.trial_updated_from,
            "real-data query trial_updated_from",
        ),
        (&query.trial_updated_to, "real-data query trial_updated_to"),
    ] {
        if let Some(date) = value {
            validate_query_filter(date, field)?;
            if !is_calendar_date(date) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("{field} must be a valid YYYY-MM-DD date"),
                });
            }
        }
    }
    if query
        .trial_updated_from
        .as_ref()
        .zip(query.trial_updated_to.as_ref())
        .is_some_and(|(from, to)| from > to)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real-data query trial update bounds are reversed".to_string(),
        });
    }
    if has_trial_filters(query)
        && query
            .record_kind
            .is_some_and(|kind| kind != RealDataRecordKind::ClinicalTrial)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "trial-specific query facets require record_kind=clinical_trial".to_string(),
        });
    }
    if has_molecular_filters(query)
        && query
            .record_kind
            .is_some_and(|kind| kind != RealDataRecordKind::PortalMolecularProfile)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "molecular-specific query facets require record_kind=portal_molecular_profile"
                .to_string(),
        });
    }
    if has_literature_filters(query)
        && query
            .record_kind
            .is_some_and(|kind| kind != RealDataRecordKind::LiteratureArticle)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "literature-specific query facets require record_kind=literature_article"
                .to_string(),
        });
    }
    if let Some(source_id) = &query.source_id {
        validate_query_filter(source_id, "real-data query source_id")?;
    }
    if let Some(related_record_id) = &query.related_record_id {
        validate_query_filter(related_record_id, "real-data query related_record_id")?;
    }
    Ok(())
}

fn validate_query_filter(value: &str, field: &'static str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty()
        || value.len() > MAX_QUERY_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} is invalid"),
        });
    }
    Ok(())
}

fn validate_text(value: &str, field: &'static str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} is empty"),
        });
    }
    if value.len() > 16_000 || value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} exceeds the real-data safety bound"),
        });
    }
    Ok(())
}

fn contains_synthetic_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("synthetic")
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn bounded_abstract_excerpt(value: &str) -> String {
    value.chars().take(MAX_ABSTRACT_EXCERPT_CHARS).collect()
}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    if ![0usize, 1, 2, 3, 5, 6, 8, 9]
        .into_iter()
        .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
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

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_genomic_data_type_counts(rows: &[GenomicProjectDataTypeCount]) -> bool {
    if rows.len() > MAX_GENOMIC_DATA_TYPES {
        return false;
    }
    let mut seen = BTreeSet::new();
    rows.iter().all(|row| {
        !row.data_type.trim().is_empty()
            && !row.data_type.chars().any(char::is_control)
            && row.file_count > 0
            && row.file_count <= MAX_GENOMIC_FILES_PER_TYPE
            && seen.insert(row.data_type.as_str())
    })
}

fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.len() != 20
        || ![0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15]
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
    let hour = (bytes[11] - b'0') * 10 + (bytes[12] - b'0');
    let minute = (bytes[14] - b'0') * 10 + (bytes[15] - b'0');
    let second = (bytes[17] - b'0') * 10 + (bytes[18] - b'0');
    is_calendar_date(&value[..10]) && hour < 24 && minute < 60 && second < 60
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn is_allow_listed_uri(uri: &str) -> bool {
    [
        "https://clinicaltrials.gov/",
        "https://api.gdc.cancer.gov/",
        "https://gdc.cancer.gov/",
        "https://www.cbioportal.org/",
        "https://www.cancer.gov/",
        "https://eutils.ncbi.nlm.nih.gov/",
    ]
    .iter()
    .any(|prefix| uri.starts_with(prefix))
}

pub(crate) fn source_kind_matches_uri(kind: RealSourceKind, uri: &str) -> bool {
    match kind {
        RealSourceKind::ClinicalTrialsRegistry => uri.starts_with("https://clinicaltrials.gov/"),
        RealSourceKind::GenomicCommons => uri.starts_with("https://api.gdc.cancer.gov/"),
        RealSourceKind::StudyPortal => uri.starts_with("https://www.cbioportal.org/"),
        RealSourceKind::Guideline => uri.starts_with("https://www.cancer.gov/"),
        RealSourceKind::LiteratureIndex => {
            uri.starts_with("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/")
        }
    }
}
