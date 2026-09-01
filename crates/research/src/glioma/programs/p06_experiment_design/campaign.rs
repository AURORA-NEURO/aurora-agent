//! Mechanism-aware closed-loop campaign planning for preclinical glioma research.
//!
//! This is the decision algorithm that turns a competing-mechanism state into an executable
//! sequence of assay batches.  It uses deterministic posterior reweighting from observed assay
//! residuals, expected information from the mechanism prediction spread, and a risk/cost-aware
//! greedy knapsack.  The result is a plan for a caller-owned local executor; it does not dispatch
//! instruments, invent observations, or make a clinical decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaClosedLoopCampaign1@1";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_ROUNDS: u16 = 128;
pub const SCORE_SCALE: u64 = 1_000_000;

/// Bounds for a campaign controller.  All weights are integer milli-units so the same decision
/// is replayable in Rust, Python, TypeScript, and a remote worker without floating point drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedLoopCampaignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_rounds: u16,
    pub max_actions_per_round: usize,
    pub budget_units: u64,
    pub min_information_gain_milli: u64,
    pub information_weight_milli: u16,
    pub effect_weight_milli: u16,
    pub feasibility_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub stop_concentration_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignMechanism {
    pub mechanism_id: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignAction {
    pub action_id: String,
    pub feature_id: String,
    pub label: String,
    pub predicted_milli_by_mechanism: BTreeMap<String, i64>,
    pub measurement_uncertainty_milli: u64,
    pub feasibility_milli: u16,
    pub expected_effect_milli: i32,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub max_replicates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignObservation {
    pub action_id: String,
    pub observed_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_index: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignMechanismPosterior {
    pub mechanism_id: String,
    pub prior_milli: u16,
    pub posterior_milli: u16,
    pub weighted_residual_milli: u64,
    pub observations_used: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignActionScore {
    pub action_id: String,
    pub feature_id: String,
    pub label: String,
    pub expected_information_milli: u64,
    pub expected_effect_milli: i32,
    pub risk_adjusted_utility_milli: u64,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub observed_replicates: u32,
    pub planned_replicates: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignRound {
    pub round: u16,
    pub selected_action_order: Vec<String>,
    pub budget_spent_units: u64,
    pub expected_information_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStopReason {
    PosteriorConverged,
    BudgetExhausted,
    NoInformativeActions,
    MaxRounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedLoopCampaignDisposition {
    Qualified,
    Partial,
    Converged,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedLoopCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub posterior_order: Vec<CampaignMechanismPosterior>,
    pub action_order: Vec<String>,
    pub ranked_actions: Vec<CampaignActionScore>,
    pub rounds: Vec<CampaignRound>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub budget_remaining_units: u64,
    pub stop_reason: CampaignStopReason,
    pub disposition: ClosedLoopCampaignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClosedLoopCampaignError {
    #[error("closed-loop campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("closed-loop campaign input is invalid: {0}")]
    InvalidInput(String),
    #[error("closed-loop campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("closed-loop campaign digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &ClosedLoopCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "posterior_order": output.posterior_order,
        "action_order": output.action_order,
        "ranked_actions": output.ranked_actions,
        "rounds": output.rounds,
        "selected_action_order": output.selected_action_order,
        "deferred_action_order": output.deferred_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "budget_remaining_units": output.budget_remaining_units,
        "stop_reason": output.stop_reason,
        "disposition": output.disposition,
    })
}

impl ClosedLoopCampaign {
    pub fn validate(&self) -> Result<(), ClosedLoopCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .mechanism_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.action_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.ranked_actions.windows(2).any(|pair| {
                pair[0].risk_adjusted_utility_milli < pair[1].risk_adjusted_utility_milli
                    || (pair[0].risk_adjusted_utility_milli == pair[1].risk_adjusted_utility_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self.posterior_order.len() != self.mechanism_order.len()
            || self.posterior_order.iter().any(|posterior| {
                posterior.posterior_milli > 1_000
                    || posterior.prior_milli > 1_000
                    || posterior.mechanism_id.trim().is_empty()
            })
            || self
                .rounds
                .windows(2)
                .any(|pair| pair[0].round >= pair[1].round)
            || self
                .rounds
                .iter()
                .any(|round| round.round == 0 || round.selected_action_order.is_empty())
            || self
                .negative_evidence
                .iter()
                .chain(self.uncertainty.iter())
                .any(|item| item.trim().is_empty())
            || self.digest.as_str().len() != 64
        {
            return Err(ClosedLoopCampaignError::InvalidOutput(
                "identity, canonical ordering, posterior, ranking, round, or digest fields are invalid".into(),
            ));
        }
        let posterior_ids = self
            .posterior_order
            .iter()
            .map(|posterior| posterior.mechanism_id.as_str())
            .collect::<BTreeSet<_>>();
        if posterior_ids.len() != self.posterior_order.len()
            || posterior_ids
                != self
                    .mechanism_order
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
            || self
                .posterior_order
                .iter()
                .map(|posterior| posterior.posterior_milli as u32)
                .sum::<u32>()
                != 1_000
        {
            return Err(ClosedLoopCampaignError::InvalidOutput(
                "mechanism posterior identities or mass do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClosedLoopCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClosedLoopCampaignError::InvalidOutput(
                "campaign digest is not bound to its ranked plan".into(),
            ));
        }
        Ok(())
    }
}

fn validate_inputs(
    request: &ClosedLoopCampaignRequest,
    mechanisms: &[CampaignMechanism],
    actions: &[CampaignAction],
    observations: &[CampaignObservation],
) -> Result<(), ClosedLoopCampaignError> {
    if request.objective.trim().is_empty()
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS
        || request.budget_units == 0
        || request.min_information_gain_milli > SCORE_SCALE
        || request.information_weight_milli > 1_000
        || request.effect_weight_milli > 1_000
        || request.feasibility_weight_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || request.stop_concentration_milli > 1_000
        || request
            .information_weight_milli
            .saturating_add(request.effect_weight_milli)
            .saturating_add(request.feasibility_weight_milli)
            == 0
    {
        return Err(ClosedLoopCampaignError::InvalidRequest(
            "objective, bounded rounds/actions, budget, thresholds, or scoring weights are invalid"
                .into(),
        ));
    }
    if mechanisms.len() < 2 || mechanisms.len() > MAX_MECHANISMS {
        return Err(ClosedLoopCampaignError::InvalidInput(
            "at least two and at most 256 competing mechanisms are required".into(),
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
        return Err(ClosedLoopCampaignError::InvalidInput(
            "mechanism ids must be unique and priors must be positive and sum to 1000".into(),
        ));
    }
    if actions.is_empty() || actions.len() > MAX_ACTIONS {
        return Err(ClosedLoopCampaignError::InvalidInput(
            "at least one and at most 4096 actions are required".into(),
        ));
    }
    let action_ids = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>();
    if action_ids.len() != actions.len()
        || actions.iter().any(|action| {
            action.action_id.trim().is_empty()
                || action.feature_id.trim().is_empty()
                || action.label.trim().is_empty()
                || action.measurement_uncertainty_milli == 0
                || action.feasibility_milli > 1_000
                || action.cost_units == 0
                || action.risk_milli > 1_000
                || action.max_replicates == 0
                || action.predicted_milli_by_mechanism.len() != mechanisms.len()
                || action
                    .predicted_milli_by_mechanism
                    .keys()
                    .any(|mechanism| !mechanism_ids.contains(mechanism))
        })
    {
        return Err(ClosedLoopCampaignError::InvalidInput(
            "action ids, contracts, costs, risks, replicate bounds, and mechanism predictions must be complete".into(),
        ));
    }
    if observations.len() > MAX_OBSERVATIONS {
        return Err(ClosedLoopCampaignError::InvalidInput(
            "observation count exceeds the bounded campaign capacity".into(),
        ));
    }
    let mut observation_keys = BTreeSet::new();
    for observation in observations {
        if !action_ids.contains(&observation.action_id)
            || observation.uncertainty_milli == 0
            || observation.replicate_index == 0
            || !observation_keys
                .insert((observation.action_id.clone(), observation.replicate_index))
        {
            return Err(ClosedLoopCampaignError::InvalidInput(
                "observations must reference known actions with unique positive replicate indexes"
                    .into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| ClosedLoopCampaignError::InvalidInput(error.to_string()))?;
    }
    Ok(())
}

fn posterior_for(
    mechanisms: &[CampaignMechanism],
    actions: &BTreeMap<String, &CampaignAction>,
    observations: &[CampaignObservation],
) -> Vec<CampaignMechanismPosterior> {
    let mut losses = Vec::with_capacity(mechanisms.len());
    for mechanism in mechanisms {
        let mut loss = 0_u64;
        let mut used = 0_u32;
        for observation in observations {
            let Some(action) = actions.get(&observation.action_id) else {
                continue;
            };
            let prediction = action.predicted_milli_by_mechanism[&mechanism.mechanism_id];
            let error = i128::from(prediction) - i128::from(observation.observed_milli);
            let squared = error.unsigned_abs().saturating_mul(error.unsigned_abs());
            let variance = action
                .measurement_uncertainty_milli
                .saturating_add(observation.uncertainty_milli)
                .max(1) as u128;
            let normalized = (squared / variance).min(u128::from(SCORE_SCALE));
            loss = loss.saturating_add(normalized.min(u128::from(u64::MAX)) as u64);
            used = used.saturating_add(1);
        }
        losses.push((loss, used));
    }
    let weights = mechanisms
        .iter()
        .zip(losses.iter())
        .map(|(mechanism, (loss, _))| {
            u128::from(mechanism.prior_milli).saturating_mul(u128::from(SCORE_SCALE))
                / u128::from(loss.saturating_add(1))
        })
        .collect::<Vec<_>>();
    let total = weights.iter().copied().sum::<u128>().max(1);
    let mut posteriors = weights
        .iter()
        .map(|weight| ((*weight * 1_000) / total) as u16)
        .collect::<Vec<_>>();
    let assigned = posteriors
        .iter()
        .map(|value| u32::from(*value))
        .sum::<u32>();
    if assigned < 1_000 {
        let remainder = (1_000 - assigned) as u16;
        let best = weights
            .iter()
            .enumerate()
            .max_by_key(|(index, weight)| (**weight, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
            .unwrap_or(0);
        posteriors[best] = posteriors[best].saturating_add(remainder);
    }
    mechanisms
        .iter()
        .zip(losses)
        .zip(posteriors)
        .map(
            |((mechanism, (loss, used)), posterior)| CampaignMechanismPosterior {
                mechanism_id: mechanism.mechanism_id.clone(),
                prior_milli: mechanism.prior_milli,
                posterior_milli: posterior,
                weighted_residual_milli: loss,
                observations_used: used,
            },
        )
        .collect()
}

fn action_score(
    request: &ClosedLoopCampaignRequest,
    action: &CampaignAction,
    posteriors: &[CampaignMechanismPosterior],
    observed_replicates: u32,
    planned_replicates: u16,
) -> CampaignActionScore {
    let posterior_mass = posteriors
        .iter()
        .map(|posterior| u128::from(posterior.posterior_milli))
        .sum::<u128>()
        .max(1);
    let mean_numerator = posteriors
        .iter()
        .map(|posterior| {
            i128::from(posterior.posterior_milli).saturating_mul(i128::from(
                action.predicted_milli_by_mechanism[&posterior.mechanism_id],
            ))
        })
        .sum::<i128>();
    let mean = mean_numerator / i128::try_from(posterior_mass).unwrap_or(i128::MAX);
    let variance = posteriors
        .iter()
        .map(|posterior| {
            let prediction =
                i128::from(action.predicted_milli_by_mechanism[&posterior.mechanism_id]);
            let delta = prediction.abs_diff(mean);
            u128::from(posterior.posterior_milli).saturating_mul(delta.saturating_mul(delta))
        })
        .sum::<u128>()
        / posterior_mass;
    let uncertainty_sq = u128::from(action.measurement_uncertainty_milli)
        .saturating_mul(u128::from(action.measurement_uncertainty_milli));
    let expected_information = (variance.saturating_mul(u128::from(SCORE_SCALE))
        / variance.saturating_add(uncertainty_sq).max(1))
    .min(u128::from(SCORE_SCALE)) as u64;
    let effect = action.expected_effect_milli;
    let positive_effect = effect.max(0) as u64;
    let weighted = u128::from(request.information_weight_milli)
        .saturating_mul(u128::from(expected_information))
        .saturating_add(
            u128::from(request.effect_weight_milli)
                .saturating_mul(u128::from(positive_effect.min(SCORE_SCALE as i32 as u64))),
        )
        .saturating_add(
            u128::from(request.feasibility_weight_milli)
                .saturating_mul(u128::from(action.feasibility_milli)),
        )
        / 1_000;
    let risk_penalty = u128::from(request.risk_penalty_milli)
        .saturating_mul(u128::from(action.risk_milli))
        / 1_000;
    let utility = weighted
        .saturating_sub(risk_penalty)
        .min(u128::from(SCORE_SCALE)) as u64;
    let rationale = if expected_information < request.min_information_gain_milli {
        "below the declared information gate; retain as a deferred or replication option"
    } else if action.risk_milli > action.feasibility_milli {
        "information-bearing but risk dominates feasibility; require explicit local approval"
    } else if observed_replicates > 0 {
        "revisit an observed action only when its posterior separation remains informative"
    } else {
        "unobserved, information-bearing action ranked by posterior separation and bounded cost"
    };
    CampaignActionScore {
        action_id: action.action_id.clone(),
        feature_id: action.feature_id.clone(),
        label: action.label.clone(),
        expected_information_milli: expected_information,
        expected_effect_milli: effect,
        risk_adjusted_utility_milli: utility,
        feasibility_milli: action.feasibility_milli,
        risk_milli: action.risk_milli,
        cost_units: action.cost_units,
        observed_replicates,
        planned_replicates,
        rationale: rationale.into(),
    }
}

/// Select a bounded sequence of mechanism-discriminating assay batches.
pub fn plan_glioma_closed_loop_campaign(
    request: &ClosedLoopCampaignRequest,
    mechanisms: &[CampaignMechanism],
    actions: &[CampaignAction],
    observations: &[CampaignObservation],
) -> Result<ClosedLoopCampaign, ClosedLoopCampaignError> {
    validate_inputs(request, mechanisms, actions, observations)?;
    let mut sorted_mechanisms = mechanisms.to_vec();
    sorted_mechanisms.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mut sorted_actions = actions.to_vec();
    sorted_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_map = sorted_actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let posteriors = posterior_for(&sorted_mechanisms, &action_map, observations);
    let concentration = posteriors
        .iter()
        .map(|posterior| posterior.posterior_milli)
        .max()
        .unwrap_or(0);
    let mut observed_counts = BTreeMap::<String, u32>::new();
    let mut negative_evidence = observations
        .iter()
        .filter(|observation| observation.observed_milli <= 0)
        .map(|observation| {
            format!(
                "negative-observation:{}:replicate-{}",
                observation.action_id, observation.replicate_index
            )
        })
        .collect::<Vec<_>>();
    for observation in observations {
        *observed_counts
            .entry(observation.action_id.clone())
            .or_default() += 1;
    }
    negative_evidence.sort();
    let mut planned_counts = BTreeMap::<String, u16>::new();
    let mut remaining_budget = request.budget_units;
    let mut rounds = Vec::new();
    let mut selected_action_order = Vec::new();
    let mut stop_reason = if concentration >= request.stop_concentration_milli {
        CampaignStopReason::PosteriorConverged
    } else {
        CampaignStopReason::MaxRounds
    };
    if concentration < request.stop_concentration_milli {
        for round in 1..=request.max_rounds {
            let mut candidates = sorted_actions
                .iter()
                .filter_map(|action| {
                    let observed = observed_counts.get(&action.action_id).copied().unwrap_or(0);
                    let planned = planned_counts.get(&action.action_id).copied().unwrap_or(0);
                    if observed.saturating_add(u32::from(planned))
                        >= u32::from(action.max_replicates)
                        || action.cost_units as u64 > remaining_budget
                    {
                        return None;
                    }
                    Some(action_score(
                        request,
                        action,
                        &posteriors,
                        observed,
                        planned.saturating_add(1),
                    ))
                })
                .filter(|score| {
                    score.expected_information_milli >= request.min_information_gain_milli
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                right
                    .risk_adjusted_utility_milli
                    .cmp(&left.risk_adjusted_utility_milli)
                    .then_with(|| left.action_id.cmp(&right.action_id))
            });
            let mut chosen = Vec::new();
            let mut round_spend = 0_u64;
            let mut round_information = 0_u64;
            for candidate in candidates {
                if chosen.len() >= request.max_actions_per_round
                    || round_spend.saturating_add(u64::from(candidate.cost_units))
                        > remaining_budget
                {
                    continue;
                }
                round_spend = round_spend.saturating_add(u64::from(candidate.cost_units));
                round_information =
                    round_information.saturating_add(candidate.expected_information_milli);
                *planned_counts
                    .entry(candidate.action_id.clone())
                    .or_default() += 1;
                selected_action_order.push(candidate.action_id.clone());
                chosen.push(candidate.action_id);
            }
            if chosen.is_empty() {
                let has_informative = sorted_actions.iter().any(|action| {
                    let observed = observed_counts.get(&action.action_id).copied().unwrap_or(0);
                    let planned = planned_counts.get(&action.action_id).copied().unwrap_or(0);
                    observed.saturating_add(u32::from(planned)) < u32::from(action.max_replicates)
                        && action_score(
                            request,
                            action,
                            &posteriors,
                            observed,
                            planned.saturating_add(1),
                        )
                        .expected_information_milli
                            >= request.min_information_gain_milli
                });
                stop_reason = if has_informative {
                    CampaignStopReason::BudgetExhausted
                } else {
                    CampaignStopReason::NoInformativeActions
                };
                break;
            }
            remaining_budget = remaining_budget.saturating_sub(round_spend);
            rounds.push(CampaignRound {
                round,
                selected_action_order: chosen,
                budget_spent_units: round_spend,
                expected_information_milli: round_information,
                rationale: "execute the selected batch locally, append observations, then replan from the resulting posterior".into(),
            });
            if remaining_budget == 0 {
                let remaining_informative = sorted_actions.iter().any(|action| {
                    let observed = observed_counts.get(&action.action_id).copied().unwrap_or(0);
                    let planned = planned_counts.get(&action.action_id).copied().unwrap_or(0);
                    observed.saturating_add(u32::from(planned)) < u32::from(action.max_replicates)
                        && action_score(
                            request,
                            action,
                            &posteriors,
                            observed,
                            planned.saturating_add(1),
                        )
                        .expected_information_milli
                            >= request.min_information_gain_milli
                });
                stop_reason = if remaining_informative {
                    CampaignStopReason::BudgetExhausted
                } else {
                    CampaignStopReason::NoInformativeActions
                };
                break;
            }
        }
    }
    let mut ranked_actions = sorted_actions
        .iter()
        .map(|action| {
            action_score(
                request,
                action,
                &posteriors,
                observed_counts.get(&action.action_id).copied().unwrap_or(0),
                planned_counts.get(&action.action_id).copied().unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    ranked_actions.sort_by(|left, right| {
        right
            .risk_adjusted_utility_milli
            .cmp(&left.risk_adjusted_utility_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let selected = selected_action_order.iter().collect::<BTreeSet<_>>();
    let deferred_action_order = sorted_actions
        .iter()
        .filter(|action| !selected.contains(&action.action_id))
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    if observations.is_empty() {
        uncertainty.push(
            "no observations supplied; posterior is prior-weighted and requires an initial batch"
                .into(),
        );
    }
    if stop_reason == CampaignStopReason::NoInformativeActions {
        uncertainty.push("no remaining action clears the declared information gate".into());
    }
    if stop_reason == CampaignStopReason::BudgetExhausted {
        uncertainty.push(
            "budget terminated the campaign before every informative action could be scheduled"
                .into(),
        );
    }
    let disposition = if stop_reason == CampaignStopReason::PosteriorConverged {
        ClosedLoopCampaignDisposition::Converged
    } else if rounds.is_empty() {
        ClosedLoopCampaignDisposition::Unresolved
    } else if stop_reason == CampaignStopReason::BudgetExhausted {
        ClosedLoopCampaignDisposition::BudgetBlocked
    } else if ranked_actions
        .iter()
        .any(|score| score.expected_information_milli >= request.min_information_gain_milli)
    {
        ClosedLoopCampaignDisposition::Qualified
    } else {
        ClosedLoopCampaignDisposition::Partial
    };
    let mut output = ClosedLoopCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order: sorted_mechanisms
            .iter()
            .map(|mechanism| mechanism.mechanism_id.clone())
            .collect(),
        posterior_order: posteriors,
        action_order: sorted_actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect(),
        ranked_actions,
        rounds,
        selected_action_order,
        deferred_action_order,
        negative_evidence,
        uncertainty,
        budget_remaining_units: remaining_budget,
        stop_reason,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-closed-loop-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClosedLoopCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::ContentHash;

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

    fn request() -> ClosedLoopCampaignRequest {
        ClosedLoopCampaignRequest {
            objective: "discriminate invasion mechanisms in glioma organoids".into(),
            model_system: GliomaModelSystem::Organoid,
            max_rounds: 3,
            max_actions_per_round: 1,
            budget_units: 8,
            min_information_gain_milli: 100,
            information_weight_milli: 700,
            effect_weight_milli: 100,
            feasibility_weight_milli: 200,
            risk_penalty_milli: 200,
            stop_concentration_milli: 900,
        }
    }

    fn mechanisms() -> Vec<CampaignMechanism> {
        vec![
            CampaignMechanism {
                mechanism_id: "integrin".into(),
                prior_milli: 500,
            },
            CampaignMechanism {
                mechanism_id: "hypoxia".into(),
                prior_milli: 500,
            },
        ]
    }

    fn actions() -> Vec<CampaignAction> {
        vec![
            CampaignAction {
                action_id: "assay-invasion".into(),
                feature_id: "invasion-score".into(),
                label: "organoid invasion assay".into(),
                predicted_milli_by_mechanism: BTreeMap::from([
                    ("integrin".into(), 800),
                    ("hypoxia".into(), 100),
                ]),
                measurement_uncertainty_milli: 50,
                feasibility_milli: 900,
                expected_effect_milli: 600,
                cost_units: 4,
                risk_milli: 100,
                max_replicates: 1,
            },
            CampaignAction {
                action_id: "assay-oxygen".into(),
                feature_id: "oxygen-response".into(),
                label: "oxygen response assay".into(),
                predicted_milli_by_mechanism: BTreeMap::from([
                    ("integrin".into(), 400),
                    ("hypoxia".into(), 700),
                ]),
                measurement_uncertainty_milli: 50,
                feasibility_milli: 800,
                expected_effect_milli: 300,
                cost_units: 4,
                risk_milli: 100,
                max_replicates: 1,
            },
        ]
    }

    #[test]
    fn controller_selects_informative_batch_and_is_digest_bound() {
        let mut ample = request();
        ample.budget_units = 12;
        let output =
            plan_glioma_closed_loop_campaign(&ample, &mechanisms(), &actions(), &[]).unwrap();
        assert_eq!(output.disposition, ClosedLoopCampaignDisposition::Qualified);
        assert_eq!(output.rounds.len(), 2);
        assert_eq!(
            output.selected_action_order,
            vec!["assay-invasion", "assay-oxygen"]
        );
        output.validate().unwrap();
    }

    #[test]
    fn observations_reweight_posterior_and_preserve_negative_evidence() {
        let observations = vec![CampaignObservation {
            action_id: "assay-invasion".into(),
            observed_milli: -100,
            uncertainty_milli: 20,
            replicate_index: 1,
            artifact: artifact("obs-1"),
        }];
        let output =
            plan_glioma_closed_loop_campaign(&request(), &mechanisms(), &actions(), &observations)
                .unwrap();
        assert_ne!(
            output.posterior_order[0].posterior_milli,
            output.posterior_order[1].posterior_milli
        );
        assert_eq!(
            output.negative_evidence,
            vec!["negative-observation:assay-invasion:replicate-1"]
        );
    }

    #[test]
    fn budget_and_information_gates_stop_honestly() {
        let mut constrained = request();
        constrained.budget_units = 1;
        let output =
            plan_glioma_closed_loop_campaign(&constrained, &mechanisms(), &actions(), &[]).unwrap();
        assert_eq!(
            output.disposition,
            ClosedLoopCampaignDisposition::Unresolved
        );
        assert_eq!(output.stop_reason, CampaignStopReason::BudgetExhausted);
        let mut gated = request();
        gated.min_information_gain_milli = 1_000_000;
        let output =
            plan_glioma_closed_loop_campaign(&gated, &mechanisms(), &actions(), &[]).unwrap();
        assert_eq!(output.stop_reason, CampaignStopReason::NoInformativeActions);
        assert!(output.selected_action_order.is_empty());
    }

    #[test]
    fn input_permutation_replays_identically() {
        let first =
            plan_glioma_closed_loop_campaign(&request(), &mechanisms(), &actions(), &[]).unwrap();
        let mut reverse_actions = actions();
        reverse_actions.reverse();
        let mut reverse_mechanisms = mechanisms();
        reverse_mechanisms.reverse();
        let second = plan_glioma_closed_loop_campaign(
            &request(),
            &reverse_mechanisms,
            &reverse_actions,
            &[],
        )
        .unwrap();
        assert_eq!(first, second);
    }
}
