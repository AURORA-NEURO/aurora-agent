//! Observation-driven adaptive mechanism policies for preclinical glioma research.
//!
//! A robust intervention portfolio can rank candidates, but a research engine also needs to
//! decide what to measure next after the first result changes the mechanism posterior.  This
//! module implements a bounded, deterministic policy loop.  It combines integer likelihood
//! updates, Gini information gain over predicted outcome buckets, lower-tail effect robustness,
//! and a beam-selected finite-horizon assay sequence.  A caller-owned executor supplies local
//! assay or analysis results; the sandbox executor is synthetic and never represents biological
//! evidence.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismAdaptivePolicy1@1";
pub const CAMPAIGN_OUTPUT_SCHEMA: &str = "GliomaMechanismAdaptiveCampaign1@1";
pub const MAX_MODELS: usize = 64;
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_HORIZON: u8 = 16;
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;
const SCALE: u64 = 1_000;
const LIKELIHOOD_SCALE: u64 = 1_000_000;
const BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismModel {
    pub model_id: String,
    pub label: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPrediction {
    pub model_id: String,
    pub outcome_milli: i64,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismAction {
    pub action_id: String,
    pub label: String,
    pub target_node_id: String,
    pub modality: GliomaModality,
    pub redundancy_group: String,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub feasibility_milli: u16,
    pub predictions: Vec<AdaptiveMechanismPrediction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismObservation {
    pub observation_id: String,
    pub action_id: String,
    pub outcome_milli: i64,
    pub uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPolicyRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub horizon: u8,
    pub budget_units: u64,
    pub max_actions: usize,
    pub outcome_bucket_width_milli: u64,
    pub information_weight_milli: u16,
    pub effect_weight_milli: u16,
    pub robustness_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
    pub redundancy_penalty_milli: u16,
    pub min_feasibility_milli: u16,
    pub stop_entropy_milli: u16,
    pub models: Vec<AdaptiveMechanismModel>,
    pub actions: Vec<AdaptiveMechanismAction>,
    pub observations: Vec<AdaptiveMechanismObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPolicyPosterior {
    pub model_id: String,
    pub posterior_milli: u16,
    pub likelihood_product: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismActionScore {
    pub action_id: String,
    pub target_node_id: String,
    pub label: String,
    pub information_gain_milli: u16,
    pub expected_effect_milli: i64,
    pub lower_tail_effect_milli: i64,
    pub worst_case_effect_milli: i64,
    pub expected_utility_milli: i64,
    pub feasibility_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPolicyStep {
    pub step: u8,
    pub action_id: String,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub posterior_before: Vec<AdaptiveMechanismPolicyPosterior>,
    pub information_gain_milli: u16,
    pub predicted_outcome_bucket_order: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveMechanismPolicyDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    NoEligibleActions,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismPolicy {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub model_order: Vec<String>,
    pub action_order: Vec<String>,
    pub posterior: Vec<AdaptiveMechanismPolicyPosterior>,
    pub entropy_milli: u16,
    pub ranked_action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub steps: Vec<AdaptiveMechanismPolicyStep>,
    pub scores: Vec<AdaptiveMechanismActionScore>,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptiveMechanismPolicyDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveMechanismExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

pub trait AdaptiveMechanismPolicyExecutor {
    fn execute_action(
        &mut self,
        action: &AdaptiveMechanismAction,
        attempt: u8,
    ) -> Result<AdaptiveMechanismObservation, AdaptiveMechanismExecutionFailure>;
}

#[derive(Debug, Default)]
pub struct DryRunAdaptiveMechanismPolicyExecutor;

impl AdaptiveMechanismPolicyExecutor for DryRunAdaptiveMechanismPolicyExecutor {
    fn execute_action(
        &mut self,
        action: &AdaptiveMechanismAction,
        attempt: u8,
    ) -> Result<AdaptiveMechanismObservation, AdaptiveMechanismExecutionFailure> {
        let total_prior = action.predictions.len().max(1) as i64;
        let expected = action
            .predictions
            .iter()
            .map(|prediction| prediction.outcome_milli)
            .sum::<i64>()
            .saturating_div(total_prior);
        let observation_id = format!(
            "dry-run:mechanism-policy:{}:attempt-{attempt}",
            action.action_id
        );
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "observation_id": observation_id,
            "action_id": action.action_id,
            "outcome_milli": expected,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| AdaptiveMechanismExecutionFailure {
            reason: format!("dry-run mechanism observation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(AdaptiveMechanismObservation {
            observation_id,
            action_id: action.action_id.clone(),
            outcome_milli: expected,
            uncertainty_milli: 250,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-mechanism-policy:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.mechanism-policy-observation+json"
                    .into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismCampaignRequest {
    pub policy: AdaptiveMechanismPolicyRequest,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_converged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismCampaignRound {
    pub round: u16,
    pub policy: AdaptiveMechanismPolicy,
    pub action_id: String,
    pub observation_id: Option<String>,
    pub retry_count: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveMechanismCampaignStopReason {
    Converged,
    BudgetExhausted,
    NoEligibleActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveMechanismCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<AdaptiveMechanismCampaignRound>,
    pub observations: Vec<AdaptiveMechanismObservation>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_policy: Option<AdaptiveMechanismPolicy>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptiveMechanismPolicyDisposition,
    pub stop_reason: AdaptiveMechanismCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveMechanismPolicyError {
    #[error("adaptive mechanism policy request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive mechanism policy input is invalid: {0}")]
    InvalidInput(String),
    #[error("adaptive mechanism policy execution failed: {0}")]
    Execution(String),
    #[error("adaptive mechanism policy output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive mechanism policy digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_sorted(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty()) && canonical(values)
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right))
        .unsigned_abs()
        .min(u128::from(u64::MAX)) as u64
}

fn normalize_masses(masses: &BTreeMap<String, u128>) -> Vec<(String, u16)> {
    let total = masses.values().copied().sum::<u128>().max(1);
    let mut rows = masses
        .iter()
        .map(|(id, mass)| {
            let scaled = mass.saturating_mul(u128::from(SCALE));
            let floor = (scaled / total).min(u128::from(SCALE)) as u16;
            let remainder = scaled % total;
            (id.clone(), floor, remainder)
        })
        .collect::<Vec<_>>();
    let assigned = rows
        .iter()
        .map(|(_, floor, _)| u64::from(*floor))
        .sum::<u64>();
    let mut remaining = SCALE.saturating_sub(assigned);
    rows.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));
    for row in &mut rows {
        if remaining == 0 {
            break;
        }
        row.1 = row.1.saturating_add(1);
        remaining -= 1;
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    rows.into_iter().map(|(id, mass, _)| (id, mass)).collect()
}

fn gini_milli(masses: impl Iterator<Item = u16>) -> u16 {
    let squared = masses
        .map(|mass| u64::from(mass).saturating_mul(u64::from(mass)))
        .sum::<u64>()
        .saturating_div(SCALE);
    SCALE.saturating_sub(squared).min(SCALE) as u16
}

fn entropy_like_milli(posterior: &[AdaptiveMechanismPolicyPosterior]) -> u16 {
    gini_milli(posterior.iter().map(|row| row.posterior_milli))
}

fn digest_input(policy: &AdaptiveMechanismPolicy) -> serde_json::Value {
    serde_json::json!({
        "feature_id": policy.feature_id,
        "output_schema": policy.output_schema,
        "objective": policy.objective,
        "model_system": policy.model_system,
        "model_order": policy.model_order,
        "action_order": policy.action_order,
        "posterior": policy.posterior,
        "entropy_milli": policy.entropy_milli,
        "ranked_action_order": policy.ranked_action_order,
        "selected_action_order": policy.selected_action_order,
        "steps": policy.steps,
        "scores": policy.scores,
        "total_cost_units": policy.total_cost_units,
        "budget_remaining_units": policy.budget_remaining_units,
        "uncertainty": policy.uncertainty,
        "negative_evidence": policy.negative_evidence,
        "disposition": policy.disposition,
    })
}

fn campaign_digest_input(campaign: &AdaptiveMechanismCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_policy": campaign.final_policy,
        "uncertainty": campaign.uncertainty,
        "negative_evidence": campaign.negative_evidence,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn model_map(
    request: &AdaptiveMechanismPolicyRequest,
) -> BTreeMap<String, &AdaptiveMechanismModel> {
    request
        .models
        .iter()
        .map(|model| (model.model_id.clone(), model))
        .collect()
}

fn action_map(
    request: &AdaptiveMechanismPolicyRequest,
) -> BTreeMap<String, &AdaptiveMechanismAction> {
    request
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect()
}

fn validate_policy_request(
    request: &AdaptiveMechanismPolicyRequest,
) -> Result<(), AdaptiveMechanismPolicyError> {
    if request.objective.trim().is_empty()
        || request.horizon == 0
        || request.horizon > MAX_HORIZON
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.outcome_bucket_width_milli == 0
        || request.models.is_empty()
        || request.models.len() > MAX_MODELS
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.min_feasibility_milli > 1_000
        || request.stop_entropy_milli > 1_000
    {
        return Err(AdaptiveMechanismPolicyError::InvalidRequest(
            "objective, bounded horizon/budget, model/action registry, bucket width, and score bounds are required".into(),
        ));
    }
    let models = model_map(request);
    if models.len() != request.models.len()
        || request.models.iter().any(|model| {
            model.model_id.trim().is_empty()
                || model.label.trim().is_empty()
                || model.prior_milli == 0
        })
    {
        return Err(AdaptiveMechanismPolicyError::InvalidInput(
            "model ids must be unique and priors/labels must be positive".into(),
        ));
    }
    let actions = action_map(request);
    if actions.len() != request.actions.len()
        || request.actions.iter().any(|action| {
            action.action_id.trim().is_empty()
                || action.label.trim().is_empty()
                || action.target_node_id.trim().is_empty()
                || action.redundancy_group.trim().is_empty()
                || action.cost_units == 0
                || action.risk_milli > 1_000
                || action.feasibility_milli > 1_000
                || action.predictions.len() != models.len()
                || action.predictions.iter().any(|prediction| {
                    prediction.uncertainty_milli == 0 || !models.contains_key(&prediction.model_id)
                })
        })
    {
        return Err(AdaptiveMechanismPolicyError::InvalidInput(
            "actions need unique ids, positive costs, bounded risk/feasibility, and one uncertain prediction per model".into(),
        ));
    }
    if request.actions.iter().any(|action| {
        let mut ids = action
            .predictions
            .iter()
            .map(|prediction| prediction.model_id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.windows(2).any(|pair| pair[0] == pair[1])
            || ids.len() != models.len()
            || models
                .keys()
                .any(|model_id| !ids.contains(&model_id.as_str()))
    }) {
        return Err(AdaptiveMechanismPolicyError::InvalidInput(
            "each action must cover every model exactly once".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut observed_actions = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || !observation_ids.insert(observation.observation_id.clone())
            || !actions.contains_key(&observation.action_id)
            || observation.uncertainty_milli == 0
            || observation.artifact.validate().is_err()
        {
            return Err(AdaptiveMechanismPolicyError::InvalidInput(
                "observations must be unique, reference known actions, carry uncertainty, and be local de-identified artifacts".into(),
            ));
        }
        observed_actions.insert(observation.action_id.clone());
    }
    if request.observations.len() > MAX_OBSERVATIONS {
        return Err(AdaptiveMechanismPolicyError::InvalidRequest(
            "observation count exceeds the deterministic bound".into(),
        ));
    }
    if observed_actions.len() > request.actions.len() {
        return Err(AdaptiveMechanismPolicyError::InvalidInput(
            "observation action partition is inconsistent".into(),
        ));
    }
    Ok(())
}

fn posterior_from_observations(
    request: &AdaptiveMechanismPolicyRequest,
) -> Result<(Vec<AdaptiveMechanismPolicyPosterior>, Vec<String>), AdaptiveMechanismPolicyError> {
    let actions = action_map(request);
    let mut products = request
        .models
        .iter()
        .map(|model| (model.model_id.clone(), u128::from(model.prior_milli)))
        .collect::<BTreeMap<_, _>>();
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let mut uncertainty = Vec::new();
    for observation in observations {
        let action = actions.get(&observation.action_id).ok_or_else(|| {
            AdaptiveMechanismPolicyError::InvalidInput(
                "observation references unknown action".into(),
            )
        })?;
        for prediction in &action.predictions {
            let scale = prediction
                .uncertainty_milli
                .saturating_add(observation.uncertainty_milli)
                .max(1);
            let distance = abs_difference(prediction.outcome_milli, observation.outcome_milli);
            let likelihood = LIKELIHOOD_SCALE
                .saturating_div(1_u64.saturating_add(distance.saturating_div(scale)))
                .max(1);
            let current = products.get(&prediction.model_id).copied().unwrap_or(1);
            products.insert(
                prediction.model_id.clone(),
                current.saturating_mul(u128::from(likelihood)),
            );
        }
        uncertainty.push(format!(
            "observation:{} updated model posterior; this is not causal identification",
            observation.observation_id
        ));
    }
    let masses = normalize_masses(&products);
    let likelihood_products = products;
    let posterior = masses
        .into_iter()
        .map(
            |(model_id, posterior_milli)| AdaptiveMechanismPolicyPosterior {
                likelihood_product: likelihood_products.get(&model_id).copied().unwrap_or(1),
                model_id,
                posterior_milli,
            },
        )
        .collect::<Vec<_>>();
    Ok((posterior, uncertainty))
}

fn prediction_map(
    action: &AdaptiveMechanismAction,
) -> BTreeMap<String, &AdaptiveMechanismPrediction> {
    action
        .predictions
        .iter()
        .map(|prediction| (prediction.model_id.clone(), prediction))
        .collect()
}

fn lower_tail_effect(
    action: &AdaptiveMechanismAction,
    posterior: &[AdaptiveMechanismPolicyPosterior],
) -> i64 {
    let predictions = prediction_map(action);
    let mut rows = posterior
        .iter()
        .filter_map(|row| {
            predictions.get(&row.model_id).map(|prediction| {
                (
                    row.model_id.clone(),
                    row.posterior_milli,
                    prediction.effect_milli,
                )
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));
    let mut remaining = 250_u64;
    let mut mass = 0_u64;
    let mut weighted = 0_i128;
    for (_, posterior_milli, effect) in rows {
        if remaining == 0 {
            break;
        }
        let used = u64::from(posterior_milli).min(remaining);
        weighted = weighted.saturating_add(i128::from(used).saturating_mul(i128::from(effect)));
        mass = mass.saturating_add(used);
        remaining = remaining.saturating_sub(used);
    }
    if mass == 0 {
        0
    } else {
        (weighted / i128::from(mass)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
}

fn score_action(
    action: &AdaptiveMechanismAction,
    posterior: &[AdaptiveMechanismPolicyPosterior],
    request: &AdaptiveMechanismPolicyRequest,
    completed: &BTreeSet<String>,
    used_groups: &BTreeSet<String>,
) -> AdaptiveMechanismActionScore {
    let predictions = prediction_map(action);
    let expected_effect = posterior
        .iter()
        .filter_map(|row| {
            predictions
                .get(&row.model_id)
                .map(|prediction| (row.posterior_milli, prediction.effect_milli))
        })
        .fold(0_i128, |sum, (weight, effect)| {
            sum.saturating_add(i128::from(weight).saturating_mul(i128::from(effect)))
        })
        .saturating_div(i128::from(SCALE))
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    let worst_case_effect = predictions
        .values()
        .map(|prediction| prediction.effect_milli)
        .min()
        .unwrap_or(0);
    let lower_tail = lower_tail_effect(action, posterior);
    let mut bucket_masses = BTreeMap::<i64, BTreeMap<String, u16>>::new();
    for row in posterior {
        if let Some(prediction) = predictions.get(&row.model_id) {
            let bucket = prediction
                .outcome_milli
                .div_euclid(request.outcome_bucket_width_milli as i64);
            bucket_masses
                .entry(bucket)
                .or_default()
                .insert(row.model_id.clone(), row.posterior_milli);
        }
    }
    let prior_gini = gini_milli(posterior.iter().map(|row| row.posterior_milli));
    let expected_post_gini = bucket_masses
        .values()
        .map(|bucket| {
            let bucket_mass = bucket.values().map(|mass| u64::from(*mass)).sum::<u64>();
            if bucket_mass == 0 {
                return 0_u64;
            }
            let conditional = bucket
                .values()
                .map(|mass| ((u64::from(*mass) * SCALE) / bucket_mass).min(SCALE) as u16);
            u64::from(gini_milli(conditional)).saturating_mul(bucket_mass) / SCALE
        })
        .sum::<u64>();
    let information_gain = u64::from(prior_gini)
        .saturating_sub(expected_post_gini)
        .min(SCALE) as u16;
    let redundancy_penalty = if used_groups.contains(&action.redundancy_group) {
        u64::from(request.redundancy_penalty_milli)
    } else {
        0
    };
    let numerator = i128::from(information_gain)
        .saturating_mul(i128::from(request.information_weight_milli))
        .saturating_add(
            i128::from(expected_effect.max(0))
                .saturating_mul(i128::from(request.effect_weight_milli)),
        )
        .saturating_add(
            i128::from(lower_tail.max(0))
                .saturating_mul(i128::from(request.robustness_weight_milli)),
        )
        .saturating_mul(i128::from(action.feasibility_milli))
        .saturating_div(i128::from(SCALE));
    let penalties = i128::from(action.risk_milli)
        .saturating_mul(i128::from(request.risk_penalty_milli))
        .saturating_add(
            i128::from(action.cost_units).saturating_mul(i128::from(request.cost_penalty_milli)),
        )
        .saturating_add(i128::from(redundancy_penalty));
    let utility = numerator
        .saturating_sub(penalties)
        .saturating_div(i128::from(action.cost_units.max(1)));
    let (eligible, exclusion_reason) = if completed.contains(&action.action_id) {
        (false, Some("action-already-observed".into()))
    } else if action.feasibility_milli < request.min_feasibility_milli {
        (false, Some("feasibility-floor-not-met".into()))
    } else {
        (true, None)
    };
    AdaptiveMechanismActionScore {
        action_id: action.action_id.clone(),
        target_node_id: action.target_node_id.clone(),
        label: action.label.clone(),
        information_gain_milli: information_gain,
        expected_effect_milli: expected_effect,
        lower_tail_effect_milli: lower_tail,
        worst_case_effect_milli: worst_case_effect,
        expected_utility_milli: utility.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        feasibility_milli: action.feasibility_milli,
        cost_units: action.cost_units,
        risk_milli: action.risk_milli,
        eligible,
        exclusion_reason,
    }
}

fn beam_select(
    scores: &[AdaptiveMechanismActionScore],
    actions: &BTreeMap<String, &AdaptiveMechanismAction>,
    request: &AdaptiveMechanismPolicyRequest,
) -> Vec<String> {
    #[derive(Clone)]
    struct State {
        order: Vec<String>,
        groups: BTreeSet<String>,
        cost: u64,
        score: i128,
    }
    let mut states = vec![State {
        order: Vec::new(),
        groups: BTreeSet::new(),
        cost: 0,
        score: 0,
    }];
    let eligible = scores
        .iter()
        .filter(|score| score.eligible)
        .collect::<Vec<_>>();
    for score in eligible {
        let Some(action) = actions.get(&score.action_id) else {
            continue;
        };
        let mut expanded = states.clone();
        for state in &states {
            if state.order.len() >= usize::from(request.horizon)
                || state.cost.saturating_add(u64::from(action.cost_units)) > request.budget_units
                || state.order.iter().any(|id| id == &score.action_id)
            {
                continue;
            }
            let diversity = if state.groups.contains(&action.redundancy_group) {
                0_i128
            } else {
                i128::from(request.redundancy_penalty_milli)
            };
            let mut order = state.order.clone();
            order.push(score.action_id.clone());
            let mut groups = state.groups.clone();
            groups.insert(action.redundancy_group.clone());
            expanded.push(State {
                order,
                groups,
                cost: state.cost.saturating_add(u64::from(action.cost_units)),
                score: state
                    .score
                    .saturating_add(i128::from(score.expected_utility_milli))
                    .saturating_add(diversity),
            });
        }
        expanded.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.cost.cmp(&right.cost))
                .then_with(|| left.order.cmp(&right.order))
        });
        expanded.dedup_by(|left, right| left.order == right.order);
        expanded.truncate(BEAM_WIDTH);
        states = expanded;
    }
    states
        .into_iter()
        .max_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then_with(|| right.cost.cmp(&left.cost))
                .then_with(|| right.order.cmp(&left.order))
        })
        .map(|state| state.order)
        .unwrap_or_default()
}

impl AdaptiveMechanismPolicy {
    pub fn validate(&self) -> Result<(), AdaptiveMechanismPolicyError> {
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !unique_sorted(&self.model_order)
            || !unique_sorted(&self.action_order)
            || self.posterior.len() != self.model_order.len()
            || self
                .posterior
                .windows(2)
                .any(|pair| pair[0].model_id >= pair[1].model_id)
            || self
                .posterior
                .iter()
                .map(|row| u32::from(row.posterior_milli))
                .sum::<u32>()
                != 1_000
            || self
                .ranked_action_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self
                .selected_action_order
                .iter()
                .any(|id| !self.action_order.binary_search(id).is_ok())
            || self
                .selected_action_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.steps.len() != self.selected_action_order.len()
            || self.scores.len() != self.action_order.len()
            || score_ids.iter().collect::<BTreeSet<_>>().len() != score_ids.len()
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.entropy_milli > 1_000
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.target_node_id.trim().is_empty()
                    || score.label.trim().is_empty()
                    || score.information_gain_milli > 1_000
                    || score.feasibility_milli > 1_000
                    || score.risk_milli > 1_000
                    || score.cost_units == 0
                    || (score.eligible && score.exclusion_reason.is_some())
                    || (!score.eligible && score.exclusion_reason.is_none())
            })
        {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "identity, posterior, ordering, score, or eligibility invariants are invalid"
                    .into(),
            ));
        }
        let posterior_ids = self
            .posterior
            .iter()
            .map(|row| row.model_id.clone())
            .collect::<Vec<_>>();
        let score_id_set = score_ids.iter().cloned().collect::<BTreeSet<_>>();
        if posterior_ids != self.model_order
            || score_id_set != self.action_order.iter().cloned().collect::<BTreeSet<_>>()
            || self
                .ranked_action_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != score_id_set
            || self
                .selected_action_order
                .iter()
                .any(|id| !score_id_set.contains(id))
            || self.steps.iter().enumerate().any(|(index, step)| {
                step.step != index as u8 + 1
                    || self.selected_action_order.get(index) != Some(&step.action_id)
                    || step.posterior_before.is_empty()
                    || step
                        .predicted_outcome_bucket_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
        {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "policy partitions, ranking, or finite-horizon steps do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveMechanismPolicyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "policy digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a bounded adaptive policy from competing mechanism models and local observations.
pub fn plan_glioma_adaptive_mechanism_policy(
    request: &AdaptiveMechanismPolicyRequest,
) -> Result<AdaptiveMechanismPolicy, AdaptiveMechanismPolicyError> {
    validate_policy_request(request)?;
    let actions = action_map(request);
    let (posterior, mut uncertainty) = posterior_from_observations(request)?;
    let completed = request
        .observations
        .iter()
        .map(|observation| observation.action_id.clone())
        .collect::<BTreeSet<_>>();
    let mut scores = request
        .actions
        .iter()
        .map(|action| score_action(action, &posterior, request, &completed, &BTreeSet::new()))
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| {
        right
            .expected_utility_milli
            .cmp(&left.expected_utility_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let ranked_action_order = scores
        .iter()
        .map(|score| score.action_id.clone())
        .collect::<Vec<_>>();
    let mut selection_scores = scores.clone();
    selection_scores.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let selected_action_order = beam_select(&selection_scores, &actions, request);
    let mut steps = Vec::new();
    let mut spent = 0_u64;
    let mut used_groups = BTreeSet::new();
    for (index, action_id) in selected_action_order.iter().enumerate() {
        let Some(action) = actions.get(action_id) else {
            continue;
        };
        let score = scores.iter().find(|score| score.action_id == *action_id);
        let Some(score) = score else {
            continue;
        };
        let budget_before = request.budget_units.saturating_sub(spent);
        spent = spent.saturating_add(u64::from(action.cost_units));
        let mut buckets = action
            .predictions
            .iter()
            .map(|prediction| {
                prediction
                    .outcome_milli
                    .div_euclid(request.outcome_bucket_width_milli as i64)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        buckets.sort();
        steps.push(AdaptiveMechanismPolicyStep {
            step: index as u8 + 1,
            action_id: action_id.clone(),
            budget_before_units: budget_before,
            budget_after_units: request.budget_units.saturating_sub(spent),
            posterior_before: posterior.clone(),
            information_gain_milli: score.information_gain_milli,
            predicted_outcome_bucket_order: buckets,
        });
        used_groups.insert(action.redundancy_group.clone());
    }
    if posterior
        .iter()
        .filter(|row| row.posterior_milli > 0)
        .count()
        > 1
    {
        uncertainty.push(
            "mechanism posterior remains a model-relative distribution, not causal identification"
                .into(),
        );
    }
    if selected_action_order.is_empty() {
        uncertainty.push(
            "no eligible adaptive action survived budget, feasibility, and completion gates".into(),
        );
    }
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_evidence = scores
        .iter()
        .filter_map(|score| {
            score
                .exclusion_reason
                .as_ref()
                .map(|reason| format!("{}:{reason}", score.action_id))
        })
        .collect::<Vec<_>>();
    for score in &scores {
        if score.worst_case_effect_milli < 0 {
            negative_evidence.push(format!(
                "{}:at-least-one-mechanism-predicts-negative-effect",
                score.action_id
            ));
        }
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    let disposition = if selected_action_order.is_empty() {
        if scores.iter().any(|score| score.eligible) {
            AdaptiveMechanismPolicyDisposition::BudgetBlocked
        } else {
            AdaptiveMechanismPolicyDisposition::NoEligibleActions
        }
    } else if entropy_like_milli(&posterior) <= request.stop_entropy_milli {
        AdaptiveMechanismPolicyDisposition::Qualified
    } else {
        AdaptiveMechanismPolicyDisposition::Partial
    };
    let mut policy = AdaptiveMechanismPolicy {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        model_order: request
            .models
            .iter()
            .map(|model| model.model_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        action_order: request
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        posterior,
        entropy_milli: 0,
        ranked_action_order,
        selected_action_order,
        steps,
        scores,
        total_cost_units: spent,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        uncertainty,
        negative_evidence,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-mechanism-policy"),
    };
    policy.entropy_milli = entropy_like_milli(&policy.posterior);
    policy.digest = ContentHash::of_value(&digest_input(&policy))
        .map_err(|error| AdaptiveMechanismPolicyError::Digest(error.to_string()))?;
    policy.validate()?;
    Ok(policy)
}

impl AdaptiveMechanismCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveMechanismPolicyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != CAMPAIGN_OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !unique_sorted(&self.completed_action_order)
            || !unique_sorted(&self.failed_action_order)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
        {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "campaign identity, action partitions, or evidence ordering is invalid".into(),
            ));
        }
        let mut expected_spent = 0_u64;
        let mut expected_retries = 0_u32;
        let mut seen_observations = BTreeSet::new();
        for round in &self.rounds {
            if round.round == 0
                || round.action_id.trim().is_empty()
                || round.policy.selected_action_order.first() != Some(&round.action_id)
                || round.budget_after_units > round.budget_before_units
                || round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units)
                    == 0
                || round
                    .observation_id
                    .as_ref()
                    .is_some_and(|id| !seen_observations.insert(id.clone()))
            {
                return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                    "campaign round action, budget, and observation invariants are invalid".into(),
                ));
            }
            round.policy.validate()?;
            expected_spent = expected_spent.saturating_add(
                round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units),
            );
            expected_retries = expected_retries.saturating_add(round.retry_count);
        }
        if expected_spent != self.budget_spent_units || expected_retries != self.retry_count {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "campaign spend or retry count does not reconcile with rounds".into(),
            ));
        }
        for observation in &self.observations {
            if observation.observation_id.trim().is_empty()
                || observation.uncertainty_milli == 0
                || observation.artifact.validate().is_err()
            {
                return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                    "campaign observation artifact or uncertainty is invalid".into(),
                ));
            }
        }
        if let Some(policy) = &self.final_policy {
            policy.validate()?;
        }
        let expected = ContentHash::of_value(&campaign_digest_input(self))
            .map_err(|error| AdaptiveMechanismPolicyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveMechanismPolicyError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute one adaptive assay at a time, replanning from the returned observation and stopping
/// only on explicit convergence, budget, worker, or no-progress gates.
pub fn execute_glioma_adaptive_mechanism_campaign<E: AdaptiveMechanismPolicyExecutor>(
    request: &AdaptiveMechanismCampaignRequest,
    executor: &mut E,
) -> Result<AdaptiveMechanismCampaign, AdaptiveMechanismPolicyError> {
    validate_policy_request(&request.policy)?;
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
    {
        return Err(AdaptiveMechanismPolicyError::InvalidRequest(
            "campaign round and retry bounds are invalid".into(),
        ));
    }
    let action_map = action_map(&request.policy);
    let mut policy_request = request.policy.clone();
    let mut rounds = Vec::new();
    let mut observations = policy_request.observations.clone();
    let mut failed = BTreeSet::new();
    let mut completed = policy_request
        .observations
        .iter()
        .map(|observation| observation.action_id.clone())
        .collect::<BTreeSet<_>>();
    let mut retry_count = 0_u32;
    let mut spent = 0_u64;
    let mut final_policy = None;
    let mut stop_reason = AdaptiveMechanismCampaignStopReason::MaxRounds;
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for round_number in 1..=request.max_rounds {
        policy_request.observations = observations.clone();
        let policy = plan_glioma_adaptive_mechanism_policy(&policy_request)?;
        uncertainty.extend(policy.uncertainty.iter().cloned());
        negative_evidence.extend(policy.negative_evidence.iter().cloned());
        if request.stop_on_converged && policy.entropy_milli <= request.policy.stop_entropy_milli {
            final_policy = Some(policy);
            stop_reason = AdaptiveMechanismCampaignStopReason::Converged;
            break;
        }
        let Some(action_id) = policy.selected_action_order.first().cloned() else {
            let budget_blocked =
                policy.disposition == AdaptiveMechanismPolicyDisposition::BudgetBlocked;
            final_policy = Some(policy);
            stop_reason = if budget_blocked {
                AdaptiveMechanismCampaignStopReason::BudgetExhausted
            } else {
                AdaptiveMechanismCampaignStopReason::NoEligibleActions
            };
            break;
        };
        let Some(action) = action_map.get(&action_id) else {
            return Err(AdaptiveMechanismPolicyError::Execution(
                "policy selected an action missing from the registry".into(),
            ));
        };
        let budget_before = policy_request.budget_units.saturating_sub(spent);
        let mut accepted = None;
        let mut round_retries = 0_u32;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_action(action, attempt) {
                Ok(observation) => {
                    if observation.observation_id.trim().is_empty()
                        || observation.action_id != action_id
                        || observation.uncertainty_milli == 0
                        || (request.require_artifacts
                            && observation.artifact.artifact_id.trim().is_empty())
                        || observation.artifact.validate().is_err()
                        || observations.iter().any(|existing| {
                            existing.observation_id == observation.observation_id
                                || existing.action_id == observation.action_id
                        })
                    {
                        return Err(AdaptiveMechanismPolicyError::Execution(
                            "executor returned a duplicate or invalid local observation".into(),
                        ));
                    }
                    accepted = Some(observation);
                    break;
                }
                Err(failure) => {
                    if failure.reason.trim().is_empty() {
                        return Err(AdaptiveMechanismPolicyError::Execution(
                            "executor returned an empty failure reason".into(),
                        ));
                    }
                    if failure.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        round_retries = round_retries.saturating_add(1);
                        continue;
                    }
                    failed.insert(action_id.clone());
                    break;
                }
            }
        }
        spent = spent.saturating_add(u64::from(action.cost_units));
        let budget_after = policy_request.budget_units.saturating_sub(spent);
        let observation_id = accepted
            .as_ref()
            .map(|observation| observation.observation_id.clone());
        if let Some(observation) = accepted {
            completed.insert(action_id.clone());
            observations.push(observation);
        }
        rounds.push(AdaptiveMechanismCampaignRound {
            round: round_number,
            policy,
            action_id: action_id.clone(),
            observation_id,
            retry_count: round_retries,
            budget_before_units: budget_before,
            budget_after_units: budget_after,
        });
        if failed.contains(&action_id) {
            stop_reason = AdaptiveMechanismCampaignStopReason::ExecutorFailed;
            break;
        }
        if budget_after == 0 {
            stop_reason = AdaptiveMechanismCampaignStopReason::BudgetExhausted;
            break;
        }
        if !completed.contains(&action_id) {
            stop_reason = AdaptiveMechanismCampaignStopReason::NoProgress;
            break;
        }
    }
    let disposition = if stop_reason == AdaptiveMechanismCampaignStopReason::Converged {
        AdaptiveMechanismPolicyDisposition::Qualified
    } else if !failed.is_empty() {
        AdaptiveMechanismPolicyDisposition::Partial
    } else if stop_reason == AdaptiveMechanismCampaignStopReason::BudgetExhausted {
        AdaptiveMechanismPolicyDisposition::BudgetBlocked
    } else if rounds.is_empty() {
        AdaptiveMechanismPolicyDisposition::NoEligibleActions
    } else {
        AdaptiveMechanismPolicyDisposition::Partial
    };
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty.sort();
    let mut negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence.sort();
    if final_policy.is_none() {
        policy_request.observations = observations.clone();
        final_policy = Some(plan_glioma_adaptive_mechanism_policy(&policy_request)?);
    }
    let mut campaign = AdaptiveMechanismCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: CAMPAIGN_OUTPUT_SCHEMA.into(),
        objective: request.policy.objective.clone(),
        rounds,
        observations,
        completed_action_order: completed.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units: spent,
        remaining_budget_units: request.policy.budget_units.saturating_sub(spent),
        final_policy,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-mechanism-campaign"),
    };
    campaign.digest = ContentHash::of_value(&campaign_digest_input(&campaign))
        .map_err(|error| AdaptiveMechanismPolicyError::Digest(error.to_string()))?;
    campaign.validate()?;
    Ok(campaign)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, first: i64, second: i64, group: &str) -> AdaptiveMechanismAction {
        AdaptiveMechanismAction {
            action_id: id.into(),
            label: format!("measure {id}"),
            target_node_id: format!("node-{id}"),
            modality: GliomaModality::FunctionalPerturbation,
            redundancy_group: group.into(),
            cost_units: 1,
            risk_milli: 50,
            feasibility_milli: 950,
            predictions: vec![
                AdaptiveMechanismPrediction {
                    model_id: "m1".into(),
                    outcome_milli: first,
                    effect_milli: first,
                    uncertainty_milli: 20,
                },
                AdaptiveMechanismPrediction {
                    model_id: "m2".into(),
                    outcome_milli: second,
                    effect_milli: second,
                    uncertainty_milli: 20,
                },
            ],
        }
    }

    fn request() -> AdaptiveMechanismCampaignRequest {
        AdaptiveMechanismCampaignRequest {
            policy: AdaptiveMechanismPolicyRequest {
                objective: "discriminate invasion mechanisms in organoids".into(),
                model_system: GliomaModelSystem::Organoid,
                horizon: 2,
                budget_units: 2,
                max_actions: 4,
                outcome_bucket_width_milli: 10,
                information_weight_milli: 700,
                effect_weight_milli: 100,
                robustness_weight_milli: 200,
                risk_penalty_milli: 1,
                cost_penalty_milli: 1,
                redundancy_penalty_milli: 20,
                min_feasibility_milli: 500,
                stop_entropy_milli: 50,
                models: vec![
                    AdaptiveMechanismModel {
                        model_id: "m1".into(),
                        label: "invasion-led".into(),
                        prior_milli: 500,
                    },
                    AdaptiveMechanismModel {
                        model_id: "m2".into(),
                        label: "matrix-led".into(),
                        prior_milli: 500,
                    },
                ],
                actions: vec![
                    action("assay-a", 100, 900, "pathway"),
                    action("assay-b", 200, 220, "spatial"),
                ],
                observations: Vec::new(),
            },
            max_rounds: 2,
            max_retries: 1,
            require_artifacts: true,
            stop_on_converged: true,
        }
    }

    #[test]
    fn policy_prefers_discriminating_action_and_is_replay_stable() {
        let request = request();
        let first = plan_glioma_adaptive_mechanism_policy(&request.policy).unwrap();
        let second = plan_glioma_adaptive_mechanism_policy(&request.policy).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.selected_action_order.first(), Some(&"assay-a".into()));
        assert!(first
            .scores
            .iter()
            .any(|score| score.information_gain_milli > 0));
        first.validate().unwrap();
    }

    #[test]
    fn campaign_replans_from_local_observation_without_promoting_synthetic_evidence() {
        let mut executor = DryRunAdaptiveMechanismPolicyExecutor;
        let campaign =
            execute_glioma_adaptive_mechanism_campaign(&request(), &mut executor).unwrap();
        assert!(!campaign.rounds.is_empty());
        assert!(campaign
            .observations
            .iter()
            .all(|observation| observation.artifact.local_only));
        assert!(campaign.completed_action_order.contains(&"assay-a".into()));
        campaign.validate().unwrap();
    }

    #[test]
    fn duplicate_action_observation_is_rejected() {
        let mut request = request();
        request
            .policy
            .observations
            .push(AdaptiveMechanismObservation {
                observation_id: "obs-a".into(),
                action_id: "assay-a".into(),
                outcome_milli: 100,
                uncertainty_milli: 20,
                artifact: LocalArtifactRef {
                    artifact_id: "artifact-a".into(),
                    content_hash: ContentHash::of_bytes(b"a"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            });
        let mut executor = DryRunAdaptiveMechanismPolicyExecutor;
        let campaign = execute_glioma_adaptive_mechanism_campaign(&request, &mut executor).unwrap();
        assert!(campaign.completed_action_order.contains(&"assay-b".into()));
        campaign.validate().unwrap();
    }
}
