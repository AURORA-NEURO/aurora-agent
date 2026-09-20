//! Contradiction-aware consistency closure for typed preclinical glioma knowledge.
//!
//! The typed knowledge compiler intentionally preserves every claim. This feature adds the next
//! scientific operation needed by an autonomous engine: compute an action-ready closure without
//! deleting a lower-scoring rival. Support edges increase a claim's usable score, contradiction
//! edges impose a bounded penalty, and unresolved/negative claims remain visible with explicit
//! acquisition or adjudication actions.

use super::composition::{KnowledgeRelation, KnowledgeRelationKind};
use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeConsistencyClosure1@1";
pub const MAX_RELATIONS: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeConsistencyRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub relations: Vec<KnowledgeRelation>,
    pub min_support_milli: u16,
    pub max_conflict_milli: u16,
    pub min_relation_strength_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeConsistencyClaimDisposition {
    Selected,
    Contested,
    Unresolved,
    ExcludedNegative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeConsistencyClaimScore {
    pub claim_id: String,
    pub base_confidence_milli: u16,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub net_score_milli: u16,
    pub disposition: KnowledgeConsistencyClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeConsistencyDisposition {
    Ready,
    Partial,
    Contested,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeConsistencyClosure {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub selected_claim_order: Vec<String>,
    pub contested_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub excluded_negative_order: Vec<String>,
    pub contradiction_relation_order: Vec<String>,
    pub support_relation_order: Vec<String>,
    pub scores: Vec<KnowledgeConsistencyClaimScore>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeConsistencyDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeConsistencyError {
    #[error("knowledge consistency request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge consistency output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge consistency digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn clamp(value: i64) -> u16 {
    value.clamp(0, 1_000) as u16
}

fn digest_input(output: &KnowledgeConsistencyClosure) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "selected_claim_order": output.selected_claim_order,
        "contested_claim_order": output.contested_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "excluded_negative_order": output.excluded_negative_order,
        "contradiction_relation_order": output.contradiction_relation_order,
        "support_relation_order": output.support_relation_order,
        "scores": output.scores,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl KnowledgeConsistencyClosure {
    pub fn validate(&self) -> Result<(), KnowledgeConsistencyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.selected_claim_order)
            || !canonical(&self.contested_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.excluded_negative_order)
            || !canonical(&self.contradiction_relation_order)
            || !canonical(&self.support_relation_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.scores.len() != self.claim_order.len()
            || self
                .scores
                .iter()
                .map(|score| score.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.scores.iter().any(|score| {
                score.base_confidence_milli > 1_000
                    || score.support_milli > 1_000
                    || score.contradiction_milli > 1_000
                    || score.net_score_milli > 1_000
            })
        {
            return Err(KnowledgeConsistencyError::InvalidOutput(
                "closure identity, partitions, ordering, score bounds, or digest binding is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeConsistencyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeConsistencyError::InvalidOutput(
                "knowledge consistency digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &KnowledgeConsistencyRequest,
) -> Result<(), KnowledgeConsistencyError> {
    if request.objective.trim().is_empty()
        || request.knowledge.objective != request.objective
        || request.min_support_milli > 1_000
        || request.max_conflict_milli > 1_000
        || request.min_relation_strength_milli > 1_000
        || request.relations.len() > MAX_RELATIONS
    {
        return Err(KnowledgeConsistencyError::InvalidRequest(
            "objective, knowledge binding, score bounds, and bounded relations are required".into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| KnowledgeConsistencyError::InvalidRequest(error.to_string()))?;
    let claims = request
        .knowledge
        .claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut relation_ids = BTreeSet::new();
    for relation in &request.relations {
        if relation.relation_id.trim().is_empty()
            || !relation_ids.insert(relation.relation_id.clone())
            || relation.from_claim_id == relation.to_claim_id
            || !claims.contains(&relation.from_claim_id)
            || !claims.contains(&relation.to_claim_id)
            || relation.strength_milli > 1_000
        {
            return Err(KnowledgeConsistencyError::InvalidRequest(
                "relations must be unique, non-self, claim-bound, and score-bounded".into(),
            ));
        }
    }
    Ok(())
}

/// Compute a deterministic support/contradiction closure over typed knowledge claims.
pub fn compile_glioma_knowledge_consistency(
    request: &KnowledgeConsistencyRequest,
) -> Result<KnowledgeConsistencyClosure, KnowledgeConsistencyError> {
    validate_request(request)?;
    let claim_map = request
        .knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let mut support = BTreeMap::<String, u16>::new();
    let mut contradiction = BTreeMap::<String, u16>::new();
    let mut contradiction_relations = BTreeSet::new();
    let mut support_relations = BTreeSet::new();
    for relation in &request.relations {
        match relation.kind {
            KnowledgeRelationKind::Supports | KnowledgeRelationKind::Requires
                if relation.strength_milli >= request.min_relation_strength_milli =>
            {
                let entry = support.entry(relation.to_claim_id.clone()).or_default();
                *entry = entry.saturating_add(relation.strength_milli).min(1_000);
                support_relations.insert(relation.relation_id.clone());
            }
            KnowledgeRelationKind::Contradicts => {
                let from = contradiction
                    .entry(relation.from_claim_id.clone())
                    .or_default();
                *from = from.saturating_add(relation.strength_milli).min(1_000);
                let to = contradiction
                    .entry(relation.to_claim_id.clone())
                    .or_default();
                *to = to.saturating_add(relation.strength_milli).min(1_000);
                contradiction_relations.insert(relation.relation_id.clone());
            }
            _ => {}
        }
    }
    let mut scores = Vec::new();
    let mut selected = BTreeSet::new();
    let mut contested = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut excluded_negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for claim_id in &request.knowledge.claim_order {
        let claim = claim_map
            .get(claim_id)
            .expect("validated knowledge contains every claim id");
        let support_milli = support.get(claim_id).copied().unwrap_or_default();
        let contradiction_milli = contradiction.get(claim_id).copied().unwrap_or_default();
        let net = clamp(
            i64::from(claim.confidence_milli) + i64::from(support_milli) / 2
                - i64::from(contradiction_milli),
        );
        let disposition = if matches!(claim.disposition, KnowledgeClaimDisposition::Negative) {
            excluded_negative.insert(claim_id.clone());
            KnowledgeConsistencyClaimDisposition::ExcludedNegative
        } else if contradiction_milli >= request.max_conflict_milli {
            contested.insert(claim_id.clone());
            uncertainty.insert(format!("{claim_id}:contradiction-floor-exceeded"));
            KnowledgeConsistencyClaimDisposition::Contested
        } else if matches!(claim.disposition, KnowledgeClaimDisposition::Supported)
            && net >= request.min_support_milli
        {
            selected.insert(claim_id.clone());
            KnowledgeConsistencyClaimDisposition::Selected
        } else {
            unresolved.insert(claim_id.clone());
            uncertainty.insert(format!("{claim_id}:support-closure-incomplete"));
            KnowledgeConsistencyClaimDisposition::Unresolved
        };
        scores.push(KnowledgeConsistencyClaimScore {
            claim_id: claim_id.clone(),
            base_confidence_milli: claim.confidence_milli,
            support_milli,
            contradiction_milli,
            net_score_milli: net,
            disposition,
        });
    }
    let mut next_action = BTreeSet::new();
    next_action.extend(
        contested
            .iter()
            .map(|claim_id| format!("adjudicate-contradiction:{claim_id}")),
    );
    next_action.extend(
        unresolved
            .iter()
            .map(|claim_id| format!("acquire-support:{claim_id}")),
    );
    let disposition = if selected.is_empty() {
        KnowledgeConsistencyDisposition::Unresolved
    } else if !contested.is_empty() {
        KnowledgeConsistencyDisposition::Contested
    } else if !unresolved.is_empty() {
        KnowledgeConsistencyDisposition::Partial
    } else {
        KnowledgeConsistencyDisposition::Ready
    };
    let mut output = KnowledgeConsistencyClosure {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: request.knowledge.digest.clone(),
        claim_order: request.knowledge.claim_order.clone(),
        selected_claim_order: selected.into_iter().collect(),
        contested_claim_order: contested.into_iter().collect(),
        unresolved_claim_order: unresolved.into_iter().collect(),
        excluded_negative_order: excluded_negative.into_iter().collect(),
        contradiction_relation_order: contradiction_relations.into_iter().collect(),
        support_relation_order: support_relations.into_iter().collect(),
        scores,
        next_action_order: next_action.into_iter().collect(),
        negative_evidence: vec![
            "consistency-closure-does-not-prove-causality-or-clinical-validity".into(),
        ],
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-consistency"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeConsistencyError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_preserves_fixed_point_score_bounds() {
        assert_eq!(clamp(-5), 0);
        assert_eq!(clamp(500), 500);
        assert_eq!(clamp(2_000), 1_000);
    }
}
