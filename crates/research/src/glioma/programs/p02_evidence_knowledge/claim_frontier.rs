//! Scientific claim-frontier prioritization for the autonomous glioma workflow.
//!
//! Typed knowledge tells the engine what the evidence currently supports.  This module decides
//! which claim should drive the next work cycle by combining coverage debt, contradiction,
//! unresolved evidence, support, and workflow leverage.  It is a prioritizer, not a truth oracle:
//! every score carries its reason and the caller can pass the selected claim ids to the P04
//! decision-context compiler.  No evidence is fetched, rewritten, or promoted by this module.

use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeFrontier1@1";
pub const MAX_CLAIMS: usize = 4_096;
const SCORE_SCALE: u64 = 1_000;

/// Weights are explicit and must sum to 1,000 milli-weight units.  This makes a frontier policy
/// reviewable and allows a lab to emphasize replication, coverage, or supported mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeFrontierWeights {
    pub coverage_debt_milli: u16,
    pub contradiction_milli: u16,
    pub uncertainty_milli: u16,
    pub support_milli: u16,
    pub workflow_leverage_milli: u16,
}

impl Default for KnowledgeFrontierWeights {
    fn default() -> Self {
        Self {
            coverage_debt_milli: 250,
            contradiction_milli: 250,
            uncertainty_milli: 200,
            support_milli: 150,
            workflow_leverage_milli: 150,
        }
    }
}

impl KnowledgeFrontierWeights {
    fn validate(self) -> Result<(), KnowledgeFrontierError> {
        let total = u32::from(self.coverage_debt_milli)
            + u32::from(self.contradiction_milli)
            + u32::from(self.uncertainty_milli)
            + u32::from(self.support_milli)
            + u32::from(self.workflow_leverage_milli);
        if total != SCORE_SCALE as u32
            || [
                self.coverage_debt_milli,
                self.contradiction_milli,
                self.uncertainty_milli,
                self.support_milli,
                self.workflow_leverage_milli,
            ]
            .iter()
            .any(|value| u64::from(*value) > SCORE_SCALE)
        {
            return Err(KnowledgeFrontierError::InvalidRequest(
                "frontier weights must sum to 1,000 milli-units and remain bounded".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeFrontierRequest {
    pub objective: String,
    pub max_selected_claims: usize,
    pub min_priority_milli: u16,
    pub weights: KnowledgeFrontierWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierActionKind {
    CloseCoverage,
    ResolveContradiction,
    ResolveUncertainty,
    RevalidateNegative,
    ValidateSupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeFrontierScore {
    pub claim_id: String,
    pub action_kind: FrontierActionKind,
    pub priority_milli: u16,
    pub coverage_debt_milli: u16,
    pub contradiction_milli: u16,
    pub uncertainty_milli: u16,
    pub support_milli: u16,
    pub workflow_leverage_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeFrontierDisposition {
    Qualified,
    Partial,
    NoClaims,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub ranking: Vec<KnowledgeFrontierScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: KnowledgeFrontierDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeFrontierError {
    #[error("knowledge frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge frontier input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge frontier digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &KnowledgeFrontier) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "ranking": output.ranking,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

impl KnowledgeFrontier {
    pub fn validate(&self) -> Result<(), KnowledgeFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.ranking.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].claim_id > pair[1].claim_id)
            })
            || self.ranking.iter().any(|score| {
                score.claim_id.trim().is_empty()
                    || score.priority_milli > SCORE_SCALE as u16
                    || score.coverage_debt_milli > SCORE_SCALE as u16
                    || score.contradiction_milli > SCORE_SCALE as u16
                    || score.uncertainty_milli > SCORE_SCALE as u16
                    || score.support_milli > SCORE_SCALE as u16
                    || score.workflow_leverage_milli > SCORE_SCALE as u16
                    || score.rationale.trim().is_empty()
            })
        {
            return Err(KnowledgeFrontierError::InvalidOutput(
                "identity, ordering, score bounds, or rationale is invalid".into(),
            ));
        }
        let claims = self.claim_order.iter().collect::<BTreeSet<_>>();
        let ranked = self
            .ranking
            .iter()
            .map(|score| &score.claim_id)
            .collect::<BTreeSet<_>>();
        if claims != ranked
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .any(|id| !claims.contains(id))
            || self
                .selected_order
                .iter()
                .any(|id| self.deferred_order.binary_search(id).is_ok())
        {
            return Err(KnowledgeFrontierError::InvalidOutput(
                "claim ranking and selected/deferred partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeFrontierError::InvalidOutput(
                "knowledge frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &KnowledgeFrontierRequest) -> Result<(), KnowledgeFrontierError> {
    if request.objective.trim().is_empty()
        || request.max_selected_claims == 0
        || request.max_selected_claims > MAX_CLAIMS
        || u64::from(request.min_priority_milli) > SCORE_SCALE
    {
        return Err(KnowledgeFrontierError::InvalidRequest(
            "objective, selected-claim bound, and bounded minimum priority are required".into(),
        ));
    }
    request.weights.validate()
}

fn coverage_debt(claim: &super::knowledge_graph::KnowledgeClaim) -> u16 {
    let missing = claim.missing_modality_order.len() + claim.missing_model_system_order.len();
    ((missing.min(4) * 250) as u16).min(SCORE_SCALE as u16)
}

fn uncertainty_score(claim: &super::knowledge_graph::KnowledgeClaim) -> u16 {
    let unresolved = claim.unresolved_evidence_order.len();
    let base = (unresolved.min(4) * 200) as u16;
    base.saturating_add(
        if claim.disposition == KnowledgeClaimDisposition::Unresolved {
            400
        } else {
            0
        },
    )
    .min(SCORE_SCALE as u16)
}

fn workflow_leverage(claim: &super::knowledge_graph::KnowledgeClaim) -> u16 {
    if !claim.missing_modality_order.is_empty() || !claim.missing_model_system_order.is_empty() {
        950
    } else if !claim.contradictory_evidence_order.is_empty() {
        900
    } else if !claim.unresolved_evidence_order.is_empty() {
        850
    } else if claim.disposition == KnowledgeClaimDisposition::Negative {
        800
    } else {
        700
    }
}

fn action_kind(claim: &super::knowledge_graph::KnowledgeClaim) -> FrontierActionKind {
    if !claim.missing_modality_order.is_empty() || !claim.missing_model_system_order.is_empty() {
        FrontierActionKind::CloseCoverage
    } else if !claim.contradictory_evidence_order.is_empty() {
        FrontierActionKind::ResolveContradiction
    } else if claim.disposition == KnowledgeClaimDisposition::Negative {
        FrontierActionKind::RevalidateNegative
    } else if !claim.unresolved_evidence_order.is_empty()
        || claim.disposition == KnowledgeClaimDisposition::Unresolved
    {
        FrontierActionKind::ResolveUncertainty
    } else {
        FrontierActionKind::ValidateSupported
    }
}

fn rationale(claim: &super::knowledge_graph::KnowledgeClaim, action: FrontierActionKind) -> String {
    match action {
        FrontierActionKind::CloseCoverage => format!(
            "close {} missing modality/model coverage before promoting this claim",
            claim.missing_modality_order.len() + claim.missing_model_system_order.len()
        ),
        FrontierActionKind::ResolveContradiction => format!(
            "reconcile {} contradictory evidence record(s) with an independent preclinical check",
            claim.contradictory_evidence_order.len()
        ),
        FrontierActionKind::ResolveUncertainty => format!(
            "resolve {} unknown, stale, or unmeasured evidence record(s) before execution",
            claim.unresolved_evidence_order.len()
        ),
        FrontierActionKind::RevalidateNegative => {
            "revalidate the negative result without erasing it from the evidence ledger".into()
        }
        FrontierActionKind::ValidateSupported => {
            "validate this supported claim with a discriminating local mechanism action".into()
        }
    }
}

/// Rank claims for the next autonomous research cycle.
pub fn prioritize_knowledge_frontier(
    request: &KnowledgeFrontierRequest,
    knowledge: &TypedKnowledge,
) -> Result<KnowledgeFrontier, KnowledgeFrontierError> {
    validate_request(request)?;
    knowledge
        .validate()
        .map_err(|error| KnowledgeFrontierError::InvalidInput(error.to_string()))?;
    if request.objective.trim() != knowledge.objective.trim() {
        return Err(KnowledgeFrontierError::InvalidInput(
            "frontier objective must match typed knowledge objective".into(),
        ));
    }
    if knowledge.claims.len() > MAX_CLAIMS {
        return Err(KnowledgeFrontierError::InvalidInput(
            "typed knowledge claim count exceeds the frontier bound".into(),
        ));
    }
    let mut ranking = knowledge
        .claims
        .iter()
        .map(|claim| {
            let coverage_debt_milli = coverage_debt(claim);
            let contradiction_milli = claim.contradiction_milli;
            let uncertainty_milli = uncertainty_score(claim);
            let support_milli = claim.confidence_milli;
            let workflow_leverage_milli = workflow_leverage(claim);
            let action_kind = action_kind(claim);
            let weighted = u64::from(coverage_debt_milli)
                .saturating_mul(u64::from(request.weights.coverage_debt_milli))
                .saturating_add(
                    u64::from(contradiction_milli)
                        .saturating_mul(u64::from(request.weights.contradiction_milli)),
                )
                .saturating_add(
                    u64::from(uncertainty_milli)
                        .saturating_mul(u64::from(request.weights.uncertainty_milli)),
                )
                .saturating_add(
                    u64::from(support_milli)
                        .saturating_mul(u64::from(request.weights.support_milli)),
                )
                .saturating_add(
                    u64::from(workflow_leverage_milli)
                        .saturating_mul(u64::from(request.weights.workflow_leverage_milli)),
                );
            let priority_milli = weighted.saturating_div(SCORE_SCALE).min(SCORE_SCALE) as u16;
            KnowledgeFrontierScore {
                claim_id: claim.claim_id.clone(),
                action_kind,
                priority_milli,
                coverage_debt_milli,
                contradiction_milli,
                uncertainty_milli,
                support_milli,
                workflow_leverage_milli,
                rationale: rationale(claim, action_kind),
            }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    let claim_order = knowledge.claim_order.clone();
    let mut selected_order = ranking
        .iter()
        .filter(|score| score.priority_milli >= request.min_priority_milli)
        .take(request.max_selected_claims)
        .map(|score| score.claim_id.clone())
        .collect::<Vec<_>>();
    selected_order.sort();
    let selected_set = selected_order.iter().collect::<BTreeSet<_>>();
    let deferred_order = ranking
        .iter()
        .filter(|score| !selected_set.contains(&score.claim_id))
        .map(|score| score.claim_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = if knowledge.claims.is_empty() {
        KnowledgeFrontierDisposition::NoClaims
    } else if selected_order.is_empty() {
        KnowledgeFrontierDisposition::Unresolved
    } else if knowledge.disposition == super::knowledge_graph::KnowledgeDisposition::Qualified
        && deferred_order.is_empty()
    {
        KnowledgeFrontierDisposition::Qualified
    } else {
        KnowledgeFrontierDisposition::Partial
    };
    let mut output = KnowledgeFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: knowledge.digest.clone(),
        claim_order,
        ranking,
        selected_order,
        deferred_order,
        negative_evidence_order: knowledge.negative_evidence_order.clone(),
        uncertainty_order: knowledge.uncertainty_order.clone(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-frontier"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact:{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn knowledge() -> TypedKnowledge {
        compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "prioritize glioma invasion frontier".into(),
                required_modalities: BTreeSet::new(),
                required_model_systems: BTreeSet::new(),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[
                EvidenceRecord {
                    evidence_id: "support-egfr".into(),
                    source_artifact: artifact("support-egfr"),
                    source_kind: EvidenceSourceKind::Dataset,
                    claim: "EGFR signaling increases invasion".into(),
                    scope: "preclinical glioma".into(),
                    modality: GliomaModality::Genomics,
                    model_system: Some(GliomaModelSystem::Organoid),
                    state: EvidenceState::Supported,
                    relevance_milli: 950,
                    quality_milli: 950,
                    reproducibility_milli: 950,
                    release_epoch: 1,
                },
                EvidenceRecord {
                    evidence_id: "negative-matrix".into(),
                    source_artifact: artifact("negative-matrix"),
                    source_kind: EvidenceSourceKind::Dataset,
                    claim: "Matrix remodeling changes invasion".into(),
                    scope: "preclinical glioma".into(),
                    modality: GliomaModality::Genomics,
                    model_system: Some(GliomaModelSystem::Organoid),
                    state: EvidenceState::Negative,
                    relevance_milli: 800,
                    quality_milli: 800,
                    reproducibility_milli: 800,
                    release_epoch: 1,
                },
            ],
        )
        .unwrap()
    }

    fn request() -> KnowledgeFrontierRequest {
        KnowledgeFrontierRequest {
            objective: "prioritize glioma invasion frontier".into(),
            max_selected_claims: 2,
            min_priority_milli: 0,
            weights: Default::default(),
        }
    }

    #[test]
    fn frontier_exposes_action_mode_and_ranked_claims() {
        let output = prioritize_knowledge_frontier(&request(), &knowledge()).unwrap();
        assert_eq!(output.ranking.len(), 2);
        assert!(output
            .ranking
            .iter()
            .any(|score| score.action_kind == FrontierActionKind::RevalidateNegative));
        assert_eq!(output.selected_order.len(), 2);
        output.validate().unwrap();
    }

    #[test]
    fn frontier_is_permutation_stable() {
        let first = prioritize_knowledge_frontier(&request(), &knowledge()).unwrap();
        let second = prioritize_knowledge_frontier(&request(), &knowledge()).unwrap();
        assert_eq!(first.ranking, second.ranking);
        assert_eq!(first.selected_order, second.selected_order);
    }

    #[test]
    fn objective_mismatch_is_rejected() {
        let mut request = request();
        request.objective = "different".into();
        let error = prioritize_knowledge_frontier(&request, &knowledge()).unwrap_err();
        assert!(error.to_string().contains("objective"));
    }
}
