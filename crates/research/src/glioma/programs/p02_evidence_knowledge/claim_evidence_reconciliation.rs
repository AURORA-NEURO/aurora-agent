//! Belief-state reconciliation after a claim-to-experiment closure.
//!
//! This is the promotion gate between a completed research action and the next autonomous
//! frontier.  It compares quantified closure support with the prior typed claim, preserving
//! stable beliefs, promoting stronger evidence, downgrading unsupported claims, and routing
//! negative or contradictory results for review.  It never silently replaces a claim.

use super::claim_experiment_closure::{ClaimExperimentClosure, ClaimExperimentDisposition};
use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaClaimEvidenceReconciliation1@1";
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimEvidenceReconciliationRequest {
    pub objective: String,
    pub prior_knowledge: TypedKnowledge,
    pub closure: ClaimExperimentClosure,
    pub min_promotion_support_milli: u16,
    pub min_downgrade_delta_milli: u16,
    pub max_claims: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimReconciliationDecision {
    Promoted,
    Retained,
    Downgraded,
    New,
    Negative,
    Contradicted,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimReconciliationDisposition {
    Advanced,
    Stable,
    NeedsReview,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimReconciliationRow {
    pub claim_id: String,
    pub prior_disposition: Option<KnowledgeClaimDisposition>,
    pub closure_disposition: ClaimExperimentDisposition,
    pub prior_confidence_milli: u16,
    pub closure_support_milli: u16,
    pub confidence_delta_milli: i32,
    pub decision: ClaimReconciliationDecision,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimEvidenceReconciliation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub prior_knowledge_digest: ContentHash,
    pub closure_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub rows: Vec<ClaimReconciliationRow>,
    pub promoted_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub downgraded_order: Vec<String>,
    pub new_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub review_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ClaimReconciliationDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClaimEvidenceReconciliationError {
    #[error("claim-evidence reconciliation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("claim-evidence reconciliation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("claim-evidence reconciliation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

pub(crate) fn digest_input(output: &ClaimEvidenceReconciliation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "prior_knowledge_digest": output.prior_knowledge_digest,
        "closure_digest": output.closure_digest,
        "claim_order": output.claim_order,
        "rows": output.rows,
        "promoted_order": output.promoted_order,
        "retained_order": output.retained_order,
        "downgraded_order": output.downgraded_order,
        "new_order": output.new_order,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "unresolved_order": output.unresolved_order,
        "review_order": output.review_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ClaimEvidenceReconciliation {
    pub fn validate(&self) -> Result<(), ClaimEvidenceReconciliationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.prior_knowledge_digest.as_str().len() != 64
            || self.closure_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.promoted_order)
            || !canonical(&self.retained_order)
            || !canonical(&self.downgraded_order)
            || !canonical(&self.new_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.review_order)
            || !canonical(&self.uncertainty)
            || self.rows.len() != self.claim_order.len()
            || self
                .rows
                .iter()
                .map(|row| row.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.rows.iter().any(|row| {
                row.claim_id.trim().is_empty()
                    || row.prior_confidence_milli > 1_000
                    || row.closure_support_milli > 1_000
                    || !row.rationale.trim().is_empty() && row.rationale.len() > 512
            })
        {
            return Err(ClaimEvidenceReconciliationError::InvalidOutput(
                "identity, ordering, row contracts, or score bounds are invalid".into(),
            ));
        }
        let partitions = [
            self.promoted_order.iter(),
            self.retained_order.iter(),
            self.downgraded_order.iter(),
            self.new_order.iter(),
            self.negative_order.iter(),
            self.contradicted_order.iter(),
            self.unresolved_order.iter(),
        ];
        let mut union = BTreeSet::new();
        let mut count = 0;
        for partition in partitions {
            for claim_id in partition {
                union.insert((*claim_id).clone());
                count += 1;
            }
        }
        if union.len() != self.claim_order.len()
            || count != union.len()
            || union != self.claim_order.iter().cloned().collect()
        {
            return Err(ClaimEvidenceReconciliationError::InvalidOutput(
                "claim reconciliation partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClaimEvidenceReconciliationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClaimEvidenceReconciliationError::Digest(
                "claim reconciliation digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Reconcile closure support against the prior typed claim state.
pub fn reconcile_glioma_claim_evidence(
    request: &ClaimEvidenceReconciliationRequest,
) -> Result<ClaimEvidenceReconciliation, ClaimEvidenceReconciliationError> {
    if request.objective.trim().is_empty()
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.prior_knowledge.claims.len() > request.max_claims
        || request.closure.claims.len() > request.max_claims
        || request.min_promotion_support_milli > 1_000
        || request.min_downgrade_delta_milli > 1_000
    {
        return Err(ClaimEvidenceReconciliationError::InvalidRequest(
            "objective, bounds, or score thresholds are invalid".into(),
        ));
    }
    request
        .prior_knowledge
        .validate()
        .map_err(|error| ClaimEvidenceReconciliationError::InvalidRequest(error.to_string()))?;
    request
        .closure
        .validate()
        .map_err(|error| ClaimEvidenceReconciliationError::InvalidRequest(error.to_string()))?;
    if request.closure.knowledge_digest != request.prior_knowledge.digest {
        return Err(ClaimEvidenceReconciliationError::InvalidRequest(
            "closure is not derived from the supplied prior knowledge digest".into(),
        ));
    }
    let prior = request
        .prior_knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::with_capacity(request.closure.claims.len());
    let mut promoted = BTreeSet::new();
    let mut retained = BTreeSet::new();
    let mut downgraded = BTreeSet::new();
    let mut new_claims = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut review = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for closure in &request.closure.claims {
        let prior_claim = prior.get(&closure.claim_id).copied();
        let prior_confidence = prior_claim.map(|claim| claim.confidence_milli).unwrap_or(0);
        let delta = closure.support_milli as i32 - prior_confidence as i32;
        let (decision, rationale) = match closure.disposition {
            ClaimExperimentDisposition::Closed
                if closure.support_milli >= request.min_promotion_support_milli =>
            {
                if prior_claim.is_none() {
                    (
                        ClaimReconciliationDecision::New,
                        "closed claim has no prior belief state".into(),
                    )
                } else if delta >= request.min_downgrade_delta_milli as i32 {
                    (
                        ClaimReconciliationDecision::Promoted,
                        "closed experiment support exceeds the prior confidence threshold".into(),
                    )
                } else {
                    (
                        ClaimReconciliationDecision::Retained,
                        "closed experiment is consistent with the prior confidence state".into(),
                    )
                }
            }
            ClaimExperimentDisposition::Negative => (
                ClaimReconciliationDecision::Negative,
                "negative result is retained as a first-class boundary condition".into(),
            ),
            ClaimExperimentDisposition::Contradicted => (
                ClaimReconciliationDecision::Contradicted,
                "contradictory result requires explicit rival-claim review".into(),
            ),
            ClaimExperimentDisposition::Partial
                if prior_claim.is_some()
                    && delta <= -(request.min_downgrade_delta_milli as i32) =>
            {
                (
                    ClaimReconciliationDecision::Downgraded,
                    "partial closure reduced confidence beyond the downgrade threshold".into(),
                )
            }
            ClaimExperimentDisposition::Partial if prior_claim.is_some() => (
                ClaimReconciliationDecision::Retained,
                "partial closure does not justify overwriting the prior belief".into(),
            ),
            ClaimExperimentDisposition::Unresolved if prior_claim.is_some() => (
                ClaimReconciliationDecision::Retained,
                "unresolved closure preserves the prior belief and opens an evidence debt".into(),
            ),
            _ => (
                ClaimReconciliationDecision::Unresolved,
                "closure does not provide enough evidence to establish a belief state".into(),
            ),
        };
        match decision {
            ClaimReconciliationDecision::Promoted => {
                promoted.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::Retained => {
                retained.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::Downgraded => {
                downgraded.insert(closure.claim_id.clone());
                review.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::New => {
                new_claims.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::Negative => {
                negative.insert(closure.claim_id.clone());
                review.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::Contradicted => {
                contradicted.insert(closure.claim_id.clone());
                review.insert(closure.claim_id.clone());
            }
            ClaimReconciliationDecision::Unresolved => {
                unresolved.insert(closure.claim_id.clone());
                review.insert(closure.claim_id.clone());
            }
        };
        if !matches!(
            decision,
            ClaimReconciliationDecision::Promoted
                | ClaimReconciliationDecision::Retained
                | ClaimReconciliationDecision::New
        ) {
            uncertainty.insert(format!("{}:manual-frontier-review", closure.claim_id));
        }
        rows.push(ClaimReconciliationRow {
            claim_id: closure.claim_id.clone(),
            prior_disposition: prior_claim.map(|claim| claim.disposition),
            closure_disposition: closure.disposition,
            prior_confidence_milli: prior_confidence,
            closure_support_milli: closure.support_milli,
            confidence_delta_milli: delta,
            decision,
            rationale,
        });
    }
    rows.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_order = rows
        .iter()
        .map(|row| row.claim_id.clone())
        .collect::<Vec<_>>();
    let disposition = if rows.is_empty() {
        ClaimReconciliationDisposition::Blocked
    } else if !review.is_empty() {
        ClaimReconciliationDisposition::NeedsReview
    } else if !promoted.is_empty() || !new_claims.is_empty() {
        ClaimReconciliationDisposition::Advanced
    } else {
        ClaimReconciliationDisposition::Stable
    };
    let next_step = match disposition {
        ClaimReconciliationDisposition::Advanced => {
            "promote the reconciled claims into the next typed-knowledge and frontier compilation".into()
        }
        ClaimReconciliationDisposition::Stable => {
            "retain the current claim state and continue prospective monitoring for new evidence".into()
        }
        ClaimReconciliationDisposition::NeedsReview => {
            "route downgraded, negative, contradictory, or unresolved claims to explicit frontier adjudication".into()
        }
        ClaimReconciliationDisposition::Blocked => {
            "execute a bounded claim-specific action before changing the knowledge state".into()
        }
    };
    let mut output = ClaimEvidenceReconciliation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        prior_knowledge_digest: request.prior_knowledge.digest.clone(),
        closure_digest: request.closure.digest.clone(),
        claim_order,
        rows,
        promoted_order: promoted.into_iter().collect(),
        retained_order: retained.into_iter().collect(),
        downgraded_order: downgraded.into_iter().collect(),
        new_order: new_claims.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        contradicted_order: contradicted.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        review_order: review.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClaimEvidenceReconciliationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::claim_experiment_closure::ClaimExperimentResult;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn knowledge(confidence: u16) -> TypedKnowledge {
        let claim = KnowledgeClaim {
            claim_id: "claim-egfr".into(),
            statement: "egfr drives resistance".into(),
            scope: "organoid".into(),
            modality_order: vec![GliomaModality::Genomics],
            model_system_order: vec![GliomaModelSystem::Organoid],
            supporting_evidence_order: vec![],
            negative_evidence_order: vec![],
            contradictory_evidence_order: vec![],
            unresolved_evidence_order: vec![],
            missing_modality_order: vec![],
            missing_model_system_order: vec![],
            support_milli: confidence,
            contradiction_milli: 0,
            confidence_milli: confidence,
            disposition: KnowledgeClaimDisposition::Supported,
        };
        let mut value = TypedKnowledge {
            feature_id: super::super::knowledge_graph::FEATURE_ID.into(),
            output_schema: super::super::knowledge_graph::OUTPUT_SCHEMA.into(),
            objective: "reconcile".into(),
            claims: vec![claim],
            claim_order: vec!["claim-egfr".into()],
            top_claim_order: vec!["claim-egfr".into()],
            omission_order: vec![],
            negative_evidence_order: vec![],
            uncertainty_order: vec![],
            disposition: KnowledgeDisposition::Qualified,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        let input = serde_json::json!({
            "feature_id": value.feature_id,
            "output_schema": value.output_schema,
            "objective": value.objective,
            "claims": value.claims,
            "claim_order": value.claim_order,
            "top_claim_order": value.top_claim_order,
            "omission_order": value.omission_order,
            "negative_evidence_order": value.negative_evidence_order,
            "uncertainty_order": value.uncertainty_order,
            "disposition": value.disposition,
        });
        value.digest = ContentHash::of_value(&input).unwrap();
        value
    }

    fn closure(
        knowledge: &TypedKnowledge,
        disposition: ClaimExperimentDisposition,
        support: u16,
    ) -> ClaimExperimentClosure {
        let result = ClaimExperimentResult {
            claim_id: "claim-egfr".into(),
            action_order: vec!["action-egfr".into()],
            completed_action_order: vec!["action-egfr".into()],
            failed_action_order: vec![],
            evidence_order: vec!["evidence-egfr".into()],
            supporting_evidence_order: vec!["evidence-egfr".into()],
            negative_evidence_order: vec![],
            contradictory_evidence_order: vec![],
            unresolved_evidence_order: vec![],
            independent_artifact_count: 1,
            support_milli: support,
            contradiction_milli: 0,
            closure_milli: support,
            missing_modality_order: vec![],
            missing_model_system_order: vec![],
            omission_order: vec![],
            disposition,
        };
        let mut output = ClaimExperimentClosure {
            feature_id: super::super::claim_experiment_closure::FEATURE_ID.into(),
            output_schema: super::super::claim_experiment_closure::OUTPUT_SCHEMA.into(),
            objective: "closure".into(),
            knowledge_digest: knowledge.digest.clone(),
            claim_order: vec!["claim-egfr".into()],
            closed_claim_order: if disposition == ClaimExperimentDisposition::Closed {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            partial_claim_order: if disposition == ClaimExperimentDisposition::Partial {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            unresolved_claim_order: if disposition == ClaimExperimentDisposition::Unresolved {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            negative_claim_order: if disposition == ClaimExperimentDisposition::Negative {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            contradicted_claim_order: if disposition == ClaimExperimentDisposition::Contradicted {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            claims: vec![result],
            orphan_action_order: vec![],
            orphan_evidence_order: vec![],
            omission_order: vec![],
            negative_evidence_order: vec![],
            uncertainty_order: vec![],
            disposition:
                super::super::claim_experiment_closure::ClaimExperimentClosureDisposition::Qualified,
            next_step: "next".into(),
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        output.digest = ContentHash::of_value(
            &super::super::claim_experiment_closure::digest_input(&output),
        )
        .unwrap();
        output
    }

    fn request(
        support: u16,
        disposition: ClaimExperimentDisposition,
    ) -> ClaimEvidenceReconciliationRequest {
        let prior = knowledge(600);
        ClaimEvidenceReconciliationRequest {
            objective: "reconcile".into(),
            closure: closure(&prior, disposition, support),
            prior_knowledge: prior,
            min_promotion_support_milli: 700,
            min_downgrade_delta_milli: 100,
            max_claims: 10,
        }
    }

    #[test]
    fn stronger_closed_result_promotes_claim() {
        let output =
            reconcile_glioma_claim_evidence(&request(900, ClaimExperimentDisposition::Closed))
                .unwrap();
        assert_eq!(output.disposition, ClaimReconciliationDisposition::Advanced);
        assert_eq!(output.promoted_order, vec!["claim-egfr"]);
        output.validate().unwrap();
    }

    #[test]
    fn partial_result_downgrades_only_after_threshold() {
        let output =
            reconcile_glioma_claim_evidence(&request(450, ClaimExperimentDisposition::Partial))
                .unwrap();
        assert_eq!(
            output.disposition,
            ClaimReconciliationDisposition::NeedsReview
        );
        assert_eq!(output.downgraded_order, vec!["claim-egfr"]);
    }

    #[test]
    fn negative_result_requires_review() {
        let output =
            reconcile_glioma_claim_evidence(&request(0, ClaimExperimentDisposition::Negative))
                .unwrap();
        assert_eq!(
            output.disposition,
            ClaimReconciliationDisposition::NeedsReview
        );
        assert_eq!(output.negative_order, vec!["claim-egfr"]);
    }
}
