//! Closed-loop multi-fidelity intervention campaigns for preclinical glioma research.
//!
//! [`super::multi_fidelity`] already provides the integer-only acquisition algorithm for
//! screening, mechanistic, and validation conditions.  This module turns that planner into a
//! usable autonomous workflow: after each local batch it appends only the returned typed
//! observations, recalibrates transfer bias/reliability, and replans the next fidelity under a
//! hard budget and replicate gate.  A transferred or neighbourhood estimate can rank a
//! condition, but it is never inserted into the observation set as if it were measured.

use super::multi_fidelity::{
    plan_glioma_multi_fidelity_optimization, FidelityCandidate, FidelityObservation,
    MultiFidelityDisposition, MultiFidelityOptimizationError, MultiFidelityOptimizationPlan,
    MultiFidelityOptimizationRequest,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiFidelityCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityCampaignRequest {
    pub optimization: MultiFidelityOptimizationRequest,
    pub candidates: Vec<FidelityCandidate>,
    pub observations: Vec<FidelityObservation>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFidelityExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, simulator, or analysis workers implement this seam.  The campaign
/// supplies the candidate and deterministic replicate index; the worker returns a typed local
/// observation and owns all actual data/tool effects.
pub trait MultiFidelityCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &FidelityCandidate,
        replicate_index: u16,
        attempt: u8,
    ) -> Result<FidelityObservation, MultiFidelityExecutionFailure>;
}

/// Deterministic sandbox worker.  It emits synthetic local artifacts only; its values are useful
/// for replay and MCP integration tests but never count as biological evidence.
#[derive(Debug, Default)]
pub struct DryRunMultiFidelityCampaignExecutor;

impl MultiFidelityCampaignExecutor for DryRunMultiFidelityCampaignExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &FidelityCandidate,
        replicate_index: u16,
        attempt: u8,
    ) -> Result<FidelityObservation, MultiFidelityExecutionFailure> {
        let level_bonus = i64::from(candidate.fidelity as u8) * 125;
        let outcome_milli = i64::from(candidate.dose_milli)
            .saturating_add(i64::from(candidate.combination_milli) / 2)
            .saturating_add(level_bonus)
            .saturating_add(i64::from(replicate_index) * 7);
        let observation_id = format!(
            "dry-run:{}:replicate-{}:attempt-{}",
            candidate.candidate_id, replicate_index, attempt
        );
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "observation_id": observation_id,
            "candidate_id": candidate.candidate_id,
            "replicate_index": replicate_index,
            "outcome_milli": outcome_milli,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| MultiFidelityExecutionFailure {
            reason: format!("dry-run observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(FidelityObservation {
            observation_id,
            candidate_id: candidate.candidate_id.clone(),
            replicate_index,
            outcome_milli,
            uncertainty_milli: 100,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-multi-fidelity:{}", candidate.candidate_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.multi-fidelity-observation+json"
                    .into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityCampaignRound {
    pub round: u16,
    pub plan: MultiFidelityOptimizationPlan,
    pub selected_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiFidelityCampaignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    NoEligibleCandidates,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiFidelityCampaignStopReason {
    Qualified,
    BudgetExhausted,
    NoEligibleCandidates,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<MultiFidelityCampaignRound>,
    pub observations: Vec<FidelityObservation>,
    pub completed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub best_observed_milli: Option<i64>,
    pub final_plan: Option<MultiFidelityOptimizationPlan>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultiFidelityCampaignDisposition,
    pub stop_reason: MultiFidelityCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiFidelityCampaignError {
    #[error("multi-fidelity campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-fidelity campaign planning failed: {0}")]
    Planning(String),
    #[error("multi-fidelity campaign execution failed: {0}")]
    Execution(String),
    #[error("multi-fidelity campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-fidelity campaign digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_strings(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &MultiFidelityCampaign) -> serde_json::Value {
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
        "best_observed_milli": campaign.best_observed_milli,
        "final_plan": campaign.final_plan,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

impl MultiFidelityCampaign {
    pub fn validate(&self) -> Result<(), MultiFidelityCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !unique_strings(&self.completed_order)
            || !unique_strings(&self.failed_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .completed_order
                .iter()
                .any(|id| self.failed_order.binary_search(id).is_ok())
        {
            return Err(MultiFidelityCampaignError::InvalidOutput(
                "identity, canonical partitions, or campaign evidence fields are invalid".into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut seen_observations = BTreeSet::new();
        let mut expected_spend = 0_u64;
        let mut expected_retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || round.selected_order != round.plan.selected_order
                || !canonical(&round.observation_order)
                || !canonical(&round.failed_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(MultiFidelityCampaignError::InvalidOutput(
                    "round ordering, selection, observation, or budget invariants are invalid"
                        .into(),
                ));
            }
            round
                .plan
                .validate()
                .map_err(|error| MultiFidelityCampaignError::InvalidOutput(error.to_string()))?;
            for observation_id in &round.observation_order {
                if !seen_observations.insert(observation_id.clone()) {
                    return Err(MultiFidelityCampaignError::InvalidOutput(
                        "an observation was emitted in more than one campaign round".into(),
                    ));
                }
            }
            expected_spend = expected_spend.saturating_add(u64::from(round.cost_units));
            expected_retries = expected_retries.saturating_add(round.retry_count);
        }
        if expected_spend != self.budget_spent_units {
            return Err(MultiFidelityCampaignError::InvalidOutput(
                "campaign budget does not reconcile with rounds".into(),
            ));
        }
        if expected_retries != self.retry_count {
            return Err(MultiFidelityCampaignError::InvalidOutput(
                "campaign retry count is inconsistent".into(),
            ));
        }
        for observation in &self.observations {
            if observation.observation_id.trim().is_empty()
                || observation.uncertainty_milli == 0
                || observation.artifact.validate().is_err()
            {
                return Err(MultiFidelityCampaignError::InvalidOutput(
                    "campaign observation identity, uncertainty, or artifact is invalid".into(),
                ));
            }
        }
        if let Some(plan) = &self.final_plan {
            plan.validate()
                .map_err(|error| MultiFidelityCampaignError::InvalidOutput(error.to_string()))?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiFidelityCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiFidelityCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MultiFidelityCampaignRequest,
) -> Result<(), MultiFidelityCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.candidates.is_empty()
    {
        return Err(MultiFidelityCampaignError::InvalidRequest(
            "bounded positive rounds/retries and a non-empty candidate registry are required"
                .into(),
        ));
    }
    plan_glioma_multi_fidelity_optimization(
        &request.optimization,
        &request.candidates,
        &request.observations,
    )
    .map_err(|error| MultiFidelityCampaignError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn best_observed(
    direction: super::multi_fidelity::OptimizationDirection,
    observations: &[FidelityObservation],
) -> Option<i64> {
    observations
        .iter()
        .map(|observation| observation.outcome_milli)
        .reduce(|left, right| match direction {
            super::multi_fidelity::OptimizationDirection::Maximize => left.max(right),
            super::multi_fidelity::OptimizationDirection::Minimize => left.min(right),
        })
}

fn next_replicate_index(observations: &[FidelityObservation], candidate_id: &str) -> u16 {
    observations
        .iter()
        .filter(|observation| observation.candidate_id == candidate_id)
        .map(|observation| observation.replicate_index)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn all_candidates_observed(
    candidates: &BTreeMap<String, &FidelityCandidate>,
    observations: &[FidelityObservation],
) -> bool {
    candidates.keys().all(|candidate_id| {
        observations
            .iter()
            .any(|observation| observation.candidate_id == *candidate_id)
    })
}

fn validate_provider_observation(
    observation: &FidelityObservation,
    candidate: &FidelityCandidate,
    replicate_index: u16,
    require_artifacts: bool,
) -> Result<(), MultiFidelityCampaignError> {
    if observation.observation_id.trim().is_empty()
        || observation.candidate_id != candidate.candidate_id
        || observation.replicate_index != replicate_index
        || observation.uncertainty_milli == 0
        || (require_artifacts && observation.artifact.artifact_id.trim().is_empty())
    {
        return Err(MultiFidelityCampaignError::Execution(format!(
            "executor returned an invalid observation for candidate {}",
            candidate.candidate_id
        )));
    }
    observation
        .artifact
        .validate()
        .map_err(|error| MultiFidelityCampaignError::Execution(error.to_string()))?;
    Ok(())
}

/// Execute a bounded multi-fidelity intervention campaign.  The planner is rerun from the full
/// returned observation set after every batch; no transfer or prior estimate is ever appended as
/// an observation, and a failed/partial provider effect terminates the campaign.
pub fn execute_glioma_multi_fidelity_campaign<E: MultiFidelityCampaignExecutor>(
    request: &MultiFidelityCampaignRequest,
    executor: &mut E,
) -> Result<MultiFidelityCampaign, MultiFidelityCampaignError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut rounds = Vec::new();
    let mut failed = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_spent_units = 0_u64;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut final_plan = None;
    let mut stop_reason = MultiFidelityCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let remaining_budget = request
            .optimization
            .budget_units
            .saturating_sub(budget_spent_units);
        if remaining_budget == 0 {
            stop_reason = MultiFidelityCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut optimization = request.optimization.clone();
        optimization.budget_units = remaining_budget;
        let plan = plan_glioma_multi_fidelity_optimization(
            &optimization,
            &request.candidates,
            &observations,
        )
        .map_err(|error: MultiFidelityOptimizationError| {
            MultiFidelityCampaignError::Planning(error.to_string())
        })?;
        negative_evidence.extend(plan.negative_evidence.iter().cloned());
        uncertainty.extend(plan.uncertainty.iter().cloned());
        // The planner can be `Qualified` while ranking a neighbourhood or transferred
        // estimate.  That is useful for choosing the next action, but it is not a stopping
        // certificate for an autonomous campaign: every catalogued condition must have direct
        // observations before the campaign is allowed to claim a measured frontier.  This keeps
        // inferred support from silently becoming evidence.
        let all_estimates_observed = plan
            .estimates
            .iter()
            .all(|estimate| estimate.observed_replicates > 0);
        if request.stop_on_qualified
            && !rounds.is_empty()
            && plan.disposition == MultiFidelityDisposition::Qualified
            && all_estimates_observed
        {
            final_plan = Some(plan);
            stop_reason = MultiFidelityCampaignStopReason::Qualified;
            break;
        }
        if plan.selected_order.is_empty() {
            final_plan = Some(plan.clone());
            stop_reason = match plan.disposition {
                MultiFidelityDisposition::BudgetBlocked => {
                    MultiFidelityCampaignStopReason::BudgetExhausted
                }
                MultiFidelityDisposition::NoEligibleCandidates
                | MultiFidelityDisposition::Unresolved => {
                    MultiFidelityCampaignStopReason::NoEligibleCandidates
                }
                MultiFidelityDisposition::Partial | MultiFidelityDisposition::Qualified => {
                    MultiFidelityCampaignStopReason::NoEligibleCandidates
                }
            };
            break;
        }
        let before_budget = remaining_budget;
        let cost_by_id = candidate_map
            .iter()
            .map(|(id, candidate)| (id.clone(), candidate.cost_units))
            .collect::<BTreeMap<_, _>>();
        let round_cost = plan
            .selected_order
            .iter()
            .map(|id| cost_by_id.get(id).copied().unwrap_or(0))
            .sum::<u32>();
        let mut observation_order = Vec::new();
        let mut failed_order = Vec::new();
        let mut round_progress = false;
        let mut round_retry_count = 0_u32;
        for candidate_id in &plan.selected_order {
            let candidate = candidate_map.get(candidate_id).ok_or_else(|| {
                MultiFidelityCampaignError::Execution(
                    "planned candidate missing from registry".into(),
                )
            })?;
            let replicate_index = next_replicate_index(&observations, candidate_id);
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_candidate(candidate, replicate_index, attempt) {
                    Ok(observation) => {
                        validate_provider_observation(
                            &observation,
                            candidate,
                            replicate_index,
                            request.require_artifacts,
                        )?;
                        if observations.iter().any(|existing| {
                            existing.observation_id == observation.observation_id
                                || (existing.candidate_id == observation.candidate_id
                                    && existing.replicate_index == observation.replicate_index)
                        }) {
                            return Err(MultiFidelityCampaignError::Execution(
                                "executor returned a duplicate observation identity".into(),
                            ));
                        }
                        accepted = Some(observation);
                        break;
                    }
                    Err(failure) => {
                        if failure.reason.trim().is_empty() {
                            return Err(MultiFidelityCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if failure.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            round_retry_count = round_retry_count.saturating_add(1);
                            continue;
                        }
                        failed_order.push(candidate_id.clone());
                        failed.insert(candidate_id.clone());
                        break;
                    }
                }
            }
            if let Some(observation) = accepted {
                observation_order.push(observation.observation_id.clone());
                observations.push(observation);
                round_progress = true;
            } else {
                break;
            }
        }
        observation_order.sort();
        failed_order.sort();
        budget_spent_units = budget_spent_units.saturating_add(u64::from(round_cost));
        let after_budget = request
            .optimization
            .budget_units
            .saturating_sub(budget_spent_units);
        rounds.push(MultiFidelityCampaignRound {
            round: round_number,
            selected_order: plan.selected_order.clone(),
            plan,
            observation_order,
            failed_order: failed_order.clone(),
            retry_count: round_retry_count,
            cost_units: round_cost,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
        });
        if !failed_order.is_empty() {
            stop_reason = MultiFidelityCampaignStopReason::ExecutorFailed;
            break;
        }
        if !round_progress {
            stop_reason = MultiFidelityCampaignStopReason::NoProgress;
            break;
        }
        // A final successful batch can consume the entire budget.  Preserve a qualified
        // disposition when that batch supplied direct observations for every catalogued
        // condition; otherwise the budget gate remains explicit rather than being mistaken for
        // an evidence-backed completion.
        if request.stop_on_qualified && all_candidates_observed(&candidate_map, &observations) {
            stop_reason = MultiFidelityCampaignStopReason::Qualified;
            break;
        }
        if after_budget == 0 {
            stop_reason = MultiFidelityCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    if request.stop_on_qualified
        && failed.is_empty()
        && all_candidates_observed(&candidate_map, &observations)
    {
        stop_reason = MultiFidelityCampaignStopReason::Qualified;
    }

    let completed_order = observations
        .iter()
        .map(|observation| observation.candidate_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let best = best_observed(request.optimization.direction, &observations);
    let disposition = if stop_reason == MultiFidelityCampaignStopReason::Qualified {
        MultiFidelityCampaignDisposition::Qualified
    } else if !failed.is_empty() {
        MultiFidelityCampaignDisposition::Failed
    } else if matches!(
        stop_reason,
        MultiFidelityCampaignStopReason::BudgetExhausted
    ) {
        MultiFidelityCampaignDisposition::BudgetBlocked
    } else if rounds.is_empty() {
        MultiFidelityCampaignDisposition::NoEligibleCandidates
    } else if observations.is_empty() {
        MultiFidelityCampaignDisposition::Unresolved
    } else {
        MultiFidelityCampaignDisposition::Partial
    };
    let mut output = MultiFidelityCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.optimization.objective.clone(),
        rounds,
        observations,
        completed_order,
        failed_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units,
        remaining_budget_units: request
            .optimization
            .budget_units
            .saturating_sub(budget_spent_units),
        best_observed_milli: best,
        final_plan,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-fidelity-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiFidelityCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn optimization() -> MultiFidelityOptimizationRequest {
        MultiFidelityOptimizationRequest {
            objective: "maximize invasion suppression in glioma models".into(),
            direction: super::super::multi_fidelity::OptimizationDirection::Maximize,
            budget_units: 8,
            max_selections: 1,
            min_replicates_per_candidate: 1,
            exploration_weight_milli: 500,
            exploitation_weight_milli: 300,
            transfer_weight_milli: 200,
            risk_penalty_milli: 1,
            cost_penalty_milli: 1,
            max_risk_milli: 800,
            min_transfer_reliability_milli: 250,
            baseline_milli: Some(0),
        }
    }

    fn candidate(
        id: &str,
        fidelity: super::super::multi_fidelity::FidelityLevel,
    ) -> FidelityCandidate {
        FidelityCandidate {
            candidate_id: id.into(),
            design_id: "design-egfr".into(),
            fidelity,
            model_system: GliomaModelSystem::Organoid,
            dose_milli: if fidelity == super::super::multi_fidelity::FidelityLevel::Screening {
                100
            } else {
                200
            },
            combination_milli: 50,
            cost_units: 2,
            risk_milli: 100,
            parent_candidate_id: if fidelity
                == super::super::multi_fidelity::FidelityLevel::Screening
            {
                None
            } else {
                Some("screening".into())
            },
            max_replicates: 2,
        }
    }

    fn request() -> MultiFidelityCampaignRequest {
        MultiFidelityCampaignRequest {
            optimization: optimization(),
            candidates: vec![
                candidate(
                    "screening",
                    super::super::multi_fidelity::FidelityLevel::Screening,
                ),
                candidate(
                    "mechanistic",
                    super::super::multi_fidelity::FidelityLevel::Mechanistic,
                ),
            ],
            observations: Vec::new(),
            max_rounds: 4,
            max_retries: 1,
            require_artifacts: true,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn campaign_executes_screening_then_replans_without_promoting_transfer() {
        let mut executor = DryRunMultiFidelityCampaignExecutor;
        let campaign = execute_glioma_multi_fidelity_campaign(&request(), &mut executor).unwrap();
        assert!(!campaign.rounds.is_empty());
        assert!(campaign.observations.iter().all(|observation| {
            observation.artifact.local_only && !observation.artifact.contains_human_data
        }));
        assert!(campaign.completed_order.contains(&"screening".into()));
        assert!(campaign
            .rounds
            .iter()
            .all(|round| round.plan.observed_order.iter().all(|id| {
                campaign
                    .observations
                    .iter()
                    .any(|observation| &observation.candidate_id == id)
            })));
        campaign.validate().unwrap();
    }

    #[test]
    fn invalid_provider_observation_cannot_enter_campaign() {
        struct BadExecutor;
        impl MultiFidelityCampaignExecutor for BadExecutor {
            fn execute_candidate(
                &mut self,
                candidate: &FidelityCandidate,
                _replicate_index: u16,
                _attempt: u8,
            ) -> Result<FidelityObservation, MultiFidelityExecutionFailure> {
                Ok(FidelityObservation {
                    observation_id: "bad".into(),
                    candidate_id: candidate.candidate_id.clone(),
                    replicate_index: 0,
                    outcome_milli: 1,
                    uncertainty_milli: 0,
                    artifact: LocalArtifactRef {
                        artifact_id: "bad".into(),
                        content_hash: ContentHash::of_bytes(b"bad"),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                })
            }
        }
        let mut executor = BadExecutor;
        assert!(matches!(
            execute_glioma_multi_fidelity_campaign(&request(), &mut executor),
            Err(MultiFidelityCampaignError::Execution(_))
        ));
    }

    #[test]
    fn retryable_executor_failure_is_bounded_and_explicit() {
        struct RetryThenFail {
            calls: u8,
        }
        impl MultiFidelityCampaignExecutor for RetryThenFail {
            fn execute_candidate(
                &mut self,
                _candidate: &FidelityCandidate,
                _replicate_index: u16,
                _attempt: u8,
            ) -> Result<FidelityObservation, MultiFidelityExecutionFailure> {
                self.calls = self.calls.saturating_add(1);
                Err(MultiFidelityExecutionFailure {
                    reason: "local assay unavailable".into(),
                    retryable: self.calls == 1,
                })
            }
        }
        let mut executor = RetryThenFail { calls: 0 };
        let campaign = execute_glioma_multi_fidelity_campaign(&request(), &mut executor).unwrap();
        assert_eq!(
            campaign.disposition,
            MultiFidelityCampaignDisposition::Failed
        );
        assert_eq!(
            campaign.stop_reason,
            MultiFidelityCampaignStopReason::ExecutorFailed
        );
        assert_eq!(campaign.retry_count, 1);
        campaign.validate().unwrap();
    }
}
