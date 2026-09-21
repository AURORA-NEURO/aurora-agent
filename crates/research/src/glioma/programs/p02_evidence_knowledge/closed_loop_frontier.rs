//! Closed-loop promotion from reconciled claim evidence to the next research frontier.
//!
//! This capability is the action-facing half of the P02 belief loop.  It converts explicit claim
//! reconciliation decisions into ranked, budget-bounded research candidates while preserving
//! negative and contradictory results.  It proposes no clinical action and never executes an
//! instrument or moves raw data.

use super::claim_evidence_reconciliation::{
    ClaimEvidenceReconciliation, ClaimReconciliationDecision,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaClosedLoopFrontier1@1";
pub const MAX_ACTIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierPromotionActionKind {
    ConfirmSupported,
    CloseEvidenceGap,
    ResolveContradiction,
    RevalidateNegative,
    ResolveUncertainty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedLoopFrontierDisposition {
    ActionsSelected,
    BudgetLimited,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedLoopFrontierRequest {
    pub objective: String,
    pub reconciliation: ClaimEvidenceReconciliation,
    pub budget_units: u64,
    pub max_actions: usize,
    pub min_priority_milli: u16,
    pub include_stable_claims: bool,
    pub allow_physical_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierPromotionCandidate {
    pub candidate_id: String,
    pub claim_id: String,
    pub kind: FrontierPromotionActionKind,
    pub priority_milli: u16,
    pub estimated_cost_units: u64,
    pub route: String,
    pub requires_approval: bool,
    pub physical_effect: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedLoopFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub reconciliation_digest: ContentHash,
    pub candidates: Vec<FrontierPromotionCandidate>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub budget_used_units: u64,
    pub selected_claim_order: Vec<String>,
    pub review_claim_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ClosedLoopFrontierDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClosedLoopFrontierError {
    #[error("closed-loop frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("closed-loop frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("closed-loop frontier digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ClosedLoopFrontier) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "reconciliation_digest": output.reconciliation_digest,
        "candidates": output.candidates,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "budget_used_units": output.budget_used_units,
        "selected_claim_order": output.selected_claim_order,
        "review_claim_order": output.review_claim_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ClosedLoopFrontier {
    pub fn validate(&self) -> Result<(), ClosedLoopFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.reconciliation_digest.as_str().len() != 64
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.selected_claim_order)
            || !canonical(&self.review_claim_order)
            || !canonical(&self.uncertainty)
            || self.candidates.iter().any(|candidate| {
                candidate.candidate_id.trim().is_empty()
                    || candidate.claim_id.trim().is_empty()
                    || candidate.route.trim().is_empty()
                    || candidate.rationale.trim().is_empty()
                    || candidate.priority_milli > 1_000
                    || candidate.estimated_cost_units == 0
                    || candidate.physical_effect && !candidate.requires_approval
            })
        {
            return Err(ClosedLoopFrontierError::InvalidOutput(
                "identity, ordering, candidate contract, priority, or approval invariant is invalid"
                    .into(),
            ));
        }
        let candidate_ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.candidates.len()
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .any(|candidate_id| !candidate_ids.contains(candidate_id))
            || self.selected_order.len() + self.deferred_order.len() != candidate_ids.len()
            || self
                .selected_order
                .iter()
                .any(|candidate_id| self.deferred_order.contains(candidate_id))
        {
            return Err(ClosedLoopFrontierError::InvalidOutput(
                "candidate selection partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClosedLoopFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClosedLoopFrontierError::Digest(
                "closed-loop frontier digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn candidate_for(
    claim_id: &str,
    decision: ClaimReconciliationDecision,
    delta: i32,
) -> Option<FrontierPromotionCandidate> {
    let (kind, priority, cost, route, rationale) = match decision {
        ClaimReconciliationDecision::Promoted | ClaimReconciliationDecision::New => (
            FrontierPromotionActionKind::ConfirmSupported,
            760_i32 + delta.clamp(0, 240),
            8,
            "glioma_knowledge_action_compile",
            "run an independent confirmation action before treating the promoted claim as durable",
        ),
        ClaimReconciliationDecision::Retained => (
            FrontierPromotionActionKind::ConfirmSupported,
            520,
            10,
            "glioma_knowledge_action_compile",
            "prospectively monitor the stable claim for evidence that would change its state",
        ),
        ClaimReconciliationDecision::Downgraded => (
            FrontierPromotionActionKind::CloseEvidenceGap,
            900,
            6,
            "glioma_knowledge_gap_compile",
            "close the evidence debt that caused the confidence downgrade",
        ),
        ClaimReconciliationDecision::Negative => (
            FrontierPromotionActionKind::RevalidateNegative,
            930,
            5,
            "glioma_knowledge_action_compile",
            "revalidate the null result under a bounded boundary-condition design",
        ),
        ClaimReconciliationDecision::Contradicted => (
            FrontierPromotionActionKind::ResolveContradiction,
            1_000,
            9,
            "glioma_belief_revision",
            "run an adjudication or replication action that keeps rival explanations explicit",
        ),
        ClaimReconciliationDecision::Unresolved => (
            FrontierPromotionActionKind::ResolveUncertainty,
            840,
            4,
            "glioma_knowledge_gap_compile",
            "acquire the smallest discriminating evidence needed to resolve the claim",
        ),
    };
    Some(FrontierPromotionCandidate {
        candidate_id: format!("frontier-{claim_id}"),
        claim_id: claim_id.into(),
        kind,
        priority_milli: priority.clamp(0, 1_000) as u16,
        estimated_cost_units: cost,
        route: route.into(),
        requires_approval: false,
        physical_effect: false,
        rationale: rationale.into(),
    })
}

/// Promote reconciled claims into a deterministic, budget-bounded next-work frontier.
pub fn promote_glioma_closed_loop_frontier(
    request: &ClosedLoopFrontierRequest,
) -> Result<ClosedLoopFrontier, ClosedLoopFrontierError> {
    if request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.min_priority_milli > 1_000
    {
        return Err(ClosedLoopFrontierError::InvalidRequest(
            "objective, action bound, or priority threshold is invalid".into(),
        ));
    }
    request
        .reconciliation
        .validate()
        .map_err(|error| ClosedLoopFrontierError::InvalidRequest(error.to_string()))?;
    let mut candidates = request
        .reconciliation
        .rows
        .iter()
        .filter_map(|row| {
            if !request.include_stable_claims
                && matches!(row.decision, ClaimReconciliationDecision::Retained)
            {
                return None;
            }
            candidate_for(&row.claim_id, row.decision, row.confidence_delta_milli)
                .filter(|candidate| candidate.priority_milli >= request.min_priority_milli)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.estimated_cost_units.cmp(&right.estimated_cost_units))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    candidates.truncate(request.max_actions);
    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    let mut budget_used = 0_u64;
    let mut selected_claims = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for candidate in &candidates {
        if candidate.physical_effect && !request.allow_physical_execution {
            deferred.push(candidate.candidate_id.clone());
            uncertainty.insert(format!(
                "{}:physical-execution-disabled",
                candidate.claim_id
            ));
        } else if budget_used.saturating_add(candidate.estimated_cost_units) <= request.budget_units
        {
            budget_used += candidate.estimated_cost_units;
            selected.push(candidate.candidate_id.clone());
            selected_claims.insert(candidate.claim_id.clone());
        } else {
            deferred.push(candidate.candidate_id.clone());
            uncertainty.insert(format!("{}:budget-deferred", candidate.claim_id));
        }
    }
    selected.sort();
    deferred.sort();
    let review_claims = request
        .reconciliation
        .review_order
        .iter()
        .filter(|claim_id| !selected_claims.contains(*claim_id))
        .cloned()
        .collect::<Vec<_>>();
    let disposition = if selected.is_empty() && candidates.is_empty() {
        ClosedLoopFrontierDisposition::Blocked
    } else if selected.is_empty() {
        ClosedLoopFrontierDisposition::BudgetLimited
    } else if !review_claims.is_empty() {
        ClosedLoopFrontierDisposition::ReviewRequired
    } else if deferred.is_empty() {
        ClosedLoopFrontierDisposition::ActionsSelected
    } else {
        ClosedLoopFrontierDisposition::BudgetLimited
    };
    let next_step = match disposition {
        ClosedLoopFrontierDisposition::ActionsSelected => {
            "compile the selected candidates into an execution-facing research workflow".into()
        }
        ClosedLoopFrontierDisposition::BudgetLimited => {
            "retain deferred candidates and increase budget or reduce action scope before execution"
                .into()
        }
        ClosedLoopFrontierDisposition::ReviewRequired => {
            "adjudicate review claims before allowing the next frontier to advance".into()
        }
        ClosedLoopFrontierDisposition::Blocked => {
            "produce a claim-specific closure or reconciliation before promoting research work"
                .into()
        }
    };
    let mut output = ClosedLoopFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        reconciliation_digest: request.reconciliation.digest.clone(),
        candidates,
        selected_order: selected,
        deferred_order: deferred,
        budget_used_units: budget_used,
        selected_claim_order: selected_claims.into_iter().collect(),
        review_claim_order: review_claims,
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClosedLoopFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::claim_evidence_reconciliation::{
        ClaimEvidenceReconciliation, ClaimReconciliationDisposition, ClaimReconciliationRow,
    };
    use crate::glioma::programs::p02_evidence_knowledge::claim_experiment_closure::ClaimExperimentDisposition;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::KnowledgeClaimDisposition;

    fn reconciliation(decision: ClaimReconciliationDecision) -> ClaimEvidenceReconciliation {
        let row = ClaimReconciliationRow {
            claim_id: "claim-egfr".into(),
            prior_disposition: Some(KnowledgeClaimDisposition::Supported),
            closure_disposition: match decision {
                ClaimReconciliationDecision::Negative => ClaimExperimentDisposition::Negative,
                ClaimReconciliationDecision::Contradicted => {
                    ClaimExperimentDisposition::Contradicted
                }
                ClaimReconciliationDecision::Downgraded => ClaimExperimentDisposition::Partial,
                ClaimReconciliationDecision::Unresolved => ClaimExperimentDisposition::Unresolved,
                _ => ClaimExperimentDisposition::Closed,
            },
            prior_confidence_milli: 600,
            closure_support_milli: 900,
            confidence_delta_milli: 300,
            decision,
            rationale: "test".into(),
        };
        let mut output = ClaimEvidenceReconciliation {
            feature_id: super::super::claim_evidence_reconciliation::FEATURE_ID.into(),
            output_schema: super::super::claim_evidence_reconciliation::OUTPUT_SCHEMA.into(),
            objective: "frontier".into(),
            prior_knowledge_digest: ContentHash::of_bytes(b"prior"),
            closure_digest: ContentHash::of_bytes(b"closure"),
            claim_order: vec!["claim-egfr".into()],
            rows: vec![row],
            promoted_order: if decision == ClaimReconciliationDecision::Promoted {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            retained_order: if decision == ClaimReconciliationDecision::Retained {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            downgraded_order: if decision == ClaimReconciliationDecision::Downgraded {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            new_order: if decision == ClaimReconciliationDecision::New {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            negative_order: if decision == ClaimReconciliationDecision::Negative {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            contradicted_order: if decision == ClaimReconciliationDecision::Contradicted {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            unresolved_order: if decision == ClaimReconciliationDecision::Unresolved {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            review_order: if matches!(
                decision,
                ClaimReconciliationDecision::Downgraded
                    | ClaimReconciliationDecision::Negative
                    | ClaimReconciliationDecision::Contradicted
                    | ClaimReconciliationDecision::Unresolved
            ) {
                vec!["claim-egfr".into()]
            } else {
                vec![]
            },
            uncertainty: vec![],
            disposition: if matches!(
                decision,
                ClaimReconciliationDecision::Downgraded
                    | ClaimReconciliationDecision::Negative
                    | ClaimReconciliationDecision::Contradicted
                    | ClaimReconciliationDecision::Unresolved
            ) {
                ClaimReconciliationDisposition::NeedsReview
            } else {
                ClaimReconciliationDisposition::Advanced
            },
            next_step: "next".into(),
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        output.digest = ContentHash::of_value(
            &super::super::claim_evidence_reconciliation::digest_input(&output),
        )
        .unwrap();
        output
    }

    #[test]
    fn contradicted_claim_is_prioritized_for_resolution() {
        let output = promote_glioma_closed_loop_frontier(&ClosedLoopFrontierRequest {
            objective: "resolve".into(),
            reconciliation: reconciliation(ClaimReconciliationDecision::Contradicted),
            budget_units: 10,
            max_actions: 10,
            min_priority_milli: 0,
            include_stable_claims: false,
            allow_physical_execution: false,
        })
        .unwrap();
        assert_eq!(output.selected_order, vec!["frontier-claim-egfr"]);
        assert_eq!(
            output.candidates[0].kind,
            FrontierPromotionActionKind::ResolveContradiction
        );
    }

    #[test]
    fn budget_defers_lower_priority_work() {
        let output = promote_glioma_closed_loop_frontier(&ClosedLoopFrontierRequest {
            objective: "budget".into(),
            reconciliation: reconciliation(ClaimReconciliationDecision::Downgraded),
            budget_units: 1,
            max_actions: 10,
            min_priority_milli: 0,
            include_stable_claims: false,
            allow_physical_execution: false,
        })
        .unwrap();
        assert!(output.selected_order.is_empty());
        assert_eq!(
            output.disposition,
            ClosedLoopFrontierDisposition::BudgetLimited
        );
    }

    #[test]
    fn negative_claim_routes_to_revalidation() {
        let output = promote_glioma_closed_loop_frontier(&ClosedLoopFrontierRequest {
            objective: "null".into(),
            reconciliation: reconciliation(ClaimReconciliationDecision::Negative),
            budget_units: 5,
            max_actions: 10,
            min_priority_milli: 0,
            include_stable_claims: false,
            allow_physical_execution: false,
        })
        .unwrap();
        assert_eq!(
            output.candidates[0].kind,
            FrontierPromotionActionKind::RevalidateNegative
        );
    }
}
