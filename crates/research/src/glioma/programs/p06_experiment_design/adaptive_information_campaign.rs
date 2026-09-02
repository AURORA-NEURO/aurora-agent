//! Adaptive information-design campaigns for preclinical glioma research.
//!
//! This module turns the one-step information-design planner into a resumable, closed-loop
//! campaign.  A caller-owned local executor returns one typed assay outcome at a time; the
//! engine updates competing-mechanism posterior mass with integer Bayes arithmetic and replans
//! the next assay.  It never invents an outcome, opens a device connection, or makes a clinical
//! decision.

use super::information_design::{
    plan_glioma_information_design, DesignAction, DesignMechanism, InformationDesignActionScore,
    InformationDesignDisposition, InformationDesignRequest,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveInformationCampaign1@1";
pub const EXECUTION_OUTPUT_SCHEMA: &str = "GliomaAdaptiveInformationCampaignExecution1@1";
pub const MAX_ROUNDS: u16 = 128;
pub const MAX_ACTIONS_PER_ROUND: usize = 64;
pub const SCORE_SCALE: u64 = 1_000;
/// One milli-unit of posterior mass keeps deterministic declared likelihoods from manufacturing
/// irreversible certainty after a single observation.
pub const POSTERIOR_SMOOTHING_MILLI: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInformationCampaignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_rounds: u16,
    pub max_actions_per_round: usize,
    pub budget_units: u64,
    pub min_information_gain_milli: u64,
    pub information_weight_milli: u16,
    pub feasibility_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
    pub risk_ceiling_milli: u16,
    pub stop_concentration_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInformationObservation {
    pub action_id: String,
    pub outcome_id: String,
    pub replicate_index: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPosterior {
    pub mechanism_id: String,
    pub prior_milli: u16,
    pub posterior_milli: u16,
    pub observations_used: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInformationCampaignPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub posterior_order: Vec<AdaptiveMechanismPosterior>,
    /// Actions that remain eligible after replicate and budget gates.
    pub action_order: Vec<String>,
    pub exhausted_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub observed_order: Vec<String>,
    pub observed_outcome_order: Vec<String>,
    pub observed_artifact_digest_order: Vec<ContentHash>,
    pub scores: Vec<InformationDesignActionScore>,
    pub budget_units: u64,
    pub budget_remaining_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptiveInformationCampaignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInformationCampaignRound {
    pub round: u16,
    pub selected_action_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub budget_spent_units: u64,
    pub expected_information_milli: u64,
    pub posterior_before_order: Vec<AdaptiveMechanismPosterior>,
    pub posterior_after_order: Vec<AdaptiveMechanismPosterior>,
    pub planner_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInformationCampaignExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub rounds: Vec<AdaptiveInformationCampaignRound>,
    pub observations: Vec<AdaptiveInformationObservation>,
    pub final_plan: AdaptiveInformationCampaignPlan,
    pub execution_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveInformationCampaignDisposition {
    Qualified,
    Converged,
    BudgetBlocked,
    NoInformativeActions,
    Partial,
    Unresolved,
}

/// The only effectful seam. An institution-local assay gateway or deterministic simulator owns
/// execution and returns an artifact-backed categorical outcome for the selected action.
pub trait GliomaInformationDesignExecutor {
    fn execute_action(
        &mut self,
        action: &DesignAction,
        round: u16,
    ) -> Result<AdaptiveInformationObservation, AdaptiveInformationExecutionFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveInformationExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveInformationCampaignError {
    #[error("adaptive information campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive information campaign input is invalid: {0}")]
    InvalidInput(String),
    #[error("adaptive information campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive information campaign digest failed: {0}")]
    Digest(String),
    #[error("adaptive information campaign executor failed: {0}")]
    Executor(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_plan(plan: &AdaptiveInformationCampaignPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "mechanism_order": plan.mechanism_order,
        "posterior_order": plan.posterior_order,
        "action_order": plan.action_order,
        "exhausted_order": plan.exhausted_order,
        "next_action_order": plan.next_action_order,
        "deferred_order": plan.deferred_order,
        "observed_order": plan.observed_order,
        "observed_outcome_order": plan.observed_outcome_order,
        "observed_artifact_digest_order": plan.observed_artifact_digest_order,
        "scores": plan.scores,
        "budget_units": plan.budget_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "uncertainty": plan.uncertainty,
        "negative_evidence": plan.negative_evidence,
        "disposition": plan.disposition,
    })
}

fn digest_execution(execution: &AdaptiveInformationCampaignExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": execution.feature_id,
        "output_schema": execution.output_schema,
        "rounds": execution.rounds,
        "observations": execution.observations,
        "final_plan": execution.final_plan,
    })
}

fn validate_request(
    request: &AdaptiveInformationCampaignRequest,
) -> Result<(), AdaptiveInformationCampaignError> {
    if request.objective.trim().is_empty()
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS_PER_ROUND
        || request.budget_units == 0
        || request.min_information_gain_milli > SCORE_SCALE
        || request.information_weight_milli > 1_000
        || request.feasibility_weight_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || request.cost_penalty_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.stop_concentration_milli > 1_000
        || request.stop_concentration_milli == 0
        || request
            .information_weight_milli
            .saturating_add(request.feasibility_weight_milli)
            .saturating_add(request.risk_penalty_milli)
            .saturating_add(request.cost_penalty_milli)
            == 0
    {
        return Err(AdaptiveInformationCampaignError::InvalidRequest(
            "objective, bounded rounds/actions, positive budget, thresholds, and score bounds are required".into(),
        ));
    }
    Ok(())
}

fn normalized_posterior(
    prior: &[u16],
    likelihood: &[u16],
) -> Result<Vec<u16>, AdaptiveInformationCampaignError> {
    if prior.len() != likelihood.len() || prior.is_empty() {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "posterior and likelihood vectors must have equal non-zero length".into(),
        ));
    }
    let masses = prior
        .iter()
        .zip(likelihood.iter())
        .map(|(left, right)| {
            u64::from(*left)
                .saturating_mul(u64::from(*right))
                .saturating_add(POSTERIOR_SMOOTHING_MILLI)
        })
        .collect::<Vec<_>>();
    let total = masses.iter().copied().sum::<u64>();
    if total == 0 {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "observed outcome has zero probability under every declared mechanism".into(),
        ));
    }
    let mut posterior = masses
        .iter()
        .map(|mass| ((u128::from(*mass) * 1_000) / u128::from(total)) as u16)
        .collect::<Vec<_>>();
    let assigned = posterior.iter().map(|value| u32::from(*value)).sum::<u32>();
    let remainder = 1_000_u32.saturating_sub(assigned);
    if remainder > 0 {
        let best = masses
            .iter()
            .enumerate()
            .max_by_key(|(index, mass)| (**mass, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
            .unwrap_or(0);
        posterior[best] = posterior[best].saturating_add(remainder as u16);
    }
    Ok(posterior)
}

fn action_map(actions: &[DesignAction]) -> BTreeMap<String, DesignAction> {
    actions
        .iter()
        .cloned()
        .map(|action| (action.action_id.clone(), action))
        .collect()
}

fn apply_observation(
    posterior: &mut [AdaptiveMechanismPosterior],
    actions: &BTreeMap<String, DesignAction>,
    observation: &AdaptiveInformationObservation,
) -> Result<(), AdaptiveInformationCampaignError> {
    let action = actions.get(&observation.action_id).ok_or_else(|| {
        AdaptiveInformationCampaignError::InvalidInput(format!(
            "observation references unknown action {}",
            observation.action_id
        ))
    })?;
    let outcome = action
        .outcomes
        .iter()
        .find(|outcome| outcome.outcome_id == observation.outcome_id)
        .ok_or_else(|| {
            AdaptiveInformationCampaignError::InvalidInput(format!(
                "observation references unknown outcome {} for action {}",
                observation.outcome_id, observation.action_id
            ))
        })?;
    let prior = posterior
        .iter()
        .map(|entry| entry.posterior_milli)
        .collect::<Vec<_>>();
    let likelihood = posterior
        .iter()
        .map(|entry| {
            outcome
                .probability_milli_by_mechanism
                .get(&entry.mechanism_id)
                .copied()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let values = normalized_posterior(&prior, &likelihood)?;
    for (entry, value) in posterior.iter_mut().zip(values) {
        entry.posterior_milli = value;
        entry.observations_used = entry.observations_used.saturating_add(1);
    }
    Ok(())
}

fn validate_inputs(
    request: &AdaptiveInformationCampaignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
    observations: &[AdaptiveInformationObservation],
) -> Result<(), AdaptiveInformationCampaignError> {
    validate_request(request)?;
    if mechanisms.len() < 2 {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "at least two competing mechanisms are required".into(),
        ));
    }
    let mechanism_ids = mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanism_ids.len() != mechanisms.len()
        || mechanisms
            .iter()
            .any(|mechanism| mechanism.mechanism_id.trim().is_empty() || mechanism.prior_milli == 0)
        || mechanisms
            .iter()
            .map(|mechanism| u32::from(mechanism.prior_milli))
            .sum::<u32>()
            != 1_000
    {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "mechanism ids must be unique and positive priors must sum to 1000".into(),
        ));
    }
    if actions.is_empty() {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "at least one assay action is required".into(),
        ));
    }
    let action_ids = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>();
    if action_ids.len() != actions.len() {
        return Err(AdaptiveInformationCampaignError::InvalidInput(
            "action ids must be unique".into(),
        ));
    }
    // Reuse the canonical one-step validator for outcome cardinality, per-mechanism probability
    // closure, risk/cost bounds, and typed action identity.
    let probe = InformationDesignRequest {
        objective: request.objective.clone(),
        model_system: request.model_system,
        budget_units: 1,
        max_selected_actions: 1,
        min_information_gain_milli: 0,
        information_weight_milli: 1_000,
        feasibility_weight_milli: 0,
        risk_penalty_milli: 0,
        cost_penalty_milli: 0,
        risk_ceiling_milli: 1_000,
    };
    plan_glioma_information_design(&probe, mechanisms, actions).map_err(|error| {
        AdaptiveInformationCampaignError::InvalidInput(format!("invalid action portfolio: {error}"))
    })?;
    let map = action_map(actions);
    let mut keys = BTreeSet::new();
    for observation in observations {
        if observation.action_id.trim().is_empty()
            || observation.outcome_id.trim().is_empty()
            || observation.replicate_index == 0
            || !keys.insert((observation.action_id.clone(), observation.replicate_index))
        {
            return Err(AdaptiveInformationCampaignError::InvalidInput(
                "observations need unique action/replicate keys and positive indexes".into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| AdaptiveInformationCampaignError::InvalidInput(error.to_string()))?;
        if !map.contains_key(&observation.action_id) {
            return Err(AdaptiveInformationCampaignError::InvalidInput(
                "observation references an unknown action".into(),
            ));
        }
    }
    Ok(())
}

impl AdaptiveInformationCampaignPlan {
    pub fn validate(&self) -> Result<(), AdaptiveInformationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.mechanism_order.len() < 2
            || !canonical(&self.mechanism_order)
            || !canonical(&self.action_order)
            || !canonical(&self.exhausted_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.observed_order)
            || !canonical(&self.observed_outcome_order)
            || self
                .observed_artifact_digest_order
                .iter()
                .any(|digest| digest.as_str().len() != 64)
            || self.observed_outcome_order.len() != self.observed_artifact_digest_order.len()
            || self.budget_remaining_units > self.budget_units
            || self.posterior_order.len() != self.mechanism_order.len()
            || self.posterior_order.iter().any(|entry| {
                entry.mechanism_id.trim().is_empty()
                    || entry.posterior_milli > 1_000
                    || entry.prior_milli > 1_000
            })
            || self
                .posterior_order
                .iter()
                .map(|entry| u32::from(entry.posterior_milli))
                .sum::<u32>()
                != 1_000
            || self.scores.windows(2).any(|pair| {
                pair[0].utility_milli < pair[1].utility_milli
                    || (pair[0].utility_milli == pair[1].utility_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self
                .scores
                .iter()
                .any(|score| score.selected_replicates > 1_000 || score.rationale.trim().is_empty())
        {
            return Err(AdaptiveInformationCampaignError::InvalidOutput(
                "identity, canonical ordering, posterior, budget, ranking, or score bounds are invalid".into(),
            ));
        }
        let posterior_ids = self
            .posterior_order
            .iter()
            .map(|entry| entry.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        let action_ids = self
            .action_order
            .iter()
            .chain(self.exhausted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<BTreeSet<_>>();
        let selected_ids = self
            .next_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred_ids = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        if posterior_ids
            != self
                .mechanism_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || (!score_ids.is_empty()
                && score_ids != self.action_order.iter().cloned().collect::<BTreeSet<_>>())
            || self
                .action_order
                .iter()
                .any(|id| self.exhausted_order.contains(id))
            || selected_ids.len() != self.next_action_order.len()
            || deferred_ids.len() != self.deferred_order.len()
            || self
                .next_action_order
                .iter()
                .any(|id| !self.action_order.iter().any(|candidate| candidate == id))
            || self.deferred_order.iter().any(|id| {
                !self
                    .action_order
                    .iter()
                    .chain(self.exhausted_order.iter())
                    .any(|candidate| candidate == id)
            })
            || selected_ids.intersection(&deferred_ids).next().is_some()
            || selected_ids
                .union(&deferred_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != action_ids
        {
            return Err(AdaptiveInformationCampaignError::InvalidOutput(
                "posterior and action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_plan(self))
            .map_err(|error| AdaptiveInformationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveInformationCampaignError::InvalidOutput(
                "adaptive campaign plan digest is not bound to its content".into(),
            ));
        }
        Ok(())
    }
}

impl AdaptiveInformationCampaignExecution {
    pub fn validate(&self) -> Result<(), AdaptiveInformationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != EXECUTION_OUTPUT_SCHEMA
            || self
                .rounds
                .windows(2)
                .any(|pair| pair[0].round >= pair[1].round)
            || self.rounds.iter().any(|round| {
                round.round == 0
                    || round.selected_action_order.is_empty()
                    || round.selected_action_order.len() != round.observation_order.len()
                    || round.planner_digest.as_str().len() != 64
                    || round.posterior_before_order.is_empty()
                    || round.posterior_after_order.is_empty()
            })
        {
            return Err(AdaptiveInformationCampaignError::InvalidOutput(
                "execution identity, round ordering, observations, or posterior history is invalid"
                    .into(),
            ));
        }
        self.final_plan.validate()?;
        let expected = ContentHash::of_value(&digest_execution(self))
            .map_err(|error| AdaptiveInformationCampaignError::Digest(error.to_string()))?;
        if expected != self.execution_digest {
            return Err(AdaptiveInformationCampaignError::InvalidOutput(
                "adaptive campaign execution digest is not bound to its content".into(),
            ));
        }
        Ok(())
    }
}

fn posterior_from_mechanisms(mechanisms: &[DesignMechanism]) -> Vec<AdaptiveMechanismPosterior> {
    let mut ordered = mechanisms.to_vec();
    ordered.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    ordered
        .into_iter()
        .map(|mechanism| AdaptiveMechanismPosterior {
            mechanism_id: mechanism.mechanism_id,
            prior_milli: mechanism.prior_milli,
            posterior_milli: mechanism.prior_milli,
            observations_used: 0,
        })
        .collect()
}

fn posterior_as_mechanisms(posterior: &[AdaptiveMechanismPosterior]) -> Vec<DesignMechanism> {
    posterior
        .iter()
        .map(|entry| DesignMechanism {
            mechanism_id: entry.mechanism_id.clone(),
            prior_milli: entry.posterior_milli,
        })
        .collect()
}

fn posterior_concentration(posterior: &[AdaptiveMechanismPosterior]) -> u16 {
    posterior
        .iter()
        .map(|entry| entry.posterior_milli)
        .max()
        .unwrap_or(0)
}

/// Replan the next bounded assay batch from typed categorical observations.
pub fn plan_glioma_adaptive_information_campaign(
    request: &AdaptiveInformationCampaignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
    observations: &[AdaptiveInformationObservation],
) -> Result<AdaptiveInformationCampaignPlan, AdaptiveInformationCampaignError> {
    validate_inputs(request, mechanisms, actions, observations)?;
    let map = action_map(actions);
    let mut posterior = posterior_from_mechanisms(mechanisms);
    let mut observed_counts = BTreeMap::<String, u16>::new();
    let mut spent = 0_u64;
    let mut ordered_observations = observations.to_vec();
    ordered_observations.sort_by(|left, right| {
        left.action_id
            .cmp(&right.action_id)
            .then_with(|| left.replicate_index.cmp(&right.replicate_index))
    });
    for observation in &ordered_observations {
        let action = map.get(&observation.action_id).expect("validated action");
        let expected_replicate = observed_counts
            .get(&observation.action_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        if observation.replicate_index != expected_replicate
            || observation.replicate_index > action.max_replicates
        {
            return Err(AdaptiveInformationCampaignError::InvalidInput(format!(
                "action {} replicate indexes must be contiguous and within max_replicates",
                observation.action_id
            )));
        }
        spent = spent.saturating_add(u64::from(action.cost_units));
        if spent > request.budget_units {
            return Err(AdaptiveInformationCampaignError::InvalidInput(
                "initial observations exceed the declared campaign budget".into(),
            ));
        }
        apply_observation(&mut posterior, &map, observation)?;
        observed_counts.insert(observation.action_id.clone(), observation.replicate_index);
    }
    let mut eligible_actions = Vec::new();
    let mut exhausted_order = Vec::new();
    for action in actions {
        let used = observed_counts.get(&action.action_id).copied().unwrap_or(0);
        if used >= action.max_replicates {
            exhausted_order.push(action.action_id.clone());
        } else {
            let mut candidate = action.clone();
            // One action call corresponds to one returned categorical outcome. Replanning after
            // every observation keeps the posterior and replicate budget aligned.
            candidate.max_replicates = 1;
            eligible_actions.push(candidate);
        }
    }
    eligible_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    exhausted_order.sort();
    let action_order = eligible_actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let observed_order = observed_counts.keys().cloned().collect::<Vec<_>>();
    let observed_outcome_order = ordered_observations
        .iter()
        .map(|observation| {
            format!(
                "{}:{}:{}",
                observation.action_id, observation.replicate_index, observation.outcome_id
            )
        })
        .collect::<Vec<_>>();
    let observed_artifact_digest_order = ordered_observations
        .iter()
        .map(|observation| observation.artifact.content_hash.clone())
        .collect::<Vec<_>>();
    let mechanism_order = posterior
        .iter()
        .map(|entry| entry.mechanism_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let concentration = posterior_concentration(&posterior);
    let (next_action_order, mut deferred_order, scores, design_disposition) =
        if concentration >= request.stop_concentration_milli {
            (
                Vec::new(),
                action_order.clone(),
                Vec::new(),
                InformationDesignDisposition::NoInformativeActions,
            )
        } else if eligible_actions.is_empty() {
            negative_evidence.insert("all-declared-assays-reached-replicate-ceilings".into());
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                InformationDesignDisposition::NoInformativeActions,
            )
        } else if request.budget_units.saturating_sub(spent) == 0 {
            uncertainty.insert("budget-exhausted-before-next-assay-batch".into());
            (
                Vec::new(),
                action_order.clone(),
                Vec::new(),
                InformationDesignDisposition::BudgetBlocked,
            )
        } else {
            let design_request = InformationDesignRequest {
                objective: request.objective.clone(),
                model_system: request.model_system,
                budget_units: request.budget_units.saturating_sub(spent),
                max_selected_actions: request.max_actions_per_round,
                min_information_gain_milli: request.min_information_gain_milli,
                information_weight_milli: request.information_weight_milli,
                feasibility_weight_milli: request.feasibility_weight_milli,
                risk_penalty_milli: request.risk_penalty_milli,
                cost_penalty_milli: request.cost_penalty_milli,
                risk_ceiling_milli: request.risk_ceiling_milli,
            };
            let design = plan_glioma_information_design(
                &design_request,
                &posterior_as_mechanisms(&posterior),
                &eligible_actions,
            )
            .map_err(|error| {
                AdaptiveInformationCampaignError::InvalidInput(format!(
                    "adaptive information-design planning failed: {error}"
                ))
            })?;
            (
                design.selected_order.clone(),
                design.deferred_order.clone(),
                design.scores.clone(),
                design.disposition,
            )
        };
    deferred_order.extend(exhausted_order.iter().cloned());
    deferred_order.sort();
    if concentration < request.stop_concentration_milli && next_action_order.is_empty() {
        match design_disposition {
            InformationDesignDisposition::BudgetBlocked => {
                uncertainty.insert("no-affordable-information-bearing-assay".into());
            }
            InformationDesignDisposition::NoInformativeActions => {
                negative_evidence.insert("no-assay-clears-information-and-risk-gates".into());
            }
            InformationDesignDisposition::Unresolved => {
                uncertainty.insert("assay-information-state-remains-unresolved".into());
            }
            InformationDesignDisposition::Qualified => {}
        }
    }
    let disposition = if concentration >= request.stop_concentration_milli {
        AdaptiveInformationCampaignDisposition::Converged
    } else if !next_action_order.is_empty() {
        AdaptiveInformationCampaignDisposition::Qualified
    } else if !uncertainty.is_empty()
        && design_disposition == InformationDesignDisposition::BudgetBlocked
    {
        AdaptiveInformationCampaignDisposition::BudgetBlocked
    } else if !negative_evidence.is_empty() {
        AdaptiveInformationCampaignDisposition::NoInformativeActions
    } else if observations.is_empty() {
        AdaptiveInformationCampaignDisposition::Unresolved
    } else {
        AdaptiveInformationCampaignDisposition::Partial
    };
    let mut output = AdaptiveInformationCampaignPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        posterior_order: posterior,
        action_order,
        exhausted_order,
        next_action_order,
        deferred_order,
        observed_order,
        observed_outcome_order,
        observed_artifact_digest_order,
        scores,
        budget_units: request.budget_units,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-information-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_plan(&output))
        .map_err(|error| AdaptiveInformationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Execute an adaptive assay campaign through the caller-owned local executor.
pub fn execute_glioma_adaptive_information_campaign<E: GliomaInformationDesignExecutor>(
    request: &AdaptiveInformationCampaignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
    initial_observations: &[AdaptiveInformationObservation],
    executor: &mut E,
) -> Result<AdaptiveInformationCampaignExecution, AdaptiveInformationCampaignError> {
    validate_inputs(request, mechanisms, actions, initial_observations)?;
    let action_map = action_map(actions);
    let mut observations = initial_observations.to_vec();
    let mut rounds = Vec::new();
    for round in 1..=request.max_rounds {
        let plan =
            plan_glioma_adaptive_information_campaign(request, mechanisms, actions, &observations)?;
        if plan.next_action_order.is_empty() {
            break;
        }
        let posterior_before = plan.posterior_order.clone();
        let mut selected = Vec::new();
        let mut observation_order = Vec::new();
        let mut spent = 0_u64;
        for action_id in &plan.next_action_order {
            let action = action_map.get(action_id).ok_or_else(|| {
                AdaptiveInformationCampaignError::InvalidOutput(format!(
                    "planned action {action_id} is absent from the action map"
                ))
            })?;
            let expected_replicate = observations
                .iter()
                .filter(|observation| observation.action_id == action.action_id)
                .count()
                .saturating_add(1) as u16;
            let observation = executor
                .execute_action(action, round)
                .map_err(|failure| AdaptiveInformationCampaignError::Executor(failure.reason))?;
            if observation.action_id != action.action_id
                || observation.replicate_index != expected_replicate
                || observations.iter().any(|prior| {
                    prior.action_id == observation.action_id
                        && prior.replicate_index == observation.replicate_index
                })
            {
                return Err(AdaptiveInformationCampaignError::InvalidOutput(
                    "executor returned an unknown, out-of-order, or duplicate observation".into(),
                ));
            }
            observation.artifact.validate().map_err(|error| {
                AdaptiveInformationCampaignError::InvalidOutput(error.to_string())
            })?;
            if !action
                .outcomes
                .iter()
                .any(|outcome| outcome.outcome_id == observation.outcome_id)
            {
                return Err(AdaptiveInformationCampaignError::InvalidOutput(
                    "executor returned an outcome not declared by the action".into(),
                ));
            }
            spent = spent.saturating_add(u64::from(action.cost_units));
            observations.push(observation.clone());
            selected.push(action.action_id.clone());
            observation_order.push(format!(
                "{}:{}",
                observation.action_id, observation.replicate_index
            ));
        }
        let after =
            plan_glioma_adaptive_information_campaign(request, mechanisms, actions, &observations)?;
        rounds.push(AdaptiveInformationCampaignRound {
            round,
            selected_action_order: selected,
            observation_order,
            budget_spent_units: spent,
            expected_information_milli: plan
                .scores
                .iter()
                .filter(|score| plan.next_action_order.contains(&score.action_id))
                .map(|score| score.expected_information_gain_milli)
                .sum(),
            posterior_before_order: posterior_before,
            posterior_after_order: after.posterior_order,
            planner_digest: plan.digest,
        });
    }
    let final_plan =
        plan_glioma_adaptive_information_campaign(request, mechanisms, actions, &observations)?;
    let mut output = AdaptiveInformationCampaignExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: EXECUTION_OUTPUT_SCHEMA.into(),
        rounds,
        observations,
        final_plan,
        execution_digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-information-execution"),
    };
    output.execution_digest = ContentHash::of_value(&digest_execution(&output))
        .map_err(|error| AdaptiveInformationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn mechanisms() -> Vec<DesignMechanism> {
        vec![
            DesignMechanism {
                mechanism_id: "egfr".into(),
                prior_milli: 500,
            },
            DesignMechanism {
                mechanism_id: "matrix".into(),
                prior_milli: 500,
            },
        ]
    }

    fn action(id: &str, separating: bool) -> DesignAction {
        let (egfr_low, matrix_low) = if separating { (900, 100) } else { (500, 500) };
        let (egfr_high, matrix_high) = if separating { (100, 900) } else { (500, 500) };
        DesignAction {
            action_id: id.into(),
            feature_id: format!("feature-{id}"),
            label: id.into(),
            outcomes: vec![
                super::super::information_design::DesignOutcome {
                    outcome_id: "low".into(),
                    label: "low invasion".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("egfr".into(), egfr_low),
                        ("matrix".into(), matrix_low),
                    ]),
                },
                super::super::information_design::DesignOutcome {
                    outcome_id: "high".into(),
                    label: "high invasion".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("egfr".into(), egfr_high),
                        ("matrix".into(), matrix_high),
                    ]),
                },
            ],
            feasibility_milli: 900,
            risk_milli: 100,
            cost_units: 2,
            max_replicates: 2,
        }
    }

    fn request() -> AdaptiveInformationCampaignRequest {
        AdaptiveInformationCampaignRequest {
            objective: "separate EGFR and matrix invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            max_rounds: 3,
            max_actions_per_round: 1,
            budget_units: 6,
            min_information_gain_milli: 10,
            information_weight_milli: 800,
            feasibility_weight_milli: 200,
            risk_penalty_milli: 100,
            cost_penalty_milli: 0,
            risk_ceiling_milli: 700,
            stop_concentration_milli: 900,
        }
    }

    struct Simulator;

    impl GliomaInformationDesignExecutor for Simulator {
        fn execute_action(
            &mut self,
            action: &DesignAction,
            _round: u16,
        ) -> Result<AdaptiveInformationObservation, AdaptiveInformationExecutionFailure> {
            Ok(AdaptiveInformationObservation {
                action_id: action.action_id.clone(),
                outcome_id: "low".into(),
                replicate_index: 1,
                artifact: artifact(&action.action_id),
            })
        }
    }

    #[test]
    fn planner_selects_information_bearing_assay_and_updates_posterior() {
        let actions = vec![action("uninformative", false), action("separating", true)];
        let plan =
            plan_glioma_adaptive_information_campaign(&request(), &mechanisms(), &actions, &[])
                .unwrap();
        assert_eq!(
            plan.disposition,
            AdaptiveInformationCampaignDisposition::Qualified
        );
        assert_eq!(plan.next_action_order, vec!["separating"]);
        let observation = AdaptiveInformationObservation {
            action_id: "separating".into(),
            outcome_id: "low".into(),
            replicate_index: 1,
            artifact: artifact("separating-1"),
        };
        let updated = plan_glioma_adaptive_information_campaign(
            &request(),
            &mechanisms(),
            &actions,
            &[observation],
        )
        .unwrap();
        assert!(
            updated.posterior_order[0].posterior_milli > updated.posterior_order[1].posterior_milli
        );
        updated.validate().unwrap();
    }

    #[test]
    fn execution_replans_from_returned_outcomes() {
        let actions = vec![action("uninformative", false), action("separating", true)];
        let mut simulator = Simulator;
        let output = execute_glioma_adaptive_information_campaign(
            &request(),
            &mechanisms(),
            &actions,
            &[],
            &mut simulator,
        )
        .unwrap();
        assert!(!output.rounds.is_empty());
        assert_eq!(output.rounds[0].selected_action_order, vec!["separating"]);
        assert_eq!(output.observations.len(), 1);
        assert_eq!(
            output.final_plan.disposition,
            AdaptiveInformationCampaignDisposition::Converged
        );
        output.validate().unwrap();
    }

    #[test]
    fn impossible_outcome_is_rejected_instead_of_collapsing_certainty() {
        let actions = vec![action("separating", true)];
        let impossible = AdaptiveInformationObservation {
            action_id: "separating".into(),
            outcome_id: "unknown".into(),
            replicate_index: 1,
            artifact: artifact("impossible"),
        };
        let error = plan_glioma_adaptive_information_campaign(
            &request(),
            &mechanisms(),
            &actions,
            &[impossible],
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown outcome"));
    }

    #[test]
    fn observation_permutation_replays_identically() {
        let actions = vec![action("uninformative", false), action("separating", true)];
        let first_observation = AdaptiveInformationObservation {
            action_id: "separating".into(),
            outcome_id: "low".into(),
            replicate_index: 1,
            artifact: artifact("separating-1"),
        };
        let second_observation = AdaptiveInformationObservation {
            action_id: "uninformative".into(),
            outcome_id: "low".into(),
            replicate_index: 1,
            artifact: artifact("uninformative-1"),
        };
        let left = plan_glioma_adaptive_information_campaign(
            &request(),
            &mechanisms(),
            &actions,
            &[first_observation.clone(), second_observation.clone()],
        )
        .unwrap();
        let right = plan_glioma_adaptive_information_campaign(
            &request(),
            &mechanisms(),
            &actions,
            &[second_observation, first_observation],
        )
        .unwrap();
        assert_eq!(left, right);
    }
}
