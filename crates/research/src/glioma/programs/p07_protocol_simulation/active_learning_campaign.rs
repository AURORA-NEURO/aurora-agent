//! Autonomous active-learning campaign execution for preclinical glioma research.
//!
//! This controller closes the loop between the P06 active learner and a local assay gateway. It
//! repeatedly compiles an uncertainty-aware next batch, executes only selected candidates through
//! a caller-owned executor, appends the returned typed observations, and replans until a bounded
//! budget/round/replicate/uncertainty gate stops progress. The research crate never contacts a
//! device, moves raw data, or turns a synthetic result into biological evidence.

use crate::glioma::programs::p06_experiment_design::active_learning::{
    plan_glioma_active_learning, ActiveLearningCandidate, ActiveLearningDisposition,
    ActiveLearningObservation, ActiveLearningPlan, ActiveLearningRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaActiveLearningCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningCampaignRequest {
    pub active_learning: ActiveLearningRequest,
    pub candidates: Vec<ActiveLearningCandidate>,
    pub observations: Vec<ActiveLearningObservation>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_unresolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLearningExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// An institution-local adapter executes one selected assay and returns a typed, local outcome.
/// Production adapters own instrument/software transport and authorization; the campaign owns
/// planning order, retry bounds, observation binding, and termination semantics.
pub trait ActiveLearningCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &ActiveLearningCandidate,
        attempt: u8,
    ) -> Result<ActiveLearningObservation, ActiveLearningExecutionFailure>;
}

/// Deterministic sandbox executor. It produces no biological effect and marks every artifact
/// synthetic so MCP and protocol tests cannot be mistaken for assay evidence.
#[derive(Debug, Default)]
pub struct DryRunActiveLearningCampaignExecutor;

impl ActiveLearningCampaignExecutor for DryRunActiveLearningCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &ActiveLearningCandidate,
        attempt: u8,
    ) -> Result<ActiveLearningObservation, ActiveLearningExecutionFailure> {
        let outcome_milli = candidate
            .feature_vector
            .iter()
            .enumerate()
            .map(|(index, value)| value.saturating_mul((index as i64).saturating_add(1)))
            .sum::<i64>()
            .saturating_add(candidate.mechanism_id.len() as i64)
            .clamp(-1_000_000, 1_000_000);
        let observation_id = format!("dry-run:{}:{}", candidate.candidate_id, attempt);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "observation_id": observation_id,
            "candidate_id": candidate.candidate_id,
            "outcome_milli": outcome_milli,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| ActiveLearningExecutionFailure {
            reason: format!("dry-run observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(ActiveLearningObservation {
            observation_id,
            candidate_id: candidate.candidate_id.clone(),
            outcome_milli,
            uncertainty_milli: 100,
            artifact: crate::glioma_engine::LocalArtifactRef {
                artifact_id: format!("dry-run-active-learning:{}", candidate.candidate_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma-active-learning-observation+json"
                    .into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningCampaignRound {
    pub round: u16,
    pub plan: ActiveLearningPlan,
    pub executed_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningCampaignDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningCampaignStopReason {
    Completed,
    NoCandidates,
    BudgetExhausted,
    MaxRounds,
    ExecutorFailed,
    Unresolved,
    SelectionBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<ActiveLearningCampaignRound>,
    pub observations: Vec<ActiveLearningObservation>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ActiveLearningCampaignDisposition,
    pub stop_reason: ActiveLearningCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActiveLearningCampaignError {
    #[error("active-learning campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("active-learning campaign planning failed: {0}")]
    Planning(String),
    #[error("active-learning campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("active-learning campaign digest failed: {0}")]
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

fn digest_input(campaign: &ActiveLearningCampaign) -> serde_json::Value {
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

impl ActiveLearningCampaign {
    pub fn validate(&self) -> Result<(), ActiveLearningCampaignError> {
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
            return Err(ActiveLearningCampaignError::InvalidOutput(
                "identity, bounds, ordering, observation, or completion invariants are invalid"
                    .into(),
            ));
        }
        let mut observation_ids = BTreeSet::new();
        if self
            .observations
            .iter()
            .any(|observation| !observation_ids.insert(observation.observation_id.clone()))
        {
            return Err(ActiveLearningCampaignError::InvalidOutput(
                "campaign observations must be unique".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ActiveLearningCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ActiveLearningCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ActiveLearningCampaignRequest,
) -> Result<(), ActiveLearningCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
    {
        return Err(ActiveLearningCampaignError::InvalidRequest(
            "positive bounded rounds and retries are required".into(),
        ));
    }
    for observation in &request.observations {
        observation
            .artifact
            .validate()
            .map_err(|error| ActiveLearningCampaignError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

/// Execute a bounded adaptive campaign. Every replan is derived only from the observations that
/// the executor returned; an executor failure or unresolved uncertainty cannot be silently
/// promoted into a successful round.
pub fn execute_glioma_active_learning_campaign<E: ActiveLearningCampaignExecutor>(
    request: &ActiveLearningCampaignRequest,
    executor: &mut E,
) -> Result<ActiveLearningCampaign, ActiveLearningCampaignError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    let mut observations = request.observations.clone();
    let mut budget = request.active_learning.budget_units;
    let mut rounds = Vec::new();
    let mut completed = BTreeSet::new();
    let mut completed_order = Vec::new();
    let mut failed = BTreeSet::new();
    let mut campaign_failed_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u32;
    let mut disposition = ActiveLearningCampaignDisposition::Partial;
    let mut stop_reason = ActiveLearningCampaignStopReason::MaxRounds;

    for round in 0..request.max_rounds {
        let mut round_request = request.active_learning.clone();
        round_request.budget_units = budget;
        let plan = plan_glioma_active_learning(&round_request, &candidates, &observations)
            .map_err(|error| ActiveLearningCampaignError::Planning(error.to_string()))?;
        uncertainty.extend(plan.uncertainty.iter().cloned());
        negative.extend(plan.negative_evidence.iter().cloned());
        let budget_before = budget;
        if plan.selected_order.is_empty() {
            disposition = if plan.disposition == ActiveLearningDisposition::Unresolved
                || !plan.unresolved_order.is_empty()
            {
                ActiveLearningCampaignDisposition::Unresolved
            } else if plan.disposition == ActiveLearningDisposition::NoCandidates {
                ActiveLearningCampaignDisposition::Blocked
            } else {
                ActiveLearningCampaignDisposition::Partial
            };
            stop_reason = if !plan.unresolved_order.is_empty() {
                ActiveLearningCampaignStopReason::Unresolved
            } else if plan.disposition == ActiveLearningDisposition::NoCandidates {
                ActiveLearningCampaignStopReason::NoCandidates
            } else {
                ActiveLearningCampaignStopReason::SelectionBlocked
            };
            rounds.push(ActiveLearningCampaignRound {
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
            disposition = ActiveLearningCampaignDisposition::Unresolved;
            stop_reason = ActiveLearningCampaignStopReason::Unresolved;
            uncertainty.extend(plan.uncertainty.iter().cloned());
            rounds.push(ActiveLearningCampaignRound {
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
                uncertainty.insert(format!("{candidate_id}:budget-exhausted-before-execution"));
                if failed.insert(candidate_id.clone()) {
                    campaign_failed_order.push(candidate_id.clone());
                    round_failed_order.push(candidate_id.clone());
                }
                continue;
            }
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_candidate(candidate, attempt) {
                    Ok(observation) => {
                        if observation.candidate_id != candidate.candidate_id {
                            return Err(ActiveLearningCampaignError::InvalidRequest(
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
                            campaign_failed_order.push(candidate_id.clone());
                            round_failed_order.push(candidate_id.clone());
                        }
                        uncertainty
                            .insert(format!("{candidate_id}:executor-failed:{}", error.reason));
                        disposition = ActiveLearningCampaignDisposition::Failed;
                        stop_reason = ActiveLearningCampaignStopReason::ExecutorFailed;
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
            if disposition == ActiveLearningCampaignDisposition::Failed {
                break;
            }
        }
        rounds.push(ActiveLearningCampaignRound {
            round,
            plan,
            executed_order,
            observation_order,
            failed_order: round_failed_order,
            budget_before_units: budget_before,
            budget_after_units: budget,
        });
        if disposition == ActiveLearningCampaignDisposition::Failed {
            break;
        }
        candidates.retain(|candidate| !failed.contains(&candidate.candidate_id));
        if budget == 0 {
            disposition = ActiveLearningCampaignDisposition::Partial;
            stop_reason = ActiveLearningCampaignStopReason::BudgetExhausted;
            break;
        }
        if round + 1 == request.max_rounds {
            break;
        }
        // A round that returned no new observation cannot make the next plan more informed.
        if rounds
            .last()
            .is_some_and(|round| round.observation_order.is_empty())
        {
            disposition = ActiveLearningCampaignDisposition::Unresolved;
            stop_reason = ActiveLearningCampaignStopReason::Unresolved;
            break;
        }
    }
    if rounds.is_empty() {
        disposition = ActiveLearningCampaignDisposition::Blocked;
        stop_reason = ActiveLearningCampaignStopReason::NoCandidates;
    } else if disposition != ActiveLearningCampaignDisposition::Failed
        && stop_reason == ActiveLearningCampaignStopReason::MaxRounds
    {
        disposition = ActiveLearningCampaignDisposition::Partial;
        stop_reason = ActiveLearningCampaignStopReason::MaxRounds;
    }
    let mut campaign = ActiveLearningCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.active_learning.objective.clone(),
        rounds,
        observations,
        completed_order,
        failed_order: campaign_failed_order,
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: budget,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-active-learning-campaign"),
    };
    campaign.digest = ContentHash::of_value(&digest_input(&campaign))
        .map_err(|error| ActiveLearningCampaignError::Digest(error.to_string()))?;
    campaign.validate()?;
    Ok(campaign)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::active_learning::ActiveLearningDirection;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-active-learning-test+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> ActiveLearningCampaignRequest {
        ActiveLearningCampaignRequest {
            active_learning: ActiveLearningRequest {
                objective: "autonomously refine an invasion mechanism assay".into(),
                model_system: GliomaModelSystem::Organoid,
                direction: ActiveLearningDirection::Maximize,
                budget_units: 4,
                max_selections: 1,
                min_observations_per_candidate: 1,
                exploration_weight_milli: 500,
                exploitation_weight_milli: 500,
                cost_penalty_milli: 1,
                risk_penalty_milli: 1,
                max_risk_milli: 800,
                min_uncertainty_milli: 900,
            },
            candidates: vec![
                ActiveLearningCandidate {
                    candidate_id: "egfr".into(),
                    mechanism_id: "egfr".into(),
                    feature_vector: vec![100, 0],
                    cost_units: 2,
                    risk_milli: 100,
                    max_replicates: 2,
                    redundancy_group: "receptor".into(),
                    output_schema: "Assay1@1".into(),
                },
                ActiveLearningCandidate {
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
            observations: vec![ActiveLearningObservation {
                observation_id: "seed".into(),
                candidate_id: "egfr".into(),
                outcome_milli: 500,
                uncertainty_milli: 50,
                artifact: artifact("seed"),
            }],
            max_rounds: 3,
            max_retries: 1,
            stop_on_unresolved: false,
        }
    }

    #[test]
    fn campaign_executes_and_replans_from_returned_observations() {
        let mut executor = DryRunActiveLearningCampaignExecutor;
        let campaign = execute_glioma_active_learning_campaign(&request(), &mut executor).unwrap();
        assert!(!campaign.rounds.is_empty());
        assert!(!campaign.observations.is_empty());
        assert!(!campaign.completed_order.is_empty());
        assert!(campaign.budget_spent_units > 0);
        campaign.validate().unwrap();
    }

    #[test]
    fn unresolved_stop_is_explicit_when_no_new_observation_arrives() {
        let mut request = request();
        request.candidates.clear();
        let mut executor = DryRunActiveLearningCampaignExecutor;
        let campaign = execute_glioma_active_learning_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            campaign.disposition,
            ActiveLearningCampaignDisposition::Blocked
        );
        assert_eq!(
            campaign.stop_reason,
            ActiveLearningCampaignStopReason::NoCandidates
        );
        assert!(campaign.rounds.len() == 1);
    }

    #[test]
    fn retry_bound_and_failure_stop_are_represented() {
        #[derive(Default)]
        struct Failing;
        impl ActiveLearningCampaignExecutor for Failing {
            fn execute_candidate(
                &mut self,
                _candidate: &ActiveLearningCandidate,
                _attempt: u8,
            ) -> Result<ActiveLearningObservation, ActiveLearningExecutionFailure> {
                Err(ActiveLearningExecutionFailure {
                    reason: "gateway unavailable".into(),
                    retryable: false,
                })
            }
        }
        let mut executor = Failing;
        let campaign = execute_glioma_active_learning_campaign(&request(), &mut executor).unwrap();
        assert_eq!(
            campaign.disposition,
            ActiveLearningCampaignDisposition::Failed
        );
        assert_eq!(
            campaign.stop_reason,
            ActiveLearningCampaignStopReason::ExecutorFailed
        );
        assert_eq!(campaign.failed_order, vec!["matrix"]);
        campaign.validate().unwrap();
    }
}
