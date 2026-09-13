//! Calibration-aware adaptive mechanism campaign for preclinical glioma research.
//!
//! The ordinary adaptive mechanism policy can select an information-rich assay from a posterior,
//! but a posterior is only useful when its model probabilities have earned trust on held-out local
//! observations.  This feature joins the prequential calibration report to the adaptive policy:
//! model-specific trust discounts expected effects, while an explicit exploration bonus can still
//! select a measurement that repairs calibration debt.  Qualification requires both posterior
//! convergence and the calibration gate; no model score, dry-run observation, or scenario forecast
//! is promoted into a biological conclusion.

use super::adaptive_policy::{
    plan_glioma_adaptive_mechanism_policy, AdaptiveMechanismCampaignRequest,
    AdaptiveMechanismObservation, AdaptiveMechanismPolicy, AdaptiveMechanismPolicyError,
    AdaptiveMechanismPolicyExecutor,
};
use super::calibration::{MechanismCalibration, MechanismCalibrationError};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaCalibratedMechanismCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_RETRIES: u8 = 8;
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedMechanismCampaignRequest {
    pub policy: AdaptiveMechanismCampaignRequest,
    pub calibration: MechanismCalibration,
    pub min_coverage_milli: u16,
    pub max_calibration_error_milli: u64,
    pub max_brier_loss_milli: u64,
    pub min_trust_milli: u16,
    pub calibration_exploration_weight_milli: u16,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_calibrated: bool,
    pub allow_uncalibrated_exploration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedMechanismTrust {
    pub model_id: String,
    pub posterior_milli: u16,
    pub calibration_error_milli: u64,
    pub brier_loss_milli: u64,
    pub coverage_milli: u16,
    pub trust_milli: u16,
    pub gate_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedMechanismActionScore {
    pub action_id: String,
    pub base_utility_milli: i64,
    pub calibrated_utility_milli: i64,
    pub calibration_exploration_bonus_milli: i64,
    pub posterior_model_trust_milli: u16,
    pub calibrated_expected_effect_milli: i64,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedMechanismCampaignRound {
    pub round: u16,
    pub policy: AdaptiveMechanismPolicy,
    pub trust: Vec<CalibratedMechanismTrust>,
    pub action_scores: Vec<CalibratedMechanismActionScore>,
    pub ranked_action_order: Vec<String>,
    pub selected_action_id: Option<String>,
    pub observation_id: Option<String>,
    pub calibration_gate_open: bool,
    pub posterior_entropy_milli: u16,
    pub retry_count: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibratedMechanismCampaignDisposition {
    Calibrated,
    Partial,
    CalibrationBlocked,
    BudgetBlocked,
    Failed,
    NoEligibleActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibratedMechanismCampaignStopReason {
    Calibrated,
    CalibrationGateClosed,
    BudgetExhausted,
    NoEligibleActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedMechanismCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub calibration_digest: ContentHash,
    pub rounds: Vec<CalibratedMechanismCampaignRound>,
    pub observations: Vec<AdaptiveMechanismObservation>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_policy: AdaptiveMechanismPolicy,
    pub final_trust: Vec<CalibratedMechanismTrust>,
    pub calibration_gate_open: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: CalibratedMechanismCampaignDisposition,
    pub stop_reason: CalibratedMechanismCampaignStopReason,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CalibratedMechanismCampaignError {
    #[error("calibrated mechanism campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("calibrated mechanism campaign policy failed: {0}")]
    Policy(#[from] AdaptiveMechanismPolicyError),
    #[error("calibrated mechanism campaign calibration failed: {0}")]
    Calibration(#[from] MechanismCalibrationError),
    #[error("calibrated mechanism campaign execution failed: {0}")]
    Execution(String),
    #[error("calibrated mechanism campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("calibrated mechanism campaign digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &CalibratedMechanismCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "model_system": campaign.model_system,
        "calibration_digest": campaign.calibration_digest,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_policy": campaign.final_policy,
        "final_trust": campaign.final_trust,
        "calibration_gate_open": campaign.calibration_gate_open,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
        "next_operator_action": campaign.next_operator_action,
    })
}

fn validate_request(
    request: &CalibratedMechanismCampaignRequest,
) -> Result<(), CalibratedMechanismCampaignError> {
    let policy = &request.policy.policy;
    if policy.objective != request.calibration.objective
        || policy.model_system != request.calibration.model_system
        || request.min_coverage_milli > 1_000
        || request.min_trust_milli > 1_000
        || request.max_calibration_error_milli > 1_000_000
        || request.max_brier_loss_milli > 1_000_000
        || request.calibration_exploration_weight_milli > 1_000
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.policy.max_rounds == 0
        || request.policy.max_rounds > MAX_ROUNDS
        || request.policy.max_retries > MAX_RETRIES
    {
        return Err(CalibratedMechanismCampaignError::InvalidRequest(
            "objective/model binding, calibration thresholds, and bounded campaign controls are required".into(),
        ));
    }
    request.calibration.validate()?;
    let calibration_ids = request
        .calibration
        .scores
        .iter()
        .map(|score| score.mechanism_id.as_str())
        .collect::<BTreeSet<_>>();
    if policy
        .models
        .iter()
        .any(|model| !calibration_ids.contains(model.model_id.as_str()))
    {
        return Err(CalibratedMechanismCampaignError::InvalidRequest(
            "every adaptive model must have a calibration score".into(),
        ));
    }
    Ok(())
}

fn trust_rows(
    policy: &AdaptiveMechanismPolicy,
    calibration: &MechanismCalibration,
    request: &CalibratedMechanismCampaignRequest,
) -> Vec<CalibratedMechanismTrust> {
    let scores = calibration
        .scores
        .iter()
        .map(|score| (score.mechanism_id.as_str(), score))
        .collect::<BTreeMap<_, _>>();
    let mut rows = policy
        .posterior
        .iter()
        .map(|posterior| {
            let score = scores.get(posterior.model_id.as_str());
            let (error, brier, coverage) = score
                .map(|score| {
                    (
                        score.calibration_error_milli,
                        score.brier_loss_milli,
                        score.coverage_milli,
                    )
                })
                .unwrap_or((1_000_000, 1_000_000, 0));
            let penalty = ((error.saturating_add(brier)) / 2).min(1_000_000);
            let calibration_quality = 1_000_000_u64.saturating_sub(penalty);
            let trust = (u64::from(coverage).saturating_mul(calibration_quality) / 1_000_000)
                .min(1_000) as u16;
            let gate_open = coverage >= request.min_coverage_milli
                && error <= request.max_calibration_error_milli
                && brier <= request.max_brier_loss_milli
                && trust >= request.min_trust_milli;
            CalibratedMechanismTrust {
                model_id: posterior.model_id.clone(),
                posterior_milli: posterior.posterior_milli,
                calibration_error_milli: error,
                brier_loss_milli: brier,
                coverage_milli: coverage,
                trust_milli: trust,
                gate_open,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.model_id.cmp(&right.model_id));
    rows
}

fn gate_open(rows: &[CalibratedMechanismTrust]) -> bool {
    !rows.is_empty() && rows.iter().all(|row| row.gate_open)
}

fn action_scores(
    policy: &AdaptiveMechanismPolicy,
    policy_request: &AdaptiveMechanismCampaignRequest,
    trust: &[CalibratedMechanismTrust],
    remaining_budget: u64,
    failed: &BTreeSet<String>,
    request: &CalibratedMechanismCampaignRequest,
    calibration_gate_open: bool,
) -> (Vec<CalibratedMechanismActionScore>, Vec<String>) {
    let trust_map = trust
        .iter()
        .map(|row| (row.model_id.as_str(), row.trust_milli))
        .collect::<BTreeMap<_, _>>();
    let action_map = policy_request
        .policy
        .actions
        .iter()
        .map(|action| (action.action_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    let base_scores = policy
        .scores
        .iter()
        .map(|score| (score.action_id.as_str(), score))
        .collect::<BTreeMap<_, _>>();
    let mut scores = Vec::new();
    for action_id in &policy.action_order {
        let Some(base) = base_scores.get(action_id.as_str()) else {
            continue;
        };
        let Some(action) = action_map.get(action_id.as_str()) else {
            continue;
        };
        let mut weighted_trust = 0_i128;
        let mut weighted_effect = 0_i128;
        let predictions = action
            .predictions
            .iter()
            .map(|prediction| (prediction.model_id.as_str(), prediction.effect_milli))
            .collect::<BTreeMap<_, _>>();
        for posterior in &policy.posterior {
            let model_trust = i128::from(
                trust_map
                    .get(posterior.model_id.as_str())
                    .copied()
                    .unwrap_or(0),
            );
            let posterior_mass = i128::from(posterior.posterior_milli);
            weighted_trust =
                weighted_trust.saturating_add(posterior_mass.saturating_mul(model_trust));
            if let Some(effect) = predictions.get(posterior.model_id.as_str()) {
                weighted_effect = weighted_effect.saturating_add(
                    posterior_mass
                        .saturating_mul(model_trust)
                        .saturating_mul(i128::from(*effect)),
                );
            }
        }
        let model_trust = (weighted_trust / 1_000).clamp(0, 1_000) as u16;
        let calibrated_effect =
            (weighted_effect / 1_000_000).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
        let calibration_debt = i64::from(1_000_u16.saturating_sub(model_trust));
        let exploration_bonus = i128::from(base.information_gain_milli)
            .saturating_mul(i128::from(calibration_debt))
            .saturating_mul(i128::from(request.calibration_exploration_weight_milli))
            .saturating_div(1_000_000)
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX))
            as i64;
        let trust_adjusted = i128::from(base.expected_utility_milli)
            .saturating_mul(i128::from(model_trust))
            .saturating_div(1_000)
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
        let calibrated_utility = trust_adjusted.saturating_add(exploration_bonus);
        let exclusion_reason = if failed.contains(action_id) {
            Some("action-failed-in-this-campaign".into())
        } else if !base.eligible {
            base.exclusion_reason.clone()
        } else if u64::from(action.cost_units) > remaining_budget {
            Some("remaining-budget-does-not-cover-action".into())
        } else if !calibration_gate_open && !request.allow_uncalibrated_exploration {
            Some("calibration-gate-closed".into())
        } else {
            None
        };
        scores.push(CalibratedMechanismActionScore {
            action_id: action.action_id.clone(),
            base_utility_milli: base.expected_utility_milli,
            calibrated_utility_milli: calibrated_utility,
            calibration_exploration_bonus_milli: exploration_bonus,
            posterior_model_trust_milli: model_trust,
            calibrated_expected_effect_milli: calibrated_effect,
            eligible: exclusion_reason.is_none(),
            exclusion_reason,
        });
    }
    let mut ranked = scores
        .iter()
        .filter(|score| score.eligible)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .calibrated_utility_milli
            .cmp(&left.calibrated_utility_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let ranked_order = ranked
        .into_iter()
        .map(|score| score.action_id.clone())
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    (scores, ranked_order)
}

fn validate_observation(
    observation: &AdaptiveMechanismObservation,
    action_id: &str,
    existing: &[AdaptiveMechanismObservation],
    require_artifacts: bool,
) -> Result<(), CalibratedMechanismCampaignError> {
    if observation.observation_id.trim().is_empty()
        || observation.action_id != action_id
        || observation.uncertainty_milli == 0
        || (require_artifacts && observation.artifact.artifact_id.trim().is_empty())
        || observation.artifact.validate().is_err()
        || existing.iter().any(|row| {
            row.observation_id == observation.observation_id
                || row.action_id == observation.action_id
        })
    {
        return Err(CalibratedMechanismCampaignError::Execution(
            "executor returned a duplicate or invalid local mechanism observation".into(),
        ));
    }
    Ok(())
}

impl CalibratedMechanismCampaign {
    pub fn validate(&self) -> Result<(), CalibratedMechanismCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.calibration_digest.as_str().len() != 64
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .final_trust
                    .iter()
                    .map(|row| row.model_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.next_operator_action.trim().is_empty()
            || !self.simulation_only
        {
            return Err(CalibratedMechanismCampaignError::InvalidOutput(
                "identity, ordering, calibration, partition, simulation, or operator-action invariant failed".into(),
            ));
        }
        self.final_policy
            .validate()
            .map_err(CalibratedMechanismCampaignError::Policy)?;
        let mut seen_rounds = BTreeSet::new();
        let mut seen_observations = BTreeSet::new();
        let mut expected_spent = 0_u64;
        let mut expected_retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || round.policy.objective != self.objective
                || round.trust.iter().any(|row| row.model_id.trim().is_empty())
                || !canonical(
                    &round
                        .trust
                        .iter()
                        .map(|row| row.model_id.clone())
                        .collect::<Vec<_>>(),
                )
                || !canonical(
                    &round
                        .action_scores
                        .iter()
                        .map(|row| row.action_id.clone())
                        .collect::<Vec<_>>(),
                )
                || round
                    .ranked_action_order
                    .windows(2)
                    .any(|pair| pair[0] == pair[1])
                || round.budget_after_units > round.budget_before_units
                || round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units)
                    == 0
                || round
                    .observation_id
                    .as_ref()
                    .is_some_and(|id| !seen_observations.insert(id.clone()))
                || round.selected_action_id.as_ref().is_some_and(|id| {
                    !round
                        .action_scores
                        .iter()
                        .any(|score| score.action_id == *id && score.eligible)
                })
            {
                return Err(CalibratedMechanismCampaignError::InvalidOutput(
                    "campaign round ordering, action, trust, or budget invariants are invalid"
                        .into(),
                ));
            }
            round
                .policy
                .validate()
                .map_err(CalibratedMechanismCampaignError::Policy)?;
            expected_spent = expected_spent.saturating_add(
                round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units),
            );
            expected_retries = expected_retries.saturating_add(round.retry_count);
        }
        if expected_spent != self.budget_spent_units || expected_retries != self.retry_count {
            return Err(CalibratedMechanismCampaignError::InvalidOutput(
                "campaign budget or retry totals do not reconcile with rounds".into(),
            ));
        }
        let mut observation_ids = BTreeSet::new();
        let mut observation_actions = BTreeSet::new();
        for observation in &self.observations {
            if observation.observation_id.trim().is_empty()
                || !observation_ids.insert(observation.observation_id.clone())
                || !observation_actions.insert(observation.action_id.clone())
                || observation.uncertainty_milli == 0
                || observation.artifact.validate().is_err()
            {
                return Err(CalibratedMechanismCampaignError::InvalidOutput(
                    "campaign observations must be local, unique, and one-per-action".into(),
                ));
            }
        }
        if self.final_trust.iter().any(|row| {
            row.coverage_milli > 1_000
                || row.trust_milli > 1_000
                || row.calibration_error_milli > 1_000_000
                || row.brier_loss_milli > 1_000_000
        }) {
            return Err(CalibratedMechanismCampaignError::InvalidOutput(
                "final calibration trust bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CalibratedMechanismCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CalibratedMechanismCampaignError::InvalidOutput(
                "calibrated mechanism campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a calibration-gated, observation-driven mechanism campaign.
pub fn execute_glioma_calibrated_mechanism_campaign<E: AdaptiveMechanismPolicyExecutor>(
    request: &CalibratedMechanismCampaignRequest,
    executor: &mut E,
) -> Result<CalibratedMechanismCampaign, CalibratedMechanismCampaignError> {
    validate_request(request)?;
    let mut observations = request.policy.policy.observations.clone();
    if observations.len() > MAX_OBSERVATIONS {
        return Err(CalibratedMechanismCampaignError::InvalidRequest(
            "mechanism observation bound exceeded".into(),
        ));
    }
    let action_map = request
        .policy
        .policy
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut completed = observations
        .iter()
        .map(|observation| observation.action_id.clone())
        .collect::<BTreeSet<_>>();
    let mut failed = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut retry_count = 0_u32;
    let mut spent = 0_u64;
    let mut remaining = request.policy.policy.budget_units;
    let mut stop_reason = CalibratedMechanismCampaignStopReason::MaxRounds;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for round_number in 1..=request.max_rounds {
        if remaining == 0 {
            stop_reason = CalibratedMechanismCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut policy_request = request.policy.clone();
        policy_request.policy.observations = observations.clone();
        policy_request.policy.budget_units = remaining;
        let policy = plan_glioma_adaptive_mechanism_policy(&policy_request.policy)?;
        let trust = trust_rows(&policy, &request.calibration, request);
        let calibration_gate_open = gate_open(&trust);
        let (action_scores, ranked_action_order) = action_scores(
            &policy,
            &policy_request,
            &trust,
            remaining,
            &failed,
            request,
            calibration_gate_open,
        );
        negative_evidence.extend(policy.negative_evidence.iter().cloned());
        uncertainty.extend(policy.uncertainty.iter().cloned());
        if !calibration_gate_open {
            uncertainty.insert(
                "calibration-gate-closed; posterior convergence cannot qualify the campaign".into(),
            );
        }
        if request.stop_on_calibrated
            && calibration_gate_open
            && policy.entropy_milli <= request.policy.policy.stop_entropy_milli
        {
            stop_reason = CalibratedMechanismCampaignStopReason::Calibrated;
            break;
        }
        let selected_action_id = ranked_action_order.first().cloned();
        if selected_action_id.is_none() {
            stop_reason = if !calibration_gate_open && !request.allow_uncalibrated_exploration {
                CalibratedMechanismCampaignStopReason::CalibrationGateClosed
            } else if remaining == 0 {
                CalibratedMechanismCampaignStopReason::BudgetExhausted
            } else {
                CalibratedMechanismCampaignStopReason::NoEligibleActions
            };
            break;
        }
        let action_id = selected_action_id.clone().unwrap_or_default();
        let action = action_map.get(&action_id).ok_or_else(|| {
            CalibratedMechanismCampaignError::Execution(
                "calibrated policy selected an action missing from the registry".into(),
            )
        })?;
        let budget_before = remaining;
        let mut accepted = None;
        let mut round_retries = 0_u32;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_action(action, attempt) {
                Ok(observation) => {
                    validate_observation(
                        &observation,
                        &action_id,
                        &observations,
                        request.require_artifacts,
                    )?;
                    accepted = Some(observation);
                    break;
                }
                Err(failure) => {
                    if failure.reason.trim().is_empty() {
                        return Err(CalibratedMechanismCampaignError::Execution(
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
        let cost = u64::from(action.cost_units).min(remaining);
        remaining = remaining.saturating_sub(cost);
        spent = spent.saturating_add(cost);
        let posterior_entropy_milli = policy.entropy_milli;
        let observation_id = accepted
            .as_ref()
            .map(|observation| observation.observation_id.clone());
        if let Some(observation) = accepted {
            completed.insert(action_id.clone());
            observations.push(observation);
        }
        rounds.push(CalibratedMechanismCampaignRound {
            round: round_number,
            policy,
            trust,
            action_scores,
            ranked_action_order,
            selected_action_id: Some(action_id.clone()),
            observation_id,
            calibration_gate_open,
            posterior_entropy_milli,
            retry_count: round_retries,
            budget_before_units: budget_before,
            budget_after_units: remaining,
        });
        if failed.contains(&action_id) {
            stop_reason = CalibratedMechanismCampaignStopReason::ExecutorFailed;
            break;
        }
        if remaining == 0 {
            stop_reason = CalibratedMechanismCampaignStopReason::BudgetExhausted;
            break;
        }
        if !completed.contains(&action_id) {
            stop_reason = CalibratedMechanismCampaignStopReason::NoProgress;
            break;
        }
        if round_number == request.max_rounds {
            stop_reason = CalibratedMechanismCampaignStopReason::MaxRounds;
        }
    }
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let mut final_request = request.policy.policy.clone();
    final_request.observations = observations.clone();
    let final_policy = plan_glioma_adaptive_mechanism_policy(&final_request)?;
    let final_trust = trust_rows(&final_policy, &request.calibration, request);
    let final_gate_open = gate_open(&final_trust);
    let disposition = match stop_reason {
        CalibratedMechanismCampaignStopReason::Calibrated => {
            CalibratedMechanismCampaignDisposition::Calibrated
        }
        CalibratedMechanismCampaignStopReason::CalibrationGateClosed => {
            CalibratedMechanismCampaignDisposition::CalibrationBlocked
        }
        CalibratedMechanismCampaignStopReason::BudgetExhausted => {
            CalibratedMechanismCampaignDisposition::BudgetBlocked
        }
        CalibratedMechanismCampaignStopReason::ExecutorFailed => {
            CalibratedMechanismCampaignDisposition::Failed
        }
        CalibratedMechanismCampaignStopReason::NoEligibleActions => {
            CalibratedMechanismCampaignDisposition::NoEligibleActions
        }
        CalibratedMechanismCampaignStopReason::NoProgress
        | CalibratedMechanismCampaignStopReason::MaxRounds => {
            CalibratedMechanismCampaignDisposition::Partial
        }
    };
    let next_operator_action = match disposition {
        CalibratedMechanismCampaignDisposition::Calibrated => {
            "review calibrated mechanism evidence and require independent replication before promotion".into()
        }
        CalibratedMechanismCampaignDisposition::CalibrationBlocked => {
            "collect held-out local calibration observations or explicitly authorize bounded calibration exploration".into()
        }
        CalibratedMechanismCampaignDisposition::BudgetBlocked => {
            "resume with an explicitly bounded budget; retain the posterior and calibration debt".into()
        }
        CalibratedMechanismCampaignDisposition::Failed => {
            "repair the institution-local mechanism executor and retry only the failed action".into()
        }
        CalibratedMechanismCampaignDisposition::NoEligibleActions => {
            "add a new calibrated action or resolve the feasibility and completion exclusions".into()
        }
        CalibratedMechanismCampaignDisposition::Partial
        | CalibratedMechanismCampaignDisposition::Unresolved => {
            "continue the bounded campaign only after inspecting posterior uncertainty and calibration debt".into()
        }
    };
    for row in &final_trust {
        if !row.gate_open {
            uncertainty.insert(format!(
                "model:{}:calibration-trust-below-gate",
                row.model_id
            ));
        }
    }
    negative_evidence.extend(request.calibration.negative_evidence.iter().cloned());
    uncertainty.extend(request.calibration.uncertainty.iter().cloned());
    let mut output = CalibratedMechanismCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.policy.policy.objective.clone(),
        model_system: request.policy.policy.model_system,
        calibration_digest: request.calibration.digest.clone(),
        rounds,
        observations,
        completed_action_order: completed.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units: spent,
        remaining_budget_units: request.policy.policy.budget_units.saturating_sub(spent),
        final_policy,
        final_trust,
        calibration_gate_open: final_gate_open,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: true,
        disposition,
        stop_reason,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-calibrated-mechanism-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CalibratedMechanismCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_calibrated_mechanism_campaign_dry_run(
    request: &CalibratedMechanismCampaignRequest,
) -> Result<CalibratedMechanismCampaign, CalibratedMechanismCampaignError> {
    let mut executor = super::adaptive_policy::DryRunAdaptiveMechanismPolicyExecutor;
    execute_glioma_calibrated_mechanism_campaign(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::adaptive_policy::{
        AdaptiveMechanismAction, AdaptiveMechanismModel, AdaptiveMechanismPolicyRequest,
        AdaptiveMechanismPrediction,
    };
    use crate::glioma::programs::p05_mechanism_exploration::calibration::{
        calibrate_glioma_mechanisms, MechanismCalibrationObservation, MechanismCalibrationRequest,
    };
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-calibrated-test+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn calibration() -> MechanismCalibration {
        calibrate_glioma_mechanisms(
            &MechanismCalibrationRequest {
                objective: "calibrated invasion policy".into(),
                model_system: GliomaModelSystem::Organoid,
                min_observations_per_mechanism: 2,
                max_mechanisms: 4,
                max_rounds: 4,
                max_calibration_error_milli: 200_000,
                max_brier_loss_milli: 200_000,
            },
            &[
                MechanismCalibrationObservation {
                    round_index: 0,
                    mechanism_id: "m1".into(),
                    feature_id: "f1".into(),
                    predicted_milli: 800_000,
                    observed_milli: 780_000,
                    uncertainty_milli: 10_000,
                    artifact: artifact("cal-m1-f1"),
                },
                MechanismCalibrationObservation {
                    round_index: 1,
                    mechanism_id: "m1".into(),
                    feature_id: "f2".into(),
                    predicted_milli: 700_000,
                    observed_milli: 680_000,
                    uncertainty_milli: 10_000,
                    artifact: artifact("cal-m1-f2"),
                },
                MechanismCalibrationObservation {
                    round_index: 0,
                    mechanism_id: "m2".into(),
                    feature_id: "f1".into(),
                    predicted_milli: 200_000,
                    observed_milli: 220_000,
                    uncertainty_milli: 10_000,
                    artifact: artifact("cal-m2-f1"),
                },
                MechanismCalibrationObservation {
                    round_index: 1,
                    mechanism_id: "m2".into(),
                    feature_id: "f2".into(),
                    predicted_milli: 300_000,
                    observed_milli: 320_000,
                    uncertainty_milli: 10_000,
                    artifact: artifact("cal-m2-f2"),
                },
            ],
        )
        .unwrap()
    }

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

    fn request() -> CalibratedMechanismCampaignRequest {
        CalibratedMechanismCampaignRequest {
            policy: AdaptiveMechanismCampaignRequest {
                policy: AdaptiveMechanismPolicyRequest {
                    objective: "calibrated invasion policy".into(),
                    model_system: GliomaModelSystem::Organoid,
                    horizon: 1,
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
                stop_on_converged: false,
            },
            calibration: calibration(),
            min_coverage_milli: 1_000,
            max_calibration_error_milli: 200_000,
            max_brier_loss_milli: 200_000,
            min_trust_milli: 500,
            calibration_exploration_weight_milli: 500,
            max_rounds: 2,
            max_retries: 1,
            require_artifacts: true,
            stop_on_calibrated: false,
            allow_uncalibrated_exploration: false,
        }
    }

    #[test]
    fn calibrated_campaign_replays_and_records_trust_adjusted_selection() {
        let request = request();
        let first = execute_glioma_calibrated_mechanism_campaign_dry_run(&request).unwrap();
        let second = execute_glioma_calibrated_mechanism_campaign_dry_run(&request).unwrap();
        assert_eq!(first, second);
        assert!(!first.rounds.is_empty());
        assert!(first.calibration_gate_open);
        assert!(!first.completed_action_order.is_empty());
        assert!(first
            .rounds
            .iter()
            .flat_map(|round| round.action_scores.iter())
            .any(|score| score.posterior_model_trust_milli > 0));
        first.validate().unwrap();
    }

    #[test]
    fn calibration_gate_blocks_unqualified_model_trust_without_exploration() {
        let mut request = request();
        request.min_coverage_milli = 1_001;
        let mut executor = super::super::adaptive_policy::DryRunAdaptiveMechanismPolicyExecutor;
        let error =
            execute_glioma_calibrated_mechanism_campaign(&request, &mut executor).unwrap_err();
        assert!(matches!(
            error,
            CalibratedMechanismCampaignError::InvalidRequest(_)
        ));
    }
}
