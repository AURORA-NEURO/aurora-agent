//! Caller-owned review work derived from the real PubMed integrity audit.
//!
//! The queue does not make the public-literature corpus better or more complete. It turns the
//! audit's explicit missingness and identifier findings into stable, source-linked tasks so a
//! human reviewer (or a local orchestration layer) can work through them without guessing what
//! an absent field means. No study-quality, biological, or clinical inference is performed.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureIntegrityAuditQuery,
    PublicLiteratureIntegrityIssue, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const PUBLIC_LITERATURE_REVIEW_QUEUE_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-review-queue/0.1";
const MAX_ITEMS: usize = 256;
const DEFAULT_MAX_ITEMS: usize = 64;

fn default_max_items() -> usize {
    DEFAULT_MAX_ITEMS
}

/// Bounded specialty scope for the public-literature review queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureReviewQueueQuery {
    /// `None` keeps all validated specialty lanes in scope; a list is an explicit lane filter.
    #[serde(default)]
    pub specialties: Option<Vec<Specialty>>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

impl Default for PublicLiteratureReviewQueueQuery {
    fn default() -> Self {
        Self {
            specialties: None,
            max_items: default_max_items(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicLiteratureReviewClass {
    Provenance,
    Completeness,
    IdentifierReconciliation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicLiteratureReviewKind {
    MissingDoi,
    MissingAbstract,
    AbstractTruncated,
    MissingPublicationTypes,
    MissingMeshTerms,
    DuplicateNormalizedDoi,
    CrossSpecialtyDuplicateDoi,
}

impl PublicLiteratureReviewKind {
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "missing_doi" => Some(Self::MissingDoi),
            "missing_abstract" => Some(Self::MissingAbstract),
            "abstract_truncated" => Some(Self::AbstractTruncated),
            "missing_publication_types" => Some(Self::MissingPublicationTypes),
            "missing_mesh_terms" => Some(Self::MissingMeshTerms),
            "duplicate_normalized_doi" => Some(Self::DuplicateNormalizedDoi),
            "cross_specialty_duplicate_doi" => Some(Self::CrossSpecialtyDuplicateDoi),
            _ => None,
        }
    }

    const fn class(self) -> PublicLiteratureReviewClass {
        match self {
            Self::MissingDoi => PublicLiteratureReviewClass::Provenance,
            Self::MissingAbstract
            | Self::AbstractTruncated
            | Self::MissingPublicationTypes
            | Self::MissingMeshTerms => PublicLiteratureReviewClass::Completeness,
            Self::DuplicateNormalizedDoi | Self::CrossSpecialtyDuplicateDoi => {
                PublicLiteratureReviewClass::IdentifierReconciliation
            }
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::MissingDoi => "missing_doi",
            Self::MissingAbstract => "missing_abstract",
            Self::AbstractTruncated => "abstract_truncated",
            Self::MissingPublicationTypes => "missing_publication_types",
            Self::MissingMeshTerms => "missing_mesh_terms",
            Self::DuplicateNormalizedDoi => "duplicate_normalized_doi",
            Self::CrossSpecialtyDuplicateDoi => "cross_specialty_duplicate_doi",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicLiteratureReviewStatus {
    NeedsHumanReview,
}

/// One stable task for a source-linked metadata or identifier obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureReviewItem {
    pub task_id: String,
    pub class: PublicLiteratureReviewClass,
    pub kind: PublicLiteratureReviewKind,
    pub status: PublicLiteratureReviewStatus,
    pub specialty: Specialty,
    pub source_id: String,
    pub source_uri: String,
    pub pmid: String,
    pub record_uri: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_pmids: Vec<String>,
    pub reason: String,
    pub reviewer_roles: Vec<String>,
}

/// Digest-addressed, provider-free review work for one validated public-literature bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureReviewQueueReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub queue_digest: String,
    pub integrity_audit_digest: String,
    pub generated_at: String,
    pub query: PublicLiteratureReviewQueueQuery,
    pub candidate_item_count: usize,
    pub returned_item_count: usize,
    pub omitted_item_count: usize,
    pub omitted_integrity_issue_count: usize,
    pub truncated: bool,
    pub items: Vec<PublicLiteratureReviewItem>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureReviewQueueReport {
    /// Validate a persisted metadata-review queue without fetching or mutating source records.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        let materialized_item_count = self
            .candidate_item_count
            .saturating_sub(self.omitted_integrity_issue_count);
        if self.schema_version != PUBLIC_LITERATURE_REVIEW_QUEUE_SCHEMA_VERSION
            || !is_sha256_hex(&self.queue_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || !is_sha256_hex(&self.integrity_audit_digest)
            || self.generated_at.trim().is_empty()
            || self.returned_item_count != self.items.len()
            || self.returned_item_count > materialized_item_count
            || self
                .returned_item_count
                .saturating_add(self.omitted_item_count)
                != materialized_item_count
            || self.truncated
                != (self.omitted_integrity_issue_count > 0 || self.omitted_item_count > 0)
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature review queue envelope is invalid".to_string(),
            });
        }
        let mut task_ids = std::collections::BTreeSet::new();
        for item in &self.items {
            if item.status != PublicLiteratureReviewStatus::NeedsHumanReview
                || item.source_id.trim().is_empty()
                || item.pmid.trim().is_empty()
                || item.title.trim().is_empty()
                || item.reason.trim().is_empty()
                || item.reviewer_roles.is_empty()
                || !item.source_uri.starts_with("https://")
                || !item
                    .record_uri
                    .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
                || item.task_id
                    != format!(
                        "public-literature-review-{}-{}",
                        item.kind.slug(),
                        item.pmid
                    )
                || item.class != item.kind.class()
                || item.related_pmids.iter().any(|pmid| pmid.trim().is_empty())
                || !task_ids.insert(item.task_id.clone())
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "public-literature review queue items are malformed or duplicated"
                        .to_string(),
                });
            }
        }
        if self.items.windows(2).any(|window| {
            (
                window[0].class,
                window[0].kind,
                window[0].specialty,
                window[0].pmid.as_str(),
            ) > (
                window[1].class,
                window[1].kind,
                window[1].specialty,
                window[1].pmid.as_str(),
            )
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature review queue items are not in canonical order"
                    .to_string(),
            });
        }
        if self.queue_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature review queue digest does not match its contents"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the review queue from the exact validated public-literature snapshot and query.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.review_queue(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "public-literature review queue does not replay to the exact supplied snapshot"
                        .to_string(),
            });
        }
        Ok(())
    }
}

impl PublicLiteratureBundle {
    /// Convert validated integrity findings into stable reviewer-owned tasks. The queue never
    /// fetches a record, repairs a field, deduplicates a DOI, or interprets source text.
    pub fn review_queue(
        &self,
        query: &PublicLiteratureReviewQueueQuery,
    ) -> Result<PublicLiteratureReviewQueueReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        let integrity = self.integrity_audit(&PublicLiteratureIntegrityAuditQuery {
            specialties: query.specialties.clone(),
            // Match the integrity audit's default projection so mission envelopes can
            // bind the queue to the exact audit digest they expose alongside it.
            ..Default::default()
        })?;
        let records = self
            .records
            .iter()
            .map(|record| (record.pmid.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let sources = self
            .sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let mut items = Vec::with_capacity(integrity.issues.len());
        for issue in &integrity.issues {
            let kind = PublicLiteratureReviewKind::from_code(&issue.code).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!("unsupported integrity issue code {:?}", issue.code),
                }
            })?;
            let record = records.get(issue.pmid.as_str()).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "integrity issue {} references missing PMID {}",
                        issue.code, issue.pmid
                    ),
                }
            })?;
            let source = sources.get(issue.source_id.as_str()).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "integrity issue {} references missing source {}",
                        issue.code, issue.source_id
                    ),
                }
            })?;
            items.push(item(issue, kind, record, source.uri.as_str()));
        }
        items.sort_by(|left, right| {
            left.class
                .cmp(&right.class)
                .then(left.kind.cmp(&right.kind))
                .then(left.specialty.cmp(&right.specialty))
                .then(left.pmid.cmp(&right.pmid))
        });
        let candidate_item_count = items.len() + integrity.omitted_issue_count;
        let omitted_item_count = items.len().saturating_sub(query.max_items);
        items.truncate(query.max_items);
        let returned_item_count = items.len();
        let truncated = integrity.truncated || omitted_item_count > 0;
        let mut report = PublicLiteratureReviewQueueReport {
            schema_version: PUBLIC_LITERATURE_REVIEW_QUEUE_SCHEMA_VERSION.to_string(),
            bundle_digest: integrity.bundle_digest.clone(),
            queue_digest: String::new(),
            integrity_audit_digest: integrity.audit_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            candidate_item_count,
            returned_item_count,
            omitted_item_count,
            omitted_integrity_issue_count: integrity.omitted_issue_count,
            truncated,
            items,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "queue tasks describe source metadata and identifier obligations, not clinical urgency, evidence quality, diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
                "missing DOI, abstract, publication-type, or MeSH metadata remains unknown; the queue never imputes a value or treats absence as negative evidence".to_string(),
                "duplicate DOI tasks require reviewer reconciliation; no citations are merged, deleted, or promoted automatically".to_string(),
                "titles and stable identifiers are copied only to help a reviewer locate the public record; abstracts and patient-level values are never copied".to_string(),
                "the queue is a caller-owned handoff; it never fetches URLs, invokes a provider, opens credentials, sends notifications, or writes durable state".to_string(),
            ],
        };
        report.queue_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &PublicLiteratureReviewQueueQuery) -> Result<(), NeurosurgeryError> {
    if query.max_items == 0 || query.max_items > MAX_ITEMS {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_review_queue.max_items",
            found: query.max_items,
            max: MAX_ITEMS,
        });
    }
    if let Some(specialties) = &query.specialties {
        if specialties.is_empty() || specialties.len() > Specialty::ALL.len() {
            return Err(NeurosurgeryError::TooMany {
                field: "public_literature_review_queue.specialties",
                found: specialties.len(),
                max: Specialty::ALL.len(),
            });
        }
        let mut unique = std::collections::BTreeSet::new();
        if specialties
            .iter()
            .any(|specialty| !unique.insert(*specialty))
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature review queue specialties must be unique".to_string(),
            });
        }
    }
    Ok(())
}

fn item(
    issue: &PublicLiteratureIntegrityIssue,
    kind: PublicLiteratureReviewKind,
    record: &crate::PublicLiteratureRecord,
    source_uri: &str,
) -> PublicLiteratureReviewItem {
    PublicLiteratureReviewItem {
        task_id: format!("public-literature-review-{}-{}", kind.slug(), issue.pmid),
        class: kind.class(),
        kind,
        status: PublicLiteratureReviewStatus::NeedsHumanReview,
        specialty: issue.specialty,
        source_id: issue.source_id.clone(),
        source_uri: source_uri.to_string(),
        pmid: issue.pmid.clone(),
        record_uri: format!("https://pubmed.ncbi.nlm.nih.gov/{}/", issue.pmid),
        title: record.title.clone(),
        related_pmids: issue.related_pmids.clone(),
        reason: issue.detail.clone(),
        reviewer_roles: issue.specialty.profile().human_review_roles,
    }
}

fn digest_report(report: &PublicLiteratureReviewQueueReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.queue_digest.clear();
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
