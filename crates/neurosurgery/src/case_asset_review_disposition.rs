//! Replay-safe, caller-owned dispositions for case-asset review obligations.
//!
//! A disposition records only that a reviewer marked one emitted asset-metadata obligation as
//! reviewed, unresolved, or not applicable. It is bound to the exact projected manifest report
//! digest and never changes the manifest, source records, asset bytes, or any clinical meaning.

use crate::{CaseAssetManifestReport, NeurosurgeryError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const CASE_ASSET_REVIEW_DISPOSITION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-case-asset-review-disposition/0.1";
pub const MAX_CASE_ASSET_REVIEW_DISPOSITIONS: usize = 512;
const MAX_REVIEWER_ID_BYTES: usize = 128;

/// A human-owned state transition for one returned asset review item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseAssetReviewDisposition {
    Reviewed,
    Unresolved,
    NotApplicable,
}

/// One disposition addressed to the sequence number in an exact manifest projection. Sequence
/// numbers are used instead of asset IDs so the workflow never needs to echo caller-local labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetReviewDecision {
    pub sequence: u16,
    pub disposition: CaseAssetReviewDisposition,
    pub reviewer_id: String,
}

/// Canonical, sorted representation of one accepted disposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetReviewDispositionItem {
    pub sequence: u16,
    pub disposition: CaseAssetReviewDisposition,
    pub reviewer_id: String,
}

/// Digest-addressed result of applying a bounded set of human dispositions to one manifest
/// projection. The caller owns persistence and any later review cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetReviewDispositionReport {
    pub schema_version: String,
    pub report_digest: String,
    pub disposition_digest: String,
    pub candidate_item_count: usize,
    pub returned_item_count: usize,
    pub omitted_item_count: usize,
    pub submitted_decision_count: usize,
    pub accepted_decision_count: usize,
    pub resolved_decision_count: usize,
    pub unresolved_decision_count: usize,
    pub undecided_returned_item_count: usize,
    pub pending_item_count: usize,
    pub decisions: Vec<CaseAssetReviewDispositionItem>,
    pub unresolved_sequences: Vec<u16>,
    pub undecided_sequences: Vec<u16>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl CaseAssetReviewDispositionReport {
    /// Validate a persisted reviewer ledger before it is used as workflow state. This proves
    /// internal counts, sequence ownership, flags, and the canonical decision digest; it cannot
    /// prove that a human actually inspected an asset or that the upstream metadata is true.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != CASE_ASSET_REVIEW_DISPOSITION_SCHEMA_VERSION
            || !is_lower_hex_digest(&self.report_digest)
            || !is_lower_hex_digest(&self.disposition_digest)
            || self
                .returned_item_count
                .checked_add(self.omitted_item_count)
                != Some(self.candidate_item_count)
            || self.returned_item_count > 512
            || self.submitted_decision_count != self.decisions.len()
            || self.accepted_decision_count != self.decisions.len()
            || self
                .resolved_decision_count
                .checked_add(self.unresolved_decision_count)
                != Some(self.decisions.len())
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations != default_limitations()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition envelope is invalid".to_string(),
            });
        }

        let mut seen = BTreeSet::new();
        let mut unresolved = Vec::new();
        for decision in &self.decisions {
            if decision.sequence == 0
                || decision.sequence as usize > self.returned_item_count
                || !seen.insert(decision.sequence)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "case-asset review disposition sequence set is invalid".to_string(),
                });
            }
            validate_reviewer_id(&decision.reviewer_id)?;
            if decision.disposition == CaseAssetReviewDisposition::Unresolved {
                unresolved.push(decision.sequence);
            }
        }
        if self.decisions.windows(2).any(|window| {
            (
                window[0].sequence,
                window[0].disposition,
                window[0].reviewer_id.as_str(),
            ) > (
                window[1].sequence,
                window[1].disposition,
                window[1].reviewer_id.as_str(),
            )
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition decisions are not canonical".to_string(),
            });
        }
        let expected_undecided = (1..=self.returned_item_count as u16)
            .filter(|sequence| !seen.contains(sequence))
            .collect::<Vec<_>>();
        let expected_pending = self
            .omitted_item_count
            .checked_add(self.unresolved_decision_count)
            .and_then(|count| count.checked_add(self.undecided_returned_item_count));
        if unresolved != self.unresolved_sequences
            || self.unresolved_decision_count != unresolved.len()
            || self.undecided_sequences != expected_undecided
            || self.undecided_returned_item_count != expected_undecided.len()
            || expected_pending != Some(self.pending_item_count)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition counts or pending set is invalid"
                    .to_string(),
            });
        }
        if digest_dispositions(&self.report_digest, &self.decisions)? != self.disposition_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition digest does not match its contents"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl CaseAssetManifestReport {
    /// Apply a bounded, deterministic set of reviewer dispositions to the returned review items.
    /// Replaying the same decisions in another order yields the same digest; unknown, duplicate,
    /// or omitted sequence numbers fail closed.
    pub fn apply_review_dispositions(
        &self,
        decisions: &[CaseAssetReviewDecision],
    ) -> Result<CaseAssetReviewDispositionReport, NeurosurgeryError> {
        self.validate_integrity()?;
        if decisions.len() > MAX_CASE_ASSET_REVIEW_DISPOSITIONS {
            return Err(NeurosurgeryError::TooMany {
                field: "case_asset_review_disposition.decisions",
                found: decisions.len(),
                max: MAX_CASE_ASSET_REVIEW_DISPOSITIONS,
            });
        }

        let known = self
            .review_items
            .iter()
            .map(|item| item.sequence)
            .collect::<BTreeSet<_>>();
        let mut normalized = decisions.to_vec();
        normalized.sort_by(|left, right| {
            (left.sequence, left.disposition, left.reviewer_id.as_str()).cmp(&(
                right.sequence,
                right.disposition,
                right.reviewer_id.as_str(),
            ))
        });
        let mut seen = BTreeSet::new();
        for decision in &normalized {
            validate_decision(decision)?;
            if !known.contains(&decision.sequence) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "case-asset review disposition sequence {} is not emitted by the report",
                        decision.sequence
                    ),
                });
            }
            if !seen.insert(decision.sequence) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "case-asset review disposition repeats sequence {}",
                        decision.sequence
                    ),
                });
            }
        }

        let decisions = normalized
            .into_iter()
            .map(|decision| CaseAssetReviewDispositionItem {
                sequence: decision.sequence,
                disposition: decision.disposition,
                reviewer_id: decision.reviewer_id,
            })
            .collect::<Vec<_>>();
        let resolved_decision_count = decisions
            .iter()
            .filter(|decision| {
                matches!(
                    decision.disposition,
                    CaseAssetReviewDisposition::Reviewed
                        | CaseAssetReviewDisposition::NotApplicable
                )
            })
            .count();
        let unresolved_sequences = decisions
            .iter()
            .filter(|decision| decision.disposition == CaseAssetReviewDisposition::Unresolved)
            .map(|decision| decision.sequence)
            .collect::<Vec<_>>();
        let undecided_sequences = self
            .review_items
            .iter()
            .filter(|item| !seen.contains(&item.sequence))
            .map(|item| item.sequence)
            .collect::<Vec<_>>();
        let unresolved_decision_count = unresolved_sequences.len();
        let disposition_digest = digest_dispositions(&self.report_digest, &decisions)?;
        let candidate_item_count = self
            .review_items
            .len()
            .checked_add(self.omitted_review_item_count)
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: "case-asset review candidate count overflows its bound".to_string(),
            })?;
        let pending_item_count = self
            .omitted_review_item_count
            .checked_add(unresolved_sequences.len())
            .and_then(|count| count.checked_add(undecided_sequences.len()))
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: "case-asset review pending count overflows its bound".to_string(),
            })?;
        let report = CaseAssetReviewDispositionReport {
            schema_version: CASE_ASSET_REVIEW_DISPOSITION_SCHEMA_VERSION.to_string(),
            report_digest: self.report_digest.clone(),
            disposition_digest,
            candidate_item_count,
            returned_item_count: self.review_items.len(),
            omitted_item_count: self.omitted_review_item_count,
            submitted_decision_count: decisions.len(),
            accepted_decision_count: decisions.len(),
            resolved_decision_count,
            unresolved_decision_count,
            undecided_returned_item_count: undecided_sequences.len(),
            pending_item_count,
            decisions,
            unresolved_sequences,
            undecided_sequences,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: default_limitations(),
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_decision(decision: &CaseAssetReviewDecision) -> Result<(), NeurosurgeryError> {
    if decision.sequence == 0 {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "case-asset review disposition sequence must be positive".to_string(),
        });
    }
    validate_reviewer_id(&decision.reviewer_id)
}

fn validate_reviewer_id(reviewer_id: &str) -> Result<(), NeurosurgeryError> {
    if reviewer_id.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField {
            field: "case_asset_review_disposition.reviewer_id",
        });
    }
    if reviewer_id.len() > MAX_REVIEWER_ID_BYTES {
        return Err(NeurosurgeryError::FieldTooLong {
            field: "case_asset_review_disposition.reviewer_id",
            max: MAX_REVIEWER_ID_BYTES,
        });
    }
    if reviewer_id.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter {
            field: "case_asset_review_disposition.reviewer_id",
        });
    }
    Ok(())
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn default_limitations() -> Vec<String> {
    vec![
        "dispositions are caller-supplied workflow metadata, not proof that an asset or source is correct and not a clinical conclusion".to_string(),
        "reviewed and not_applicable do not change the manifest, fill missing provenance, or open asset bytes; unresolved, undecided, and omitted items remain pending".to_string(),
        "the envelope is caller-owned and stateless; it never stores raw asset IDs, fetches a URL, invokes a model, opens credentials, sends notifications, or writes external state".to_string(),
    ]
}

fn digest_dispositions(
    report_digest: &str,
    decisions: &[CaseAssetReviewDispositionItem],
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(report_digest, decisions))
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
