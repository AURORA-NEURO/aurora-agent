//! Scientific-qualification preservation audit for glioma research-object releases.
//!
//! The auditor checks that uncertainty, null outcomes, failed replications, contradictions,
//! omissions, limitations, and negative evidence survive a transformation into a release object.
//! It compares typed strength and lineage declarations only; it never reads or rewrites raw
//! payloads and never turns a release audit into a publication or clinical decision.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaQualificationPreservationAudit1@1";
pub const MAX_RECORDS: usize = 512;
pub const MAX_CLAIMS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationKind {
    Uncertainty,
    Interval,
    NullResult,
    FailedReplication,
    Contradiction,
    Omission,
    Limitation,
    NegativeEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationFindingKind {
    MissingSource,
    MissingRelease,
    WeakenedQualification,
    UnboundLineage,
    UnsupportedReleaseClaim,
    MissingRequiredKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationFindingSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationPreservationDisposition {
    Complete,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationRecord {
    pub qualification_id: String,
    pub kind: QualificationKind,
    pub source_digest: Option<ContentHash>,
    pub release_digest: Option<ContentHash>,
    pub source_present: bool,
    pub release_present: bool,
    pub source_strength_milli: u16,
    pub release_strength_milli: u16,
    pub lineage_bound: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseClaimRecord {
    pub claim_id: String,
    pub claim_strength_milli: u16,
    pub evidence_strength_milli: u16,
    pub qualification_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationPreservationRequest {
    pub research_object_digest: ContentHash,
    pub qualifications: Vec<QualificationRecord>,
    pub claims: Vec<ReleaseClaimRecord>,
    pub required_kind_order: Vec<QualificationKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationFinding {
    pub finding_id: String,
    pub qualification_id: Option<String>,
    pub claim_id: Option<String>,
    pub kind: QualificationFindingKind,
    pub severity: QualificationFindingSeverity,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationPreservationAudit {
    pub feature_id: String,
    pub output_schema: String,
    pub research_object_digest: ContentHash,
    pub qualification_order: Vec<String>,
    pub preserved_order: Vec<String>,
    pub missing_release_order: Vec<String>,
    pub weakened_order: Vec<String>,
    pub unbound_lineage_order: Vec<String>,
    pub missing_required_kind_order: Vec<QualificationKind>,
    pub overclaim_order: Vec<String>,
    pub findings: Vec<QualificationFinding>,
    pub disposition: QualificationPreservationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualificationPreservationError {
    #[error("qualification preservation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("qualification preservation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("qualification preservation digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(audit: &QualificationPreservationAudit) -> serde_json::Value {
    serde_json::json!({
        "feature_id": audit.feature_id,
        "output_schema": audit.output_schema,
        "research_object_digest": audit.research_object_digest,
        "qualification_order": audit.qualification_order,
        "preserved_order": audit.preserved_order,
        "missing_release_order": audit.missing_release_order,
        "weakened_order": audit.weakened_order,
        "unbound_lineage_order": audit.unbound_lineage_order,
        "missing_required_kind_order": audit.missing_required_kind_order,
        "overclaim_order": audit.overclaim_order,
        "findings": audit.findings,
        "disposition": audit.disposition,
    })
}

fn validate_request(
    request: &QualificationPreservationRequest,
) -> Result<(), QualificationPreservationError> {
    if request.research_object_digest.as_str().len() != 64
        || request.qualifications.is_empty()
        || request.qualifications.len() > MAX_RECORDS
        || request.claims.len() > MAX_CLAIMS
        || !canonical(&request.required_kind_order)
    {
        return Err(QualificationPreservationError::InvalidRequest(
            "object digest, bounded qualifications/claims, and canonical required kinds are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in &request.qualifications {
        if !safe_text(&record.qualification_id)
            || !ids.insert(record.qualification_id.clone())
            || record.source_strength_milli > 1_000
            || record.release_strength_milli > 1_000
            || (record.source_present && record.source_digest.is_none())
            || (record.release_present && record.release_digest.is_none())
        {
            return Err(QualificationPreservationError::InvalidRequest(format!(
                "qualification {} is malformed or duplicated",
                record.qualification_id
            )));
        }
    }
    let mut claim_ids = BTreeSet::new();
    for claim in &request.claims {
        if !safe_text(&claim.claim_id)
            || !claim_ids.insert(claim.claim_id.clone())
            || claim.claim_strength_milli > 1_000
            || claim.evidence_strength_milli > 1_000
            || !canonical(&claim.qualification_order)
            || claim.qualification_order.iter().any(|id| !ids.contains(id))
        {
            return Err(QualificationPreservationError::InvalidRequest(format!(
                "release claim {} is malformed, duplicated, or unbound",
                claim.claim_id
            )));
        }
    }
    Ok(())
}

impl QualificationPreservationAudit {
    pub fn validate(&self) -> Result<(), QualificationPreservationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_object_digest.as_str().len() != 64
            || !canonical(&self.qualification_order)
            || !canonical(&self.preserved_order)
            || !canonical(&self.missing_release_order)
            || !canonical(&self.weakened_order)
            || !canonical(&self.unbound_lineage_order)
            || !canonical(&self.missing_required_kind_order)
            || !canonical(&self.overclaim_order)
            || self.digest.as_str().len() != 64
            || self.findings.iter().any(|finding| {
                !safe_text(&finding.finding_id)
                    || !safe_text(&finding.reason)
                    || finding
                        .qualification_id
                        .as_ref()
                        .is_some_and(|id| !safe_text(id))
                    || finding.claim_id.as_ref().is_some_and(|id| !safe_text(id))
            })
        {
            return Err(QualificationPreservationError::InvalidOutput(
                "qualification identity, ordering, findings, or digest invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| QualificationPreservationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(QualificationPreservationError::InvalidOutput(
                "qualification audit digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Audit source-to-release preservation of scientific qualifications and claim strength.
pub fn audit_glioma_qualification_preservation(
    request: &QualificationPreservationRequest,
) -> Result<QualificationPreservationAudit, QualificationPreservationError> {
    validate_request(request)?;
    let mut qualifications = request.qualifications.clone();
    qualifications.sort_by(|left, right| left.qualification_id.cmp(&right.qualification_id));
    let qualification_ids = qualifications
        .iter()
        .map(|record| record.qualification_id.clone())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();
    let mut missing_release = BTreeSet::new();
    let mut weakened = BTreeSet::new();
    let mut unbound = BTreeSet::new();
    let mut preserved = BTreeSet::new();
    let mut sequence = 0usize;
    let mut emit = |qualification_id: Option<String>,
                    claim_id: Option<String>,
                    kind: QualificationFindingKind,
                    severity: QualificationFindingSeverity,
                    reason: String| {
        sequence += 1;
        findings.push(QualificationFinding {
            finding_id: format!("qualification-{sequence:06}"),
            qualification_id,
            claim_id,
            kind,
            severity,
            reason,
        });
    };
    for record in &qualifications {
        if record.source_present && !record.release_present {
            missing_release.insert(record.qualification_id.clone());
            emit(
                Some(record.qualification_id.clone()),
                None,
                QualificationFindingKind::MissingRelease,
                QualificationFindingSeverity::Critical,
                "source qualification is absent from the release representation".into(),
            );
        } else if !record.source_present && record.release_present {
            emit(
                Some(record.qualification_id.clone()),
                None,
                QualificationFindingKind::MissingSource,
                QualificationFindingSeverity::Critical,
                "release qualification has no source evidence".into(),
            );
        } else if record.source_present && record.release_present {
            if record.release_strength_milli < record.source_strength_milli {
                weakened.insert(record.qualification_id.clone());
                emit(
                    Some(record.qualification_id.clone()),
                    None,
                    QualificationFindingKind::WeakenedQualification,
                    QualificationFindingSeverity::Critical,
                    "release qualification is weaker than the source declaration".into(),
                );
            }
            if !record.lineage_bound {
                unbound.insert(record.qualification_id.clone());
                emit(
                    Some(record.qualification_id.clone()),
                    None,
                    QualificationFindingKind::UnboundLineage,
                    QualificationFindingSeverity::Warning,
                    "release qualification is present but lacks a source-to-release lineage binding".into(),
                );
            }
            if record.release_strength_milli >= record.source_strength_milli && record.lineage_bound
            {
                preserved.insert(record.qualification_id.clone());
            }
        }
    }
    let present_kinds = qualifications
        .iter()
        .filter(|record| record.source_present)
        .map(|record| record.kind)
        .collect::<BTreeSet<_>>();
    let mut missing_required_kind = BTreeSet::new();
    for kind in &request.required_kind_order {
        if !present_kinds.contains(kind) {
            missing_required_kind.insert(*kind);
            emit(
                None,
                None,
                QualificationFindingKind::MissingRequiredKind,
                QualificationFindingSeverity::Critical,
                format!("required qualification kind {kind:?} is absent from the source"),
            );
        }
    }
    let mut overclaim = BTreeSet::new();
    for claim in &request.claims {
        let referenced_preserved = claim
            .qualification_order
            .iter()
            .all(|id| preserved.contains(id));
        if claim.claim_strength_milli > claim.evidence_strength_milli || !referenced_preserved {
            overclaim.insert(claim.claim_id.clone());
            emit(
                None,
                Some(claim.claim_id.clone()),
                QualificationFindingKind::UnsupportedReleaseClaim,
                QualificationFindingSeverity::Critical,
                "release claim exceeds explicit evidence or references an unpreserved qualification".into(),
            );
        }
    }
    let blockers = findings
        .iter()
        .any(|finding| finding.severity == QualificationFindingSeverity::Critical);
    let unresolved = !unbound.is_empty() || !missing_required_kind.is_empty();
    let disposition = if blockers {
        QualificationPreservationDisposition::Blocked
    } else if unresolved {
        QualificationPreservationDisposition::Unresolved
    } else if preserved.len() < qualification_ids.len() {
        QualificationPreservationDisposition::Partial
    } else {
        QualificationPreservationDisposition::Complete
    };
    let mut audit = QualificationPreservationAudit {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_object_digest: request.research_object_digest.clone(),
        qualification_order: qualification_ids.into_iter().collect(),
        preserved_order: preserved.into_iter().collect(),
        missing_release_order: missing_release.into_iter().collect(),
        weakened_order: weakened.into_iter().collect(),
        unbound_lineage_order: unbound.into_iter().collect(),
        missing_required_kind_order: missing_required_kind.into_iter().collect(),
        overclaim_order: overclaim.into_iter().collect(),
        findings,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-qualification-audit"),
    };
    audit.digest = ContentHash::of_value(&digest_body(&audit))
        .map_err(|error| QualificationPreservationError::Digest(error.to_string()))?;
    audit.validate()?;
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn record(id: &str, kind: QualificationKind) -> QualificationRecord {
        QualificationRecord {
            qualification_id: id.into(),
            kind,
            source_digest: Some(digest(&format!("source-{id}"))),
            release_digest: Some(digest(&format!("release-{id}"))),
            source_present: true,
            release_present: true,
            source_strength_milli: 800,
            release_strength_milli: 800,
            lineage_bound: true,
            required: true,
        }
    }

    fn request(qualifications: Vec<QualificationRecord>) -> QualificationPreservationRequest {
        QualificationPreservationRequest {
            research_object_digest: digest("object"),
            qualifications,
            claims: vec![ReleaseClaimRecord {
                claim_id: "claim-a".into(),
                claim_strength_milli: 700,
                evidence_strength_milli: 800,
                qualification_order: vec!["negative".into()],
            }],
            required_kind_order: vec![QualificationKind::NegativeEvidence],
        }
    }

    #[test]
    fn complete_qualifications_are_preserved() {
        let audit = audit_glioma_qualification_preservation(&request(vec![record(
            "negative",
            QualificationKind::NegativeEvidence,
        )]))
        .unwrap();
        assert_eq!(
            audit.disposition,
            QualificationPreservationDisposition::Complete
        );
        assert_eq!(audit.preserved_order, vec!["negative"]);
        assert!(audit.findings.is_empty());
    }

    #[test]
    fn missing_release_and_weakened_strength_block() {
        let mut missing = record("negative", QualificationKind::NegativeEvidence);
        missing.release_present = false;
        let audit = audit_glioma_qualification_preservation(&request(vec![missing])).unwrap();
        assert_eq!(
            audit.disposition,
            QualificationPreservationDisposition::Blocked
        );
        assert!(audit.missing_release_order.contains(&"negative".into()));
        let mut weak = record("negative", QualificationKind::NegativeEvidence);
        weak.release_strength_milli = 200;
        let weak_audit = audit_glioma_qualification_preservation(&request(vec![weak])).unwrap();
        assert!(weak_audit.weakened_order.contains(&"negative".into()));
    }

    #[test]
    fn required_uncertainty_and_null_kinds_cannot_be_omitted() {
        let mut req = request(vec![record(
            "negative",
            QualificationKind::NegativeEvidence,
        )]);
        req.required_kind_order = vec![
            QualificationKind::Uncertainty,
            QualificationKind::NullResult,
            QualificationKind::NegativeEvidence,
        ];
        let audit = audit_glioma_qualification_preservation(&req).unwrap();
        assert_eq!(
            audit.disposition,
            QualificationPreservationDisposition::Blocked
        );
        assert_eq!(audit.missing_required_kind_order.len(), 2);
    }

    #[test]
    fn overclaim_and_unbound_lineage_are_explicit() {
        let mut item = record("negative", QualificationKind::NegativeEvidence);
        item.lineage_bound = false;
        let mut req = request(vec![item]);
        req.claims[0].claim_strength_milli = 950;
        req.claims[0].evidence_strength_milli = 500;
        let audit = audit_glioma_qualification_preservation(&req).unwrap();
        assert!(audit.unbound_lineage_order.contains(&"negative".into()));
        assert_eq!(audit.overclaim_order, vec!["claim-a"]);
        assert_eq!(
            audit.disposition,
            QualificationPreservationDisposition::Blocked
        );
    }

    #[test]
    fn source_absence_cannot_become_release_evidence() {
        let mut item = record("negative", QualificationKind::NegativeEvidence);
        item.source_present = false;
        let audit = audit_glioma_qualification_preservation(&request(vec![item])).unwrap();
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.kind == QualificationFindingKind::MissingSource));
        assert_eq!(
            audit.disposition,
            QualificationPreservationDisposition::Blocked
        );
    }
}
