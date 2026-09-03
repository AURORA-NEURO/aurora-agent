//! Cross-source identifier reconciliation for a validated real glioma snapshot.
//!
//! A snapshot can pass per-source hash checks while still leaving a reviewer with ambiguous
//! crosswalks: a portal PMID may not be present in the local literature window, multiple portal
//! studies may point at one PMID, or two literature rows may carry the same normalized DOI. This
//! module makes those cases explicit without repairing, merging, ranking, or interpreting them.
//! It is intentionally provider-free and read-only; every finding is metadata review work, never
//! a biological, clinical, or evidence-quality conclusion.

use crate::{NeurosurgeryError, RealDataRecordKind, RealGliomaBundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const REAL_DATA_RECONCILIATION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-reconciliation/0.1";
pub const MAX_REAL_DATA_RECONCILIATION_ISSUES: usize = 256;
const DEFAULT_MAX_ISSUES: usize = 64;

fn default_max_issues() -> usize {
    DEFAULT_MAX_ISSUES
}

/// Bounds for a single local reconciliation pass. The query cannot widen the source snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReconciliationQuery {
    #[serde(default = "default_max_issues")]
    pub max_issues: usize,
}

impl Default for RealDataReconciliationQuery {
    fn default() -> Self {
        Self {
            max_issues: default_max_issues(),
        }
    }
}

/// Metadata-only class of cross-source inconsistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataReconciliationIssueKind {
    PortalPmidMissingLiterature,
    PortalPmidSharedByStudies,
    LiteratureDoiSharedByRecords,
}

/// One deterministic identifier finding. `identifier` is a public PMID or normalized DOI; no
/// abstract, title, sample identifier, or patient value is copied into the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReconciliationIssue {
    pub kind: RealDataReconciliationIssueKind,
    pub identifier: String,
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_record_ids: Vec<String>,
    pub detail: String,
}

/// Counts retained independently so truncation never hides the size of a reconciliation finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReconciliationCounts {
    pub portal_study_count: usize,
    pub portal_study_with_pmid_count: usize,
    pub portal_study_without_pmid_count: usize,
    pub portal_pmid_missing_literature_count: usize,
    pub shared_portal_pmid_count: usize,
    pub literature_article_count: usize,
    pub literature_with_doi_count: usize,
    pub shared_literature_doi_count: usize,
}

/// Digest-bound, provider-free reconciliation of a single validated public snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReconciliationReport {
    pub schema_version: String,
    pub reconciliation_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataReconciliationQuery,
    pub counts: RealDataReconciliationCounts,
    pub candidate_issue_count: usize,
    pub returned_issue_count: usize,
    pub omitted_issue_count: usize,
    pub truncated: bool,
    pub issues: Vec<RealDataReconciliationIssue>,
    pub requires_review: bool,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataReconciliationReport {
    /// Validate a persisted reconciliation without reopening the source snapshot.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_RECONCILIATION_SCHEMA_VERSION
            || !is_sha256_hex(&self.reconciliation_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.query.max_issues == 0
            || self.query.max_issues > MAX_REAL_DATA_RECONCILIATION_ISSUES
            || self.returned_issue_count != self.issues.len()
            || self.returned_issue_count > self.candidate_issue_count
            || self.omitted_issue_count
                != self
                    .candidate_issue_count
                    .saturating_sub(self.returned_issue_count)
            || self.truncated != (self.omitted_issue_count > 0)
            || self.requires_review != (self.candidate_issue_count > 0)
            || self
                .counts
                .portal_study_with_pmid_count
                .saturating_add(self.counts.portal_study_without_pmid_count)
                != self.counts.portal_study_count
            || self.counts.portal_pmid_missing_literature_count
                > self.counts.portal_study_with_pmid_count
            || self.counts.shared_portal_pmid_count > self.counts.portal_study_with_pmid_count
            || self.counts.literature_with_doi_count > self.counts.literature_article_count
            || self.counts.shared_literature_doi_count > self.counts.literature_with_doi_count
            || self.candidate_issue_count
                != self
                    .counts
                    .portal_pmid_missing_literature_count
                    .saturating_add(self.counts.shared_portal_pmid_count)
                    .saturating_add(self.counts.shared_literature_doi_count)
            || self
                .issues
                .windows(2)
                .any(|window| issue_sort_key(&window[0]) >= issue_sort_key(&window[1]))
            || self.issues.iter().any(|issue| {
                issue.identifier.trim().is_empty()
                    || issue.identifier.len() > 512
                    || issue.identifier.chars().any(char::is_control)
                    || issue.record_id.trim().is_empty()
                    || issue.source_id.trim().is_empty()
                    || issue.detail.trim().is_empty()
                    || issue.detail.chars().any(char::is_control)
                    || issue
                        .related_record_ids
                        .windows(2)
                        .any(|window| window[0] >= window[1])
                    || issue.related_record_ids.iter().any(|id| {
                        id.trim().is_empty() || id.len() > 512 || id.chars().any(char::is_control)
                    })
                    || !issue_semantics_are_valid(issue)
            })
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data reconciliation envelope is invalid".to_string(),
            });
        }
        if self.reconciliation_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data reconciliation digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Replay the report against the exact validated snapshot and persisted bounds.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.reconcile(&self.query)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data reconciliation does not replay to the supplied snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Reconcile exact public identifiers already present in one validated bundle.
    pub fn reconcile(
        &self,
        query: &RealDataReconciliationQuery,
    ) -> Result<RealDataReconciliationReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        let bundle_digest = self.summary()?.bundle_digest;
        let literature_pmids = self
            .literature
            .iter()
            .map(|article| article.pmid.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut portal_by_pmid = BTreeMap::<&str, Vec<&str>>::new();
        for study in &self.portal_studies {
            if let Some(pmid) = study.pmid.as_deref() {
                portal_by_pmid
                    .entry(pmid)
                    .or_default()
                    .push(&study.study_id);
            }
        }
        for studies in portal_by_pmid.values_mut() {
            studies.sort_unstable();
        }
        let mut doi_to_pmids = BTreeMap::<String, Vec<&str>>::new();
        for article in &self.literature {
            if let Some(doi) = article.doi.as_deref() {
                doi_to_pmids
                    .entry(normalize_doi(doi))
                    .or_default()
                    .push(&article.pmid);
            }
        }
        for pmids in doi_to_pmids.values_mut() {
            pmids.sort_unstable();
        }

        let mut issues = Vec::new();
        let portal_study_with_pmid_count = self
            .portal_studies
            .iter()
            .filter(|s| s.pmid.is_some())
            .count();
        for study in &self.portal_studies {
            let Some(pmid) = study.pmid.as_deref() else {
                continue;
            };
            if !literature_pmids.contains(pmid) {
                issues.push(RealDataReconciliationIssue {
                    kind: RealDataReconciliationIssueKind::PortalPmidMissingLiterature,
                    identifier: pmid.to_string(),
                    record_kind: RealDataRecordKind::PortalStudy,
                    record_id: study.study_id.clone(),
                    source_id: study.source_id.clone(),
                    related_record_ids: Vec::new(),
                    detail: "portal PMID is not present in the bounded literature records of this snapshot; verify the citation window or source linkage".to_string(),
                });
            }
        }
        for (pmid, studies) in &portal_by_pmid {
            if studies.len() > 1 {
                issues.push(RealDataReconciliationIssue {
                    kind: RealDataReconciliationIssueKind::PortalPmidSharedByStudies,
                    identifier: (*pmid).to_string(),
                    record_kind: RealDataRecordKind::PortalStudy,
                    record_id: studies[0].to_string(),
                    source_id: self.portal_studies.iter().find(|s| s.study_id == studies[0]).map(|s| s.source_id.clone()).unwrap_or_default(),
                    related_record_ids: studies[1..].iter().map(|id| (*id).to_string()).collect(),
                    detail: "one PMID is attached to multiple portal studies; confirm whether the source crosswalk is intentional".to_string(),
                });
            }
        }
        for (doi, pmids) in &doi_to_pmids {
            if pmids.len() > 1 {
                issues.push(RealDataReconciliationIssue {
                    kind: RealDataReconciliationIssueKind::LiteratureDoiSharedByRecords,
                    identifier: doi.clone(),
                    record_kind: RealDataRecordKind::LiteratureArticle,
                    record_id: pmids[0].to_string(),
                    source_id: self.literature.iter().find(|a| a.pmid == pmids[0]).map(|a| a.source_id.clone()).unwrap_or_default(),
                    related_record_ids: pmids[1..].iter().map(|id| (*id).to_string()).collect(),
                    detail: "one normalized DOI is attached to multiple literature records; verify identifier normalization and citation identity".to_string(),
                });
            }
        }
        issues.sort_by(|left, right| issue_sort_key(left).cmp(&issue_sort_key(right)));
        let candidate_issue_count = issues.len();
        let returned_issue_count = candidate_issue_count.min(query.max_issues);
        issues.truncate(returned_issue_count);
        let counts = RealDataReconciliationCounts {
            portal_study_count: self.portal_studies.len(),
            portal_study_with_pmid_count,
            portal_study_without_pmid_count: self.portal_studies.len()
                - portal_study_with_pmid_count,
            portal_pmid_missing_literature_count: self
                .portal_studies
                .iter()
                .filter(|s| {
                    s.pmid
                        .as_deref()
                        .is_some_and(|pmid| !literature_pmids.contains(pmid))
                })
                .count(),
            shared_portal_pmid_count: portal_by_pmid
                .values()
                .filter(|studies| studies.len() > 1)
                .count(),
            literature_article_count: self.literature.len(),
            literature_with_doi_count: self.literature.iter().filter(|a| a.doi.is_some()).count(),
            shared_literature_doi_count: doi_to_pmids
                .values()
                .filter(|pmids| pmids.len() > 1)
                .count(),
        };
        let mut report = RealDataReconciliationReport {
            schema_version: REAL_DATA_RECONCILIATION_SCHEMA_VERSION.to_string(),
            reconciliation_digest: String::new(),
            bundle_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            counts,
            candidate_issue_count,
            returned_issue_count,
            omitted_issue_count: candidate_issue_count - returned_issue_count,
            truncated: candidate_issue_count > returned_issue_count,
            issues,
            requires_review: candidate_issue_count > 0,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the reconciliation compares public identifiers already present in one validated snapshot; it never fetches, repairs, merges, or promotes records".to_string(),
                "missing or shared identifiers are metadata review obligations, not evidence-quality scores, cohort identity, biological relationships, or clinical findings".to_string(),
                "a bounded literature window can make a valid upstream citation appear missing; the report preserves that uncertainty instead of imputing it".to_string(),
                "the report cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.reconciliation_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &RealDataReconciliationQuery) -> Result<(), NeurosurgeryError> {
    if query.max_issues == 0 || query.max_issues > MAX_REAL_DATA_RECONCILIATION_ISSUES {
        return Err(NeurosurgeryError::TooMany {
            field: "real_data_reconciliation.max_issues",
            found: query.max_issues,
            max: MAX_REAL_DATA_RECONCILIATION_ISSUES,
        });
    }
    Ok(())
}

fn issue_sort_key(
    issue: &RealDataReconciliationIssue,
) -> (RealDataReconciliationIssueKind, RealDataRecordKind, &str) {
    (issue.kind, issue.record_kind, issue.record_id.as_str())
}

fn issue_semantics_are_valid(issue: &RealDataReconciliationIssue) -> bool {
    match issue.kind {
        RealDataReconciliationIssueKind::PortalPmidMissingLiterature => {
            issue.record_kind == RealDataRecordKind::PortalStudy
                && issue.identifier.bytes().all(|byte| byte.is_ascii_digit())
                && issue.related_record_ids.is_empty()
        }
        RealDataReconciliationIssueKind::PortalPmidSharedByStudies => {
            issue.record_kind == RealDataRecordKind::PortalStudy
                && issue.identifier.bytes().all(|byte| byte.is_ascii_digit())
                && !issue.related_record_ids.is_empty()
                && !issue
                    .related_record_ids
                    .iter()
                    .any(|record_id| record_id == &issue.record_id)
        }
        RealDataReconciliationIssueKind::LiteratureDoiSharedByRecords => {
            issue.record_kind == RealDataRecordKind::LiteratureArticle
                && issue.identifier.starts_with("10.")
                && !issue.related_record_ids.is_empty()
                && !issue
                    .related_record_ids
                    .iter()
                    .any(|record_id| record_id == &issue.record_id)
                && issue.record_id.bytes().all(|byte| byte.is_ascii_digit())
                && issue
                    .related_record_ids
                    .iter()
                    .all(|record_id| record_id.bytes().all(|byte| byte.is_ascii_digit()))
        }
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

fn digest_report(report: &RealDataReconciliationReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.reconciliation_digest.clear();
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
