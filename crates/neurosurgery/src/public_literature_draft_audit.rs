//! Source-bound packet and draft audit for the cross-specialty PubMed snapshot.
//!
//! The glioma population bundle has registry and assay metadata, while the other neurosurgical
//! lanes are represented by a separate, source-hashed PubMed corpus. This module gives those
//! lanes the same local-model handoff contract: a bounded query packet, stable PMID citations,
//! and structural claim auditing. It does not judge abstract truth, study quality, or clinical
//! applicability.

use crate::real_data_draft_audit::{
    audit_real_data_draft_claim, validate_real_data_draft_claim_shape,
};
use crate::real_data_freshness::{RealDataFreshnessQuery, RealDataFreshnessReport};
use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery, PublicLiteratureQueryResult,
    PublicLiteratureSummary, RealDataDraftClaim, RealDataDraftClaimReport,
    RealDataDraftClaimStatus, RealDataRecordKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PUBLIC_LITERATURE_EVIDENCE_PACKET_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-evidence-packet/0.1";
pub const PUBLIC_LITERATURE_DRAFT_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-draft-audit/0.1";
pub const MAX_PUBLIC_LITERATURE_DRAFT_CLAIMS: usize = 128;

/// Query bounds for a cross-specialty evidence handoff. The underlying bundle is never fetched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureEvidencePacketQuery {
    #[serde(default)]
    pub query: PublicLiteratureQuery,
    /// Optional explicit caller-owned source-age policy. Omitting it leaves freshness unclaimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessQuery>,
}

/// One bounded, digest-addressed PubMed handoff for a local model or human reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureEvidencePacketReport {
    pub schema_version: String,
    pub packet_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: PublicLiteratureEvidencePacketQuery,
    pub summary: PublicLiteratureSummary,
    pub query_result: PublicLiteratureQueryResult,
    /// Optional digest-bound source-age posture requested by the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessReport>,
    pub source_count: usize,
    pub record_count: usize,
    pub query_match_count: usize,
    pub abstract_count: usize,
    pub abstract_truncated_count: usize,
    pub specialty_counts: Vec<crate::PublicLiteratureSpecialtyCount>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureEvidencePacketReport {
    /// Validate a persisted public-literature packet without fetching PubMed or invoking a
    /// provider. This checks nested count closure, freshness binding, and the packet digest; it
    /// does not assess study quality, cohort applicability, or clinical truth.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != PUBLIC_LITERATURE_EVIDENCE_PACKET_SCHEMA_VERSION
            || !is_sha256_hex(&self.packet_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || self.bundle_digest != self.summary.bundle_digest
            || self.bundle_digest != self.query_result.bundle_digest
            || self.generated_at.trim().is_empty()
            || self.query.query != self.query_result.query
            || self.source_count != self.summary.source_count
            || self.source_count == 0
            || self.record_count != self.summary.record_count
            || self.record_count == 0
            || self.query_match_count != self.query_result.total_matches
            || self.query_result.returned_matches != self.query_result.hits.len()
            || self.query_result.total_matches < self.query_result.returned_matches
            || self.query_result.truncated
                != (self.query_result.total_matches > self.query_result.returned_matches)
            || self.abstract_count != self.summary.abstract_count
            || self.abstract_truncated_count != self.summary.abstract_truncated_count
            || self.query_result.abstract_count != self.abstract_count
            || self.query_result.abstract_truncated_count != self.abstract_truncated_count
            || self.specialty_counts != self.summary.specialty_counts
            || self.query_result.specialty_counts != self.specialty_counts
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
            || !self.summary.provenance_bound
            || self.summary.synthetic_data
            || self.query_result.schema_version
                != crate::public_literature::PUBLIC_LITERATURE_SCHEMA_VERSION
        {
            return Err(packet_rejected(
                "public-literature evidence packet envelope is invalid",
            ));
        }
        let mut seen = BTreeSet::new();
        for hit in &self.query_result.hits {
            if hit.pmid.trim().is_empty()
                || hit.title.trim().is_empty()
                || hit.journal.trim().is_empty()
                || hit.source_id.trim().is_empty()
                || !hit.source_uri.starts_with("https://")
                || !hit
                    .record_uri
                    .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
                || !seen.insert((hit.specialty, hit.pmid.clone()))
            {
                return Err(packet_rejected(
                    "public-literature packet hits must be unique and source-addressable",
                ));
            }
        }
        if let Some(freshness) = self.freshness.as_ref() {
            if freshness.bundle_digest != self.bundle_digest
                || !is_sha256_hex(&freshness.freshness_digest)
                || !freshness.provenance_bound
                || freshness.synthetic_data
                || !freshness.human_review_required
                || freshness.provider != "none"
                || freshness.network
                || freshness.effect != "read_only"
                || self.query.freshness.as_ref() != Some(&freshness.query)
            {
                return Err(packet_rejected(
                    "public-literature packet freshness binding is invalid",
                ));
            }
        } else if self.query.freshness.is_some() {
            return Err(packet_rejected(
                "public-literature packet freshness query is missing its report",
            ));
        }
        if self.packet_digest
            != digest_public_literature_packet(
                &self.bundle_digest,
                &self.query,
                &self.query_result,
                self.freshness
                    .as_ref()
                    .map(|report| report.freshness_digest.as_str()),
            )?
        {
            return Err(packet_rejected(
                "public-literature packet digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the packet from the exact validated public-literature snapshot and persisted query.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.evidence_packet(&self.query)?;
        if &expected != self {
            return Err(packet_rejected(
                "public-literature evidence packet does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

/// Caller-owned claims to audit against the packet's bounded PMID set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureDraftAuditRequest {
    #[serde(default)]
    pub query: PublicLiteratureEvidencePacketQuery,
    pub claims: Vec<RealDataDraftClaim>,
}

/// Structural audit report for a cross-specialty local-model or reviewer draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureDraftAuditReport {
    pub schema_version: String,
    pub draft_digest: String,
    pub packet_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub packet: PublicLiteratureEvidencePacketReport,
    pub claims: Vec<RealDataDraftClaimReport>,
    pub claim_count: usize,
    pub grounded_claim_count: usize,
    pub blocked_claim_count: usize,
    pub status: RealDataDraftClaimStatus,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureBundle {
    /// Compose one bounded packet from the already validated cross-specialty corpus.
    pub fn evidence_packet(
        &self,
        query: &PublicLiteratureEvidencePacketQuery,
    ) -> Result<PublicLiteratureEvidencePacketReport, NeurosurgeryError> {
        self.validate()?;
        let summary = self.summary()?;
        let query_result = self.query(&query.query)?;
        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness_query| self.freshness_report(freshness_query))
            .transpose()?;
        let packet_digest = digest_public_literature_packet(
            &summary.bundle_digest,
            query,
            &query_result,
            freshness
                .as_ref()
                .map(|report| report.freshness_digest.as_str()),
        )?;
        let report = PublicLiteratureEvidencePacketReport {
            schema_version: PUBLIC_LITERATURE_EVIDENCE_PACKET_SCHEMA_VERSION.to_string(),
            packet_digest,
            bundle_digest: summary.bundle_digest.clone(),
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            source_count: summary.source_count,
            record_count: summary.record_count,
            query_match_count: query_result.total_matches,
            abstract_count: summary.abstract_count,
            abstract_truncated_count: summary.abstract_truncated_count,
            specialty_counts: summary.specialty_counts.clone(),
            summary,
            query_result,
            freshness,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the packet contains citation metadata and bounded abstract excerpts only; it is not a diagnosis, prognosis, treatment recommendation, triage decision, or procedural plan".to_string(),
                "specialty tags record the retrieval lane and do not establish cohort identity, study quality, applicability, or a patient-level finding".to_string(),
                "the packet is a caller-owned handoff for a local model or qualified human reviewer; it never fetches URLs, invokes a provider, opens credentials, stores patient files, or performs an external effect".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }

    /// Audit local-model or reviewer claims against PMID records emitted by one packet.
    /// Grounding is structural only; abstract text is never interpreted.
    pub fn audit_draft(
        &self,
        request: &PublicLiteratureDraftAuditRequest,
    ) -> Result<PublicLiteratureDraftAuditReport, NeurosurgeryError> {
        if request.claims.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature draft audit requires at least one claim".to_string(),
            });
        }
        if request.claims.len() > MAX_PUBLIC_LITERATURE_DRAFT_CLAIMS {
            return Err(NeurosurgeryError::TooMany {
                field: "public_literature_draft_audit.claims",
                found: request.claims.len(),
                max: MAX_PUBLIC_LITERATURE_DRAFT_CLAIMS,
            });
        }
        let packet = self.evidence_packet(&request.query)?;
        let packet_records = packet_record_set(&packet);
        let mut claims = request.claims.clone();
        let mut seen_claim_ids = BTreeSet::new();
        for claim in &mut claims {
            validate_real_data_draft_claim_shape(claim)?;
            if !seen_claim_ids.insert(claim.claim_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature draft claim id {:?} appears more than once",
                        claim.claim_id
                    ),
                });
            }
            claim.citations.sort();
        }
        claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        let canonical_request = PublicLiteratureDraftAuditRequest {
            query: request.query.clone(),
            claims,
        };
        let claims = canonical_request
            .claims
            .iter()
            .map(|claim| audit_real_data_draft_claim(claim, &packet_records))
            .collect::<Vec<_>>();
        let grounded_claim_count = claims
            .iter()
            .filter(|claim| claim.status == RealDataDraftClaimStatus::GroundedForHumanReview)
            .count();
        let blocked_claim_count = claims.len().saturating_sub(grounded_claim_count);
        let status = if blocked_claim_count == 0 {
            RealDataDraftClaimStatus::GroundedForHumanReview
        } else {
            RealDataDraftClaimStatus::Blocked
        };
        let draft_digest = digest_public_literature_draft(&packet, &canonical_request)?;
        Ok(PublicLiteratureDraftAuditReport {
            schema_version: PUBLIC_LITERATURE_DRAFT_AUDIT_SCHEMA_VERSION.to_string(),
            draft_digest,
            packet_digest: packet.packet_digest.clone(),
            bundle_digest: packet.bundle_digest.clone(),
            generated_at: packet.generated_at.clone(),
            packet,
            claim_count: claims.len(),
            grounded_claim_count,
            blocked_claim_count,
            status,
            claims,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "grounded means only that declared citations occur in the composed packet and the declared posture is allowed; claim text is not fact-checked or clinically interpreted".to_string(),
                "PubMed citation metadata and abstracts never become patient observations, diagnoses, prognoses, treatments, triage decisions, or procedures".to_string(),
                "clinical_action and patient_case claims are blocked; every accepted claim remains a caller-owned human-review handoff".to_string(),
                "the audit never fetches URLs, invokes a model, opens credentials, stores patient files, or performs an external effect".to_string(),
            ],
        })
    }
}

fn packet_record_set(
    packet: &PublicLiteratureEvidencePacketReport,
) -> BTreeSet<(RealDataRecordKind, String)> {
    packet
        .query_result
        .hits
        .iter()
        .map(|hit| (RealDataRecordKind::LiteratureArticle, hit.pmid.clone()))
        .collect()
}

fn digest_public_literature_packet(
    bundle_digest: &str,
    query: &PublicLiteratureEvidencePacketQuery,
    query_result: &PublicLiteratureQueryResult,
    freshness_digest: Option<&str>,
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(bundle_digest, query, query_result, freshness_digest))
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn digest_public_literature_draft(
    packet: &PublicLiteratureEvidencePacketReport,
    request: &PublicLiteratureDraftAuditRequest,
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(packet.packet_digest.as_str(), request))
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn packet_rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
