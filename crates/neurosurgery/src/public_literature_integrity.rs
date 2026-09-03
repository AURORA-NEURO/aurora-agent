//! Record-level integrity and missingness audit for the real PubMed snapshot.
//!
//! `PublicLiteratureBundle::validate` is intentionally fail-closed: malformed records, unknown
//! sources, duplicate PMIDs, and bad source hashes are rejected before analysis. This module adds
//! the complementary review projection for valid bundles. It reports explicit metadata gaps
//! (missing DOI/abstract/indexing tags), duplicate normalized DOIs, and bounded lane selection;
//! it never grades study quality, infers biology, or produces a clinical conclusion.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureRecord, PublicLiteratureSummary,
    Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PUBLIC_LITERATURE_INTEGRITY_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-integrity-audit/0.1";
const MAX_ISSUES: usize = 256;
const DEFAULT_MAX_ISSUES: usize = 128;

fn default_max_issues() -> usize {
    DEFAULT_MAX_ISSUES
}

/// Bounded lane/issue projection controls for the integrity audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureIntegrityAuditQuery {
    /// `None` audits all six lanes; a list keeps the requested scope explicit.
    #[serde(default)]
    pub specialties: Option<Vec<Specialty>>,
    #[serde(default = "default_max_issues")]
    pub max_issues: usize,
}

impl Default for PublicLiteratureIntegrityAuditQuery {
    fn default() -> Self {
        Self {
            specialties: None,
            max_issues: default_max_issues(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureIntegrityCounts {
    pub selected_record_count: usize,
    pub selected_source_count: usize,
    pub unique_pmid_count: usize,
    pub doi_count: usize,
    pub missing_doi_count: usize,
    pub abstract_count: usize,
    pub missing_abstract_count: usize,
    pub abstract_truncated_count: usize,
    pub empty_publication_type_count: usize,
    pub empty_mesh_term_count: usize,
    pub duplicate_doi_group_count: usize,
    pub cross_specialty_duplicate_doi_group_count: usize,
}

/// A source-addressable issue. Record text is not copied into the audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureIntegrityIssue {
    pub code: String,
    pub specialty: Specialty,
    pub pmid: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_pmids: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureIntegrityReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Digest-bound, provider-free integrity and missingness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureIntegrityAuditReport {
    pub schema_version: String,
    pub audit_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: PublicLiteratureIntegrityAuditQuery,
    pub summary: PublicLiteratureSummary,
    pub counts: PublicLiteratureIntegrityCounts,
    pub issues: Vec<PublicLiteratureIntegrityIssue>,
    pub omitted_issue_count: usize,
    pub truncated: bool,
    pub requires_integrity_review: bool,
    pub review_reasons: Vec<PublicLiteratureIntegrityReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureIntegrityAuditReport {
    /// Validate a persisted public-literature integrity projection without fetching sources.
    /// This checks issue/count closure and digest shape, not study quality or clinical truth.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        if self.schema_version != PUBLIC_LITERATURE_INTEGRITY_AUDIT_SCHEMA_VERSION
            || !is_sha256_hex(&self.audit_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || self.generated_at.trim().is_empty()
            || self.bundle_digest != self.summary.bundle_digest
            || self.summary.schema_version
                != crate::public_literature::PUBLIC_LITERATURE_SCHEMA_VERSION
            || !self.summary.provenance_bound
            || self.summary.synthetic_data
            || self.counts.selected_record_count != self.counts.unique_pmid_count
            || self
                .counts
                .abstract_count
                .saturating_add(self.counts.missing_abstract_count)
                != self.counts.selected_record_count
            || self.counts.abstract_truncated_count > self.counts.abstract_count
            || self.counts.missing_doi_count > self.counts.selected_record_count
            || self.counts.empty_publication_type_count > self.counts.selected_record_count
            || self.counts.empty_mesh_term_count > self.counts.selected_record_count
            || self.counts.duplicate_doi_group_count > self.counts.doi_count
            || self.counts.cross_specialty_duplicate_doi_group_count
                > self.counts.duplicate_doi_group_count
            || self.issues.len() > self.query.max_issues
            || self.truncated != (self.omitted_issue_count > 0)
            || self.requires_integrity_review != !self.review_reasons.is_empty()
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature integrity audit envelope is invalid".to_string(),
            });
        }
        let mut issue_keys = BTreeSet::new();
        for issue in &self.issues {
            if !matches!(
                issue.code.as_str(),
                "missing_doi"
                    | "missing_abstract"
                    | "abstract_truncated"
                    | "missing_publication_types"
                    | "missing_mesh_terms"
                    | "duplicate_normalized_doi"
                    | "cross_specialty_duplicate_doi"
            ) || issue.pmid.trim().is_empty()
                || issue.source_id.trim().is_empty()
                || issue.detail.trim().is_empty()
                || {
                    let mut related = BTreeSet::new();
                    issue
                        .related_pmids
                        .iter()
                        .any(|pmid| pmid.trim().is_empty() || !related.insert(pmid))
                }
                || !issue_keys.insert((issue.code.clone(), issue.specialty, issue.pmid.clone()))
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "public-literature integrity issues are malformed or duplicated"
                        .to_string(),
                });
            }
        }
        if self.issues.windows(2).any(|window| {
            (
                window[0].code.as_str(),
                window[0].specialty,
                window[0].pmid.as_str(),
            ) > (
                window[1].code.as_str(),
                window[1].specialty,
                window[1].pmid.as_str(),
            )
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature integrity issues are not in canonical order".to_string(),
            });
        }
        if self.audit_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature integrity audit digest does not match its contents"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the integrity projection from the exact validated public-literature snapshot.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.integrity_audit(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature integrity audit does not replay to the exact supplied snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl PublicLiteratureBundle {
    /// Audit explicit provenance/missingness facts in a validated public-literature snapshot.
    pub fn integrity_audit(
        &self,
        query: &PublicLiteratureIntegrityAuditQuery,
    ) -> Result<PublicLiteratureIntegrityAuditReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        let summary = self.summary()?;
        let selected = self
            .records
            .iter()
            .filter(|record| {
                query
                    .specialties
                    .as_ref()
                    .is_none_or(|specialties| specialties.contains(&record.specialty))
            })
            .collect::<Vec<_>>();
        let mut ordered = selected;
        ordered.sort_by(|left, right| {
            left.specialty
                .cmp(&right.specialty)
                .then_with(|| left.pmid.cmp(&right.pmid))
        });

        let mut selected_source_ids = BTreeSet::new();
        let mut doi_groups: BTreeMap<String, Vec<&PublicLiteratureRecord>> = BTreeMap::new();
        let mut abstract_count = 0;
        let mut missing_doi_count = 0;
        let mut missing_abstract_count = 0;
        let mut abstract_truncated_count = 0;
        let mut empty_publication_type_count = 0;
        let mut empty_mesh_term_count = 0;
        let mut issues = Vec::new();

        for record in &ordered {
            selected_source_ids.insert(record.source_id.clone());
            if let Some(doi) = record.doi.as_deref() {
                doi_groups
                    .entry(normalize_doi(doi))
                    .or_default()
                    .push(*record);
            } else {
                missing_doi_count += 1;
                issues.push(issue(
                    "missing_doi",
                    record,
                    Vec::new(),
                    "the citation has no DOI metadata in this snapshot; inspect the source before treating the identifier as complete",
                ));
            }
            if record.abstract_text.is_some() {
                abstract_count += 1;
            } else {
                missing_abstract_count += 1;
                issues.push(issue(
                    "missing_abstract",
                    record,
                    Vec::new(),
                    "the citation has no abstract text in this snapshot; absence is an acquisition/indexing gap, not a negative finding",
                ));
            }
            if record.abstract_truncated {
                abstract_truncated_count += 1;
                issues.push(issue(
                    "abstract_truncated",
                    record,
                    Vec::new(),
                    "the source marks the abstract as truncated; do not treat the local text as exhaustive",
                ));
            }
            if record.publication_types.is_empty() {
                empty_publication_type_count += 1;
                issues.push(issue(
                    "missing_publication_types",
                    record,
                    Vec::new(),
                    "no PubMed publication-type labels are present for this citation",
                ));
            }
            if record.mesh_terms.is_empty() {
                empty_mesh_term_count += 1;
                issues.push(issue(
                    "missing_mesh_terms",
                    record,
                    Vec::new(),
                    "no MeSH descriptors are present for this citation; indexing completeness remains unresolved",
                ));
            }
        }

        let mut duplicate_doi_group_count = 0;
        let mut cross_specialty_duplicate_doi_group_count = 0;
        for records in doi_groups.values().filter(|records| records.len() > 1) {
            duplicate_doi_group_count += 1;
            let specialties = records
                .iter()
                .map(|record| record.specialty)
                .collect::<BTreeSet<_>>();
            let cross_specialty = specialties.len() > 1;
            if cross_specialty {
                cross_specialty_duplicate_doi_group_count += 1;
            }
            let code = if cross_specialty {
                "cross_specialty_duplicate_doi"
            } else {
                "duplicate_normalized_doi"
            };
            let anchor = records[0];
            let related_pmids = records
                .iter()
                .skip(1)
                .map(|record| record.pmid.clone())
                .collect::<Vec<_>>();
            issues.push(issue(
                code,
                anchor,
                related_pmids,
                if cross_specialty {
                    "the same normalized DOI appears in multiple specialty lanes; lane membership is retrieval metadata and requires reviewer reconciliation"
                } else {
                    "the same normalized DOI appears more than once in the selected lane; inspect source identity before deduplicating"
                },
            ));
        }

        let counts = PublicLiteratureIntegrityCounts {
            selected_record_count: ordered.len(),
            selected_source_count: selected_source_ids.len(),
            unique_pmid_count: ordered.len(),
            doi_count: doi_groups.len(),
            missing_doi_count,
            abstract_count,
            missing_abstract_count,
            abstract_truncated_count,
            empty_publication_type_count,
            empty_mesh_term_count,
            duplicate_doi_group_count,
            cross_specialty_duplicate_doi_group_count,
        };

        let mut review_reasons = Vec::new();
        add_reason(
            &mut review_reasons,
            "missing_doi",
            missing_doi_count,
            "some selected citations lack DOI metadata; this is a provenance completeness obligation, not a quality judgment",
        );
        add_reason(
            &mut review_reasons,
            "missing_abstract",
            missing_abstract_count,
            "some selected citations lack abstract text; local absence must not be interpreted as negative evidence",
        );
        add_reason(
            &mut review_reasons,
            "abstract_truncated",
            abstract_truncated_count,
            "some selected abstracts are explicitly truncated and require source review before exhaustive claims",
        );
        add_reason(
            &mut review_reasons,
            "missing_publication_types",
            empty_publication_type_count,
            "some selected citations have no publication-type labels",
        );
        add_reason(
            &mut review_reasons,
            "missing_mesh_terms",
            empty_mesh_term_count,
            "some selected citations have no MeSH descriptors; indexing coverage is unresolved",
        );
        add_reason(
            &mut review_reasons,
            "duplicate_doi",
            duplicate_doi_group_count,
            "duplicate normalized DOIs require source-level reconciliation and must not be silently collapsed",
        );
        if ordered.is_empty() {
            review_reasons.push(PublicLiteratureIntegrityReviewReason {
                code: "empty_selected_scope".to_string(),
                count: 1,
                detail: "the requested specialty scope contains no records, so this audit cannot establish corpus coverage".to_string(),
            });
        }

        issues.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then(left.specialty.cmp(&right.specialty))
                .then(left.pmid.cmp(&right.pmid))
        });
        let omitted_issue_count = issues.len().saturating_sub(query.max_issues);
        issues.truncate(query.max_issues);
        if omitted_issue_count > 0 {
            review_reasons.push(PublicLiteratureIntegrityReviewReason {
                code: "projection_truncated".to_string(),
                count: omitted_issue_count,
                detail:
                    "caller bounds omitted issue rows; the returned projection is not exhaustive"
                        .to_string(),
            });
        }
        let truncated = omitted_issue_count > 0;
        let mut report = PublicLiteratureIntegrityAuditReport {
            schema_version: PUBLIC_LITERATURE_INTEGRITY_AUDIT_SCHEMA_VERSION.to_string(),
            audit_digest: String::new(),
            bundle_digest: summary.bundle_digest.clone(),
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            summary,
            counts,
            issues,
            omitted_issue_count,
            truncated,
            requires_integrity_review: !review_reasons.is_empty(),
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the audit reports source metadata completeness and identifier hygiene only; it never scores studies, evidence quality, biological relevance, or clinical applicability".to_string(),
                "missing DOI, abstract, publication-type, or MeSH metadata is an acquisition/indexing gap and not evidence that a record or finding is absent".to_string(),
                "duplicate DOI groups and lane membership are source-reconciliation obligations; no records are merged or deleted".to_string(),
                "the report is a provider-free research handoff and cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.audit_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &PublicLiteratureIntegrityAuditQuery) -> Result<(), NeurosurgeryError> {
    if query.max_issues == 0 || query.max_issues > MAX_ISSUES {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_integrity_audit.max_issues",
            found: query.max_issues,
            max: MAX_ISSUES,
        });
    }
    if let Some(specialties) = &query.specialties {
        if specialties.is_empty() || specialties.len() > 6 {
            return Err(NeurosurgeryError::TooMany {
                field: "public_literature_integrity_audit.specialties",
                found: specialties.len(),
                max: 6,
            });
        }
        let mut unique = BTreeSet::new();
        if specialties
            .iter()
            .any(|specialty| !unique.insert(*specialty))
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature integrity specialties must be unique".to_string(),
            });
        }
    }
    Ok(())
}

fn issue(
    code: &str,
    record: &PublicLiteratureRecord,
    related_pmids: Vec<String>,
    detail: &str,
) -> PublicLiteratureIntegrityIssue {
    PublicLiteratureIntegrityIssue {
        code: code.to_string(),
        specialty: record.specialty,
        pmid: record.pmid.clone(),
        source_id: record.source_id.clone(),
        related_pmids,
        detail: detail.to_string(),
    }
}

fn add_reason(
    reasons: &mut Vec<PublicLiteratureIntegrityReviewReason>,
    code: &str,
    count: usize,
    detail: &str,
) {
    if count > 0 {
        reasons.push(PublicLiteratureIntegrityReviewReason {
            code: code.to_string(),
            count,
            detail: detail.to_string(),
        });
    }
}

fn normalize_doi(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    let normalized = normalized
        .strip_prefix("https://doi.org/")
        .or_else(|| normalized.strip_prefix("http://doi.org/"))
        .or_else(|| normalized.strip_prefix("doi:"))
        .unwrap_or(&normalized);
    normalized
        .trim_end_matches(['.', ',', ';', ')', ']', '}'])
        .to_string()
}

fn digest_report(
    report: &PublicLiteratureIntegrityAuditReport,
) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.audit_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
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
