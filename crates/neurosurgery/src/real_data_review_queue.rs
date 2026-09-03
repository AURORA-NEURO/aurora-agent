//! Deterministic review obligations derived from a validated public glioma snapshot.
//!
//! The queue is intentionally narrower than a clinical decision system. It turns explicit
//! metadata gaps—missing cross-source links, absent abstracts, unknown dates, and unavailable
//! sample counts—into reviewer-owned tasks. It never ranks patient risk, recommends care, or
//! treats a population record as case evidence.

use crate::{
    NeurosurgeryError, RealDataRecordKind, RealDataSource, RealGliomaBundle, RealSourceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_REVIEW_QUEUE_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-review-queue/0.1";
pub const MAX_REAL_DATA_REVIEW_ITEMS: usize = 256;

fn default_max_items() -> usize {
    64
}

/// Bounded facets over explicit metadata obligations in one validated snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewQueueQuery {
    #[serde(default)]
    pub record_kind: Option<RealDataRecordKind>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

impl Default for RealDataReviewQueueQuery {
    fn default() -> Self {
        Self {
            record_kind: None,
            source_id: None,
            max_items: default_max_items(),
        }
    }
}

/// Structural review class. The labels are not clinical urgency or evidence-quality scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataReviewClass {
    Provenance,
    Completeness,
    Context,
}

/// One explicit metadata obligation found in the local snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataReviewKind {
    MissingPortalPublicationLink,
    UnlinkedLiteratureCitation,
    MissingLiteratureAbstract,
    TruncatedLiteratureAbstract,
    MissingClinicalTrialUpdate,
    MissingPortalSampleCount,
}

impl RealDataReviewKind {
    pub(crate) const fn class(self) -> RealDataReviewClass {
        match self {
            Self::MissingPortalPublicationLink => RealDataReviewClass::Provenance,
            Self::UnlinkedLiteratureCitation => RealDataReviewClass::Context,
            Self::MissingLiteratureAbstract
            | Self::TruncatedLiteratureAbstract
            | Self::MissingClinicalTrialUpdate
            | Self::MissingPortalSampleCount => RealDataReviewClass::Completeness,
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::MissingPortalPublicationLink => "missing_portal_publication_link",
            Self::UnlinkedLiteratureCitation => "unlinked_literature_citation",
            Self::MissingLiteratureAbstract => "missing_literature_abstract",
            Self::TruncatedLiteratureAbstract => "truncated_literature_abstract",
            Self::MissingClinicalTrialUpdate => "missing_clinical_trial_update",
            Self::MissingPortalSampleCount => "missing_portal_sample_count",
        }
    }
}

/// All current queue rows await a qualified human's metadata review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataReviewStatus {
    NeedsHumanReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewItem {
    pub task_id: String,
    pub class: RealDataReviewClass,
    pub kind: RealDataReviewKind,
    pub status: RealDataReviewStatus,
    pub source_id: String,
    pub source_kind: RealSourceKind,
    pub source_uri: String,
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    pub title: String,
    pub reason: String,
    pub reviewer_roles: Vec<String>,
}

/// Digest-addressed, caller-owned queue for structural real-data review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewQueueReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub queue_digest: String,
    pub generated_at: String,
    pub query: RealDataReviewQueueQuery,
    pub source_count: usize,
    pub record_count: usize,
    pub candidate_item_count: usize,
    pub returned_item_count: usize,
    pub omitted_item_count: usize,
    pub truncated: bool,
    pub items: Vec<RealDataReviewItem>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealGliomaBundle {
    /// Derive explicit metadata-review work from this validated snapshot. No source is fetched,
    /// and no abstract, sample value, or patient-level record is copied into a task.
    pub fn review_queue(
        &self,
        query: &RealDataReviewQueueQuery,
    ) -> Result<RealDataReviewQueueReport, NeurosurgeryError> {
        self.validate()?;
        validate_query(self, query)?;
        let summary = self.summary()?;
        let sources = self
            .sources
            .iter()
            .map(|source| (source.source_id.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let linked_pmids = self
            .portal_studies
            .iter()
            .filter_map(|study| study.pmid.as_deref())
            .collect::<BTreeSet<_>>();
        let mut items = Vec::new();

        for study in &self.portal_studies {
            if study.pmid.is_none() {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::MissingPortalPublicationLink,
                        record_kind: RealDataRecordKind::PortalStudy,
                        record_id: &study.study_id,
                        title: &study.name,
                        source_id: &study.source_id,
                        reason: "The public study has no explicit PMID crosswalk; a reviewer must verify whether a publication link exists or is genuinely absent.",
                    },
                )?;
            }
            if study.sample_count.is_none() {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::MissingPortalSampleCount,
                        record_kind: RealDataRecordKind::PortalStudy,
                        record_id: &study.study_id,
                        title: &study.name,
                        source_id: &study.source_id,
                        reason: "The snapshot preserves an unknown public sample count; verify the upstream study metadata before using cohort-size context.",
                    },
                )?;
            }
        }
        for trial in &self.clinical_trials {
            if trial.last_update.is_none() {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::MissingClinicalTrialUpdate,
                        record_kind: RealDataRecordKind::ClinicalTrial,
                        record_id: &trial.nct_id,
                        title: &trial.title,
                        source_id: &trial.source_id,
                        reason: "The registry row has no supplied last-update date; verify the public record and retain the date as unknown until confirmed.",
                    },
                )?;
            }
        }
        for article in &self.literature {
            if article.abstract_text.is_none() {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::MissingLiteratureAbstract,
                        record_kind: RealDataRecordKind::LiteratureArticle,
                        record_id: &article.pmid,
                        title: &article.title,
                        source_id: &article.source_id,
                        reason: "The indexed citation has no abstract in the supplied snapshot; a reviewer must inspect the source before treating it as substantive evidence.",
                    },
                )?;
            } else if article.abstract_truncated {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::TruncatedLiteratureAbstract,
                        record_kind: RealDataRecordKind::LiteratureArticle,
                        record_id: &article.pmid,
                        title: &article.title,
                        source_id: &article.source_id,
                        reason: "The abstract was explicitly clipped at the ingestion bound; a reviewer must inspect the full public citation before relying on omitted text.",
                    },
                )?;
            }
            if !linked_pmids.contains(article.pmid.as_str()) {
                push_item(
                    &mut items,
                    &sources,
                    query,
                    ReviewCandidate {
                        kind: RealDataReviewKind::UnlinkedLiteratureCitation,
                        record_kind: RealDataRecordKind::LiteratureArticle,
                        record_id: &article.pmid,
                        title: &article.title,
                        source_id: &article.source_id,
                        reason: "The citation has no explicit cBioPortal-study PMID crosswalk in this bundle; review relevance and cohort identity rather than inferring a linkage.",
                    },
                )?;
            }
        }

        items.sort_by(|left, right| {
            (
                left.class,
                left.kind,
                left.source_id.as_str(),
                left.record_kind,
                left.record_id.as_str(),
            )
                .cmp(&(
                    right.class,
                    right.kind,
                    right.source_id.as_str(),
                    right.record_kind,
                    right.record_id.as_str(),
                ))
        });
        let candidate_item_count = items.len();
        let omitted_item_count = candidate_item_count.saturating_sub(query.max_items);
        items.truncate(query.max_items);
        let returned_item_count = items.len();
        let queue_digest = digest_queue(
            &summary.bundle_digest,
            query,
            candidate_item_count,
            omitted_item_count,
            &items,
        )?;
        Ok(RealDataReviewQueueReport {
            schema_version: REAL_DATA_REVIEW_QUEUE_SCHEMA_VERSION.to_string(),
            bundle_digest: summary.bundle_digest,
            queue_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            source_count: summary.source_count,
            record_count: summary.record_count,
            candidate_item_count,
            returned_item_count,
            omitted_item_count,
            truncated: omitted_item_count > 0,
            items,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "queue classes describe structural metadata obligations, not clinical urgency, evidence quality, diagnosis, prognosis, treatment, or triage".to_string(),
                "an absent link, date, abstract, or sample count remains unknown; the queue never imputes a value or infers a cohort relationship".to_string(),
                "titles and stable identifiers are copied only to help a reviewer locate the public record; abstracts, samples, and patient-level values are never copied".to_string(),
                "the queue is a caller-owned review handoff; it never fetches URLs, invokes a model, opens credentials, sends notifications, or writes durable state".to_string(),
            ],
        })
    }
}

fn validate_query(
    bundle: &RealGliomaBundle,
    query: &RealDataReviewQueueQuery,
) -> Result<(), NeurosurgeryError> {
    validate_query_shape(query)?;
    if let Some(source_id) = query.source_id.as_deref() {
        if !bundle
            .sources
            .iter()
            .any(|source| source.source_id == source_id)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("review queue source_id {source_id:?} is not in the bundle"),
            });
        }
    }
    Ok(())
}

fn validate_query_shape(query: &RealDataReviewQueueQuery) -> Result<(), NeurosurgeryError> {
    if query.max_items == 0 || query.max_items > MAX_REAL_DATA_REVIEW_ITEMS {
        return Err(NeurosurgeryError::TooMany {
            field: "real_data_review_queue.max_items",
            found: query.max_items,
            max: MAX_REAL_DATA_REVIEW_ITEMS,
        });
    }
    if let Some(source_id) = query.source_id.as_deref() {
        if source_id.is_empty() || source_id.len() > 512 || source_id.chars().any(char::is_control)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review queue source_id is empty, too long, or contains a control character".to_string(),
            });
        }
    }
    Ok(())
}

struct ReviewCandidate<'a> {
    kind: RealDataReviewKind,
    record_kind: RealDataRecordKind,
    record_id: &'a str,
    title: &'a str,
    source_id: &'a str,
    reason: &'a str,
}

fn push_item(
    items: &mut Vec<RealDataReviewItem>,
    sources: &BTreeMap<&str, &RealDataSource>,
    query: &RealDataReviewQueueQuery,
    candidate: ReviewCandidate<'_>,
) -> Result<(), NeurosurgeryError> {
    if query
        .record_kind
        .is_some_and(|selected| selected != candidate.record_kind)
        || query
            .source_id
            .as_deref()
            .is_some_and(|selected| selected != candidate.source_id)
    {
        return Ok(());
    }
    let source =
        sources
            .get(candidate.source_id)
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "review queue record {:?} points to unknown source {:?}",
                    candidate.record_id, candidate.source_id
                ),
            })?;
    items.push(RealDataReviewItem {
        task_id: format!(
            "real-review-{}-{}-{}",
            candidate.kind.slug(),
            candidate.record_kind.slug(),
            candidate.record_id
        ),
        class: candidate.kind.class(),
        kind: candidate.kind,
        status: RealDataReviewStatus::NeedsHumanReview,
        source_id: source.source_id.clone(),
        source_kind: source.kind,
        source_uri: source.uri.clone(),
        record_kind: candidate.record_kind,
        record_id: candidate.record_id.to_string(),
        title: candidate.title.to_string(),
        reason: candidate.reason.to_string(),
        reviewer_roles: vec![
            "neuro-oncology".to_string(),
            "biostatistics and data governance".to_string(),
        ],
    });
    Ok(())
}

fn digest_queue(
    bundle_digest: &str,
    query: &RealDataReviewQueueQuery,
    candidate_item_count: usize,
    omitted_item_count: usize,
    items: &[RealDataReviewItem],
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(
        bundle_digest,
        query,
        candidate_item_count,
        omitted_item_count,
        items,
    ))
    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
