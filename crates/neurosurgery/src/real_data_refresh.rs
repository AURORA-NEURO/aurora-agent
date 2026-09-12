//! Deterministic refresh reconciliation for validated real-glioma snapshots.
//!
//! A public-data snapshot is useful only when a worker can tell what changed, what remains
//! incomplete, and what a reviewer should inspect next. This module composes the existing
//! structural diff, coverage audit, freshness posture, metadata review queue, and topic brief
//! into one digest-bound report. It never fetches, merges, ranks, or silently promotes records;
//! the caller still owns acquisition and human disposition of every change.

use crate::{
    NeurosurgeryError, NeurosurgicalResearchBriefQuery, NeurosurgicalResearchBriefReport,
    RealDataCoverageQuery, RealDataCoverageReport, RealDataDiffQuery, RealDataDiffReport,
    RealDataFreshnessReport, RealDataReviewQueueQuery, RealDataReviewQueueReport, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REAL_DATA_REFRESH_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-refresh-audit/0.1";

/// Bounded composition of the existing snapshot-audit queries. The nested reports retain their
/// own bounds and digests so a refresh audit can be replayed without hidden worker state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataRefreshAuditQuery {
    #[serde(default)]
    pub diff: RealDataDiffQuery,
    #[serde(default)]
    pub coverage: RealDataCoverageQuery,
    #[serde(default)]
    pub review_queue: RealDataReviewQueueQuery,
    #[serde(default)]
    pub brief: NeurosurgicalResearchBriefQuery,
}

/// One reason the candidate snapshot needs explicit reviewer attention. Counts are structural
/// observations, not severity, quality, urgency, or clinical-risk scores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataRefreshReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Digest-bound reconciliation of a previous snapshot and a candidate refresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataRefreshAuditReport {
    pub schema_version: String,
    pub audit_digest: String,
    pub before_bundle_digest: String,
    pub after_bundle_digest: String,
    pub before_generated_at: String,
    pub after_generated_at: String,
    pub query: RealDataRefreshAuditQuery,
    pub diff: RealDataDiffReport,
    pub coverage: RealDataCoverageReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessReport>,
    pub review_queue: RealDataReviewQueueReport,
    pub research_brief: NeurosurgicalResearchBriefReport,
    pub structural_change_detected: bool,
    pub source_identity_stable: bool,
    pub record_identity_stable: bool,
    pub requires_refresh_review: bool,
    pub review_reasons: Vec<RealDataRefreshReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataRefreshAuditReport {
    /// Validate a persisted refresh reconciliation without accepting or fetching a candidate.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_REFRESH_AUDIT_SCHEMA_VERSION
            || !is_sha256_hex(&self.audit_digest)
            || !is_sha256_hex(&self.before_bundle_digest)
            || !is_sha256_hex(&self.after_bundle_digest)
            || !crate::temporal::is_utc_timestamp(&self.before_generated_at)
            || !crate::temporal::is_utc_timestamp(&self.after_generated_at)
            || self.diff.before_bundle_digest != self.before_bundle_digest
            || self.diff.after_bundle_digest != self.after_bundle_digest
            || self.diff.validate_integrity().is_err()
            || self.coverage.bundle_digest != self.after_bundle_digest
            || self.coverage.validate_integrity().is_err()
            || self.review_queue.bundle_digest != self.after_bundle_digest
            || self.review_queue.validate_integrity().is_err()
            || self.research_brief.bundle_digest != self.after_bundle_digest
            || self.research_brief.validate_integrity().is_err()
            || self.freshness.as_ref().is_some_and(|freshness| {
                freshness.bundle_digest != self.after_bundle_digest
                    || freshness.validate_integrity().is_err()
            })
            || self.structural_change_detected != (self.diff.total_change_count > 0)
            || self.source_identity_stable
                != (self.diff.source_counts.added == 0 && self.diff.source_counts.removed == 0)
            || self.record_identity_stable
                != (self.diff.record_counts.added == 0 && self.diff.record_counts.removed == 0)
            || self.requires_refresh_review != !self.review_reasons.is_empty()
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data refresh audit envelope is invalid".to_string(),
            });
        }
        if self.audit_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data refresh audit digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild a refresh reconciliation from exact before/after snapshots and request inputs.
    pub fn validate_for_inputs(
        &self,
        before: &RealGliomaBundle,
        after: &RealGliomaBundle,
        request: &crate::CaseRequest,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = before.refresh_audit(after, &self.query, request)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data refresh audit does not replay to the exact supplied snapshots"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Reconcile two validated public snapshots and retain every component report. `self` is the
    /// before snapshot; `after` is the candidate refresh. No source text or patient-level value is
    /// copied beyond the nested reports' already-bounded metadata projections.
    pub fn refresh_audit(
        &self,
        after: &RealGliomaBundle,
        query: &RealDataRefreshAuditQuery,
        request: &crate::CaseRequest,
    ) -> Result<RealDataRefreshAuditReport, NeurosurgeryError> {
        self.validate()?;
        after.validate()?;
        let diff = self.diff(after, &query.diff)?;
        let coverage = after.coverage_report(&query.coverage)?;
        let freshness = query
            .brief
            .freshness
            .as_ref()
            .map(|freshness| after.freshness_report(freshness))
            .transpose()?;
        let review_queue = after.review_queue(&query.review_queue)?;
        let research_brief = after.research_brief(request, &query.brief)?;
        let mut review_reasons = Vec::new();
        if diff.total_change_count > 0 {
            review_reasons.push(RealDataRefreshReviewReason {
                code: "structural_changes".to_string(),
                count: diff.total_change_count,
                detail: "the candidate snapshot differs in public record or source metadata; inspect the bounded diff before accepting the refresh".to_string(),
            });
        }
        if diff.truncated {
            review_reasons.push(RealDataRefreshReviewReason {
                code: "diff_truncated".to_string(),
                count: diff.omitted_record_change_count + diff.omitted_source_change_count,
                detail: "the diff bound omitted changes; the returned rows are not an exhaustive refresh review".to_string(),
            });
        }
        if review_queue.candidate_item_count > 0 {
            review_reasons.push(RealDataRefreshReviewReason {
                code: "metadata_review_obligations".to_string(),
                count: review_queue.candidate_item_count,
                detail: "the candidate snapshot contains explicit provenance, completeness, or context obligations from the metadata review queue".to_string(),
            });
        }
        if !research_brief.unknowns.is_empty() {
            review_reasons.push(RealDataRefreshReviewReason {
                code: "brief_unknowns".to_string(),
                count: research_brief.unknowns.len(),
                detail: "the source-linked topic brief contains empty, truncated, or otherwise unresolved lanes".to_string(),
            });
        }
        if let Some(freshness) = freshness.as_ref() {
            let count = freshness.stale_source_count + freshness.future_dated_source_count;
            if count > 0 {
                review_reasons.push(RealDataRefreshReviewReason {
                    code: "freshness_review".to_string(),
                    count,
                    detail: "the explicit caller clock marks one or more candidate sources stale or future-dated; age is not an evidence-quality score".to_string(),
                });
            }
        }
        let structural_change_detected = diff.total_change_count > 0;
        let source_identity_stable =
            diff.source_counts.added == 0 && diff.source_counts.removed == 0;
        let record_identity_stable =
            diff.record_counts.added == 0 && diff.record_counts.removed == 0;
        let mut report = RealDataRefreshAuditReport {
            schema_version: REAL_DATA_REFRESH_AUDIT_SCHEMA_VERSION.to_string(),
            audit_digest: String::new(),
            before_bundle_digest: diff.before_bundle_digest.clone(),
            after_bundle_digest: diff.after_bundle_digest.clone(),
            before_generated_at: diff.before_generated_at.clone(),
            after_generated_at: diff.after_generated_at.clone(),
            query: query.clone(),
            diff,
            coverage,
            freshness,
            review_queue,
            research_brief,
            structural_change_detected,
            source_identity_stable,
            record_identity_stable,
            requires_refresh_review: !review_reasons.is_empty(),
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the audit compares validated public snapshots; it never fetches, merges, or writes the candidate bundle".to_string(),
                "structural changes, freshness states, metadata obligations, and lexical brief unknowns are review signals, not clinical findings or evidence-quality scores".to_string(),
                "source and record identity stability only describe public identifiers; they do not establish biological continuity, cohort comparability, or applicability".to_string(),
                "population records remain separate from caller-supplied observations and cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.audit_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn digest_report(report: &RealDataRefreshAuditReport) -> Result<String, NeurosurgeryError> {
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
