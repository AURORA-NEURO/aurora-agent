//! Verified evidence-to-knowledge handoff for the autonomous preclinical glioma engine.
//!
//! Evidence qualification and typed-knowledge compilation are deliberately separate product
//! capabilities. This bridge makes their boundary executable: it checks that every knowledge
//! claim can be traced to the verification report that authorized it, preserves negative and
//! contradictory evidence, exposes stale or omitted evidence, and emits a deterministic route
//! instead of silently promoting an unverified claim. It exchanges identifiers, dispositions,
//! scores, and content digests only; raw source bytes and clinical decisions never cross this
//! boundary.

use super::verification_gate::{EvidenceVerificationDisposition, EvidenceVerificationReport};
use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
    KnowledgeClaimDisposition, TypedKnowledge,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceKnowledgeBridge1@1";
pub const MAX_CLAIMS: usize = 16_384;
pub const MAX_OMISSIONS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKnowledgeBridgeDecision {
    Admit,
    Conditional,
    PreserveNegative,
    RouteContradiction,
    AcquireCoverage,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKnowledgeBridgeDisposition {
    Admitted,
    Conditional,
    Blocked,
    NoClaims,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceKnowledgeBridgeRequest {
    pub objective: String,
    pub verification: EvidenceVerificationReport,
    pub knowledge: TypedKnowledge,
    pub require_verified: bool,
    pub allow_conditional: bool,
    pub max_claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceKnowledgeBridgeOmission {
    pub claim_id: String,
    pub evidence_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceKnowledgeClaimLink {
    pub claim_id: String,
    pub evidence_order: Vec<String>,
    pub verified_support_order: Vec<String>,
    pub verified_negative_order: Vec<String>,
    pub verified_contradiction_order: Vec<String>,
    pub verified_uncertain_order: Vec<String>,
    pub unverified_order: Vec<String>,
    pub unmapped_order: Vec<String>,
    pub knowledge_disposition: KnowledgeClaimDisposition,
    pub verification_disposition: EvidenceVerificationDisposition,
    pub support_milli: u16,
    pub confidence_milli: u16,
    pub decision: EvidenceKnowledgeBridgeDecision,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceKnowledgeBridge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub verification_digest: ContentHash,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub links: Vec<EvidenceKnowledgeClaimLink>,
    pub omissions: Vec<EvidenceKnowledgeBridgeOmission>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub coverage_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub unmapped_eligible_order: Vec<String>,
    pub disposition: EvidenceKnowledgeBridgeDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceKnowledgeBridgeError {
    #[error("evidence-to-knowledge bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-to-knowledge bridge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-to-knowledge bridge digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn evidence_ids(
    claim: &crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::KnowledgeClaim,
) -> BTreeSet<String> {
    claim
        .supporting_evidence_order
        .iter()
        .chain(claim.negative_evidence_order.iter())
        .chain(claim.contradictory_evidence_order.iter())
        .chain(claim.unresolved_evidence_order.iter())
        .cloned()
        .collect()
}

fn digest_input(output: &EvidenceKnowledgeBridge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "verification_digest": output.verification_digest,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "links": output.links,
        "omissions": output.omissions,
        "admitted_order": output.admitted_order,
        "conditional_order": output.conditional_order,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "coverage_order": output.coverage_order,
        "hold_order": output.hold_order,
        "unmapped_eligible_order": output.unmapped_eligible_order,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn decision_route(decision: EvidenceKnowledgeBridgeDecision) -> &'static str {
    match decision {
        EvidenceKnowledgeBridgeDecision::Admit => "glioma_knowledge_protocol_gateway",
        EvidenceKnowledgeBridgeDecision::Conditional => "glioma_multimodal_evidence_gap_router",
        EvidenceKnowledgeBridgeDecision::PreserveNegative => {
            "glioma_federated_evidence_acquisition_policy"
        }
        EvidenceKnowledgeBridgeDecision::RouteContradiction => {
            "plan_glioma_evidence_contradiction_cut"
        }
        EvidenceKnowledgeBridgeDecision::AcquireCoverage => {
            "glioma_federated_evidence_acquisition_policy"
        }
        EvidenceKnowledgeBridgeDecision::Hold => "glioma_evidence_verification_gate",
    }
}

fn validate_request(
    request: &EvidenceKnowledgeBridgeRequest,
) -> Result<(), EvidenceKnowledgeBridgeError> {
    if request.objective.trim().is_empty()
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.knowledge.claims.len() > request.max_claims
    {
        return Err(EvidenceKnowledgeBridgeError::InvalidRequest(
            "objective and bounded claim policy are required".into(),
        ));
    }
    request
        .verification
        .validate()
        .map_err(|error| EvidenceKnowledgeBridgeError::InvalidRequest(error.to_string()))?;
    request
        .knowledge
        .validate()
        .map_err(|error| EvidenceKnowledgeBridgeError::InvalidRequest(error.to_string()))?;
    if request.objective.trim() != request.verification.objective.trim()
        || request.objective.trim() != request.knowledge.objective.trim()
    {
        return Err(EvidenceKnowledgeBridgeError::InvalidRequest(
            "verification, knowledge, and bridge objectives must match exactly".into(),
        ));
    }
    Ok(())
}

impl EvidenceKnowledgeBridge {
    pub fn validate(&self) -> Result<(), EvidenceKnowledgeBridgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.verification_digest.as_str().len() != 64
            || self.knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !unique_nonempty(&self.claim_order)
            || self.links.len() != self.claim_order.len()
            || self
                .links
                .iter()
                .map(|link| link.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.links.iter().any(|link| {
                link.claim_id.trim().is_empty()
                    || !canonical(&link.evidence_order)
                    || !canonical(&link.verified_support_order)
                    || !canonical(&link.verified_negative_order)
                    || !canonical(&link.verified_contradiction_order)
                    || !canonical(&link.verified_uncertain_order)
                    || !canonical(&link.unverified_order)
                    || !canonical(&link.unmapped_order)
                    || link.support_milli > 1_000
                    || link.confidence_milli > 1_000
                    || link.explanation.trim().is_empty()
            })
            || !canonical(&self.admitted_order)
            || !canonical(&self.conditional_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.coverage_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.unmapped_eligible_order)
            || !canonical(
                &self
                    .omissions
                    .iter()
                    .map(|item| format!("{}::{}", item.claim_id, item.evidence_id))
                    .collect::<Vec<_>>(),
            )
            || self.omissions.len() > MAX_OMISSIONS
        {
            return Err(EvidenceKnowledgeBridgeError::InvalidOutput(
                "identity, claim alignment, ordering, score bounds, or omission bounds are invalid"
                    .into(),
            ));
        }
        let partitions = [
            &self.admitted_order,
            &self.conditional_order,
            &self.negative_order,
            &self.contradicted_order,
            &self.coverage_order,
            &self.hold_order,
        ];
        let mut union = BTreeSet::new();
        let mut count = 0;
        for partition in partitions {
            for claim_id in partition {
                union.insert(claim_id.clone());
                count += 1;
            }
        }
        if union != self.claim_order.iter().cloned().collect::<BTreeSet<_>>()
            || count != union.len()
            || self
                .links
                .iter()
                .flat_map(|link| link.evidence_order.iter())
                .any(|evidence_id| evidence_id.trim().is_empty())
            || self.omissions.iter().any(|item| {
                item.claim_id.trim().is_empty()
                    || item.evidence_id.trim().is_empty()
                    || item.reason.trim().is_empty()
            })
        {
            return Err(EvidenceKnowledgeBridgeError::InvalidOutput(
                "claim decision partitions or omission identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceKnowledgeBridgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceKnowledgeBridgeError::Digest(
                "evidence-to-knowledge bridge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Join the P01 verification gate and P02 typed knowledge compiler without silently promoting
/// records that were omitted, stale, contradictory, or outside the verification report.
pub fn bridge_glioma_evidence_to_knowledge(
    request: &EvidenceKnowledgeBridgeRequest,
) -> Result<EvidenceKnowledgeBridge, EvidenceKnowledgeBridgeError> {
    validate_request(request)?;
    let verification = &request.verification;
    let knowledge = &request.knowledge;
    let record_ids = verification
        .record_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let eligible_ids = verification
        .eligible_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let support_ids = verification
        .support_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative_ids = verification
        .negative_order
        .iter()
        .filter(|id| eligible_ids.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let contradiction_ids = verification
        .contradicted_order
        .iter()
        .filter(|id| eligible_ids.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let uncertain_ids = verification
        .uncertain_order
        .iter()
        .filter(|id| eligible_ids.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut links = Vec::with_capacity(knowledge.claims.len());
    let mut omissions = Vec::new();
    let mut admitted = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut coverage = BTreeSet::new();
    let mut hold = BTreeSet::new();
    let mut mapped_eligible = BTreeSet::new();

    for claim in &knowledge.claims {
        let all_evidence = evidence_ids(claim);
        let evidence_order = all_evidence.iter().cloned().collect::<Vec<_>>();
        let verified_support_order = all_evidence
            .intersection(&support_ids)
            .cloned()
            .collect::<Vec<_>>();
        let verified_negative_order = all_evidence
            .intersection(&negative_ids)
            .cloned()
            .collect::<Vec<_>>();
        let verified_contradiction_order = all_evidence
            .intersection(&contradiction_ids)
            .cloned()
            .collect::<Vec<_>>();
        let verified_uncertain_order = all_evidence
            .intersection(&uncertain_ids)
            .cloned()
            .collect::<Vec<_>>();
        let unverified_order = all_evidence
            .difference(&eligible_ids)
            .cloned()
            .collect::<Vec<_>>();
        let unmapped_order = all_evidence
            .difference(&record_ids)
            .cloned()
            .collect::<Vec<_>>();
        mapped_eligible.extend(all_evidence.intersection(&eligible_ids).cloned());
        for evidence_id in &unverified_order {
            omissions.push(EvidenceKnowledgeBridgeOmission {
                claim_id: claim.claim_id.clone(),
                evidence_id: evidence_id.clone(),
                reason: if unmapped_order.contains(evidence_id) {
                    "not-present-in-verification-report".into()
                } else {
                    "not-verification-eligible".into()
                },
            });
        }
        let decision = if !unmapped_order.is_empty() || !unverified_order.is_empty() {
            EvidenceKnowledgeBridgeDecision::AcquireCoverage
        } else if !verified_contradiction_order.is_empty()
            || claim.disposition == KnowledgeClaimDisposition::Contested
        {
            EvidenceKnowledgeBridgeDecision::RouteContradiction
        } else {
            match claim.disposition {
                KnowledgeClaimDisposition::Negative => {
                    EvidenceKnowledgeBridgeDecision::PreserveNegative
                }
                KnowledgeClaimDisposition::Supported
                    if verification.disposition == EvidenceVerificationDisposition::Verified =>
                {
                    EvidenceKnowledgeBridgeDecision::Admit
                }
                KnowledgeClaimDisposition::Supported if request.allow_conditional => {
                    EvidenceKnowledgeBridgeDecision::Conditional
                }
                KnowledgeClaimDisposition::Supported | KnowledgeClaimDisposition::Unresolved => {
                    EvidenceKnowledgeBridgeDecision::AcquireCoverage
                }
                KnowledgeClaimDisposition::Contested => {
                    EvidenceKnowledgeBridgeDecision::RouteContradiction
                }
            }
        };
        let decision = if request.require_verified
            && verification.disposition != EvidenceVerificationDisposition::Verified
            && matches!(
                decision,
                EvidenceKnowledgeBridgeDecision::Admit
                    | EvidenceKnowledgeBridgeDecision::Conditional
            ) {
            EvidenceKnowledgeBridgeDecision::AcquireCoverage
        } else {
            decision
        };
        match decision {
            EvidenceKnowledgeBridgeDecision::Admit => {
                admitted.insert(claim.claim_id.clone());
            }
            EvidenceKnowledgeBridgeDecision::Conditional => {
                conditional.insert(claim.claim_id.clone());
            }
            EvidenceKnowledgeBridgeDecision::PreserveNegative => {
                negative.insert(claim.claim_id.clone());
            }
            EvidenceKnowledgeBridgeDecision::RouteContradiction => {
                contradicted.insert(claim.claim_id.clone());
            }
            EvidenceKnowledgeBridgeDecision::AcquireCoverage => {
                coverage.insert(claim.claim_id.clone());
            }
            EvidenceKnowledgeBridgeDecision::Hold => {
                hold.insert(claim.claim_id.clone());
            }
        }
        let explanation = format!(
            "knowledge={:?};verification={:?};evidence={};eligible={};support={};negative={};contradiction={};unverified={};unmapped={};route={}",
            claim.disposition,
            verification.disposition,
            evidence_order.len(),
            evidence_order
                .iter()
                .filter(|id| eligible_ids.contains(*id))
                .count(),
            verified_support_order.len(),
            verified_negative_order.len(),
            verified_contradiction_order.len(),
            unverified_order.len(),
            unmapped_order.len(),
            decision_route(decision),
        );
        links.push(EvidenceKnowledgeClaimLink {
            claim_id: claim.claim_id.clone(),
            evidence_order,
            verified_support_order,
            verified_negative_order,
            verified_contradiction_order,
            verified_uncertain_order,
            unverified_order,
            unmapped_order,
            knowledge_disposition: claim.disposition,
            verification_disposition: verification.disposition,
            support_milli: claim.support_milli,
            confidence_milli: claim.confidence_milli,
            decision,
            explanation,
        });
    }

    let unmapped_eligible_order = eligible_ids
        .difference(&mapped_eligible)
        .cloned()
        .collect::<Vec<_>>();
    for evidence_id in &unmapped_eligible_order {
        omissions.push(EvidenceKnowledgeBridgeOmission {
            claim_id: "__unmapped__".into(),
            evidence_id: evidence_id.clone(),
            reason: "eligible-evidence-not-represented-in-knowledge".into(),
        });
    }
    omissions.sort_by(|left, right| {
        left.claim_id
            .cmp(&right.claim_id)
            .then(left.evidence_id.cmp(&right.evidence_id))
            .then(left.reason.cmp(&right.reason))
    });
    omissions.dedup_by(|left, right| {
        left.claim_id == right.claim_id
            && left.evidence_id == right.evidence_id
            && left.reason == right.reason
    });
    if omissions.len() > MAX_OMISSIONS {
        return Err(EvidenceKnowledgeBridgeError::InvalidOutput(
            "bridge omission bound exceeded".into(),
        ));
    }
    let claim_order = links
        .iter()
        .map(|link| link.claim_id.clone())
        .collect::<Vec<_>>();
    let next_route = if !contradicted.is_empty() {
        "plan_glioma_evidence_contradiction_cut"
    } else if !coverage.is_empty() || !unmapped_eligible_order.is_empty() {
        "glioma_federated_evidence_acquisition_policy"
    } else if !conditional.is_empty() {
        "glioma_multimodal_evidence_gap_router"
    } else if !hold.is_empty() {
        "glioma_evidence_verification_gate"
    } else {
        "glioma_knowledge_protocol_gateway"
    };
    let disposition = if claim_order.is_empty() {
        EvidenceKnowledgeBridgeDisposition::NoClaims
    } else if !contradicted.is_empty()
        || !coverage.is_empty()
        || !hold.is_empty()
        || !unmapped_eligible_order.is_empty()
    {
        EvidenceKnowledgeBridgeDisposition::Blocked
    } else if !conditional.is_empty() {
        EvidenceKnowledgeBridgeDisposition::Conditional
    } else {
        EvidenceKnowledgeBridgeDisposition::Admitted
    };
    let mut output = EvidenceKnowledgeBridge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        verification_digest: verification.digest.clone(),
        knowledge_digest: knowledge.digest.clone(),
        claim_order,
        links,
        omissions,
        admitted_order: admitted.into_iter().collect(),
        conditional_order: conditional.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        contradicted_order: contradicted.into_iter().collect(),
        coverage_order: coverage.into_iter().collect(),
        hold_order: hold.into_iter().collect(),
        unmapped_eligible_order,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-knowledge-bridge"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceKnowledgeBridgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p01_evidence_surveillance::verification_gate::EvidenceVerificationRequest;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn record(id: &str, state: EvidenceState) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(&format!("artifact-{id}")),
            source_kind: if id == "e2" {
                EvidenceSourceKind::Dataset
            } else {
                EvidenceSourceKind::Literature
            },
            claim: "EGFR invasion".into(),
            scope: "organoid preclinical glioma".into(),
            modality: if id == "e2" {
                GliomaModality::Transcriptomics
            } else {
                GliomaModality::Imaging
            },
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 5,
        }
    }

    fn surfaces(states: &[EvidenceState]) -> (EvidenceVerificationReport, TypedKnowledge) {
        let records = states
            .iter()
            .enumerate()
            .map(|(index, state)| record(&format!("e{}", index + 1), *state))
            .collect::<Vec<_>>();
        let verification_request = EvidenceVerificationRequest {
            objective: "compile EGFR invasion knowledge".into(),
            claim_terms: vec!["invasion".into()],
            scope_terms: vec!["organoid".into()],
            required_modalities: BTreeSet::new(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            required_source_kinds: BTreeSet::new(),
            minimum_quality_milli: 500,
            minimum_reproducibility_milli: 500,
            current_epoch: 5,
            maximum_age_epochs: 10,
            minimum_supporting_records: 1,
            minimum_independent_source_kinds: 1,
            minimum_modalities: 1,
            minimum_model_systems: 1,
            allow_unknown: false,
            require_negative_review: false,
            require_contradiction_resolution: false,
            records: records.clone(),
        };
        let verification =
            super::super::verification_gate::verify_glioma_evidence(&verification_request).unwrap();
        let knowledge_request = KnowledgeRequest {
            objective: verification_request.objective.clone(),
            required_modalities: BTreeSet::new(),
            required_model_systems: BTreeSet::new(),
            min_support_milli: 500,
            min_sources_per_claim: 1,
            max_claims: 8,
        };
        let knowledge = compile_typed_knowledge(&knowledge_request, &records).unwrap();
        (verification, knowledge)
    }

    fn request(
        verification: EvidenceVerificationReport,
        knowledge: TypedKnowledge,
    ) -> EvidenceKnowledgeBridgeRequest {
        EvidenceKnowledgeBridgeRequest {
            objective: "compile EGFR invasion knowledge".into(),
            verification,
            knowledge,
            require_verified: true,
            allow_conditional: false,
            max_claims: 8,
        }
    }

    #[test]
    fn verified_supported_claim_is_admitted_with_digest_bound_handoff() {
        let (verification, knowledge) = surfaces(&[EvidenceState::Supported]);
        let bridge =
            bridge_glioma_evidence_to_knowledge(&request(verification, knowledge)).unwrap();
        assert_eq!(
            bridge.disposition,
            EvidenceKnowledgeBridgeDisposition::Admitted
        );
        assert_eq!(bridge.admitted_order.len(), 1);
        assert_eq!(bridge.next_route, "glioma_knowledge_protocol_gateway");
        bridge.validate().unwrap();
    }

    #[test]
    fn contradiction_is_never_admitted_by_supporting_majority() {
        let (verification, knowledge) =
            surfaces(&[EvidenceState::Supported, EvidenceState::Contradicted]);
        let bridge =
            bridge_glioma_evidence_to_knowledge(&request(verification, knowledge)).unwrap();
        assert_eq!(
            bridge.disposition,
            EvidenceKnowledgeBridgeDisposition::Blocked
        );
        assert_eq!(bridge.contradicted_order.len(), 1);
        assert_eq!(bridge.next_route, "plan_glioma_evidence_contradiction_cut");
        bridge.validate().unwrap();
    }

    #[test]
    fn stale_evidence_becomes_explicit_coverage_debt() {
        let (verification, knowledge) = surfaces(&[EvidenceState::Stale]);
        let bridge =
            bridge_glioma_evidence_to_knowledge(&request(verification, knowledge)).unwrap();
        assert_eq!(
            bridge.disposition,
            EvidenceKnowledgeBridgeDisposition::Blocked
        );
        assert_eq!(bridge.coverage_order.len(), 1);
        assert!(bridge
            .omissions
            .iter()
            .any(|item| item.reason == "not-verification-eligible"));
        bridge.validate().unwrap();
    }

    #[test]
    fn bridge_replays_identically_for_same_verified_surfaces() {
        let (verification, knowledge) =
            surfaces(&[EvidenceState::Supported, EvidenceState::Negative]);
        let first =
            bridge_glioma_evidence_to_knowledge(&request(verification.clone(), knowledge.clone()))
                .unwrap();
        let second =
            bridge_glioma_evidence_to_knowledge(&request(verification, knowledge)).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.omissions, second.omissions);
    }
}
