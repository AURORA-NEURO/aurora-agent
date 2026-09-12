//! Structural audit for local-model or reviewer drafts grounded in the real glioma packet.
//!
//! This module is deliberately not a truth oracle. It verifies that a draft is tied to one
//! freshly composed, source-linked packet, that every claim cites records actually present in
//! that packet, and that the caller has not declared a patient-case or clinical-action posture.
//! The accepted state is still `grounded_for_human_review`; the core never interprets claim text.

use crate::{
    NeurosurgeryError, RealDataEvidencePacketQuery, RealDataEvidencePacketReport,
    RealDataRecordKind, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const REAL_DATA_DRAFT_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-draft-audit/0.1";
pub const MAX_REAL_DATA_DRAFT_CLAIMS: usize = 128;
pub const MAX_REAL_DATA_DRAFT_CITATIONS: usize = 16;
const MAX_DRAFT_CLAIM_ID_BYTES: usize = 128;
const MAX_DRAFT_CLAIM_TEXT_BYTES: usize = 8_000;

/// Claim posture declared by a local model or reviewer. The verifier does not infer this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataDraftClaimKind {
    SourceObservation,
    PopulationSummary,
    ResearchHypothesis,
    Limitation,
    ClinicalAction,
}

/// Scope keeps population/citation metadata separate from a patient case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataDraftScope {
    PublicRecordMetadata,
    PopulationAggregate,
    CitationMetadata,
    PatientCase,
}

/// Stable record identity used as a required citation for every draft claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RealDataDraftCitation {
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
}

/// One caller-owned claim candidate. Text is retained only in the transient audit response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDraftClaim {
    pub claim_id: String,
    pub kind: RealDataDraftClaimKind,
    pub scope: RealDataDraftScope,
    pub text: String,
    pub citations: Vec<RealDataDraftCitation>,
    /// Required for hypotheses because the verifier cannot infer hypothetical intent from text.
    #[serde(default)]
    pub explicitly_hypothetical: bool,
}

/// Packet bounds and caller-owned draft claims submitted for one audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDraftAuditRequest {
    #[serde(default)]
    pub query: RealDataEvidencePacketQuery,
    pub claims: Vec<RealDataDraftClaim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataDraftClaimStatus {
    GroundedForHumanReview,
    Blocked,
}

/// Per-claim structural result. A grounded row means citation and posture checks passed only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDraftClaimReport {
    pub claim_id: String,
    pub kind: RealDataDraftClaimKind,
    pub scope: RealDataDraftScope,
    pub status: RealDataDraftClaimStatus,
    pub citation_count: usize,
    pub matched_citation_count: usize,
    pub missing_citations: Vec<RealDataDraftCitation>,
    pub blockers: Vec<String>,
}

/// Digest-bound audit of a local-model/reviewer draft against one real-data packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDraftAuditReport {
    pub schema_version: String,
    pub draft_digest: String,
    pub packet_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub packet: RealDataEvidencePacketReport,
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

impl RealGliomaBundle {
    /// Compose a packet and structurally audit claims against its emitted record identities.
    /// No claim text is interpreted, summarized, or promoted to a clinical conclusion.
    pub fn audit_draft(
        &self,
        request: &RealDataDraftAuditRequest,
    ) -> Result<RealDataDraftAuditReport, NeurosurgeryError> {
        if request.claims.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data draft audit requires at least one claim".to_string(),
            });
        }
        if request.claims.len() > MAX_REAL_DATA_DRAFT_CLAIMS {
            return Err(NeurosurgeryError::TooMany {
                field: "real_data_draft_audit.claims",
                found: request.claims.len(),
                max: MAX_REAL_DATA_DRAFT_CLAIMS,
            });
        }
        let packet = self.evidence_packet(&request.query)?;
        let packet_records = packet_record_set(&packet);
        let mut seen_claim_ids = BTreeSet::new();
        let mut claims = request.claims.clone();
        for claim in &mut claims {
            validate_real_data_draft_claim_shape(claim)?;
            if !seen_claim_ids.insert(claim.claim_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "real-data draft claim id {:?} appears more than once",
                        claim.claim_id
                    ),
                });
            }
            claim.citations.sort();
        }
        claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        let canonical_request = RealDataDraftAuditRequest {
            query: request.query.clone(),
            claims,
        };
        let mut reports = Vec::with_capacity(canonical_request.claims.len());
        for claim in &canonical_request.claims {
            reports.push(audit_real_data_draft_claim(claim, &packet_records));
        }
        let grounded_claim_count = reports
            .iter()
            .filter(|report| report.status == RealDataDraftClaimStatus::GroundedForHumanReview)
            .count();
        let blocked_claim_count = reports.len().saturating_sub(grounded_claim_count);
        let status = if blocked_claim_count == 0 {
            RealDataDraftClaimStatus::GroundedForHumanReview
        } else {
            RealDataDraftClaimStatus::Blocked
        };
        let draft_digest = digest_draft(&packet, &canonical_request)?;
        Ok(RealDataDraftAuditReport {
            schema_version: REAL_DATA_DRAFT_AUDIT_SCHEMA_VERSION.to_string(),
            draft_digest,
            packet_digest: packet.packet_digest.clone(),
            bundle_digest: packet.bundle_digest.clone(),
            generated_at: packet.generated_at.clone(),
            packet,
            claims: reports,
            claim_count: canonical_request.claims.len(),
            grounded_claim_count,
            blocked_claim_count,
            status,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "grounded means only that declared citations occur in the composed packet and the declared posture is allowed; claim text is not fact-checked or clinically interpreted".to_string(),
                "population aggregates and citation metadata never become patient observations, diagnoses, prognoses, treatments, triage decisions, or procedures".to_string(),
                "clinical_action and patient_case claims are blocked; every accepted claim remains a caller-owned human-review handoff".to_string(),
                "the audit never fetches URLs, invokes a model, opens credentials, stores patient files, or performs an external effect".to_string(),
            ],
        })
    }
}

pub(crate) fn validate_real_data_draft_claim_shape(
    claim: &RealDataDraftClaim,
) -> Result<(), NeurosurgeryError> {
    if claim.claim_id.trim().is_empty()
        || claim.claim_id.len() > MAX_DRAFT_CLAIM_ID_BYTES
        || claim.claim_id.chars().any(char::is_control)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real-data draft claim_id is empty, too long, or contains a control character"
                .to_string(),
        });
    }
    if claim.text.trim().is_empty()
        || claim.text.len() > MAX_DRAFT_CLAIM_TEXT_BYTES
        || claim.text.chars().any(char::is_control)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("real-data draft claim {:?} text is empty, too long, or contains a control character", claim.claim_id),
        });
    }
    if claim.citations.len() > MAX_REAL_DATA_DRAFT_CITATIONS {
        return Err(NeurosurgeryError::TooMany {
            field: "real_data_draft_audit.citations",
            found: claim.citations.len(),
            max: MAX_REAL_DATA_DRAFT_CITATIONS,
        });
    }
    let mut seen = BTreeSet::new();
    for citation in &claim.citations {
        if citation.record_id.trim().is_empty()
            || citation.record_id.len() > MAX_DRAFT_CLAIM_ID_BYTES
            || citation.record_id.chars().any(char::is_control)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "real-data draft claim {:?} has an invalid citation id",
                    claim.claim_id
                ),
            });
        }
        if !seen.insert(citation) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "real-data draft claim {:?} repeats a citation",
                    claim.claim_id
                ),
            });
        }
    }
    Ok(())
}

fn packet_record_set(
    packet: &RealDataEvidencePacketReport,
) -> BTreeSet<(RealDataRecordKind, String)> {
    let mut records = BTreeSet::new();
    records.extend(
        packet
            .data_query
            .hits
            .iter()
            .map(|hit| (hit.record_kind, hit.record_id.clone())),
    );
    records.extend(
        packet
            .graph
            .nodes
            .iter()
            .map(|node| (node.record_kind, node.record_id.clone())),
    );
    if let Some(cohort) = packet.cohort_landscape.as_ref() {
        // Cohort rows are aggregate genomic-project records emitted by the packet itself. Keep
        // them in the citation closure so a local model can ground a project/file-availability
        // observation without widening the packet to patient values or asset bytes.
        records.extend(
            cohort
                .project_rows
                .iter()
                .map(|row| (RealDataRecordKind::GenomicProject, row.project_id.clone())),
        );
    }
    records
}

pub(crate) fn audit_real_data_draft_claim(
    claim: &RealDataDraftClaim,
    packet_records: &BTreeSet<(RealDataRecordKind, String)>,
) -> RealDataDraftClaimReport {
    let missing_citations = claim
        .citations
        .iter()
        .filter(|citation| {
            !packet_records.contains(&(citation.record_kind, citation.record_id.clone()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if claim.citations.is_empty() {
        blockers.push("claim must cite at least one record emitted by the packet".to_string());
    }
    if !missing_citations.is_empty() {
        blockers.push(
            "one or more citations are not present in the packet's bounded record set".to_string(),
        );
    }
    if claim.scope == RealDataDraftScope::PatientCase {
        blockers.push(
            "patient_case scope is not accepted for a population/citation snapshot".to_string(),
        );
    }
    if claim.kind == RealDataDraftClaimKind::ClinicalAction {
        blockers.push(
            "clinical_action claims are prohibited by the provider-free research boundary"
                .to_string(),
        );
    }
    if claim.kind == RealDataDraftClaimKind::ResearchHypothesis && !claim.explicitly_hypothetical {
        blockers
            .push("research_hypothesis claims must set explicitly_hypothetical=true".to_string());
    }
    if claim.kind == RealDataDraftClaimKind::ResearchHypothesis
        && claim.scope == RealDataDraftScope::PatientCase
    {
        blockers
            .push("patient-case hypotheses cannot be grounded by population metadata".to_string());
    }
    let matched_citation_count = claim
        .citations
        .len()
        .saturating_sub(missing_citations.len());
    let status = if blockers.is_empty() {
        RealDataDraftClaimStatus::GroundedForHumanReview
    } else {
        RealDataDraftClaimStatus::Blocked
    };
    RealDataDraftClaimReport {
        claim_id: claim.claim_id.clone(),
        kind: claim.kind,
        scope: claim.scope,
        status,
        citation_count: claim.citations.len(),
        matched_citation_count,
        missing_citations,
        blockers,
    }
}

fn digest_draft(
    packet: &RealDataEvidencePacketReport,
    request: &RealDataDraftAuditRequest,
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&(packet.packet_digest.as_str(), request))
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
