//! Closed-loop multi-objective experiment frontier control for preclinical glioma research.
//!
//! The existing P06 planners each optimize one design dimension.  This feature is the
//! composition layer used by a research team when those objectives compete: it estimates
//! mechanism information gain from declared outcome likelihoods, applies power/risk/budget and
//! prerequisite gates, rewards clone and modality coverage, escalates fidelity in order, and
//! executes a bounded batch before updating the posterior and replanning.  The executor is
//! caller-owned, so a laboratory gateway can supply real local observations while the MCP route
//! remains a deterministic simulation.  A null or negative result is retained as evidence and
//! never silently converted into a success.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaExperimentFrontierController1@1";
pub const EXECUTION_OUTPUT_SCHEMA: &str = "GliomaExperimentFrontierExecution1@1";
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_OUTCOMES: usize = 64;
pub const MAX_ROUNDS: u16 = 128;
pub const MAX_ACTIONS_PER_ROUND: usize = 64;
pub const SCORE_SCALE: u64 = 1_000;
const POSTERIOR_SMOOTHING_MILLI: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierMechanism {
    pub mechanism_id: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierOutcome {
    pub outcome_id: String,
    pub label: String,
    pub probability_milli_by_mechanism: BTreeMap<String, u16>,
    pub effect_milli: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierCandidate {
    pub candidate_id: String,
    pub action_family: String,
    pub label: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub fidelity_level: u16,
    pub clone_ids: Vec<String>,
    pub outcomes: Vec<GliomaFrontierOutcome>,
    pub prerequisites: Vec<String>,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub power_milli: u16,
    pub feasibility_milli: u16,
    pub supports_replication: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierObservation {
    pub candidate_id: String,
    pub outcome_id: String,
    pub replicate_index: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaExperimentFrontierRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<GliomaFrontierMechanism>,
    pub candidates: Vec<GliomaFrontierCandidate>,
    pub initial_observations: Vec<GliomaFrontierObservation>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_actions_per_round: usize,
    pub max_retries: u8,
    pub min_information_gain_milli: u64,
    pub min_power_milli: u16,
    pub risk_ceiling_milli: u16,
    pub require_fidelity_escalation: bool,
    pub information_weight_milli: u16,
    pub power_weight_milli: u16,
    pub diversity_weight_milli: u16,
    pub feasibility_weight_milli: u16,
    pub replication_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierScore {
    pub candidate_id: String,
    pub information_gain_milli: u64,
    pub expected_posterior_gini_milli: u64,
    pub power_milli: u16,
    pub clone_novelty_milli: u16,
    pub modality_novelty_milli: u16,
    pub fidelity_milli: u16,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub utility_milli: u64,
    pub eligible: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaFrontierRound {
    pub round: u16,
    pub posterior_before_milli: Vec<u16>,
    pub ranked_candidate_order: Vec<String>,
    pub selected_candidate_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub observations: Vec<GliomaFrontierObservation>,
    pub cost_units: u64,
    pub information_gain_milli: u64,
    pub posterior_after_milli: Vec<u16>,
    pub uncertainty: Vec<String>,
    pub planner_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaFrontierDisposition {
    Qualified,
    Partial,
    BudgetExhausted,
    NoRunnableActions,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaFrontierStopReason {
    Completed,
    BudgetExhausted,
    NoRunnableActions,
    NoProgress,
    MaxRounds,
    ExecutorFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaExperimentFrontierRun {
    pub feature_id: String,
    pub output_schema: String,
    pub execution_output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub posterior_milli: Vec<u16>,
    pub rounds: Vec<GliomaFrontierRound>,
    pub ranked_candidate_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub retired_order: Vec<String>,
    pub budget_units: u64,
    pub budget_spent_units: u64,
    pub budget_remaining_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: GliomaFrontierDisposition,
    pub stop_reason: GliomaFrontierStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaFrontierExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

pub trait GliomaExperimentFrontierExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &GliomaFrontierCandidate,
        round: u16,
        attempt: u8,
    ) -> Result<GliomaFrontierObservation, GliomaFrontierExecutionFailure>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DryRunGliomaExperimentFrontierExecutor;

impl GliomaExperimentFrontierExecutor for DryRunGliomaExperimentFrontierExecutor {
    fn execute_candidate(
        &mut self,
        candidate: &GliomaFrontierCandidate,
        round: u16,
        attempt: u8,
    ) -> Result<GliomaFrontierObservation, GliomaFrontierExecutionFailure> {
        let outcome = candidate
            .outcomes
            .first()
            .ok_or_else(|| GliomaFrontierExecutionFailure {
                reason: "candidate has no declared outcome".into(),
                retryable: false,
            })?;
        let artifact_id = format!(
            "dry-frontier-{}-{}-{}",
            candidate.candidate_id, round, attempt
        );
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "candidate_id": candidate.candidate_id,
            "outcome_id": outcome.outcome_id,
            "round": round,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| GliomaFrontierExecutionFailure {
            reason: format!("cannot hash dry-run artifact: {error}"),
            retryable: false,
        })?;
        Ok(GliomaFrontierObservation {
            candidate_id: candidate.candidate_id.clone(),
            outcome_id: outcome.outcome_id.clone(),
            replicate_index: round,
            artifact: LocalArtifactRef {
                artifact_id,
                content_hash,
                content_type: "application/vnd.aurora.glioma.frontier-simulation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaExperimentFrontierError {
    #[error("frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("frontier input is invalid: {0}")]
    InvalidInput(String),
    #[error("frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("frontier digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.iter().any(|value| value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn gini_milli(posterior: &[u16]) -> u64 {
    SCORE_SCALE.saturating_sub(
        posterior
            .iter()
            .map(|value| u64::from(*value) * u64::from(*value) / SCORE_SCALE)
            .sum::<u64>(),
    )
}

fn posterior_update(
    posterior: &[u16],
    outcome: &GliomaFrontierOutcome,
    mechanism_order: &[String],
) -> Result<Vec<u16>, GliomaExperimentFrontierError> {
    let masses = mechanism_order
        .iter()
        .zip(posterior.iter())
        .map(|(mechanism_id, prior)| {
            let likelihood = outcome
                .probability_milli_by_mechanism
                .get(mechanism_id)
                .copied()
                .ok_or_else(|| {
                    GliomaExperimentFrontierError::InvalidInput(format!(
                        "outcome {} omits mechanism {}",
                        outcome.outcome_id, mechanism_id
                    ))
                })?;
            Ok(u64::from(*prior)
                .saturating_mul(u64::from(likelihood))
                .saturating_add(POSTERIOR_SMOOTHING_MILLI))
        })
        .collect::<Result<Vec<_>, GliomaExperimentFrontierError>>()?;
    let total = masses.iter().copied().sum::<u64>();
    if total == 0 {
        return Err(GliomaExperimentFrontierError::InvalidInput(
            "observation has no posterior mass".into(),
        ));
    }
    let mut next = masses
        .iter()
        .map(|mass| ((u128::from(*mass) * u128::from(SCORE_SCALE)) / u128::from(total)) as u16)
        .collect::<Vec<_>>();
    let assigned = next.iter().map(|value| u32::from(*value)).sum::<u32>();
    let remainder = u32::try_from(SCORE_SCALE)
        .unwrap_or(u32::MAX)
        .saturating_sub(assigned);
    if remainder > 0 {
        let best = masses
            .iter()
            .enumerate()
            .max_by_key(|(index, mass)| (**mass, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
            .unwrap_or(0);
        next[best] = next[best].saturating_add(remainder as u16);
    }
    Ok(next)
}

fn predictive_probability(
    posterior: &[u16],
    outcome: &GliomaFrontierOutcome,
    mechanism_order: &[String],
) -> Result<u64, GliomaExperimentFrontierError> {
    let mut total = 0_u64;
    for (mechanism_id, prior) in mechanism_order.iter().zip(posterior.iter()) {
        let likelihood = outcome
            .probability_milli_by_mechanism
            .get(mechanism_id)
            .copied()
            .ok_or_else(|| {
                GliomaExperimentFrontierError::InvalidInput(format!(
                    "outcome {} omits mechanism {}",
                    outcome.outcome_id, mechanism_id
                ))
            })?;
        total = total.saturating_add(u64::from(*prior) * u64::from(likelihood) / SCORE_SCALE);
    }
    Ok(total.min(SCORE_SCALE))
}

fn expected_information_gain(
    posterior: &[u16],
    candidate: &GliomaFrontierCandidate,
    mechanism_order: &[String],
) -> Result<(u64, u64), GliomaExperimentFrontierError> {
    let prior_gini = gini_milli(posterior);
    let mut expected_gini = 0_u64;
    for outcome in &candidate.outcomes {
        let predictive = predictive_probability(posterior, outcome, mechanism_order)?;
        if predictive == 0 {
            continue;
        }
        let updated = posterior_update(posterior, outcome, mechanism_order)?;
        expected_gini =
            expected_gini.saturating_add(predictive * gini_milli(&updated) / SCORE_SCALE);
    }
    Ok((
        prior_gini.saturating_sub(expected_gini),
        expected_gini.min(SCORE_SCALE),
    ))
}

fn validate_candidate(
    candidate: &GliomaFrontierCandidate,
    mechanism_ids: &BTreeSet<String>,
    model_system: GliomaModelSystem,
) -> Result<(), GliomaExperimentFrontierError> {
    if candidate.candidate_id.trim().is_empty()
        || candidate.action_family.trim().is_empty()
        || candidate.label.trim().is_empty()
        || candidate.model_system != model_system
        || candidate.fidelity_level == 0
        || candidate.outcomes.is_empty()
        || candidate.outcomes.len() > MAX_OUTCOMES
        || candidate
            .prerequisites
            .iter()
            .any(|id| id == &candidate.candidate_id)
        || !unique_nonempty(&candidate.clone_ids)
        || candidate.cost_units == 0
        || candidate.risk_milli > SCORE_SCALE as u16
        || candidate.power_milli > SCORE_SCALE as u16
        || candidate.feasibility_milli > SCORE_SCALE as u16
    {
        return Err(GliomaExperimentFrontierError::InvalidInput(format!(
            "candidate {} has invalid identity, model, bounds, outcomes, clones, or cost",
            candidate.candidate_id
        )));
    }
    if candidate
        .outcomes
        .iter()
        .map(|outcome| outcome.outcome_id.clone())
        .collect::<BTreeSet<_>>()
        .len()
        != candidate.outcomes.len()
        || candidate.outcomes.iter().any(|outcome| {
            outcome.outcome_id.trim().is_empty()
                || outcome.label.trim().is_empty()
                || outcome
                    .probability_milli_by_mechanism
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    != *mechanism_ids
                || outcome
                    .probability_milli_by_mechanism
                    .values()
                    .any(|value| u32::from(*value) > 1_000)
        })
    {
        return Err(GliomaExperimentFrontierError::InvalidInput(format!(
            "candidate {} has invalid outcome likelihoods",
            candidate.candidate_id
        )));
    }
    for mechanism_id in mechanism_ids {
        let sum = candidate
            .outcomes
            .iter()
            .map(|outcome| u32::from(outcome.probability_milli_by_mechanism[mechanism_id]))
            .sum::<u32>();
        if sum != 1_000 {
            return Err(GliomaExperimentFrontierError::InvalidInput(format!(
                "candidate {} probabilities for {} sum to {}, expected 1000",
                candidate.candidate_id, mechanism_id, sum
            )));
        }
    }
    Ok(())
}

fn digest_input(run: &GliomaExperimentFrontierRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "execution_output_schema": run.execution_output_schema,
        "objective": run.objective,
        "model_system": run.model_system,
        "mechanism_order": run.mechanism_order,
        "posterior_milli": run.posterior_milli,
        "rounds": run.rounds,
        "ranked_candidate_order": run.ranked_candidate_order,
        "completed_order": run.completed_order,
        "negative_order": run.negative_order,
        "failed_order": run.failed_order,
        "retired_order": run.retired_order,
        "budget_units": run.budget_units,
        "budget_spent_units": run.budget_spent_units,
        "budget_remaining_units": run.budget_remaining_units,
        "uncertainty": run.uncertainty,
        "negative_evidence": run.negative_evidence,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "next_step": run.next_step,
    })
}

impl GliomaExperimentFrontierRun {
    pub fn validate(&self) -> Result<(), GliomaExperimentFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.execution_output_schema != EXECUTION_OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.mechanism_order.len() < 2
            || !canonical(&self.mechanism_order)
            || self.posterior_milli.len() != self.mechanism_order.len()
            || self
                .posterior_milli
                .iter()
                .map(|value| u32::from(*value))
                .sum::<u32>()
                != 1_000
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.retired_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self
                .budget_spent_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
            || self
                .completed_order
                .iter()
                .any(|id| self.negative_order.contains(id) || self.failed_order.contains(id))
            || self
                .negative_order
                .iter()
                .any(|id| self.failed_order.contains(id))
        {
            return Err(GliomaExperimentFrontierError::InvalidOutput(
                "frontier identity, ordering, posterior, partition, budget, or uncertainty is invalid".into(),
            ));
        }
        let expected_retired = self
            .negative_order
            .iter()
            .chain(self.failed_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if self.retired_order.iter().cloned().collect::<BTreeSet<_>>() != expected_retired {
            return Err(GliomaExperimentFrontierError::InvalidOutput(
                "retired candidates must equal negative and failed candidates".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaExperimentFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaExperimentFrontierError::InvalidOutput(
                "frontier digest is not bound to the run".into(),
            ));
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn score_candidate(
    candidate: &GliomaFrontierCandidate,
    posterior: &[u16],
    mechanism_order: &[String],
    covered_clones: &BTreeSet<String>,
    covered_modalities: &BTreeSet<GliomaModality>,
    request: &GliomaExperimentFrontierRequest,
    max_completed_fidelity: u16,
    completed: &BTreeSet<String>,
    retired: &BTreeSet<String>,
    remaining_budget: u64,
) -> Result<GliomaFrontierScore, GliomaExperimentFrontierError> {
    let (information_gain, expected_posterior_gini) =
        expected_information_gain(posterior, candidate, mechanism_order)?;
    let clone_novelty_milli: u16 = if candidate
        .clone_ids
        .iter()
        .any(|id| !covered_clones.contains(id))
    {
        1_000
    } else {
        0
    };
    let modality_novelty_milli = if covered_modalities.contains(&candidate.modality) {
        0
    } else {
        1_000
    };
    let fidelity_milli = (u32::from(candidate.fidelity_level).saturating_mul(1_000)
        / u32::from(candidate.fidelity_level.max(max_completed_fidelity).max(1)))
    .min(1_000) as u16;
    let prerequisites_ready = candidate
        .prerequisites
        .iter()
        .all(|id| completed.contains(id));
    let fidelity_ready = !request.require_fidelity_escalation
        || candidate.fidelity_level <= max_completed_fidelity.saturating_add(1);
    let eligible = !completed.contains(&candidate.candidate_id)
        && !retired.contains(&candidate.candidate_id)
        && prerequisites_ready
        && fidelity_ready
        && u64::from(candidate.cost_units) <= remaining_budget
        && candidate.risk_milli <= request.risk_ceiling_milli
        && candidate.power_milli >= request.min_power_milli
        && information_gain >= request.min_information_gain_milli;
    let positive = u64::from(request.information_weight_milli)
        .saturating_mul(information_gain)
        .saturating_add(u64::from(request.power_weight_milli) * u64::from(candidate.power_milli))
        .saturating_add(
            u64::from(request.diversity_weight_milli)
                * u64::from(clone_novelty_milli.saturating_add(modality_novelty_milli) / 2),
        )
        .saturating_add(
            u64::from(request.feasibility_weight_milli) * u64::from(candidate.feasibility_milli),
        )
        .saturating_add(
            u64::from(request.replication_weight_milli)
                * if candidate.supports_replication {
                    1_000
                } else {
                    0
                },
        );
    let penalty = u64::from(request.risk_penalty_milli) * u64::from(candidate.risk_milli)
        + u64::from(request.cost_penalty_milli) * u64::from(candidate.cost_units.min(1_000));
    let utility_milli = positive.saturating_sub(penalty) / 1_000;
    let rationale = if !prerequisites_ready {
        "held until prerequisite candidates complete".into()
    } else if !fidelity_ready {
        "held until lower-fidelity evidence is available".into()
    } else if candidate.risk_milli > request.risk_ceiling_milli {
        "risk ceiling exceeded".into()
    } else if candidate.power_milli < request.min_power_milli {
        "power floor not met".into()
    } else if information_gain < request.min_information_gain_milli {
        "declared information-gain floor not met".into()
    } else {
        format!(
            "information gain {}, power {}, clone novelty {}, modality novelty {}, fidelity {}, utility {}",
            information_gain, candidate.power_milli, clone_novelty_milli, modality_novelty_milli, fidelity_milli, utility_milli
        )
    };
    Ok(GliomaFrontierScore {
        candidate_id: candidate.candidate_id.clone(),
        information_gain_milli: information_gain,
        expected_posterior_gini_milli: expected_posterior_gini,
        power_milli: candidate.power_milli,
        clone_novelty_milli,
        modality_novelty_milli,
        fidelity_milli,
        feasibility_milli: candidate.feasibility_milli,
        risk_milli: candidate.risk_milli,
        cost_units: candidate.cost_units,
        utility_milli,
        eligible,
        rationale,
    })
}

#[allow(clippy::type_complexity)]
fn validate_request(
    request: &GliomaExperimentFrontierRequest,
) -> Result<
    (
        Vec<String>,
        Vec<u16>,
        BTreeMap<String, GliomaFrontierCandidate>,
    ),
    GliomaExperimentFrontierError,
> {
    if request.objective.trim().is_empty()
        || request.mechanisms.len() < 2
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS_PER_ROUND
        || request
            .information_weight_milli
            .saturating_add(request.power_weight_milli)
            .saturating_add(request.diversity_weight_milli)
            .saturating_add(request.feasibility_weight_milli)
            .saturating_add(request.replication_weight_milli)
            .saturating_add(request.risk_penalty_milli)
            .saturating_add(request.cost_penalty_milli)
            == 0
        || request.min_information_gain_milli > SCORE_SCALE
        || request.min_power_milli > 1_000
        || request.risk_ceiling_milli > 1_000
    {
        return Err(GliomaExperimentFrontierError::InvalidRequest(
            "objective, bounded mechanisms/candidates/rounds, positive budget, and score bounds are required".into(),
        ));
    }
    let mechanism_ids = request
        .mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanism_ids.len() != request.mechanisms.len()
        || request
            .mechanisms
            .iter()
            .any(|mechanism| mechanism.mechanism_id.trim().is_empty() || mechanism.prior_milli == 0)
        || request
            .mechanisms
            .iter()
            .map(|mechanism| u32::from(mechanism.prior_milli))
            .sum::<u32>()
            != 1_000
    {
        return Err(GliomaExperimentFrontierError::InvalidInput(
            "mechanism ids must be unique and positive priors must sum to 1000".into(),
        ));
    }
    let mut candidates = BTreeMap::new();
    for candidate in &request.candidates {
        validate_candidate(candidate, &mechanism_ids, request.model_system)?;
        if candidates
            .insert(candidate.candidate_id.clone(), candidate.clone())
            .is_some()
        {
            return Err(GliomaExperimentFrontierError::InvalidInput(format!(
                "duplicate candidate {}",
                candidate.candidate_id
            )));
        }
    }
    for candidate in &request.candidates {
        if candidate
            .prerequisites
            .iter()
            .any(|id| !candidates.contains_key(id))
        {
            return Err(GliomaExperimentFrontierError::InvalidInput(format!(
                "candidate {} has an unknown prerequisite",
                candidate.candidate_id
            )));
        }
    }
    let mechanism_order = mechanism_ids.into_iter().collect::<Vec<_>>();
    let posterior = mechanism_order
        .iter()
        .map(|mechanism_id| {
            request
                .mechanisms
                .iter()
                .find(|mechanism| &mechanism.mechanism_id == mechanism_id)
                .map(|mechanism| mechanism.prior_milli)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let candidate_ids = candidates.keys().cloned().collect::<BTreeSet<_>>();
    let mut seen_observations = BTreeSet::new();
    for observation in &request.initial_observations {
        let candidate = candidates.get(&observation.candidate_id).ok_or_else(|| {
            GliomaExperimentFrontierError::InvalidInput(format!(
                "initial observation references unknown candidate {}",
                observation.candidate_id
            ))
        })?;
        if !candidate
            .outcomes
            .iter()
            .any(|outcome| outcome.outcome_id == observation.outcome_id)
        {
            return Err(GliomaExperimentFrontierError::InvalidInput(format!(
                "initial observation references unknown outcome {}",
                observation.outcome_id
            )));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| GliomaExperimentFrontierError::InvalidInput(error.to_string()))?;
        if !seen_observations.insert((
            observation.candidate_id.clone(),
            observation.replicate_index,
        )) {
            return Err(GliomaExperimentFrontierError::InvalidInput(
                "duplicate candidate replicate in initial observations".into(),
            ));
        }
    }
    if candidate_ids.is_empty() {
        return Err(GliomaExperimentFrontierError::InvalidInput(
            "no candidates remain".into(),
        ));
    }
    Ok((mechanism_order, posterior, candidates))
}

pub fn execute_glioma_experiment_frontier_controller<
    E: GliomaExperimentFrontierExecutor + ?Sized,
>(
    request: &GliomaExperimentFrontierRequest,
    executor: &mut E,
) -> Result<GliomaExperimentFrontierRun, GliomaExperimentFrontierError> {
    let (mechanism_order, mut posterior, candidates) = validate_request(request)?;
    let mut completed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut observations = request.initial_observations.clone();
    let mut budget_spent = 0_u64;
    let mut rounds = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();

    for observation in &request.initial_observations {
        let candidate = &candidates[&observation.candidate_id];
        let outcome = candidate
            .outcomes
            .iter()
            .find(|outcome| outcome.outcome_id == observation.outcome_id)
            .ok_or_else(|| {
                GliomaExperimentFrontierError::InvalidInput("initial outcome vanished".into())
            })?;
        posterior = posterior_update(&posterior, outcome, &mechanism_order)?;
        completed.insert(observation.candidate_id.clone());
        if outcome.effect_milli <= 0 {
            negative.insert(observation.candidate_id.clone());
            retired.insert(observation.candidate_id.clone());
            negative_evidence.insert(format!(
                "{} returned non-positive effect {}",
                observation.candidate_id, outcome.effect_milli
            ));
        }
    }

    let mut stop_reason = GliomaFrontierStopReason::MaxRounds;
    for round_index in 0..request.max_rounds {
        let round_number = round_index.saturating_add(1);
        let budget_remaining = request.budget_units.saturating_sub(budget_spent);
        if budget_remaining == 0 {
            stop_reason = GliomaFrontierStopReason::BudgetExhausted;
            break;
        }
        let covered_clones = completed
            .iter()
            .chain(negative.iter())
            .flat_map(|id| {
                candidates
                    .get(id)
                    .into_iter()
                    .flat_map(|candidate| candidate.clone_ids.iter().cloned())
            })
            .collect::<BTreeSet<_>>();
        let covered_modalities = completed
            .iter()
            .chain(negative.iter())
            .filter_map(|id| candidates.get(id).map(|candidate| candidate.modality))
            .collect::<BTreeSet<_>>();
        let max_completed_fidelity = completed
            .iter()
            .chain(negative.iter())
            .filter_map(|id| candidates.get(id).map(|candidate| candidate.fidelity_level))
            .max()
            .unwrap_or(0);
        let mut scores = candidates
            .values()
            .map(|candidate| {
                score_candidate(
                    candidate,
                    &posterior,
                    &mechanism_order,
                    &covered_clones,
                    &covered_modalities,
                    request,
                    max_completed_fidelity,
                    &completed,
                    &retired,
                    budget_remaining,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        scores.sort_by(|left, right| {
            right
                .utility_milli
                .cmp(&left.utility_milli)
                .then_with(|| left.candidate_id.cmp(&right.candidate_id))
        });
        let ranked_candidate_order = scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut selected_families = BTreeSet::new();
        let mut selected_cost = 0_u64;
        for score in &scores {
            if !score.eligible || selected.len() >= request.max_actions_per_round {
                continue;
            }
            let candidate = &candidates[&score.candidate_id];
            if selected_cost.saturating_add(u64::from(candidate.cost_units)) > budget_remaining
                || !selected_families.insert(candidate.action_family.clone())
            {
                continue;
            }
            selected_cost = selected_cost.saturating_add(u64::from(candidate.cost_units));
            selected.push(candidate.candidate_id.clone());
        }
        if selected.is_empty() {
            stop_reason = if scores.iter().any(|score| score.eligible) {
                GliomaFrontierStopReason::NoProgress
            } else {
                GliomaFrontierStopReason::NoRunnableActions
            };
            break;
        }
        let posterior_before = posterior.clone();
        let mut completed_round = Vec::new();
        let mut negative_round = Vec::new();
        let mut failed_round = Vec::new();
        let mut round_observations = Vec::new();
        let mut round_information_gain = 0_u64;
        let mut progressed = false;
        for candidate_id in &selected {
            let candidate = &candidates[candidate_id];
            // A dispatched local action consumes its allocated budget even when the gateway
            // later reports a failure; otherwise repeated failures would create free retries.
            budget_spent = budget_spent.saturating_add(u64::from(candidate.cost_units));
            let mut result = None;
            for attempt in 0..=request.max_retries {
                match executor.execute_candidate(candidate, round_number, attempt) {
                    Ok(observation) => {
                        result = Some(observation);
                        break;
                    }
                    Err(failure) if failure.retryable && attempt < request.max_retries => {
                        uncertainty.insert(format!(
                            "{} retry {}: {}",
                            candidate_id,
                            attempt.saturating_add(1),
                            failure.reason
                        ));
                    }
                    Err(failure) => {
                        uncertainty.insert(format!(
                            "{} execution failed: {}",
                            candidate_id, failure.reason
                        ));
                        break;
                    }
                }
            }
            let Some(observation) = result else {
                failed.insert(candidate_id.clone());
                retired.insert(candidate_id.clone());
                failed_round.push(candidate_id.clone());
                continue;
            };
            let Some(outcome) = candidate
                .outcomes
                .iter()
                .find(|outcome| outcome.outcome_id == observation.outcome_id)
            else {
                failed.insert(candidate_id.clone());
                retired.insert(candidate_id.clone());
                failed_round.push(candidate_id.clone());
                uncertainty.insert(format!("{} returned an undeclared outcome", candidate_id));
                continue;
            };
            observation
                .artifact
                .validate()
                .map_err(|error| GliomaExperimentFrontierError::InvalidInput(error.to_string()))?;
            let prior_gini = gini_milli(&posterior);
            posterior = posterior_update(&posterior, outcome, &mechanism_order)?;
            round_information_gain = round_information_gain
                .saturating_add(prior_gini.saturating_sub(gini_milli(&posterior)));
            observations.push(observation.clone());
            round_observations.push(observation);
            progressed = true;
            if outcome.effect_milli <= 0 {
                negative.insert(candidate_id.clone());
                retired.insert(candidate_id.clone());
                negative_round.push(candidate_id.clone());
                negative_evidence.insert(format!(
                    "{} returned non-positive effect {}",
                    candidate_id, outcome.effect_milli
                ));
            } else {
                completed.insert(candidate_id.clone());
                completed_round.push(candidate_id.clone());
            }
        }
        let mut round_uncertainty = uncertainty.iter().cloned().collect::<Vec<_>>();
        round_uncertainty.sort();
        let round = GliomaFrontierRound {
            round: round_number,
            posterior_before_milli: posterior_before,
            ranked_candidate_order,
            selected_candidate_order: selected,
            completed_order: completed_round,
            negative_order: negative_round,
            failed_order: failed_round,
            observations: round_observations,
            cost_units: selected_cost,
            information_gain_milli: round_information_gain,
            posterior_after_milli: posterior.clone(),
            uncertainty: round_uncertainty,
            planner_digest: ContentHash::of_value(&serde_json::json!({
                "round": round_number,
                "posterior": posterior,
                "scores": scores,
            }))
            .map_err(|error| GliomaExperimentFrontierError::Digest(error.to_string()))?,
        };
        rounds.push(round);
        if !progressed {
            stop_reason = GliomaFrontierStopReason::ExecutorFailed;
            break;
        }
        if budget_spent >= request.budget_units {
            stop_reason = GliomaFrontierStopReason::BudgetExhausted;
            break;
        }
        if completed
            .len()
            .saturating_add(negative.len())
            .saturating_add(failed.len())
            >= candidates.len()
        {
            stop_reason = GliomaFrontierStopReason::Completed;
            break;
        }
    }

    let budget_remaining = request.budget_units.saturating_sub(budget_spent);
    let covered_clones = completed
        .iter()
        .chain(negative.iter())
        .flat_map(|id| {
            candidates
                .get(id)
                .into_iter()
                .flat_map(|candidate| candidate.clone_ids.iter().cloned())
        })
        .collect::<BTreeSet<_>>();
    let covered_modalities = completed
        .iter()
        .chain(negative.iter())
        .filter_map(|id| candidates.get(id).map(|candidate| candidate.modality))
        .collect::<BTreeSet<_>>();
    let max_completed_fidelity = completed
        .iter()
        .chain(negative.iter())
        .filter_map(|id| candidates.get(id).map(|candidate| candidate.fidelity_level))
        .max()
        .unwrap_or(0);
    let final_scores = candidates
        .values()
        .map(|candidate| {
            score_candidate(
                candidate,
                &posterior,
                &mechanism_order,
                &covered_clones,
                &covered_modalities,
                request,
                max_completed_fidelity,
                &completed,
                &retired,
                budget_remaining,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut final_ranked = final_scores;
    final_ranked.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_candidate_order = final_ranked
        .iter()
        .map(|score| score.candidate_id.clone())
        .collect::<Vec<_>>();
    let disposition = match stop_reason {
        GliomaFrontierStopReason::Completed => GliomaFrontierDisposition::Qualified,
        GliomaFrontierStopReason::BudgetExhausted => GliomaFrontierDisposition::BudgetExhausted,
        GliomaFrontierStopReason::NoRunnableActions => GliomaFrontierDisposition::NoRunnableActions,
        GliomaFrontierStopReason::ExecutorFailed => GliomaFrontierDisposition::Partial,
        GliomaFrontierStopReason::NoProgress | GliomaFrontierStopReason::MaxRounds => {
            GliomaFrontierDisposition::Unresolved
        }
    };
    let mut next_step: String = match disposition {
        GliomaFrontierDisposition::Qualified => {
            "review the posterior and promote only the declared next frontier action".into()
        }
        GliomaFrontierDisposition::Partial => {
            "inspect failed or missing local observations before retrying the frontier".into()
        }
        GliomaFrontierDisposition::BudgetExhausted => {
            "allocate additional approved preclinical budget or release a negative-result bundle"
                .into()
        }
        GliomaFrontierDisposition::NoRunnableActions => {
            "satisfy prerequisites, risk, power, or fidelity gates before replanning".into()
        }
        GliomaFrontierDisposition::Unresolved => {
            "add discriminating evidence without treating the unresolved posterior as a conclusion"
                .into()
        }
    };
    if !negative_evidence.is_empty() {
        next_step.push_str("; retain the negative evidence in the next research object");
    }
    let mut completed_order = completed.into_iter().collect::<Vec<_>>();
    let mut negative_order = negative.into_iter().collect::<Vec<_>>();
    let mut failed_order = failed.into_iter().collect::<Vec<_>>();
    let mut retired_order = retired.into_iter().collect::<Vec<_>>();
    completed_order.sort();
    negative_order.sort();
    failed_order.sort();
    retired_order.sort();
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let mut negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    uncertainty.sort();
    negative_evidence.sort();
    let mut run = GliomaExperimentFrontierRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        execution_output_schema: EXECUTION_OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        posterior_milli: posterior,
        rounds,
        ranked_candidate_order,
        completed_order,
        negative_order,
        failed_order,
        retired_order,
        budget_units: request.budget_units,
        budget_spent_units: budget_spent,
        budget_remaining_units: budget_remaining,
        uncertainty,
        negative_evidence,
        disposition,
        stop_reason,
        next_step,
        digest: ContentHash::of_value(&serde_json::json!({"placeholder": true}))
            .map_err(|error| GliomaExperimentFrontierError::Digest(error.to_string()))?,
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| GliomaExperimentFrontierError::Digest(error.to_string()))?;
    run.validate()?;
    let _ = observations;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> GliomaExperimentFrontierRequest {
        let probabilities =
            |a: u16, b: u16| BTreeMap::from([("invasion".into(), a), ("repair".into(), b)]);
        GliomaExperimentFrontierRequest {
            objective: "select a discriminating invasion assay".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![
                GliomaFrontierMechanism {
                    mechanism_id: "invasion".into(),
                    prior_milli: 600,
                },
                GliomaFrontierMechanism {
                    mechanism_id: "repair".into(),
                    prior_milli: 400,
                },
            ],
            candidates: vec![
                GliomaFrontierCandidate {
                    candidate_id: "low-fidelity-imaging".into(),
                    action_family: "imaging".into(),
                    label: "low fidelity invasion imaging".into(),
                    model_system: GliomaModelSystem::Organoid,
                    modality: GliomaModality::Imaging,
                    fidelity_level: 1,
                    clone_ids: vec!["clone-a".into()],
                    outcomes: vec![
                        GliomaFrontierOutcome {
                            outcome_id: "signal".into(),
                            label: "invasion signal".into(),
                            probability_milli_by_mechanism: probabilities(800, 200),
                            effect_milli: 400,
                        },
                        GliomaFrontierOutcome {
                            outcome_id: "null".into(),
                            label: "null".into(),
                            probability_milli_by_mechanism: probabilities(200, 800),
                            effect_milli: -100,
                        },
                    ],
                    prerequisites: vec![],
                    cost_units: 1,
                    risk_milli: 100,
                    power_milli: 900,
                    feasibility_milli: 900,
                    supports_replication: true,
                },
                GliomaFrontierCandidate {
                    candidate_id: "high-fidelity-spatial".into(),
                    action_family: "spatial".into(),
                    label: "high fidelity spatial assay".into(),
                    model_system: GliomaModelSystem::Organoid,
                    modality: GliomaModality::Spatial,
                    fidelity_level: 2,
                    clone_ids: vec!["clone-b".into()],
                    outcomes: vec![
                        GliomaFrontierOutcome {
                            outcome_id: "signal".into(),
                            label: "invasion signal".into(),
                            probability_milli_by_mechanism: probabilities(900, 100),
                            effect_milli: 500,
                        },
                        GliomaFrontierOutcome {
                            outcome_id: "null".into(),
                            label: "null".into(),
                            probability_milli_by_mechanism: probabilities(100, 900),
                            effect_milli: -100,
                        },
                    ],
                    prerequisites: vec!["low-fidelity-imaging".into()],
                    cost_units: 2,
                    risk_milli: 100,
                    power_milli: 950,
                    feasibility_milli: 700,
                    supports_replication: true,
                },
            ],
            initial_observations: vec![],
            budget_units: 4,
            max_rounds: 3,
            max_actions_per_round: 1,
            max_retries: 1,
            min_information_gain_milli: 1,
            min_power_milli: 500,
            risk_ceiling_milli: 500,
            require_fidelity_escalation: true,
            information_weight_milli: 500,
            power_weight_milli: 100,
            diversity_weight_milli: 100,
            feasibility_weight_milli: 100,
            replication_weight_milli: 100,
            risk_penalty_milli: 10,
            cost_penalty_milli: 5,
        }
    }

    #[test]
    fn frontier_controller_escalates_fidelity_and_replans_from_observations() {
        let mut executor = DryRunGliomaExperimentFrontierExecutor;
        let run = execute_glioma_experiment_frontier_controller(&request(), &mut executor).unwrap();
        assert_eq!(run.disposition, GliomaFrontierDisposition::Qualified);
        assert_eq!(
            run.completed_order,
            vec!["high-fidelity-spatial", "low-fidelity-imaging"]
        );
        assert_eq!(run.rounds.len(), 2);
        assert_eq!(
            run.rounds[0].selected_candidate_order,
            vec!["low-fidelity-imaging"]
        );
        assert_eq!(
            run.rounds[1].selected_candidate_order,
            vec!["high-fidelity-spatial"]
        );
        run.validate().unwrap();
    }

    #[test]
    fn frontier_controller_holds_candidates_below_power_or_above_risk() {
        let mut request = request();
        request.candidates[0].power_milli = 400;
        request.candidates[1].risk_milli = 900;
        let mut executor = DryRunGliomaExperimentFrontierExecutor;
        let run = execute_glioma_experiment_frontier_controller(&request, &mut executor).unwrap();
        assert_eq!(
            run.disposition,
            GliomaFrontierDisposition::NoRunnableActions
        );
        assert_eq!(run.stop_reason, GliomaFrontierStopReason::NoRunnableActions);
    }

    #[test]
    fn frontier_controller_aligns_unsorted_priors_to_canonical_mechanism_order() {
        let mut request = request();
        request.mechanisms.reverse();
        let mut executor = DryRunGliomaExperimentFrontierExecutor;
        let run = execute_glioma_experiment_frontier_controller(&request, &mut executor).unwrap();
        assert_eq!(run.mechanism_order, vec!["invasion", "repair"]);
        assert_eq!(run.posterior_milli.iter().sum::<u16>(), 1_000);
        run.validate().unwrap();
    }
}
