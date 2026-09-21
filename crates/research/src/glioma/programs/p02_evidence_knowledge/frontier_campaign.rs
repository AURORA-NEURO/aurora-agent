//! Campaign-level scheduling for the reconciled glioma research frontier.
//!
//! This feature turns a selected closed-loop frontier into deterministic, budgeted rounds that
//! can be admitted to the local workflow compiler.  Review gates, deferred candidates, and
//! negative/contradictory work remain visible; this module does not execute any action.

use super::closed_loop_frontier::{ClosedLoopFrontier, FrontierPromotionCandidate};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaFrontierCampaign1@1";
pub const MAX_ROUNDS: usize = 1_024;
pub const MAX_ACTIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierCampaignRequest {
    pub objective: String,
    pub frontier: ClosedLoopFrontier,
    pub round_budget_units: u64,
    pub max_rounds: usize,
    pub max_actions_per_round: usize,
    pub block_on_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCampaignRoundStatus {
    Ready,
    ReviewBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierCampaignRound {
    pub round_index: usize,
    pub candidate_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub estimated_cost_units: u64,
    pub status: FrontierCampaignRoundStatus,
    pub gate_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCampaignDisposition {
    Ready,
    ReviewBlocked,
    BudgetLimited,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub frontier_digest: ContentHash,
    pub rounds: Vec<FrontierCampaignRound>,
    pub deferred_order: Vec<String>,
    pub review_claim_order: Vec<String>,
    pub estimated_cost_units: u64,
    pub disposition: FrontierCampaignDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrontierCampaignError {
    #[error("frontier campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("frontier campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("frontier campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FrontierCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "frontier_digest": output.frontier_digest,
        "rounds": output.rounds,
        "deferred_order": output.deferred_order,
        "review_claim_order": output.review_claim_order,
        "estimated_cost_units": output.estimated_cost_units,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl FrontierCampaign {
    pub fn validate(&self) -> Result<(), FrontierCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.frontier_digest.as_str().len() != 64
            || !canonical(&self.deferred_order)
            || !canonical(&self.review_claim_order)
            || self
                .rounds
                .windows(2)
                .any(|pair| pair[0].round_index >= pair[1].round_index)
            || self.rounds.iter().any(|round| {
                round.round_index == 0
                    || round.candidate_order.is_empty()
                    || !canonical(&round.candidate_order)
                    || !canonical(&round.claim_order)
                    || round.estimated_cost_units == 0
                    || round.gate_reason.trim().is_empty()
                    || matches!(round.status, FrontierCampaignRoundStatus::ReviewBlocked)
                        && !round
                            .claim_order
                            .iter()
                            .any(|claim| self.review_claim_order.contains(claim))
            })
        {
            return Err(FrontierCampaignError::InvalidOutput(
                "identity, round ordering, candidate contracts, or review gates are invalid".into(),
            ));
        }
        let round_candidates = self
            .rounds
            .iter()
            .flat_map(|round| round.candidate_order.iter().cloned())
            .collect::<BTreeSet<_>>();
        if round_candidates
            .intersection(&self.deferred_order.iter().cloned().collect())
            .next()
            .is_some()
        {
            return Err(FrontierCampaignError::InvalidOutput(
                "candidate appears in both a round and deferred partition".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FrontierCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FrontierCampaignError::Digest(
                "frontier campaign digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Batch a selected frontier into deterministic execution-facing campaign rounds.
pub fn schedule_glioma_frontier_campaign(
    request: &FrontierCampaignRequest,
) -> Result<FrontierCampaign, FrontierCampaignError> {
    if request.objective.trim().is_empty()
        || request.round_budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS
    {
        return Err(FrontierCampaignError::InvalidRequest(
            "objective, budgets, round bounds, or action bounds are invalid".into(),
        ));
    }
    request
        .frontier
        .validate()
        .map_err(|error| FrontierCampaignError::InvalidRequest(error.to_string()))?;
    let candidates = request
        .frontier
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let review_claims = request.frontier.review_claim_order.clone();
    let mut selected = request
        .frontier
        .selected_order
        .iter()
        .filter_map(|candidate_id| candidates.get(candidate_id).copied())
        .collect::<Vec<&FrontierPromotionCandidate>>();
    selected.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let mut rounds = Vec::new();
    let mut deferred = request
        .frontier
        .deferred_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.block_on_review && !review_claims.is_empty() {
        deferred.extend(
            selected
                .iter()
                .map(|candidate| candidate.candidate_id.clone()),
        );
        selected.clear();
    }
    let mut current = Vec::<&FrontierPromotionCandidate>::new();
    let mut current_cost = 0_u64;
    let mut round_index = 1_usize;
    for candidate in selected {
        if current.is_empty() && candidate.estimated_cost_units > request.round_budget_units {
            deferred.insert(candidate.candidate_id.clone());
            continue;
        }
        let exceeds = !current.is_empty()
            && (current.len() >= request.max_actions_per_round
                || current_cost.saturating_add(candidate.estimated_cost_units)
                    > request.round_budget_units);
        if exceeds {
            rounds.push(FrontierCampaignRound {
                round_index,
                candidate_order: current
                    .iter()
                    .map(|item| item.candidate_id.clone())
                    .collect(),
                claim_order: current
                    .iter()
                    .map(|item| item.claim_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                estimated_cost_units: current_cost,
                status: if request.block_on_review && !review_claims.is_empty() {
                    FrontierCampaignRoundStatus::ReviewBlocked
                } else {
                    FrontierCampaignRoundStatus::Ready
                },
                gate_reason: if request.block_on_review && !review_claims.is_empty() {
                    "review claims must be adjudicated before execution".into()
                } else {
                    "round satisfies budget and action-capacity bounds".into()
                },
            });
            round_index += 1;
            current.clear();
            current_cost = 0;
        }
        if round_index > request.max_rounds {
            deferred.insert(candidate.candidate_id.clone());
        } else {
            current_cost += candidate.estimated_cost_units;
            current.push(candidate);
        }
    }
    if !current.is_empty() && round_index <= request.max_rounds {
        rounds.push(FrontierCampaignRound {
            round_index,
            candidate_order: current
                .iter()
                .map(|item| item.candidate_id.clone())
                .collect(),
            claim_order: current
                .iter()
                .map(|item| item.claim_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            estimated_cost_units: current_cost,
            status: if request.block_on_review && !review_claims.is_empty() {
                FrontierCampaignRoundStatus::ReviewBlocked
            } else {
                FrontierCampaignRoundStatus::Ready
            },
            gate_reason: if request.block_on_review && !review_claims.is_empty() {
                "review claims must be adjudicated before execution".into()
            } else {
                "round satisfies budget and action-capacity bounds".into()
            },
        });
    }
    let estimated_cost = rounds
        .iter()
        .map(|round| round.estimated_cost_units)
        .sum::<u64>();
    let disposition = if rounds.is_empty() && !review_claims.is_empty() {
        FrontierCampaignDisposition::ReviewBlocked
    } else if rounds.is_empty() && deferred.is_empty() {
        FrontierCampaignDisposition::Empty
    } else if rounds.is_empty() {
        FrontierCampaignDisposition::BudgetLimited
    } else if !deferred.is_empty() {
        FrontierCampaignDisposition::BudgetLimited
    } else {
        FrontierCampaignDisposition::Ready
    };
    let next_step = match disposition {
        FrontierCampaignDisposition::Ready => {
            "admit campaign rounds to glioma_local_research_workflow".into()
        }
        FrontierCampaignDisposition::ReviewBlocked => {
            "resolve review claims before admitting any campaign round".into()
        }
        FrontierCampaignDisposition::BudgetLimited => {
            "retain deferred candidates and extend campaign budget or round capacity".into()
        }
        FrontierCampaignDisposition::Empty => {
            "reconcile a claim or produce a new closed-loop frontier before scheduling".into()
        }
    };
    let mut output = FrontierCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        frontier_digest: request.frontier.digest.clone(),
        rounds,
        deferred_order: deferred.into_iter().collect(),
        review_claim_order: review_claims,
        estimated_cost_units: estimated_cost,
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FrontierCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::closed_loop_frontier::{
        ClosedLoopFrontierDisposition, FrontierPromotionActionKind,
    };

    fn frontier(review: bool) -> ClosedLoopFrontier {
        let candidate = FrontierPromotionCandidate {
            candidate_id: "frontier-claim-a".into(),
            claim_id: "claim-a".into(),
            kind: FrontierPromotionActionKind::ResolveUncertainty,
            priority_milli: 800,
            estimated_cost_units: 4,
            route: "glioma_knowledge_gap_compile".into(),
            requires_approval: false,
            physical_effect: false,
            rationale: "resolve".into(),
        };
        let mut output = ClosedLoopFrontier {
            feature_id: super::super::closed_loop_frontier::FEATURE_ID.into(),
            output_schema: super::super::closed_loop_frontier::OUTPUT_SCHEMA.into(),
            objective: "campaign".into(),
            reconciliation_digest: ContentHash::of_bytes(b"reconciliation"),
            candidates: vec![candidate],
            selected_order: vec!["frontier-claim-a".into()],
            deferred_order: vec![],
            budget_used_units: 4,
            selected_claim_order: vec!["claim-a".into()],
            review_claim_order: if review {
                vec!["claim-a".into()]
            } else {
                vec![]
            },
            uncertainty: vec![],
            disposition: if review {
                ClosedLoopFrontierDisposition::ReviewRequired
            } else {
                ClosedLoopFrontierDisposition::ActionsSelected
            },
            next_step: "next".into(),
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        output.digest =
            ContentHash::of_value(&super::super::closed_loop_frontier::digest_input(&output))
                .unwrap();
        output
    }

    #[test]
    fn selected_frontier_becomes_ready_round() {
        let output = schedule_glioma_frontier_campaign(&FrontierCampaignRequest {
            objective: "schedule".into(),
            frontier: frontier(false),
            round_budget_units: 5,
            max_rounds: 2,
            max_actions_per_round: 1,
            block_on_review: true,
        })
        .unwrap();
        assert_eq!(output.disposition, FrontierCampaignDisposition::Ready);
        assert_eq!(output.rounds[0].candidate_order, vec!["frontier-claim-a"]);
        output.validate().unwrap();
    }

    #[test]
    fn review_gate_blocks_campaign() {
        let output = schedule_glioma_frontier_campaign(&FrontierCampaignRequest {
            objective: "review".into(),
            frontier: frontier(true),
            round_budget_units: 5,
            max_rounds: 2,
            max_actions_per_round: 1,
            block_on_review: true,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            FrontierCampaignDisposition::ReviewBlocked
        );
        assert!(output.rounds.is_empty());
    }

    #[test]
    fn round_budget_defers_work() {
        let output = schedule_glioma_frontier_campaign(&FrontierCampaignRequest {
            objective: "defer".into(),
            frontier: frontier(false),
            round_budget_units: 3,
            max_rounds: 2,
            max_actions_per_round: 1,
            block_on_review: false,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            FrontierCampaignDisposition::BudgetLimited
        );
        assert_eq!(output.deferred_order, vec!["frontier-claim-a"]);
    }
}
