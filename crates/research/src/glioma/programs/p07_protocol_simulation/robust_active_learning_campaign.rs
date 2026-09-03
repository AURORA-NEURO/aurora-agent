//! Autonomous execution loop for robust ensemble active learning.
//!
//! P06's robust planner is useful only when its conservative, model-disagreement-aware decisions
//! can be carried through repeated local rounds. This controller provides that seam: it invokes a
//! caller-owned assay/analysis executor, verifies candidate-bound observations, spends a bounded
//! budget, and replans until a declared gate stops the campaign. The bundled dry-run executor is
//! synthetic and has no instrument or biological side effects.

use crate::glioma::programs::p06_experiment_design::robust_active_learning::{
    plan_glioma_robust_active_learning, RobustActiveLearningCandidate,
    RobustActiveLearningDisposition, RobustActiveLearningObservation, RobustActiveLearningPlan,
    RobustActiveLearningRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustActiveLearningCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningCampaignRequest {
    pub robust_active_learning: RobustActiveLearningRequest,
    pub candidates: Vec<RobustActiveLearningCandidate>,
    pub observations: Vec<RobustActiveLearningObservation>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_unresolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobustActiveLearningExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

pub trait RobustActiveLearningCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &RobustActiveLearningCandidate,
        attempt: u8,
    ) -> Result<RobustActiveLearningObservation, RobustActiveLearningExecutionFailure>;
}

#[derive(Debug, Default)]
pub struct DryRunRobustActiveLearningCampaignExecutor {
    sequence: u64,
}

impl RobustActiveLearningCampaignExecutor for DryRunRobustActiveLearningCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &RobustActiveLearningCandidate,
        attempt: u8,
    ) -> Result<RobustActiveLearningObservation, RobustActiveLearningExecutionFailure> {
        self.sequence = self.sequence.saturating_add(1);
        let outcome_milli = candidate
            .feature_vector
            .iter()
            .enumerate()
            .map(|(index, value)| value.saturating_mul((index as i64).saturating_add(3)))
            .sum::<i64>()
            .saturating_add(candidate.mechanism_id.len() as i64)
            .clamp(-1_000_000, 1_000_000);
        let observation_id = format!(
            "dry-run-robust:{}:{}:{}",
            candidate.candidate_id, attempt, self.sequence
        );
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "observation_id": observation_id,
            "candidate_id": candidate.candidate_id,
            "outcome_milli": outcome_milli,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| RobustActiveLearningExecutionFailure {
            reason: format!("dry-run observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(RobustActiveLearningObservation {
            observation_id,
            candidate_id: candidate.candidate_id.clone(),
            outcome_milli,
            uncertainty_milli: 125,
            artifact: crate::glioma_engine::LocalArtifactRef {
                artifact_id: format!("dry-run-robust-active-learning:{}", candidate.candidate_id),
                content_hash,
                content_type:
                    "application/vnd.aurora.glioma-robust-active-learning-observation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningCampaignRound {
    pub round: u16,
    pub plan: RobustActiveLearningPlan,
    pub executed_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustActiveLearningCampaignDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustActiveLearningCampaignStopReason {
    Completed,
    NoCandidates,
    BudgetExhausted,
    MaxRounds,
    ExecutorFailed,
    Unresolved,
    SelectionBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<RobustActiveLearningCampaignRound>,
    pub observations: Vec<RobustActiveLearningObservation>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: RobustActiveLearningCampaignDisposition,
    pub stop_reason: RobustActiveLearningCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustActiveLearningCampaignError {
    #[error("robust active-learning campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robust active-learning campaign planning failed: {0}")]
    Planning(String),
    #[error("robust active-learning campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robust active-learning campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(campaign: &RobustActiveLearningCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "completed_order": campaign.completed_order,
        "failed_order": campaign.failed_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "uncertainty": campaign.uncertainty,
        "negative_evidence": campaign.negative_evidence,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

impl RobustActiveLearningCampaign {
    pub fn validate(&self) -> Result<(), RobustActiveLearningCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !unique(&self.completed_order)
            || !unique(&self.failed_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self
                .completed_order
                .iter()
                .any(|id| self.failed_order.contains(id))
            || self.observations.iter().any(|observation| {
                observation.observation_id.trim().is_empty()
                    || observation.candidate_id.trim().is_empty()
                    || observation.uncertainty_milli > 1_000
                    || observation.artifact.validate().is_err()
            })
        {
            return Err(RobustActiveLearningCampaignError::InvalidOutput(
                "identity, bounds, ordering, observation, or completion invariants are invalid"
                    .into(),
            ));
        }
        let mut ids = BTreeSet::new();
        if self
            .observations
            .iter()
            .any(|observation| !ids.insert(observation.observation_id.clone()))
        {
            return Err(RobustActiveLearningCampaignError::InvalidOutput(
                "campaign observations must be unique".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RobustActiveLearningCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RobustActiveLearningCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &RobustActiveLearningCampaignRequest,
) -> Result<(), RobustActiveLearningCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
    {
        return Err(RobustActiveLearningCampaignError::InvalidRequest(
            "positive bounded rounds and retries are required".into(),
        ));
    }
    Ok(())
}

/// Execute robust ensemble-guided assay rounds with bounded retries and explicit holds.
pub fn execute_glioma_robust_active_learning_campaign<E: RobustActiveLearningCampaignExecutor>(
    request: &RobustActiveLearningCampaignRequest,
    executor: &mut E,
) -> Result<RobustActiveLearningCampaign, RobustActiveLearningCampaignError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    let mut observations = request.observations.clone();
    let mut budget = request.robust_active_learning.budget_units;
    let mut rounds = Vec::new();
    let mut completed = BTreeSet::new();
    let mut completed_order = Vec::new();
    let mut failed = BTreeSet::new();
    let mut failed_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u32;
    let mut disposition = RobustActiveLearningCampaignDisposition::Partial;
    let mut stop_reason = RobustActiveLearningCampaignStopReason::MaxRounds;

    for round in 0..request.max_rounds {
        let mut planner_request = request.robust_active_learning.clone();
        planner_request.budget_units = budget;
        let plan = plan_glioma_robust_active_learning(&planner_request, &candidates, &observations)
            .map_err(|error| RobustActiveLearningCampaignError::Planning(error.to_string()))?;
        uncertainty.extend(plan.uncertainty.iter().cloned());
        negative.extend(plan.negative_evidence.iter().cloned());
        let budget_before = budget;
        if plan.selected_order.is_empty() {
            disposition = if !plan.unresolved_order.is_empty() {
                RobustActiveLearningCampaignDisposition::Unresolved
            } else if plan.disposition == RobustActiveLearningDisposition::NoCandidates {
                RobustActiveLearningCampaignDisposition::Blocked
            } else {
                RobustActiveLearningCampaignDisposition::Partial
            };
            stop_reason = if !plan.unresolved_order.is_empty() {
                RobustActiveLearningCampaignStopReason::Unresolved
            } else if plan.disposition == RobustActiveLearningDisposition::NoCandidates {
                RobustActiveLearningCampaignStopReason::NoCandidates
            } else {
                RobustActiveLearningCampaignStopReason::SelectionBlocked
            };
            rounds.push(RobustActiveLearningCampaignRound {
                round,
                plan,
                executed_order: Vec::new(),
                observation_order: Vec::new(),
                failed_order: Vec::new(),
                budget_before_units: budget_before,
                budget_after_units: budget,
            });
            break;
        }
        if request.stop_on_unresolved && !plan.unresolved_order.is_empty() {
            disposition = RobustActiveLearningCampaignDisposition::Unresolved;
            stop_reason = RobustActiveLearningCampaignStopReason::Unresolved;
            rounds.push(RobustActiveLearningCampaignRound {
                round,
                plan,
                executed_order: Vec::new(),
                observation_order: Vec::new(),
                failed_order: Vec::new(),
                budget_before_units: budget_before,
                budget_after_units: budget,
            });
            break;
        }
        let candidate_map = candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.clone(), candidate))
            .collect::<BTreeMap<_, _>>();
        let mut executed_order = Vec::new();
        let mut observation_order = Vec::new();
        let mut round_failed_order = Vec::new();
        for candidate_id in &plan.selected_order {
            let candidate = candidate_map[candidate_id];
            if candidate.cost_units > budget {
                if failed.insert(candidate_id.clone()) {
                    failed_order.push(candidate_id.clone());
                    round_failed_order.push(candidate_id.clone());
                }
                negative.insert(format!("{candidate_id}:budget-exhausted-before-execution"));
                continue;
            }
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_candidate(candidate, attempt) {
                    Ok(observation) => {
                        if observation.candidate_id != candidate.candidate_id {
                            return Err(RobustActiveLearningCampaignError::InvalidRequest(
                                "executor observation candidate binding does not match selected candidate".into(),
                            ));
                        }
                        accepted = Some(observation);
                        break;
                    }
                    Err(error) if error.retryable && attempt <= request.max_retries => {
                        retry_count = retry_count.saturating_add(1);
                    }
                    Err(error) => {
                        if failed.insert(candidate_id.clone()) {
                            failed_order.push(candidate_id.clone());
                            round_failed_order.push(candidate_id.clone());
                        }
                        uncertainty
                            .insert(format!("{candidate_id}:executor-failed:{}", error.reason));
                        disposition = RobustActiveLearningCampaignDisposition::Failed;
                        stop_reason = RobustActiveLearningCampaignStopReason::ExecutorFailed;
                        break;
                    }
                }
            }
            if let Some(observation) = accepted {
                budget = budget.saturating_sub(candidate.cost_units);
                budget_spent = budget_spent.saturating_add(candidate.cost_units);
                if completed.insert(candidate_id.clone()) {
                    completed_order.push(candidate_id.clone());
                }
                observation_order.push(observation.observation_id.clone());
                observations.push(observation);
                executed_order.push(candidate_id.clone());
            }
            if disposition == RobustActiveLearningCampaignDisposition::Failed {
                break;
            }
        }
        rounds.push(RobustActiveLearningCampaignRound {
            round,
            plan,
            executed_order,
            observation_order,
            failed_order: round_failed_order,
            budget_before_units: budget_before,
            budget_after_units: budget,
        });
        if disposition == RobustActiveLearningCampaignDisposition::Failed {
            break;
        }
        candidates.retain(|candidate| !failed.contains(&candidate.candidate_id));
        if budget == 0 {
            disposition = RobustActiveLearningCampaignDisposition::Partial;
            stop_reason = RobustActiveLearningCampaignStopReason::BudgetExhausted;
            break;
        }
        if round + 1 == request.max_rounds {
            break;
        }
        if rounds
            .last()
            .is_some_and(|round| round.observation_order.is_empty())
        {
            disposition = RobustActiveLearningCampaignDisposition::Unresolved;
            stop_reason = RobustActiveLearningCampaignStopReason::Unresolved;
            break;
        }
    }
    if rounds.is_empty() {
        disposition = RobustActiveLearningCampaignDisposition::Blocked;
        stop_reason = RobustActiveLearningCampaignStopReason::NoCandidates;
    } else if disposition != RobustActiveLearningCampaignDisposition::Failed
        && stop_reason == RobustActiveLearningCampaignStopReason::MaxRounds
    {
        disposition = RobustActiveLearningCampaignDisposition::Partial;
        stop_reason = RobustActiveLearningCampaignStopReason::MaxRounds;
    }
    let mut campaign = RobustActiveLearningCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.robust_active_learning.objective.clone(),
        rounds,
        observations,
        completed_order,
        failed_order,
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: budget,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-robust-active-learning-campaign"),
    };
    campaign.digest = ContentHash::of_value(&digest_input(&campaign))
        .map_err(|error| RobustActiveLearningCampaignError::Digest(error.to_string()))?;
    campaign.validate()?;
    Ok(campaign)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::active_learning::ActiveLearningDirection;
    use crate::glioma::programs::p06_experiment_design::robust_active_learning::RobustActiveLearningModel;
    use crate::glioma_engine::GliomaModelSystem;

    fn request() -> RobustActiveLearningCampaignRequest {
        RobustActiveLearningCampaignRequest {
            robust_active_learning: RobustActiveLearningRequest {
                objective: "execute robust invasion assays".into(),
                model_system: GliomaModelSystem::Organoid,
                direction: ActiveLearningDirection::Maximize,
                budget_units: 4,
                max_selections: 1,
                min_observations_per_candidate: 1,
                lower_tail_weight_milli: 600,
                disagreement_weight_milli: 300,
                information_weight_milli: 100,
                cost_penalty_milli: 1,
                risk_penalty_milli: 1,
                max_risk_milli: 800,
                min_model_reliability_milli: 500,
                models: vec![RobustActiveLearningModel {
                    model_id: "mechanistic".into(),
                    prior_weight_milli: 1_000,
                    intercept_milli: 0,
                    feature_weights: vec![1, 2],
                    residual_milli: 40,
                    reliability_milli: 900,
                }],
            },
            candidates: vec![
                RobustActiveLearningCandidate {
                    candidate_id: "egfr".into(),
                    mechanism_id: "egfr".into(),
                    feature_vector: vec![100, 0],
                    cost_units: 2,
                    risk_milli: 100,
                    max_replicates: 2,
                    redundancy_group: "receptor".into(),
                    output_schema: "Assay1@1".into(),
                },
                RobustActiveLearningCandidate {
                    candidate_id: "matrix".into(),
                    mechanism_id: "matrix".into(),
                    feature_vector: vec![0, 100],
                    cost_units: 2,
                    risk_milli: 100,
                    max_replicates: 2,
                    redundancy_group: "matrix".into(),
                    output_schema: "Assay1@1".into(),
                },
            ],
            observations: Vec::new(),
            max_rounds: 3,
            max_retries: 1,
            stop_on_unresolved: false,
        }
    }

    #[test]
    fn campaign_replans_from_synthetic_observations() {
        let mut executor = DryRunRobustActiveLearningCampaignExecutor::default();
        let campaign =
            execute_glioma_robust_active_learning_campaign(&request(), &mut executor).unwrap();
        assert!(!campaign.rounds.is_empty());
        assert!(!campaign.completed_order.is_empty());
        assert!(campaign.budget_spent_units > 0);
        campaign.validate().unwrap();
    }

    #[test]
    fn no_candidates_is_a_first_class_block() {
        let mut request = request();
        request.candidates.clear();
        let mut executor = DryRunRobustActiveLearningCampaignExecutor::default();
        let campaign =
            execute_glioma_robust_active_learning_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            campaign.disposition,
            RobustActiveLearningCampaignDisposition::Blocked
        );
        assert_eq!(
            campaign.stop_reason,
            RobustActiveLearningCampaignStopReason::NoCandidates
        );
    }
}
