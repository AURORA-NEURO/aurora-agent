//! Coverage and temporal projections for the validated real glioma snapshot.
//!
//! This module is intentionally descriptive. It reports which source-backed record kinds are
//! present, how dates and abstracts are distributed, and where explicit cross-source links are
//! missing. It never scores study quality, imputes dates, merges cohorts, or turns population
//! metadata into a clinical conclusion.

use crate::{
    NeurosurgeryError, RealDataRecordKind, RealGliomaBundle, RealMolecularProfileTypeCount,
    RealSourceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const REAL_DATA_COVERAGE_SCHEMA_VERSION: &str = "bioprism-neurosurgery-real-data-coverage/0.1";

/// Optional facets over the already validated local snapshot. A date range excludes records
/// whose relevant date is unknown; the report counts those records as missing rather than
/// silently treating them as inside or outside the interval.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageQuery {
    #[serde(default)]
    pub record_kind: Option<RealDataRecordKind>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub from_year: Option<u16>,
    #[serde(default)]
    pub to_year: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageSource {
    pub source_id: String,
    pub kind: RealSourceKind,
    pub authority: String,
    pub uri: String,
    pub retrieved_at: String,
    pub declared_record_count: usize,
    pub observed_record_count: usize,
    pub selected_record_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageRecordKindCount {
    pub record_kind: RealDataRecordKind,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageYearBucket {
    pub year: u16,
    pub count: usize,
}

/// One temporal axis keeps observed, missing, and clipped values distinct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageTimeAxis {
    pub axis: String,
    pub observed_count: usize,
    pub missing_count: usize,
    pub earliest: Option<String>,
    pub latest: Option<String>,
    pub year_buckets: Vec<RealDataCoverageYearBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageLinkage {
    pub portal_study_count: usize,
    pub portal_study_with_pmid_count: usize,
    pub portal_study_without_pmid_count: usize,
    pub portal_molecular_profile_count: usize,
    pub explicit_profile_relationship_count: usize,
    pub literature_article_count: usize,
    pub literature_linked_to_portal_count: usize,
    pub literature_without_portal_count: usize,
    pub explicit_publication_relationship_count: usize,
    pub literature_abstract_count: usize,
    pub literature_abstract_missing_count: usize,
    pub literature_abstract_truncated_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageGap {
    pub code: String,
    pub count: usize,
    pub description: String,
}

/// A digest-addressed corpus projection for reviewer-owned research planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataCoverageReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub coverage_digest: String,
    pub generated_at: String,
    pub query: RealDataCoverageQuery,
    pub total_record_count: usize,
    pub matched_record_count: usize,
    pub source_count: usize,
    pub sources: Vec<RealDataCoverageSource>,
    pub record_kind_counts: Vec<RealDataCoverageRecordKindCount>,
    pub time_axes: Vec<RealDataCoverageTimeAxis>,
    pub portal_profile_type_counts: Vec<RealMolecularProfileTypeCount>,
    /// Linkage is bundle-wide context, deliberately separate from the query's matched count.
    pub linkage: RealDataCoverageLinkage,
    pub gaps: Vec<RealDataCoverageGap>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataCoverageReport {
    /// Validate a persisted descriptive coverage projection without fetching any source.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query_shape(&self.query)?;
        if self.schema_version != REAL_DATA_COVERAGE_SCHEMA_VERSION
            || !is_sha256_hex(&self.coverage_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.source_count == 0
            || self.source_count != self.sources.len()
            || self.total_record_count < self.matched_record_count
            || self.record_kind_counts.iter().any(|entry| entry.count == 0)
            || self
                .record_kind_counts
                .windows(2)
                .any(|window| window[0].record_kind >= window[1].record_kind)
            || self
                .record_kind_counts
                .iter()
                .map(|entry| entry.count)
                .fold(0usize, usize::saturating_add)
                != self.matched_record_count
            || self.sources.iter().any(|source| {
                source.source_id.trim().is_empty()
                    || source.authority.trim().is_empty()
                    || !crate::real_data::is_allow_listed_uri(&source.uri)
                    || !crate::real_data::source_kind_matches_uri(source.kind, &source.uri)
                    || !crate::temporal::is_utc_timestamp(&source.retrieved_at)
                    || source.selected_record_count > source.observed_record_count
                    || source.observed_record_count > source.declared_record_count
            })
            || self
                .sources
                .windows(2)
                .any(|window| window[0].source_id >= window[1].source_id)
            || self.time_axes.len() != 2
            || self.time_axes.iter().any(|axis| {
                axis.axis.trim().is_empty()
                    || axis.observed_count.saturating_add(axis.missing_count)
                        > self.matched_record_count
                    || axis
                        .year_buckets
                        .iter()
                        .any(|bucket| bucket.count == 0 || !(1900..=2200).contains(&bucket.year))
                    || axis
                        .year_buckets
                        .windows(2)
                        .any(|window| window[0].year >= window[1].year)
            })
            || self
                .portal_profile_type_counts
                .windows(2)
                .any(|window| window[0].alteration_type >= window[1].alteration_type)
            || self
                .portal_profile_type_counts
                .iter()
                .any(|entry| entry.alteration_type.trim().is_empty() || entry.count == 0)
            || self
                .linkage
                .portal_study_with_pmid_count
                .saturating_add(self.linkage.portal_study_without_pmid_count)
                != self.linkage.portal_study_count
            || self
                .linkage
                .literature_linked_to_portal_count
                .saturating_add(self.linkage.literature_without_portal_count)
                != self.linkage.literature_article_count
            || self
                .linkage
                .literature_abstract_count
                .saturating_add(self.linkage.literature_abstract_missing_count)
                != self.linkage.literature_article_count
            || self.linkage.literature_abstract_truncated_count
                > self.linkage.literature_abstract_count
            || self.gaps.iter().any(|gap| {
                gap.code.trim().is_empty() || gap.description.trim().is_empty() || gap.count == 0
            })
            || {
                let mut gap_codes = std::collections::BTreeSet::new();
                self.gaps
                    .iter()
                    .any(|gap| !gap_codes.insert(gap.code.as_str()))
            }
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data coverage envelope is invalid".to_string(),
            });
        }
        if self.coverage_digest
            != digest_coverage(CoverageDigestInput {
                bundle_digest: &self.bundle_digest,
                query: &self.query,
                matched_record_count: self.matched_record_count,
                sources: &self.sources,
                record_kind_counts: &self.record_kind_counts,
                time_axes: &self.time_axes,
                linkage: &self.linkage,
                gaps: &self.gaps,
            })?
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data coverage digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild coverage from the exact validated snapshot and persisted query.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.coverage_report(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data coverage does not replay to the exact supplied snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct RecordRow<'a> {
    kind: RealDataRecordKind,
    source_id: &'a str,
    date: Option<&'a str>,
}

impl RealGliomaBundle {
    /// Build a bounded descriptive report over the validated public snapshot.
    pub fn coverage_report(
        &self,
        query: &RealDataCoverageQuery,
    ) -> Result<RealDataCoverageReport, NeurosurgeryError> {
        self.validate()?;
        validate_query(self, query)?;
        let rows = record_rows(self);
        let selected = rows
            .iter()
            .copied()
            .filter(|row| selected_row(row, query))
            .collect::<Vec<_>>();
        let bundle_digest = self.summary()?.bundle_digest;
        let mut observed_by_source = BTreeMap::<&str, usize>::new();
        let mut selected_by_source = BTreeMap::<&str, usize>::new();
        for row in &rows {
            *observed_by_source.entry(row.source_id).or_default() += 1;
        }
        for row in &selected {
            *selected_by_source.entry(row.source_id).or_default() += 1;
        }
        let mut sources = self
            .sources
            .iter()
            .map(|source| RealDataCoverageSource {
                source_id: source.source_id.clone(),
                kind: source.kind,
                authority: source.authority.clone(),
                uri: source.uri.clone(),
                retrieved_at: source.retrieved_at.clone(),
                declared_record_count: source.record_count,
                observed_record_count: observed_by_source
                    .get(source.source_id.as_str())
                    .copied()
                    .unwrap_or(0),
                selected_record_count: selected_by_source
                    .get(source.source_id.as_str())
                    .copied()
                    .unwrap_or(0),
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));

        let mut record_kind_counts = BTreeMap::<RealDataRecordKind, usize>::new();
        for row in &selected {
            *record_kind_counts.entry(row.kind).or_default() += 1;
        }
        let record_kind_counts = record_kind_counts
            .into_iter()
            .map(|(record_kind, count)| RealDataCoverageRecordKindCount { record_kind, count })
            .collect::<Vec<_>>();
        let time_axes = vec![
            time_axis(
                "clinical_trial_last_update",
                selected
                    .iter()
                    .filter(|row| row.kind == RealDataRecordKind::ClinicalTrial),
            ),
            time_axis(
                "literature_publication_date",
                selected
                    .iter()
                    .filter(|row| row.kind == RealDataRecordKind::LiteratureArticle),
            ),
        ];
        let portal_profile_type_counts = profile_type_counts(self);
        let linkage = linkage(self);
        let gaps = coverage_gaps(&linkage);
        let coverage_digest = digest_coverage(CoverageDigestInput {
            bundle_digest: &bundle_digest,
            query,
            matched_record_count: selected.len(),
            sources: &sources,
            record_kind_counts: &record_kind_counts,
            time_axes: &time_axes,
            linkage: &linkage,
            gaps: &gaps,
        })?;
        let report = RealDataCoverageReport {
            schema_version: REAL_DATA_COVERAGE_SCHEMA_VERSION.to_string(),
            bundle_digest,
            coverage_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            total_record_count: rows.len(),
            matched_record_count: selected.len(),
            source_count: self.sources.len(),
            sources,
            record_kind_counts,
            time_axes,
            portal_profile_type_counts,
            linkage,
            gaps,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "counts describe the caller-supplied snapshot, not the complete public literature or registry".to_string(),
                "source retrieval timestamps and publication/update dates are reported, but freshness is not a quality or clinical-relevance judgment".to_string(),
                "unknown dates remain missing and are excluded from explicit year-range facets".to_string(),
                "linkage is stable-ID metadata only; it does not establish cohort identity, study quality, applicability, biology, causality, or treatment effect".to_string(),
                "the report never fetches URLs, invokes a model, opens credentials, exposes patient/sample values, or writes durable state".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(
    bundle: &RealGliomaBundle,
    query: &RealDataCoverageQuery,
) -> Result<(), NeurosurgeryError> {
    validate_query_shape(query)?;
    if let Some(source_id) = query.source_id.as_deref() {
        if !bundle
            .sources
            .iter()
            .any(|source| source.source_id == source_id)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("coverage source_id {source_id:?} is not in the bundle"),
            });
        }
    }
    Ok(())
}

fn validate_query_shape(query: &RealDataCoverageQuery) -> Result<(), NeurosurgeryError> {
    if let Some(source_id) = query.source_id.as_deref() {
        if source_id.is_empty() || source_id.len() > 512 || source_id.chars().any(char::is_control)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "coverage source_id is empty, too long, or contains a control character"
                    .to_string(),
            });
        }
    }
    for year in [query.from_year, query.to_year].into_iter().flatten() {
        if !(1900..=2200).contains(&year) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("coverage year {year} must be between 1900 and 2200"),
            });
        }
    }
    if let (Some(from), Some(to)) = (query.from_year, query.to_year) {
        if from > to {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "coverage from_year cannot be later than to_year".to_string(),
            });
        }
    }
    Ok(())
}

fn record_rows(bundle: &RealGliomaBundle) -> Vec<RecordRow<'_>> {
    let mut rows = Vec::with_capacity(
        bundle.clinical_trials.len()
            + bundle.genomic_projects.len()
            + bundle.portal_studies.len()
            + bundle.portal_molecular_profiles.len()
            + bundle.references.len()
            + bundle.literature.len(),
    );
    rows.extend(bundle.clinical_trials.iter().map(|record| RecordRow {
        kind: RealDataRecordKind::ClinicalTrial,
        source_id: &record.source_id,
        date: record.last_update.as_deref(),
    }));
    rows.extend(bundle.genomic_projects.iter().map(|record| RecordRow {
        kind: RealDataRecordKind::GenomicProject,
        source_id: &record.source_id,
        date: None,
    }));
    rows.extend(bundle.portal_studies.iter().map(|record| RecordRow {
        kind: RealDataRecordKind::PortalStudy,
        source_id: &record.source_id,
        date: None,
    }));
    rows.extend(
        bundle
            .portal_molecular_profiles
            .iter()
            .map(|record| RecordRow {
                kind: RealDataRecordKind::PortalMolecularProfile,
                source_id: &record.source_id,
                date: None,
            }),
    );
    rows.extend(bundle.references.iter().map(|record| RecordRow {
        kind: RealDataRecordKind::GuidelineReference,
        source_id: &record.source_id,
        date: None,
    }));
    rows.extend(bundle.literature.iter().map(|record| RecordRow {
        kind: RealDataRecordKind::LiteratureArticle,
        source_id: &record.source_id,
        date: record.publication_date.as_deref(),
    }));
    rows
}

fn selected_row(row: &RecordRow<'_>, query: &RealDataCoverageQuery) -> bool {
    if query.record_kind.is_some_and(|kind| kind != row.kind) {
        return false;
    }
    if query
        .source_id
        .as_deref()
        .is_some_and(|source_id| source_id != row.source_id)
    {
        return false;
    }
    let Some(year) = row.date.and_then(year) else {
        return query.from_year.is_none() && query.to_year.is_none();
    };
    query.from_year.is_none_or(|from| year >= from) && query.to_year.is_none_or(|to| year <= to)
}

fn year(value: &str) -> Option<u16> {
    value.get(..4)?.parse().ok()
}

fn time_axis<'a, I>(axis: &str, rows: I) -> RealDataCoverageTimeAxis
where
    I: Iterator<Item = &'a RecordRow<'a>>,
{
    let mut dates = Vec::new();
    let mut missing_count = 0;
    for row in rows {
        if let Some(date) = row.date {
            dates.push(date);
        } else {
            missing_count += 1;
        }
    }
    dates.sort_unstable();
    let mut buckets = BTreeMap::<u16, usize>::new();
    for date in &dates {
        if let Some(year) = year(date) {
            *buckets.entry(year).or_default() += 1;
        }
    }
    RealDataCoverageTimeAxis {
        axis: axis.to_string(),
        observed_count: dates.len(),
        missing_count,
        earliest: dates.first().map(|date| (*date).to_string()),
        latest: dates.last().map(|date| (*date).to_string()),
        year_buckets: buckets
            .into_iter()
            .map(|(year, count)| RealDataCoverageYearBucket { year, count })
            .collect(),
    }
}

fn profile_type_counts(bundle: &RealGliomaBundle) -> Vec<RealMolecularProfileTypeCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for profile in &bundle.portal_molecular_profiles {
        *counts
            .entry(profile.molecular_alteration_type.to_ascii_uppercase())
            .or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(alteration_type, count)| RealMolecularProfileTypeCount {
            alteration_type,
            count,
        })
        .collect()
}

fn linkage(bundle: &RealGliomaBundle) -> RealDataCoverageLinkage {
    let literature_pmids = bundle
        .literature
        .iter()
        .map(|article| article.pmid.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let portal_pmids = bundle
        .portal_studies
        .iter()
        .filter_map(|study| study.pmid.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    let linked = portal_pmids.intersection(&literature_pmids).count();
    let without_portal = literature_pmids.len().saturating_sub(linked);
    RealDataCoverageLinkage {
        portal_study_count: bundle.portal_studies.len(),
        portal_study_with_pmid_count: bundle
            .portal_studies
            .iter()
            .filter(|study| study.pmid.is_some())
            .count(),
        portal_study_without_pmid_count: bundle
            .portal_studies
            .iter()
            .filter(|study| study.pmid.is_none())
            .count(),
        portal_molecular_profile_count: bundle.portal_molecular_profiles.len(),
        explicit_profile_relationship_count: bundle
            .portal_molecular_profiles
            .iter()
            .filter(|profile| {
                bundle
                    .portal_studies
                    .iter()
                    .any(|study| study.study_id == profile.study_id)
            })
            .count(),
        literature_article_count: bundle.literature.len(),
        literature_linked_to_portal_count: linked,
        literature_without_portal_count: without_portal,
        explicit_publication_relationship_count: linked,
        literature_abstract_count: bundle
            .literature
            .iter()
            .filter(|article| article.abstract_text.is_some())
            .count(),
        literature_abstract_missing_count: bundle
            .literature
            .iter()
            .filter(|article| article.abstract_text.is_none())
            .count(),
        literature_abstract_truncated_count: bundle
            .literature
            .iter()
            .filter(|article| article.abstract_truncated)
            .count(),
    }
}

fn coverage_gaps(linkage: &RealDataCoverageLinkage) -> Vec<RealDataCoverageGap> {
    let mut gaps = Vec::new();
    if linkage.portal_study_without_pmid_count > 0 {
        gaps.push(RealDataCoverageGap {
            code: "portal_study_without_pmid".to_string(),
            count: linkage.portal_study_without_pmid_count,
            description:
                "portal studies without an explicit PMID cannot be publication-crosswalked"
                    .to_string(),
        });
    }
    if linkage.literature_without_portal_count > 0 {
        gaps.push(RealDataCoverageGap {
            code: "literature_without_portal".to_string(),
            count: linkage.literature_without_portal_count,
            description: "indexed literature records have no selected portal-study crosswalk"
                .to_string(),
        });
    }
    if linkage.literature_abstract_missing_count > 0 {
        gaps.push(RealDataCoverageGap {
            code: "literature_abstract_missing".to_string(),
            count: linkage.literature_abstract_missing_count,
            description: "literature records have no abstract text in the captured snapshot"
                .to_string(),
        });
    }
    if linkage.literature_abstract_truncated_count > 0 {
        gaps.push(RealDataCoverageGap {
            code: "literature_abstract_truncated".to_string(),
            count: linkage.literature_abstract_truncated_count,
            description: "literature abstracts were clipped at the ingestion bound".to_string(),
        });
    }
    gaps
}

struct CoverageDigestInput<'a> {
    bundle_digest: &'a str,
    query: &'a RealDataCoverageQuery,
    matched_record_count: usize,
    sources: &'a [RealDataCoverageSource],
    record_kind_counts: &'a [RealDataCoverageRecordKindCount],
    time_axes: &'a [RealDataCoverageTimeAxis],
    linkage: &'a RealDataCoverageLinkage,
    gaps: &'a [RealDataCoverageGap],
}

fn digest_coverage(input: CoverageDigestInput<'_>) -> Result<String, NeurosurgeryError> {
    let payload = (
        input.bundle_digest,
        input.query,
        input.matched_record_count,
        input.sources,
        input.record_kind_counts,
        input.time_axes,
        input.linkage,
        input.gaps,
    );
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
