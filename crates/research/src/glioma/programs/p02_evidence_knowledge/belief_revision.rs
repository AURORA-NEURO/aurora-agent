//! Contradiction-aware belief revision for the preclinical glioma knowledge graph.
//!
//! Typed knowledge compilation intentionally preserves competing claims instead of averaging
//! them away.  This feature turns that preserved state into an executable reasoning artifact: an
//! explicit conflict graph is searched for a bounded maximal-consistency portfolio, while weaker
//! rival claims, negative evidence, unresolved coverage, and the next frontier remain visible.
//! The algorithm never infers a contradiction from wording; callers must provide typed conflict
//! edges from an evidence adjudicator or researcher.  It is a research prioritisation tool, not a
//! clinical conclusion or a treatment recommender.

use super::knowledge_graph::{KnowledgeClaimDisposition, KnowledgeError, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaBeliefRevision1@1";
pub const MAX_CLAIMS: usize = 2_048;
pub const MAX_CONFLICTS: usize = 8_192;
pub const MAX_BEAM_WIDTH: usize = 128;

/// An explicit conflict supplied by a typed evidence adjudicator.  The revision engine never
/// manufactures conflicts from lexical similarity or model confidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefConflict {
    pub conflict_id: String,
    pub left_claim_id: String,
    pub right_claim_id: String,
    pub contradiction_milli: u16,
    pub evidence_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefRevisionRequest {
    pub objective: String,
    pub min_support_milli: u16,
    pub min_conflict_milli: u16,
    pub max_hypotheses: usize,
    pub beam_width: usize,
    pub allow_contested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefRevisionDecisionKind {
    Retained,
    Rival,
    Unresolved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefRevisionDecision {
    pub claim_id: String,
    pub kind: BeliefRevisionDecisionKind,
    pub utility_milli: u16,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub conflict_order: Vec<String>,
    pub rationale: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefRevisionDisposition {
    Qualified,
    Contested,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefRevision {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub retained_claim_order: Vec<String>,
    pub rival_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub rejected_claim_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub conflicts: Vec<BeliefConflict>,
    pub decisions: Vec<BeliefRevisionDecision>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: BeliefRevisionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BeliefRevisionError {
    #[error("belief revision request is invalid: {0}")]
    InvalidRequest(String),
    #[error("belief revision knowledge is invalid: {0}")]
    InvalidKnowledge(String),
    #[error("belief revision conflict is invalid: {0}")]
    InvalidConflict(String),
    #[error("belief revision output is invalid: {0}")]
    InvalidOutput(String),
    #[error("belief revision digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct Candidate {
    claim_id: String,
    utility_milli: u16,
    support_milli: u16,
    contradiction_milli: u16,
}

#[derive(Debug, Clone)]
struct BeamState {
    selected: Vec<String>,
    selected_set: BTreeSet<String>,
    score: i64,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &BeliefRevision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_knowledge_digest": output.source_knowledge_digest,
        "claim_order": output.claim_order,
        "retained_claim_order": output.retained_claim_order,
        "rival_claim_order": output.rival_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "rejected_claim_order": output.rejected_claim_order,
        "frontier_order": output.frontier_order,
        "conflicts": output.conflicts,
        "decisions": output.decisions,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn claim_utility(
    claim: &super::knowledge_graph::KnowledgeClaim,
    request: &BeliefRevisionRequest,
) -> Option<Candidate> {
    let eligible = match claim.disposition {
        KnowledgeClaimDisposition::Supported => claim.support_milli >= request.min_support_milli,
        KnowledgeClaimDisposition::Contested => {
            request.allow_contested && claim.support_milli >= request.min_support_milli
        }
        KnowledgeClaimDisposition::Negative | KnowledgeClaimDisposition::Unresolved => false,
    };
    if !eligible {
        return None;
    }
    let evidence_diversity = (claim.supporting_evidence_order.len() as u16)
        .saturating_mul(25)
        .min(250);
    let contradiction_penalty = claim.contradiction_milli / 4;
    let utility = claim
        .confidence_milli
        .saturating_mul(3)
        .saturating_add(claim.support_milli / 2)
        .saturating_add(evidence_diversity)
        .saturating_sub(contradiction_penalty)
        .min(1_000);
    Some(Candidate {
        claim_id: claim.claim_id.clone(),
        utility_milli: utility,
        support_milli: claim.support_milli,
        contradiction_milli: claim.contradiction_milli,
    })
}

fn conflict_key(left: &str, right: &str) -> (String, String) {
    if left < right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

fn state_score(state: &BeamState, candidates: &BTreeMap<String, Candidate>) -> i64 {
    let utility = state
        .selected
        .iter()
        .filter_map(|id| candidates.get(id))
        .map(|candidate| i64::from(candidate.utility_milli))
        .sum::<i64>();
    utility * 1_000 + i64::try_from(state.selected.len()).unwrap_or(i64::MAX)
}

impl BeliefRevision {
    pub fn validate(&self) -> Result<(), BeliefRevisionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.source_knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.retained_claim_order)
            || !canonical(&self.rival_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.rejected_claim_order)
            || self.frontier_order.iter().any(|id| id.trim().is_empty())
            || self.frontier_order.iter().collect::<BTreeSet<_>>().len()
                != self.frontier_order.len()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.decisions.len() != self.claim_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.decisions.iter().any(|decision| {
                decision.utility_milli > 1_000
                    || decision.support_milli > 1_000
                    || decision.contradiction_milli > 1_000
                    || !canonical(&decision.conflict_order)
                    || !canonical(&decision.rationale)
                    || decision.rationale.iter().any(|line| line.trim().is_empty())
            })
            || self.conflicts.windows(2).any(|pair| {
                pair[0].conflict_id >= pair[1].conflict_id
                    || pair[0].left_claim_id >= pair[0].right_claim_id
            })
            || self.conflicts.iter().any(|conflict| {
                conflict.conflict_id.trim().is_empty()
                    || conflict.left_claim_id >= conflict.right_claim_id
                    || conflict.contradiction_milli > 1_000
                    || !canonical(&conflict.evidence_order)
            })
        {
            return Err(BeliefRevisionError::InvalidOutput(
                "identity, ordering, partition, decision, conflict, or score invariants are invalid".into(),
            ));
        }
        let claim_set = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let retained = self
            .retained_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let rival = self
            .rival_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let rejected = self
            .rejected_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if retained
            .union(&rival)
            .chain(unresolved.iter())
            .chain(rejected.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            != claim_set
            || retained.intersection(&rival).next().is_some()
            || retained.intersection(&unresolved).next().is_some()
            || retained.intersection(&rejected).next().is_some()
            || rival.intersection(&unresolved).next().is_some()
            || rival.intersection(&rejected).next().is_some()
            || unresolved.intersection(&rejected).next().is_some()
            || !self.frontier_order.iter().all(|id| rival.contains(id))
            || self.conflicts.iter().any(|conflict| {
                !claim_set.contains(&conflict.left_claim_id)
                    || !claim_set.contains(&conflict.right_claim_id)
            })
        {
            return Err(BeliefRevisionError::InvalidOutput(
                "claim partitions, frontier subset, or conflict endpoints do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| BeliefRevisionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(BeliefRevisionError::InvalidOutput(
                "belief revision digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Resolve an explicit claim-conflict graph into a maximal-consistency research portfolio.
pub fn revise_glioma_beliefs(
    request: &BeliefRevisionRequest,
    knowledge: &TypedKnowledge,
    conflicts: &[BeliefConflict],
) -> Result<BeliefRevision, BeliefRevisionError> {
    knowledge.validate().map_err(|error: KnowledgeError| {
        BeliefRevisionError::InvalidKnowledge(error.to_string())
    })?;
    if request.objective.trim().is_empty()
        || request.objective != knowledge.objective
        || request.min_support_milli > 1_000
        || request.min_conflict_milli == 0
        || request.min_conflict_milli > 1_000
        || request.max_hypotheses == 0
        || request.max_hypotheses > knowledge.claims.len().max(1)
        || request.beam_width == 0
        || request.beam_width > MAX_BEAM_WIDTH
        || knowledge.claims.is_empty()
        || knowledge.claims.len() > MAX_CLAIMS
        || conflicts.len() > MAX_CONFLICTS
    {
        return Err(BeliefRevisionError::InvalidRequest(
            "objective binding, bounded support/conflict thresholds, hypothesis/beam limits, and claim bounds are required".into(),
        ));
    }
    let claim_ids = knowledge
        .claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut conflict_ids = BTreeSet::new();
    let mut conflict_pairs = BTreeMap::<(String, String), String>::new();
    let mut normalized_conflicts = Vec::new();
    for conflict in conflicts {
        let (left, right) = conflict_key(&conflict.left_claim_id, &conflict.right_claim_id);
        if conflict.conflict_id.trim().is_empty()
            || left == right
            || !claim_ids.contains(&left)
            || !claim_ids.contains(&right)
            || conflict.contradiction_milli < request.min_conflict_milli
            || conflict.contradiction_milli > 1_000
            || !conflict_ids.insert(conflict.conflict_id.clone())
            || conflict
                .evidence_order
                .iter()
                .any(|id| id.trim().is_empty())
            || !canonical(&conflict.evidence_order)
            || conflict_pairs
                .insert((left.clone(), right.clone()), conflict.conflict_id.clone())
                .is_some()
        {
            return Err(BeliefRevisionError::InvalidConflict(
                "conflict identity, endpoints, threshold, evidence ordering, or uniqueness is invalid".into(),
            ));
        }
        normalized_conflicts.push(BeliefConflict {
            conflict_id: conflict.conflict_id.clone(),
            left_claim_id: left,
            right_claim_id: right,
            contradiction_milli: conflict.contradiction_milli,
            evidence_order: conflict.evidence_order.clone(),
        });
    }
    normalized_conflicts.sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
    let mut conflict_neighbors = BTreeMap::<String, BTreeSet<String>>::new();
    for conflict in &normalized_conflicts {
        conflict_neighbors
            .entry(conflict.left_claim_id.clone())
            .or_default()
            .insert(conflict.right_claim_id.clone());
        conflict_neighbors
            .entry(conflict.right_claim_id.clone())
            .or_default()
            .insert(conflict.left_claim_id.clone());
    }
    let mut candidates = BTreeMap::<String, Candidate>::new();
    let mut unresolved = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for claim in &knowledge.claims {
        if let Some(candidate) = claim_utility(claim, request) {
            candidates.insert(claim.claim_id.clone(), candidate);
        } else if claim.disposition == KnowledgeClaimDisposition::Negative {
            rejected.insert(claim.claim_id.clone());
        } else {
            unresolved.insert(claim.claim_id.clone());
        }
        if claim.disposition == KnowledgeClaimDisposition::Contested && !request.allow_contested {
            uncertainty.insert(format!("{}:contested-claim-held", claim.claim_id));
        }
        if claim.disposition == KnowledgeClaimDisposition::Unresolved {
            uncertainty.insert(format!("{}:unresolved-coverage", claim.claim_id));
        }
    }
    let mut ranked_candidates = candidates.values().cloned().collect::<Vec<_>>();
    ranked_candidates.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    let initial = BeamState {
        selected: Vec::new(),
        selected_set: BTreeSet::new(),
        score: 0,
    };
    let mut beam = vec![initial.clone()];
    let mut best = initial;
    for _ in 0..request.max_hypotheses {
        let mut next = beam.clone();
        for state in &beam {
            for candidate in &ranked_candidates {
                if state.selected_set.contains(&candidate.claim_id)
                    || state.selected.len() >= request.max_hypotheses
                    || conflict_neighbors
                        .get(&candidate.claim_id)
                        .is_some_and(|neighbors| {
                            neighbors
                                .iter()
                                .any(|neighbor| state.selected_set.contains(neighbor))
                        })
                {
                    continue;
                }
                let mut expanded = state.clone();
                expanded.selected.push(candidate.claim_id.clone());
                expanded.selected_set.insert(candidate.claim_id.clone());
                expanded.score = state_score(&expanded, &candidates);
                next.push(expanded);
            }
        }
        next.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.selected.cmp(&right.selected))
        });
        let mut seen = BTreeSet::new();
        beam = next
            .into_iter()
            .filter(|state| seen.insert(state.selected.clone()))
            .take(request.beam_width)
            .collect();
        if let Some(candidate) = beam.iter().find(|state| {
            state.score > best.score
                || (state.score == best.score && state.selected < best.selected)
        }) {
            best = candidate.clone();
        }
        if beam.is_empty() {
            break;
        }
    }
    let retained = best.selected.iter().cloned().collect::<BTreeSet<_>>();
    let mut rivals = BTreeSet::new();
    let mut frontier_scored = Vec::new();
    let mut decisions_by_id = BTreeMap::new();
    for claim in &knowledge.claims {
        let candidate = candidates.get(&claim.claim_id);
        let mut conflict_order = conflict_neighbors
            .get(&claim.claim_id)
            .into_iter()
            .flat_map(|neighbors| neighbors.iter())
            .filter_map(|neighbor| {
                conflict_pairs
                    .get(&conflict_key(&claim.claim_id, neighbor))
                    .cloned()
            })
            .collect::<Vec<_>>();
        conflict_order.sort();
        let (kind, rationale) = if retained.contains(&claim.claim_id) {
            (
                BeliefRevisionDecisionKind::Retained,
                vec!["maximal-consistency-beam-retained".into()],
            )
        } else if candidate.is_some() {
            rivals.insert(claim.claim_id.clone());
            frontier_scored.push((
                candidate.map(|item| item.utility_milli).unwrap_or(0),
                claim.claim_id.clone(),
            ));
            if conflict_order.is_empty() {
                (
                    BeliefRevisionDecisionKind::Rival,
                    vec!["eligible-but-outside-hypothesis-portfolio".into()],
                )
            } else {
                (
                    BeliefRevisionDecisionKind::Rival,
                    vec!["explicit-conflict-with-retained-or-higher-utility-claim".into()],
                )
            }
        } else if rejected.contains(&claim.claim_id) {
            (
                BeliefRevisionDecisionKind::Rejected,
                vec!["negative-claim-preserved-as-rejection-not-support".into()],
            )
        } else {
            (
                BeliefRevisionDecisionKind::Unresolved,
                vec!["coverage-or-contested-policy-prevents-retention".into()],
            )
        };
        let candidate = candidate.cloned();
        decisions_by_id.insert(
            claim.claim_id.clone(),
            BeliefRevisionDecision {
                claim_id: claim.claim_id.clone(),
                kind,
                utility_milli: candidate
                    .as_ref()
                    .map(|item| item.utility_milli)
                    .unwrap_or(0),
                support_milli: candidate
                    .as_ref()
                    .map(|item| item.support_milli)
                    .unwrap_or(claim.support_milli),
                contradiction_milli: candidate
                    .as_ref()
                    .map(|item| item.contradiction_milli)
                    .unwrap_or(claim.contradiction_milli),
                conflict_order,
                rationale,
            },
        );
    }
    frontier_scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let frontier_order = frontier_scored
        .into_iter()
        .map(|(_, claim_id)| claim_id)
        .collect::<Vec<_>>();
    if normalized_conflicts.is_empty() {
        uncertainty.insert("no-explicit-conflict-edges-supplied".into());
    }
    if retained.len() >= request.max_hypotheses
        && rivals.iter().any(|id| {
            conflict_neighbors.get(id).is_some_and(|neighbors| {
                neighbors.iter().any(|neighbor| retained.contains(neighbor))
            })
        })
    {
        uncertainty.insert("hypothesis-cap-limits-consistent-portfolio".into());
    }
    let retained_claim_order = knowledge
        .claim_order
        .iter()
        .filter(|id| retained.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let rival_claim_order = knowledge
        .claim_order
        .iter()
        .filter(|id| rivals.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_claim_order = knowledge
        .claim_order
        .iter()
        .filter(|id| unresolved.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let rejected_claim_order = knowledge
        .claim_order
        .iter()
        .filter(|id| rejected.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let decisions = knowledge
        .claim_order
        .iter()
        .filter_map(|id| decisions_by_id.remove(id))
        .collect::<Vec<_>>();
    let disposition = if retained_claim_order.is_empty() {
        BeliefRevisionDisposition::Unresolved
    } else if rival_claim_order.is_empty() && unresolved_claim_order.is_empty() {
        BeliefRevisionDisposition::Qualified
    } else {
        BeliefRevisionDisposition::Contested
    };
    let mut output = BeliefRevision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_knowledge_digest: knowledge.digest.clone(),
        claim_order: knowledge.claim_order.clone(),
        retained_claim_order,
        rival_claim_order,
        unresolved_claim_order,
        rejected_claim_order,
        frontier_order,
        conflicts: normalized_conflicts,
        decisions,
        negative_evidence: knowledge.negative_evidence_order.clone(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-belief-revision"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| BeliefRevisionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        KnowledgeClaim, TypedKnowledge,
    };

    fn knowledge() -> TypedKnowledge {
        let claims = vec![
            KnowledgeClaim {
                claim_id: "claim-a".into(),
                statement: "EGFR increases invasion".into(),
                scope: "organoid invasion".into(),
                modality_order: Vec::new(),
                model_system_order: Vec::new(),
                supporting_evidence_order: vec!["e-a".into()],
                negative_evidence_order: Vec::new(),
                contradictory_evidence_order: Vec::new(),
                unresolved_evidence_order: Vec::new(),
                missing_modality_order: Vec::new(),
                missing_model_system_order: Vec::new(),
                support_milli: 900,
                contradiction_milli: 0,
                confidence_milli: 900,
                disposition: KnowledgeClaimDisposition::Supported,
            },
            KnowledgeClaim {
                claim_id: "claim-b".into(),
                statement: "EGFR decreases invasion".into(),
                scope: "organoid invasion".into(),
                modality_order: Vec::new(),
                model_system_order: Vec::new(),
                supporting_evidence_order: vec!["e-b".into()],
                negative_evidence_order: Vec::new(),
                contradictory_evidence_order: Vec::new(),
                unresolved_evidence_order: Vec::new(),
                missing_modality_order: Vec::new(),
                missing_model_system_order: Vec::new(),
                support_milli: 650,
                contradiction_milli: 0,
                confidence_milli: 650,
                disposition: KnowledgeClaimDisposition::Supported,
            },
            KnowledgeClaim {
                claim_id: "claim-c".into(),
                statement: "EGFR effect is unmeasured".into(),
                scope: "organoid invasion".into(),
                modality_order: Vec::new(),
                model_system_order: Vec::new(),
                supporting_evidence_order: Vec::new(),
                negative_evidence_order: vec!["e-c".into()],
                contradictory_evidence_order: Vec::new(),
                unresolved_evidence_order: Vec::new(),
                missing_modality_order: Vec::new(),
                missing_model_system_order: Vec::new(),
                support_milli: 0,
                contradiction_milli: 0,
                confidence_milli: 0,
                disposition: KnowledgeClaimDisposition::Negative,
            },
        ];
        let mut knowledge = TypedKnowledge {
            feature_id: super::super::knowledge_graph::FEATURE_ID.into(),
            output_schema: super::super::knowledge_graph::OUTPUT_SCHEMA.into(),
            objective: "resolve invasion mechanism".into(),
            claims,
            claim_order: vec!["claim-a".into(), "claim-b".into(), "claim-c".into()],
            top_claim_order: vec!["claim-a".into(), "claim-b".into()],
            omission_order: Vec::new(),
            negative_evidence_order: vec!["e-c".into()],
            uncertainty_order: Vec::new(),
            disposition: super::super::knowledge_graph::KnowledgeDisposition::Partial,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        knowledge.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": &knowledge.feature_id,
            "output_schema": &knowledge.output_schema,
            "objective": &knowledge.objective,
            "claims": &knowledge.claims,
            "claim_order": &knowledge.claim_order,
            "top_claim_order": &knowledge.top_claim_order,
            "omission_order": &knowledge.omission_order,
            "negative_evidence_order": &knowledge.negative_evidence_order,
            "uncertainty_order": &knowledge.uncertainty_order,
            "disposition": &knowledge.disposition,
        }))
        .unwrap();
        knowledge.validate().unwrap();
        knowledge
    }

    fn request() -> BeliefRevisionRequest {
        BeliefRevisionRequest {
            objective: "resolve invasion mechanism".into(),
            min_support_milli: 500,
            min_conflict_milli: 500,
            max_hypotheses: 2,
            beam_width: 32,
            allow_contested: false,
        }
    }

    #[test]
    fn revision_retains_stronger_claim_and_exposes_rival_and_negative() {
        let output = revise_glioma_beliefs(
            &request(),
            &knowledge(),
            &[BeliefConflict {
                conflict_id: "conflict-1".into(),
                left_claim_id: "claim-b".into(),
                right_claim_id: "claim-a".into(),
                contradiction_milli: 900,
                evidence_order: vec!["e-a".into(), "e-b".into()],
            }],
        )
        .unwrap();
        assert_eq!(output.retained_claim_order, vec!["claim-a"]);
        assert_eq!(output.rival_claim_order, vec!["claim-b"]);
        assert_eq!(output.rejected_claim_order, vec!["claim-c"]);
        assert_eq!(output.disposition, BeliefRevisionDisposition::Contested);
        output.validate().unwrap();
    }

    #[test]
    fn revision_is_permutation_stable_and_requires_explicit_conflicts() {
        let conflicts = vec![BeliefConflict {
            conflict_id: "conflict-1".into(),
            left_claim_id: "claim-a".into(),
            right_claim_id: "claim-b".into(),
            contradiction_milli: 900,
            evidence_order: vec!["e-a".into(), "e-b".into()],
        }];
        let first = revise_glioma_beliefs(&request(), &knowledge(), &conflicts).unwrap();
        let mut reversed = conflicts.clone();
        reversed.reverse();
        let second = revise_glioma_beliefs(&request(), &knowledge(), &reversed).unwrap();
        assert_eq!(first, second);
        let no_conflicts = revise_glioma_beliefs(&request(), &knowledge(), &[]).unwrap();
        assert!(no_conflicts
            .uncertainty
            .contains(&"no-explicit-conflict-edges-supplied".to_string()));
    }

    #[test]
    fn contested_claims_hold_when_policy_disallows_them() {
        let mut knowledge = knowledge();
        knowledge.claims[0].disposition = KnowledgeClaimDisposition::Contested;
        knowledge.claims[0].contradiction_milli = 300;
        knowledge.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": &knowledge.feature_id,
            "output_schema": &knowledge.output_schema,
            "objective": &knowledge.objective,
            "claims": &knowledge.claims,
            "claim_order": &knowledge.claim_order,
            "top_claim_order": &knowledge.top_claim_order,
            "omission_order": &knowledge.omission_order,
            "negative_evidence_order": &knowledge.negative_evidence_order,
            "uncertainty_order": &knowledge.uncertainty_order,
            "disposition": &knowledge.disposition,
        }))
        .unwrap();
        let output = revise_glioma_beliefs(&request(), &knowledge, &[]).unwrap();
        assert!(output
            .unresolved_claim_order
            .contains(&"claim-a".to_string()));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "claim-a:contested-claim-held"));
    }
}
