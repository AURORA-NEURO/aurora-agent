//! Replay-safe, caller-owned dispositions for the real-data review queue.
//!
//! A disposition records only that a human has marked a metadata obligation as reviewed,
//! unresolved, or not applicable. It is bound to the exact queue digest and never changes the
//! underlying public snapshot. This keeps workflow state resumable without adding hidden storage,
//! provider access, patient data, or a clinical decision layer.

use crate::{NeurosurgeryError, RealDataReviewQueueReport, RealDataReviewStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_REVIEW_DISPOSITION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-review-disposition/0.1";
pub const MAX_REAL_DATA_REVIEW_DISPOSITIONS: usize = 256;
const MAX_REVIEWER_ID_BYTES: usize = 128;

/// A human-owned state transition for one emitted review task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataReviewDisposition {
    Reviewed,
    Unresolved,
    NotApplicable,
}

/// One disposition submitted against an exact queue item. Reviewer identity is metadata only;
/// no credential, note, patient identifier, or clinical instruction is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewDecision {
    pub task_id: String,
    pub disposition: RealDataReviewDisposition,
    pub reviewer_id: String,
}

/// Canonical, sorted representation of one accepted disposition in the returned envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewDispositionItem {
    pub task_id: String,
    pub disposition: RealDataReviewDisposition,
    pub reviewer_id: String,
}

/// Wire request for applying caller-owned dispositions to one queue projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewDispositionRequest {
    pub queue: RealDataReviewQueueReport,
    #[serde(default)]
    pub decisions: Vec<RealDataReviewDecision>,
}

/// Digest-addressed result of applying a bounded set of dispositions. The report is still a
/// handoff envelope: the caller is responsible for durable storage and any later re-review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataReviewDispositionReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub queue_digest: String,
    pub disposition_digest: String,
    pub candidate_item_count: usize,
    pub queue_returned_item_count: usize,
    pub queue_omitted_item_count: usize,
    pub submitted_decision_count: usize,
    pub accepted_decision_count: usize,
    pub resolved_decision_count: usize,
    pub unresolved_decision_count: usize,
    pub undecided_returned_item_count: usize,
    pub pending_item_count: usize,
    pub decisions: Vec<RealDataReviewDispositionItem>,
    pub unresolved_task_ids: Vec<String>,
    pub undecided_task_ids: Vec<String>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataReviewDispositionReport {
    /// Validate a persisted disposition envelope before it is replayed into another workflow.
    /// This checks the queue binding, deterministic counts, decision identities, and digest
    /// without changing the underlying queue or source snapshot.
    pub fn validate_integrity(
        &self,
        queue: &RealDataReviewQueueReport,
    ) -> Result<(), NeurosurgeryError> {
        queue.validate_integrity()?;
        if self.schema_version != REAL_DATA_REVIEW_DISPOSITION_SCHEMA_VERSION
            || self.bundle_digest != queue.bundle_digest
            || self.queue_digest != queue.queue_digest
            || self.candidate_item_count != queue.candidate_item_count
            || self.queue_returned_item_count != queue.returned_item_count
            || self.queue_omitted_item_count != queue.omitted_item_count
            || self.submitted_decision_count != self.decisions.len()
            || self.accepted_decision_count != self.decisions.len()
            || self.resolved_decision_count
                != self
                    .decisions
                    .iter()
                    .filter(|decision| {
                        matches!(
                            decision.disposition,
                            RealDataReviewDisposition::Reviewed
                                | RealDataReviewDisposition::NotApplicable
                        )
                    })
                    .count()
            || self.unresolved_decision_count
                != self
                    .decisions
                    .iter()
                    .filter(|decision| {
                        decision.disposition == RealDataReviewDisposition::Unresolved
                    })
                    .count()
            || self.pending_item_count
                != self.queue_omitted_item_count
                    + self.unresolved_task_ids.len()
                    + self.undecided_task_ids.len()
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review disposition envelope invariants are invalid".to_string(),
            });
        }
        let known = queue
            .items
            .iter()
            .map(|item| item.task_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut decision_ids = BTreeSet::new();
        let mut canonical_decisions = self.decisions.clone();
        canonical_decisions.sort_by(|left, right| {
            (
                left.task_id.as_str(),
                left.disposition,
                left.reviewer_id.as_str(),
            )
                .cmp(&(
                    right.task_id.as_str(),
                    right.disposition,
                    right.reviewer_id.as_str(),
                ))
        });
        if canonical_decisions != self.decisions {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review disposition decisions are not in canonical order"
                    .to_string(),
            });
        }
        for decision in &self.decisions {
            validate_decision(&RealDataReviewDecision {
                task_id: decision.task_id.clone(),
                disposition: decision.disposition,
                reviewer_id: decision.reviewer_id.clone(),
            })?;
            if !known.contains(decision.task_id.as_str())
                || !decision_ids.insert(decision.task_id.as_str())
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "real-data review disposition references an unknown or duplicate task_id"
                            .to_string(),
                });
            }
        }
        let undecided = queue
            .items
            .iter()
            .filter(|item| !decision_ids.contains(item.task_id.as_str()))
            .map(|item| item.task_id.clone())
            .collect::<Vec<_>>();
        let unresolved = self
            .decisions
            .iter()
            .filter(|decision| decision.disposition == RealDataReviewDisposition::Unresolved)
            .map(|decision| decision.task_id.clone())
            .collect::<Vec<_>>();
        if self.undecided_task_ids != undecided || self.unresolved_task_ids != unresolved {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review disposition task lists are inconsistent".to_string(),
            });
        }
        let expected = digest_dispositions(&self.queue_digest, &self.decisions)?;
        if expected != self.disposition_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review disposition digest does not match its decisions"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl RealDataReviewQueueReport {
    /// Validate the queue envelope and its deterministic digest before it is used as workflow
    /// state. This cannot prove that an upstream source was correct; it proves only that the
    /// serialized queue projection has not been altered since it was produced.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != crate::REAL_DATA_REVIEW_QUEUE_SCHEMA_VERSION
            || self.bundle_digest.len() != 64
            || self.queue_digest.len() != 64
            || !is_lower_hex(&self.bundle_digest)
            || !is_lower_hex(&self.queue_digest)
            || !crate::temporal::is_utc_timestamp(&self.generated_at)
            || self.source_count == 0
            || self.record_count == 0
            || self.query.max_items == 0
            || self.query.max_items > crate::MAX_REAL_DATA_REVIEW_ITEMS
            || self.query.source_id.as_deref().is_some_and(|source_id| {
                source_id.is_empty()
                    || source_id.len() > 512
                    || source_id.chars().any(char::is_control)
            })
            || self.returned_item_count != self.items.len()
            || self.candidate_item_count < self.returned_item_count
            || self.omitted_item_count
                != self
                    .candidate_item_count
                    .saturating_sub(self.returned_item_count)
            || self.truncated != (self.omitted_item_count > 0)
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review queue envelope invariants are invalid".to_string(),
            });
        }
        let mut task_ids = BTreeSet::new();
        for item in &self.items {
            if item.task_id.is_empty()
                || item.task_id.len() > 512
                || item.task_id.chars().any(char::is_control)
                || !task_ids.insert(item.task_id.as_str())
                || item.class != item.kind.class()
                || item.source_id.trim().is_empty()
                || item.source_id.len() > 512
                || item.source_id.chars().any(char::is_control)
                || self
                    .query
                    .source_id
                    .as_deref()
                    .is_some_and(|source_id| source_id != item.source_id)
                || !crate::real_data::is_allow_listed_uri(&item.source_uri)
                || !crate::real_data::source_kind_matches_uri(item.source_kind, &item.source_uri)
                || item.record_id.trim().is_empty()
                || item.record_id.len() > 512
                || item.record_id.chars().any(char::is_control)
                || item.title.trim().is_empty()
                || item.title.len() > 4_096
                || item.title.chars().any(char::is_control)
                || item.reason.trim().is_empty()
                || item.reason.len() > 4_096
                || item.reason.chars().any(char::is_control)
                || item.status != RealDataReviewStatus::NeedsHumanReview
                || item.reviewer_roles.is_empty()
                || item.reviewer_roles.iter().any(|role| {
                    role.trim().is_empty() || role.len() > 256 || role.chars().any(char::is_control)
                })
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "real-data review queue contains an empty, invalid, or duplicate task_id"
                            .to_string(),
                });
            }
        }
        if self.items.windows(2).any(|window| {
            (
                window[0].class,
                window[0].kind,
                window[0].source_id.as_str(),
                window[0].record_kind,
                window[0].record_id.as_str(),
            ) >= (
                window[1].class,
                window[1].kind,
                window[1].source_id.as_str(),
                window[1].record_kind,
                window[1].record_id.as_str(),
            )
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review queue items are not in canonical order".to_string(),
            });
        }
        let expected = digest_queue(
            &self.bundle_digest,
            &self.query,
            self.candidate_item_count,
            self.omitted_item_count,
            &self.items,
        )?;
        if expected != self.queue_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data review queue digest does not match its serialized projection"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Apply a bounded, deterministic set of human dispositions to the emitted queue items.
    /// Decisions are sorted before digesting, so replaying the same set in another order yields
    /// the same disposition digest. Unknown and duplicate task ids fail closed.
    pub fn apply_dispositions(
        &self,
        decisions: &[RealDataReviewDecision],
    ) -> Result<RealDataReviewDispositionReport, NeurosurgeryError> {
        self.validate_integrity()?;
        if decisions.len() > MAX_REAL_DATA_REVIEW_DISPOSITIONS {
            return Err(NeurosurgeryError::TooMany {
                field: "real_data_review_disposition.decisions",
                found: decisions.len(),
                max: MAX_REAL_DATA_REVIEW_DISPOSITIONS,
            });
        }
        let known = self
            .items
            .iter()
            .map(|item| (item.task_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::<String>::new();
        let mut normalized = decisions.to_vec();
        normalized.sort_by(|left, right| {
            (
                left.task_id.as_str(),
                left.disposition,
                left.reviewer_id.as_str(),
            )
                .cmp(&(
                    right.task_id.as_str(),
                    right.disposition,
                    right.reviewer_id.as_str(),
                ))
        });
        for decision in &normalized {
            validate_decision(decision)?;
            if !seen.insert(decision.task_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "real-data review disposition repeats task_id {:?}",
                        decision.task_id
                    ),
                });
            }
            if !known.contains_key(decision.task_id.as_str()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "real-data review disposition task_id {:?} is not emitted by the queue",
                        decision.task_id
                    ),
                });
            }
        }

        let decisions = normalized
            .into_iter()
            .map(|decision| RealDataReviewDispositionItem {
                task_id: decision.task_id,
                disposition: decision.disposition,
                reviewer_id: decision.reviewer_id,
            })
            .collect::<Vec<_>>();
        let resolved_decision_count = decisions
            .iter()
            .filter(|decision| {
                matches!(
                    decision.disposition,
                    RealDataReviewDisposition::Reviewed | RealDataReviewDisposition::NotApplicable
                )
            })
            .count();
        let unresolved_decision_count = decisions
            .iter()
            .filter(|decision| decision.disposition == RealDataReviewDisposition::Unresolved)
            .count();
        let undecided_task_ids = self
            .items
            .iter()
            .filter(|item| !seen.contains(&item.task_id))
            .map(|item| item.task_id.clone())
            .collect::<Vec<_>>();
        let unresolved_task_ids = decisions
            .iter()
            .filter(|decision| decision.disposition == RealDataReviewDisposition::Unresolved)
            .map(|decision| decision.task_id.clone())
            .collect::<Vec<_>>();
        let pending_item_count =
            self.omitted_item_count + unresolved_task_ids.len() + undecided_task_ids.len();
        let disposition_digest = digest_dispositions(&self.queue_digest, &decisions)?;
        Ok(RealDataReviewDispositionReport {
            schema_version: REAL_DATA_REVIEW_DISPOSITION_SCHEMA_VERSION.to_string(),
            bundle_digest: self.bundle_digest.clone(),
            queue_digest: self.queue_digest.clone(),
            disposition_digest,
            candidate_item_count: self.candidate_item_count,
            queue_returned_item_count: self.returned_item_count,
            queue_omitted_item_count: self.omitted_item_count,
            submitted_decision_count: decisions.len(),
            accepted_decision_count: decisions.len(),
            resolved_decision_count,
            unresolved_decision_count,
            undecided_returned_item_count: undecided_task_ids.len(),
            pending_item_count,
            decisions,
            unresolved_task_ids,
            undecided_task_ids,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "dispositions are caller-supplied workflow metadata, not proof that a source is correct and not a clinical conclusion".to_string(),
                "reviewed and not_applicable do not fill missing source fields or change the underlying real-data snapshot; unresolved and omitted tasks remain pending".to_string(),
                "the envelope is caller-owned and stateless; it never stores a task, fetches a URL, invokes a model, opens credentials, sends notifications, or writes external state".to_string(),
            ],
        })
    }
}

fn validate_decision(decision: &RealDataReviewDecision) -> Result<(), NeurosurgeryError> {
    if decision.task_id.is_empty()
        || decision.task_id.len() > 512
        || decision.task_id.chars().any(char::is_control)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real-data review disposition task_id is empty, too long, or contains a control character".to_string(),
        });
    }
    if decision.reviewer_id.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField {
            field: "real_data_review_disposition.reviewer_id",
        });
    }
    if decision.reviewer_id.len() > MAX_REVIEWER_ID_BYTES {
        return Err(NeurosurgeryError::FieldTooLong {
            field: "real_data_review_disposition.reviewer_id",
            max: MAX_REVIEWER_ID_BYTES,
        });
    }
    if decision.reviewer_id.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter {
            field: "real_data_review_disposition.reviewer_id",
        });
    }
    Ok(())
}

fn digest_queue(
    bundle_digest: &str,
    query: &crate::RealDataReviewQueueQuery,
    candidate_item_count: usize,
    omitted_item_count: usize,
    items: &[crate::RealDataReviewItem],
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(
        bundle_digest,
        query,
        candidate_item_count,
        omitted_item_count,
        items,
    ))
    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn digest_dispositions(
    queue_digest: &str,
    decisions: &[RealDataReviewDispositionItem],
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(queue_digest, decisions))
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_lower_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
