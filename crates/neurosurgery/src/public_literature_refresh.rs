//! Deterministic refresh reconciliation for the cross-specialty PubMed snapshot.
//!
//! The literature refresh script is an acquisition boundary, while this module is the local
//! decision boundary after acquisition. It compares two already validated snapshots, preserves
//! PMID/source identity facts, composes the bounded lane matrix and optional freshness posture,
//! and emits reviewer obligations without copying abstracts or making a clinical claim. No
//! network, provider, credential, merge, or promotion is reachable from this API.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureMatrixQuery,
    PublicLiteratureMatrixReport, PublicLiteratureRecord, PublicLiteratureSource,
    PublicLiteratureSummary, RealDataFreshnessQuery, RealDataFreshnessReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const PUBLIC_LITERATURE_REFRESH_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-refresh-audit/0.1";
pub const PUBLIC_LITERATURE_REFRESH_DIFF_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-refresh-diff/0.1";
const MAX_SOURCE_CHANGES: usize = 128;
const MAX_RECORD_CHANGES: usize = 512;
const DEFAULT_MAX_SOURCE_CHANGES: usize = 64;
const DEFAULT_MAX_RECORD_CHANGES: usize = 256;

fn default_max_source_changes() -> usize {
    DEFAULT_MAX_SOURCE_CHANGES
}

fn default_max_record_changes() -> usize {
    DEFAULT_MAX_RECORD_CHANGES
}

/// Bounded composition controls for a before/after public-literature reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRefreshAuditQuery {
    #[serde(default)]
    pub matrix: PublicLiteratureMatrixQuery,
    /// Optional caller-owned UTC clock for source-age review.
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
    #[serde(default = "default_max_source_changes")]
    pub max_source_changes: usize,
    #[serde(default = "default_max_record_changes")]
    pub max_record_changes: usize,
}

impl Default for PublicLiteratureRefreshAuditQuery {
    fn default() -> Self {
        Self {
            matrix: PublicLiteratureMatrixQuery::default(),
            freshness: None,
            max_source_changes: default_max_source_changes(),
            max_record_changes: default_max_record_changes(),
        }
    }
}

/// Number of added, removed, and changed identities in one refresh projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRefreshCounts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
}

impl PublicLiteratureRefreshCounts {
    fn total(&self) -> usize {
        self.added + self.removed + self.changed
    }
}

/// A source-level metadata change. Content is represented only by field names and identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureSourceChange {
    pub source_id: String,
    pub changed_fields: Vec<String>,
}

/// A PMID-level change. Abstracts and titles are intentionally not copied into the diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRecordChange {
    pub pmid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_specialty: Option<crate::Specialty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_specialty: Option<crate::Specialty>,
    pub changed_fields: Vec<String>,
}

/// Digest-bound structural diff of two validated cross-specialty snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRefreshDiffReport {
    pub schema_version: String,
    pub diff_digest: String,
    pub before_bundle_digest: String,
    pub after_bundle_digest: String,
    pub before_generated_at: String,
    pub after_generated_at: String,
    pub source_counts: PublicLiteratureRefreshCounts,
    pub record_counts: PublicLiteratureRefreshCounts,
    pub source_changes: Vec<PublicLiteratureSourceChange>,
    pub record_changes: Vec<PublicLiteratureRecordChange>,
    pub omitted_source_change_count: usize,
    pub omitted_record_change_count: usize,
    pub truncated: bool,
    pub source_identity_stable: bool,
    pub record_identity_stable: bool,
}

/// Why a literature candidate must remain in explicit human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRefreshReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Complete provider-free reconciliation for a cross-specialty literature refresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRefreshAuditReport {
    pub schema_version: String,
    pub audit_digest: String,
    pub before_bundle_digest: String,
    pub after_bundle_digest: String,
    pub before_generated_at: String,
    pub after_generated_at: String,
    pub query: PublicLiteratureRefreshAuditQuery,
    pub before_summary: PublicLiteratureSummary,
    pub after_summary: PublicLiteratureSummary,
    pub diff: PublicLiteratureRefreshDiffReport,
    pub matrix: PublicLiteratureMatrixReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessReport>,
    pub structural_change_detected: bool,
    pub specialty_coverage_changed: bool,
    pub source_identity_stable: bool,
    pub record_identity_stable: bool,
    pub requires_refresh_review: bool,
    pub review_reasons: Vec<PublicLiteratureRefreshReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureBundle {
    /// Reconcile two validated snapshots without fetching, merging, accepting, or promoting the
    /// candidate. `self` is the before snapshot and `after` is the candidate refresh.
    pub fn refresh_audit(
        &self,
        after: &PublicLiteratureBundle,
        query: &PublicLiteratureRefreshAuditQuery,
    ) -> Result<PublicLiteratureRefreshAuditReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        after.validate()?;
        let before_summary = self.summary()?;
        let after_summary = after.summary()?;
        let diff = build_diff(self, after, query)?;
        let matrix = after.literature_matrix(&query.matrix)?;
        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness| after.freshness_report(freshness))
            .transpose()?;

        let specialty_coverage_changed =
            before_summary.specialty_counts != after_summary.specialty_counts;
        let mut review_reasons = Vec::new();
        if diff.source_counts.total() + diff.record_counts.total() > 0 {
            review_reasons.push(PublicLiteratureRefreshReviewReason {
                code: "structural_changes".to_string(),
                count: diff.source_counts.total() + diff.record_counts.total(),
                detail: "the candidate differs in public source metadata or PMID metadata; inspect the bounded diff before accepting the refresh".to_string(),
            });
        }
        if diff.truncated {
            review_reasons.push(PublicLiteratureRefreshReviewReason {
                code: "diff_truncated".to_string(),
                count: diff.omitted_source_change_count + diff.omitted_record_change_count,
                detail: "the diff bound omitted changes; returned rows are not an exhaustive refresh review".to_string(),
            });
        }
        if specialty_coverage_changed {
            review_reasons.push(PublicLiteratureRefreshReviewReason {
                code: "specialty_coverage_changed".to_string(),
                count: 1,
                detail: "the candidate changes PMID counts in at least one retrieval lane; verify lane scope rather than treating the change as a biological trend".to_string(),
            });
        }
        if !matrix.empty_lane_specialties.is_empty() {
            review_reasons.push(PublicLiteratureRefreshReviewReason {
                code: "empty_specialty_lanes".to_string(),
                count: matrix.empty_lane_specialties.len(),
                detail: "the bounded lane matrix found no matching records in one or more requested specialties".to_string(),
            });
        }
        if matrix.truncated_lane_count > 0 {
            review_reasons.push(PublicLiteratureRefreshReviewReason {
                code: "matrix_truncated".to_string(),
                count: matrix.truncated_lane_count,
                detail: "one or more specialty lanes exceeded the query limit; the matrix is not a complete corpus view".to_string(),
            });
        }
        if let Some(freshness) = freshness.as_ref() {
            let count = freshness.stale_source_count + freshness.future_dated_source_count;
            if count > 0 {
                review_reasons.push(PublicLiteratureRefreshReviewReason {
                    code: "freshness_review".to_string(),
                    count,
                    detail: "the explicit caller clock marks one or more candidate PubMed sources stale or future-dated; age is not an evidence-quality score".to_string(),
                });
            }
        }

        let structural_change_detected =
            diff.source_counts.total() + diff.record_counts.total() > 0;
        let source_identity_stable = diff.source_identity_stable;
        let record_identity_stable = diff.record_identity_stable;
        let mut report = PublicLiteratureRefreshAuditReport {
            schema_version: PUBLIC_LITERATURE_REFRESH_AUDIT_SCHEMA_VERSION.to_string(),
            audit_digest: String::new(),
            before_bundle_digest: before_summary.bundle_digest.clone(),
            after_bundle_digest: after_summary.bundle_digest.clone(),
            before_generated_at: self.generated_at.clone(),
            after_generated_at: after.generated_at.clone(),
            query: query.clone(),
            before_summary,
            after_summary,
            diff,
            matrix,
            freshness,
            structural_change_detected,
            specialty_coverage_changed,
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
                "the audit compares validated public PubMed snapshots; it never fetches, merges, or writes the candidate bundle".to_string(),
                "PMID/source identity and lane counts are structural metadata, not evidence quality, cohort comparability, applicability, or clinical findings".to_string(),
                "empty, truncated, missing-abstract, and freshness states remain explicit; no citation or abstract content is imputed".to_string(),
                "the report is a caller-owned review handoff and cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.audit_digest = digest_report(&report)?;
        Ok(report)
    }
}

fn validate_query(query: &PublicLiteratureRefreshAuditQuery) -> Result<(), NeurosurgeryError> {
    if query.max_source_changes == 0 || query.max_source_changes > MAX_SOURCE_CHANGES {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_refresh.max_source_changes",
            found: query.max_source_changes,
            max: MAX_SOURCE_CHANGES,
        });
    }
    if query.max_record_changes == 0 || query.max_record_changes > MAX_RECORD_CHANGES {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_refresh.max_record_changes",
            found: query.max_record_changes,
            max: MAX_RECORD_CHANGES,
        });
    }
    Ok(())
}

fn build_diff(
    before: &PublicLiteratureBundle,
    after: &PublicLiteratureBundle,
    query: &PublicLiteratureRefreshAuditQuery,
) -> Result<PublicLiteratureRefreshDiffReport, NeurosurgeryError> {
    let before_summary = before.summary()?;
    let after_summary = after.summary()?;
    let before_sources = before
        .sources
        .iter()
        .map(|source| (source.source_id.clone(), source))
        .collect::<BTreeMap<_, _>>();
    let after_sources = after
        .sources
        .iter()
        .map(|source| (source.source_id.clone(), source))
        .collect::<BTreeMap<_, _>>();
    let mut source_counts = PublicLiteratureRefreshCounts {
        added: 0,
        removed: 0,
        changed: 0,
    };
    let mut source_changes = Vec::new();
    for source_id in before_sources
        .keys()
        .chain(after_sources.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_sources.get(source_id), after_sources.get(source_id)) {
            (Some(before_source), Some(after_source)) => {
                let changed_fields = source_changed_fields(before_source, after_source);
                if !changed_fields.is_empty() {
                    source_counts.changed += 1;
                    source_changes.push(PublicLiteratureSourceChange {
                        source_id: (*source_id).clone(),
                        changed_fields,
                    });
                }
            }
            (Some(_), None) => {
                source_counts.removed += 1;
                source_changes.push(PublicLiteratureSourceChange {
                    source_id: (*source_id).clone(),
                    changed_fields: vec!["source_removed".to_string()],
                });
            }
            (None, Some(_)) => {
                source_counts.added += 1;
                source_changes.push(PublicLiteratureSourceChange {
                    source_id: (*source_id).clone(),
                    changed_fields: vec!["source_added".to_string()],
                });
            }
            (None, None) => unreachable!("source key union contains an existing side"),
        }
    }

    let before_records = before
        .records
        .iter()
        .map(|record| (record.pmid.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let after_records = after
        .records
        .iter()
        .map(|record| (record.pmid.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut record_counts = PublicLiteratureRefreshCounts {
        added: 0,
        removed: 0,
        changed: 0,
    };
    let mut record_changes = Vec::new();
    for pmid in before_records
        .keys()
        .chain(after_records.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_records.get(pmid), after_records.get(pmid)) {
            (Some(before_record), Some(after_record)) => {
                let changed_fields = record_changed_fields(before_record, after_record);
                if !changed_fields.is_empty() {
                    record_counts.changed += 1;
                    record_changes.push(PublicLiteratureRecordChange {
                        pmid: (*pmid).clone(),
                        before_source_id: Some(before_record.source_id.clone()),
                        after_source_id: Some(after_record.source_id.clone()),
                        before_specialty: Some(before_record.specialty),
                        after_specialty: Some(after_record.specialty),
                        changed_fields,
                    });
                }
            }
            (Some(before_record), None) => {
                record_counts.removed += 1;
                record_changes.push(PublicLiteratureRecordChange {
                    pmid: (*pmid).clone(),
                    before_source_id: Some(before_record.source_id.clone()),
                    after_source_id: None,
                    before_specialty: Some(before_record.specialty),
                    after_specialty: None,
                    changed_fields: vec!["record_removed".to_string()],
                });
            }
            (None, Some(after_record)) => {
                record_counts.added += 1;
                record_changes.push(PublicLiteratureRecordChange {
                    pmid: (*pmid).clone(),
                    before_source_id: None,
                    after_source_id: Some(after_record.source_id.clone()),
                    before_specialty: None,
                    after_specialty: Some(after_record.specialty),
                    changed_fields: vec!["record_added".to_string()],
                });
            }
            (None, None) => unreachable!("record key union contains an existing side"),
        }
    }

    let omitted_source_change_count = source_changes
        .len()
        .saturating_sub(query.max_source_changes);
    let omitted_record_change_count = record_changes
        .len()
        .saturating_sub(query.max_record_changes);
    source_changes.truncate(query.max_source_changes);
    record_changes.truncate(query.max_record_changes);
    let source_identity_stable = source_counts.added == 0 && source_counts.removed == 0;
    let record_identity_stable = record_counts.added == 0 && record_counts.removed == 0;
    let mut report = PublicLiteratureRefreshDiffReport {
        schema_version: PUBLIC_LITERATURE_REFRESH_DIFF_SCHEMA_VERSION.to_string(),
        diff_digest: String::new(),
        before_bundle_digest: before_summary.bundle_digest,
        after_bundle_digest: after_summary.bundle_digest,
        before_generated_at: before.generated_at.clone(),
        after_generated_at: after.generated_at.clone(),
        source_counts,
        record_counts,
        source_changes,
        record_changes,
        omitted_source_change_count,
        omitted_record_change_count,
        truncated: omitted_source_change_count > 0 || omitted_record_change_count > 0,
        source_identity_stable,
        record_identity_stable,
    };
    report.diff_digest = digest_diff(&report)?;
    Ok(report)
}

fn source_changed_fields(
    before: &PublicLiteratureSource,
    after: &PublicLiteratureSource,
) -> Vec<String> {
    let mut fields = Vec::new();
    if before.authority != after.authority {
        fields.push("authority".to_string());
    }
    if before.uri != after.uri {
        fields.push("uri".to_string());
    }
    if before.retrieved_at != after.retrieved_at {
        fields.push("retrieved_at".to_string());
    }
    if before.content_sha256 != after.content_sha256 {
        fields.push("content_sha256".to_string());
    }
    if before.record_count != after.record_count {
        fields.push("record_count".to_string());
    }
    fields
}

fn record_changed_fields(
    before: &PublicLiteratureRecord,
    after: &PublicLiteratureRecord,
) -> Vec<String> {
    let mut fields = Vec::new();
    if before.source_id != after.source_id {
        fields.push("source_id".to_string());
    }
    if before.specialty != after.specialty {
        fields.push("specialty".to_string());
    }
    if before.title != after.title {
        fields.push("title".to_string());
    }
    if before.journal != after.journal {
        fields.push("journal".to_string());
    }
    if before.publication_date != after.publication_date {
        fields.push("publication_date".to_string());
    }
    if before.doi != after.doi {
        fields.push("doi".to_string());
    }
    if before.abstract_text != after.abstract_text {
        fields.push("abstract_text".to_string());
    }
    if before.abstract_truncated != after.abstract_truncated {
        fields.push("abstract_truncated".to_string());
    }
    if before.publication_types != after.publication_types {
        fields.push("publication_types".to_string());
    }
    if before.mesh_terms != after.mesh_terms {
        fields.push("mesh_terms".to_string());
    }
    fields
}

fn digest_diff(report: &PublicLiteratureRefreshDiffReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.diff_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn digest_report(report: &PublicLiteratureRefreshAuditReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.audit_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
