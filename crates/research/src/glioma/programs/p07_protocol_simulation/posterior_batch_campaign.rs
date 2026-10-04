//! Closed-loop posterior-guided assay execution for preclinical glioma research.
//!
//! P06 scores batches from model-supplied posterior draws. This P07 controller asks a
//! caller-owned local model to update its posterior after returned outcomes, then selects and
//! dispatches the next batch. Model data and raw assay payloads stay with those local providers.
//! The controller enforces a sequential experiment loop; it does not claim model calibration or
//! biological efficacy. There is no synthetic default model or executor. Ambiguous assay
//! failures consume budget and stop; physical work is never blindly retried.

use super::super::p06_experiment_design::posterior_batch::{
    plan_glioma_posterior_batch, PosteriorBatchCandidate, PosteriorBatchDisposition,
    PosteriorBatchPlan, PosteriorBatchRequest, PosteriorBatchTarget, PosteriorPredictiveDraw,
    MAX_CANDIDATES, MAX_OUTCOME_BINS,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Alternate posterior-model backend for the existing P07 active-learning campaign feature.
pub const FEATURE_ID: &str = super::active_learning_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaPosteriorBatchCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_SELECTIONS_PER_ROUND: usize = 64;
pub const MAX_OBSERVATIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchCampaignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u32,
    pub max_rounds: u16,
    pub max_selections_per_round: usize,
    pub max_risk_milli: u16,
    pub min_marginal_reduction_milli: u32,
}

/// A model-defined outcome bin plus a reference to the local assay artifact; not an efficacy
/// label, success state, or clinical category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchCampaignObservation {
    pub observation_id: String,
    pub candidate_id: String,
    pub round: u16,
    pub outcome_bin: u8,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchAssayOutcome {
    pub observation_id: String,
    pub candidate_id: String,
    pub outcome_bin: u8,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchModelState {
    /// Current assay frontier. A posterior update may introduce new candidates.
    pub candidates: Vec<PosteriorBatchCandidate>,
    pub targets: Vec<PosteriorBatchTarget>,
    pub posterior_draws: Vec<PosteriorPredictiveDraw>,
}

pub struct PosteriorBatchModelContext<'a> {
    pub objective: &'a str,
    pub model_system: GliomaModelSystem,
    pub round: u16,
    pub remaining_budget_units: u32,
    pub observations: &'a [PosteriorBatchCampaignObservation],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosteriorBatchProviderFailure {
    pub reason: String,
}

/// Refit/update an institution-local model and return candidate-level posterior predictions.
pub trait PosteriorBatchModel {
    fn infer(
        &mut self,
        context: &PosteriorBatchModelContext<'_>,
    ) -> Result<PosteriorBatchModelState, PosteriorBatchProviderFailure>;
}

/// Execute one selected assay and return its locally stored, schema-bound observation.
pub trait PosteriorBatchCampaignExecutor {
    fn execute_assay(
        &mut self,
        candidate: &PosteriorBatchCandidate,
        round: u16,
    ) -> Result<PosteriorBatchAssayOutcome, PosteriorBatchProviderFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchCampaignRound {
    pub round: u16,
    pub plan: PosteriorBatchPlan,
    pub attempted_order: Vec<String>,
    pub observed_order: Vec<String>,
    pub failed_candidate: Option<String>,
    pub failure_reason: Option<String>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosteriorBatchCampaignDisposition {
    Partial,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosteriorBatchCampaignStopReason {
    BudgetExhausted,
    NoEligibleAssays,
    InformationFloor,
    MaxRounds,
    ModelFailed,
    ExecutorFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u32,
    pub budget_spent_units: u32,
    pub budget_remaining_units: u32,
    pub rounds: Vec<PosteriorBatchCampaignRound>,
    pub observations: Vec<PosteriorBatchCampaignObservation>,
    pub selected_order: Vec<String>,
    pub attempted_order: Vec<String>,
    pub stop_reason: PosteriorBatchCampaignStopReason,
    pub disposition: PosteriorBatchCampaignDisposition,
    pub stop_detail: String,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PosteriorBatchCampaignError {
    #[error("posterior batch campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("posterior batch model state is invalid: {0}")]
    InvalidModelState(String),
    #[error("posterior batch assay outcome is invalid: {0}")]
    InvalidOutcome(String),
    #[error("posterior batch planner failed: {0}")]
    Planner(String),
    #[error("posterior batch campaign output is invalid: {0}")]
    InvalidOutput(String),
}

fn digest_input(campaign: &PosteriorBatchCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "model_system": campaign.model_system,
        "budget_units": campaign.budget_units,
        "budget_spent_units": campaign.budget_spent_units,
        "budget_remaining_units": campaign.budget_remaining_units,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "selected_order": campaign.selected_order,
        "attempted_order": campaign.attempted_order,
        "stop_reason": campaign.stop_reason,
        "disposition": campaign.disposition,
        "stop_detail": campaign.stop_detail,
        "next_step": campaign.next_step,
    })
}

fn validate_request(
    request: &PosteriorBatchCampaignRequest,
) -> Result<(), PosteriorBatchCampaignError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_selections_per_round == 0
        || request.max_selections_per_round > MAX_SELECTIONS_PER_ROUND
        || request.max_risk_milli > 1_000
        || request.min_marginal_reduction_milli > 2_000_000
    {
        return Err(PosteriorBatchCampaignError::InvalidRequest(
            "objective, positive budget and bounded rounds/batch, and bounded risk/information thresholds are required".into(),
        ));
    }
    Ok(())
}

fn same_candidate_definition(
    left: &PosteriorBatchCandidate,
    right: &PosteriorBatchCandidate,
) -> bool {
    left.candidate_id == right.candidate_id
        && left.mechanism_id == right.mechanism_id
        && left.output_schema == right.output_schema
        && left.cost_units == right.cost_units
        && left.risk_milli == right.risk_milli
        && left.max_replicates == right.max_replicates
        && left.redundancy_group == right.redundancy_group
}

fn observation_count(observations: &[PosteriorBatchCampaignObservation], id: &str) -> usize {
    observations
        .iter()
        .filter(|observation| observation.candidate_id == id)
        .count()
}

fn outcome_bin_count(id: &str, draws: &[PosteriorPredictiveDraw]) -> Option<usize> {
    draws
        .first()?
        .candidate_outcome_probabilities
        .get(id)
        .map(Vec::len)
}

impl PosteriorBatchCampaign {
    pub fn validate(&self) -> Result<(), PosteriorBatchCampaignError> {
        let selected = self
            .rounds
            .iter()
            .flat_map(|round| round.plan.selected_order.iter().cloned())
            .collect::<Vec<_>>();
        let attempted = self
            .rounds
            .iter()
            .flat_map(|round| round.attempted_order.iter().cloned())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.budget_units == 0
            || self
                .budget_spent_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
            || self.rounds.len() > MAX_ROUNDS as usize
            || self.observations.len() > MAX_OBSERVATIONS
            || self.stop_detail.trim().is_empty()
            || self.next_step.trim().is_empty()
            || selected != self.selected_order
            || attempted != self.attempted_order
        {
            return Err(PosteriorBatchCampaignError::InvalidOutput(
                "identity, bounded history, budget, action order, or stop guidance invariant failed".into(),
            ));
        }
        let mut observation_ids = BTreeSet::new();
        if self.observations.iter().any(|observation| {
            observation.observation_id.trim().is_empty()
                || observation.candidate_id.trim().is_empty()
                || observation.round == 0
                || observation.outcome_bin as usize >= MAX_OUTCOME_BINS
                || observation.artifact.validate().is_err()
                || !observation_ids.insert(observation.observation_id.as_str())
        }) {
            return Err(PosteriorBatchCampaignError::InvalidOutput(
                "observations must be unique, bounded, local, and content-addressed".into(),
            ));
        }
        let mut spent = 0_u32;
        let observation_by_id = self
            .observations
            .iter()
            .map(|observation| (observation.observation_id.as_str(), observation))
            .collect::<BTreeMap<_, _>>();
        for (index, round) in self.rounds.iter().enumerate() {
            round
                .plan
                .validate()
                .map_err(|error| PosteriorBatchCampaignError::InvalidOutput(error.to_string()))?;
            let expected_attempted = round
                .plan
                .selected_order
                .iter()
                .take(round.attempted_order.len())
                .cloned()
                .collect::<Vec<_>>();
            let observed_candidates = round
                .observed_order
                .iter()
                .filter_map(|observation_id| {
                    observation_by_id
                        .get(observation_id.as_str())
                        .filter(|observation| observation.round == round.round)
                        .map(|observation| observation.candidate_id.clone())
                })
                .collect::<Vec<_>>();
            let expected_observed = round
                .attempted_order
                .iter()
                .take(round.observed_order.len())
                .cloned()
                .collect::<Vec<_>>();
            let failed_dispatch_is_last = round.failed_candidate.as_ref().is_some_and(|failed| {
                round.attempted_order.last() == Some(failed)
                    && round.observed_order.len().saturating_add(1) == round.attempted_order.len()
            });
            if round.round as usize != index + 1
                || round.budget_before_units < round.budget_after_units
                || round.budget_before_units
                    != if index == 0 {
                        self.budget_units
                    } else {
                        self.rounds[index - 1].budget_after_units
                    }
                || round.failed_candidate.is_some() != round.failure_reason.is_some()
                || round
                    .failure_reason
                    .as_ref()
                    .is_some_and(|reason| reason.trim().is_empty())
                || expected_attempted != round.attempted_order
                || observed_candidates != expected_observed
                || round.observed_order.len() > round.attempted_order.len()
                || round
                    .observed_order
                    .iter()
                    .any(|id| !observation_by_id.contains_key(id.as_str()))
                || round.observed_order.iter().collect::<BTreeSet<_>>().len()
                    != round.observed_order.len()
                || if round.failed_candidate.is_some() {
                    !failed_dispatch_is_last
                } else {
                    round.attempted_order != round.plan.selected_order
                        || round.observed_order.len() != round.attempted_order.len()
                }
            {
                return Err(PosteriorBatchCampaignError::InvalidOutput(
                    "round sequence, action/outcome binding, budget, or executor failure state is invalid".into(),
                ));
            }
            spent = spent.saturating_add(round.budget_before_units - round.budget_after_units);
        }
        if spent != self.budget_spent_units
            || self
                .rounds
                .last()
                .is_some_and(|round| round.budget_after_units != self.budget_remaining_units)
            || self.observations.iter().any(|observation| {
                observation.round as usize > self.rounds.len()
                    || !self.rounds[observation.round as usize - 1]
                        .observed_order
                        .contains(&observation.observation_id)
            })
        {
            return Err(PosteriorBatchCampaignError::InvalidOutput(
                "round costs or observation-to-round links are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PosteriorBatchCampaignError::InvalidOutput(error.to_string()))?;
        if expected != self.digest {
            return Err(PosteriorBatchCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn seal(
    mut campaign: PosteriorBatchCampaign,
) -> Result<PosteriorBatchCampaign, PosteriorBatchCampaignError> {
    campaign.digest = ContentHash::of_value(&digest_input(&campaign))
        .map_err(|error| PosteriorBatchCampaignError::InvalidOutput(error.to_string()))?;
    campaign.validate()?;
    Ok(campaign)
}

/// Execute posterior-guided glioma assay batches, updating the local model after every returned
/// outcome. Only returned observations advance the next posterior; negative/null outcomes remain
/// in the history. A `Partial` result means the bounded campaign stopped, not that a hypothesis
/// was proved or biological convergence was reached.
pub fn execute_glioma_posterior_batch_campaign<
    M: PosteriorBatchModel,
    E: PosteriorBatchCampaignExecutor,
>(
    request: &PosteriorBatchCampaignRequest,
    model: &mut M,
    executor: &mut E,
) -> Result<PosteriorBatchCampaign, PosteriorBatchCampaignError> {
    validate_request(request)?;
    let mut observations = Vec::new();
    let mut rounds = Vec::new();
    let mut selected_order = Vec::new();
    let mut attempted_order = Vec::new();
    let mut candidate_baselines = BTreeMap::<String, PosteriorBatchCandidate>::new();
    let mut declared_targets: Option<Vec<PosteriorBatchTarget>> = None;
    let mut remaining = request.budget_units;
    let mut stop_reason = PosteriorBatchCampaignStopReason::MaxRounds;
    let mut disposition = PosteriorBatchCampaignDisposition::Partial;
    let mut stop_detail = "maximum campaign rounds reached".to_owned();
    let mut next_step =
        "review observed outcomes and decide whether another bounded campaign is justified"
            .to_owned();

    for round_number in 1..=request.max_rounds {
        if remaining == 0 {
            stop_reason = PosteriorBatchCampaignStopReason::BudgetExhausted;
            stop_detail = "the declared assay budget is exhausted".into();
            next_step =
                "review collected outcomes; budget exhaustion is not evidence of convergence"
                    .into();
            break;
        }
        let context = PosteriorBatchModelContext {
            objective: &request.objective,
            model_system: request.model_system,
            round: round_number,
            remaining_budget_units: remaining,
            observations: &observations,
        };
        let state = match model.infer(&context) {
            Ok(state) => state,
            Err(error) => {
                disposition = PosteriorBatchCampaignDisposition::Failed;
                stop_reason = PosteriorBatchCampaignStopReason::ModelFailed;
                stop_detail = error.reason;
                next_step = "inspect the local model-fitting failure; preserve prior observations and resume only with a validated posterior".into();
                break;
            }
        };
        if state.candidates.len() > MAX_CANDIDATES {
            return Err(PosteriorBatchCampaignError::InvalidModelState(
                "model candidate frontier exceeds the P06 planner bound".into(),
            ));
        }
        match &declared_targets {
            None if state.targets.is_empty() => {
                return Err(PosteriorBatchCampaignError::InvalidModelState(
                    "model must provide at least one fixed research target".into(),
                ));
            }
            None => declared_targets = Some(state.targets.clone()),
            Some(targets) if targets != &state.targets => {
                return Err(PosteriorBatchCampaignError::InvalidModelState(
                    "target identities/order/weights must remain fixed during the campaign".into(),
                ));
            }
            Some(_) => {}
        }
        let mut candidates = state.candidates;
        for candidate in &mut candidates {
            match candidate_baselines.get(&candidate.candidate_id) {
                Some(baseline)
                    if !same_candidate_definition(candidate, baseline)
                        || candidate.completed_replicates != baseline.completed_replicates =>
                {
                    return Err(PosteriorBatchCampaignError::InvalidModelState(format!(
                        "candidate {} changed its declared design or baseline replicate count",
                        candidate.candidate_id
                    )));
                }
                Some(_) => {}
                None => {
                    candidate_baselines.insert(candidate.candidate_id.clone(), candidate.clone());
                }
            }
            let baseline = &candidate_baselines[&candidate.candidate_id];
            candidate.completed_replicates = baseline
                .completed_replicates
                .saturating_add(observation_count(&observations, &candidate.candidate_id));
        }
        let plan_request = PosteriorBatchRequest {
            objective: request.objective.clone(),
            model_system: request.model_system,
            budget_units: remaining,
            max_selections: request.max_selections_per_round,
            max_risk_milli: request.max_risk_milli,
            min_marginal_reduction_milli: request.min_marginal_reduction_milli,
            targets: state.targets,
            posterior_draws: state.posterior_draws,
        };
        let plan = plan_glioma_posterior_batch(&plan_request, &candidates)
            .map_err(|error| PosteriorBatchCampaignError::Planner(error.to_string()))?;
        if plan.selected_order.is_empty() {
            stop_reason = if plan.disposition == PosteriorBatchDisposition::NoCandidates {
                PosteriorBatchCampaignStopReason::NoEligibleAssays
            } else {
                PosteriorBatchCampaignStopReason::InformationFloor
            };
            disposition = PosteriorBatchCampaignDisposition::Unresolved;
            stop_detail = if stop_reason == PosteriorBatchCampaignStopReason::NoEligibleAssays {
                "the local model supplied no eligible preclinical assays".into()
            } else {
                "no candidate cleared the declared posterior-disagreement and risk gates".into()
            };
            next_step = "review model support, risk and replicate limits, and the information threshold before changing the campaign".into();
            rounds.push(PosteriorBatchCampaignRound {
                round: round_number,
                plan,
                attempted_order: Vec::new(),
                observed_order: Vec::new(),
                failed_candidate: None,
                failure_reason: None,
                budget_before_units: remaining,
                budget_after_units: remaining,
            });
            break;
        }
        let candidate_map = candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        let budget_before = remaining;
        let mut attempted = Vec::new();
        let mut observed = Vec::new();
        let mut failed_candidate = None;
        let mut failure_reason = None;
        for candidate_id in &plan.selected_order {
            let candidate = candidate_map[candidate_id.as_str()];
            if candidate.cost_units > remaining {
                return Err(PosteriorBatchCampaignError::InvalidOutput(
                    "P06 selected an assay above the remaining campaign budget".into(),
                ));
            }
            // Charge on dispatch, not success: an ambiguous worker failure may have consumed
            // material or instrument time. The campaign never retries a dispatched assay.
            remaining -= candidate.cost_units;
            attempted.push(candidate_id.clone());
            attempted_order.push(candidate_id.clone());
            let outcome = match executor.execute_assay(candidate, round_number) {
                Ok(outcome) => outcome,
                Err(error) => {
                    failed_candidate = Some(candidate_id.clone());
                    failure_reason = Some(error.reason);
                    disposition = PosteriorBatchCampaignDisposition::Failed;
                    stop_reason = PosteriorBatchCampaignStopReason::ExecutorFailed;
                    stop_detail = format!("assay executor failed for {candidate_id}");
                    next_step = "reconcile the local assay and artifact state; do not automatically retry an ambiguous effect".into();
                    break;
                }
            };
            let bins = outcome_bin_count(candidate_id, &plan_request.posterior_draws);
            let invalid = outcome.candidate_id != *candidate_id
                || outcome.observation_id.trim().is_empty()
                || observations
                    .iter()
                    .any(|previous| previous.observation_id == outcome.observation_id)
                || bins.is_none_or(|count| outcome.outcome_bin as usize >= count)
                || outcome.artifact.validate().is_err()
                || outcome.artifact.content_type != candidate.output_schema;
            if invalid {
                failed_candidate = Some(candidate_id.clone());
                failure_reason = Some(
                    "executor returned a candidate-mismatched, out-of-support, or non-local assay outcome".into(),
                );
                disposition = PosteriorBatchCampaignDisposition::Failed;
                stop_reason = PosteriorBatchCampaignStopReason::ExecutorFailed;
                stop_detail = format!("assay outcome validation failed for {candidate_id}");
                next_step = "inspect the local assay output and schema binding; preserve the charged dispatch and do not retry automatically".into();
                break;
            }
            // A negative/null bin is still an observed result and is passed to the next fit.
            observed.push(outcome.observation_id.clone());
            observations.push(PosteriorBatchCampaignObservation {
                observation_id: outcome.observation_id,
                candidate_id: outcome.candidate_id,
                round: round_number,
                outcome_bin: outcome.outcome_bin,
                artifact: outcome.artifact,
            });
            if observations.len() > MAX_OBSERVATIONS {
                return Err(PosteriorBatchCampaignError::InvalidOutput(
                    "campaign observation history exceeded its bound".into(),
                ));
            }
        }
        selected_order.extend(plan.selected_order.iter().cloned());
        rounds.push(PosteriorBatchCampaignRound {
            round: round_number,
            plan,
            attempted_order: attempted,
            observed_order: observed,
            failed_candidate,
            failure_reason,
            budget_before_units: budget_before,
            budget_after_units: remaining,
        });
        if disposition == PosteriorBatchCampaignDisposition::Failed {
            break;
        }
        if remaining == 0 {
            stop_reason = PosteriorBatchCampaignStopReason::BudgetExhausted;
            stop_detail = "the declared assay budget is exhausted".into();
            next_step =
                "review collected outcomes; budget exhaustion is not evidence of convergence"
                    .into();
            break;
        }
        if round_number == request.max_rounds {
            stop_reason = PosteriorBatchCampaignStopReason::MaxRounds;
            stop_detail =
                "the configured maximum number of posterior-update rounds was reached".into();
            next_step = "review the updated model and start another bounded campaign only if new assays or evidence are justified".into();
        }
    }

    let spent = request.budget_units - remaining;
    seal(PosteriorBatchCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        budget_units: request.budget_units,
        budget_spent_units: spent,
        budget_remaining_units: remaining,
        rounds,
        observations,
        selected_order,
        attempted_order,
        stop_reason,
        disposition,
        stop_detail,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-posterior-batch-campaign"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, redundancy_group: &str) -> PosteriorBatchCandidate {
        PosteriorBatchCandidate {
            candidate_id: id.into(),
            mechanism_id: format!("mechanism-{id}"),
            output_schema: "GliomaOrganoidAssay1@1".into(),
            cost_units: 1,
            risk_milli: 100,
            completed_replicates: 0,
            max_replicates: 1,
            redundancy_group: redundancy_group.into(),
        }
    }

    fn draw(
        id: &str,
        weight: u32,
        x: i64,
        y: i64,
        a: Vec<u32>,
        b: Vec<u32>,
    ) -> PosteriorPredictiveDraw {
        PosteriorPredictiveDraw {
            draw_id: id.into(),
            prior_weight_millionths: weight,
            target_predictions_milli: vec![x, y],
            candidate_outcome_probabilities: BTreeMap::from([
                ("a-primary".into(), a),
                ("b-orthogonal".into(), b),
            ]),
        }
    }

    fn request() -> PosteriorBatchCampaignRequest {
        PosteriorBatchCampaignRequest {
            objective: "resolve preclinical organoid invasion and stemness mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_units: 2,
            max_rounds: 4,
            max_selections_per_round: 1,
            max_risk_milli: 500,
            min_marginal_reduction_milli: 1,
        }
    }

    fn targets() -> Vec<PosteriorBatchTarget> {
        vec![
            PosteriorBatchTarget {
                target_id: "invasion".into(),
                weight_milli: 500,
            },
            PosteriorBatchTarget {
                target_id: "stemness".into(),
                weight_milli: 500,
            },
        ]
    }

    #[derive(Default)]
    struct UpdatingModel {
        calls: usize,
        observations_seen: Vec<usize>,
    }

    impl PosteriorBatchModel for UpdatingModel {
        fn infer(
            &mut self,
            context: &PosteriorBatchModelContext<'_>,
        ) -> Result<PosteriorBatchModelState, PosteriorBatchProviderFailure> {
            self.calls += 1;
            self.observations_seen.push(context.observations.len());
            let (candidates, posterior_draws) = if context.observations.is_empty() {
                (
                    vec![
                        candidate("a-primary", "axis-a"),
                        candidate("b-orthogonal", "axis-b"),
                    ],
                    vec![
                        draw(
                            "draw-00",
                            250_000,
                            0,
                            0,
                            vec![1_000_000, 0],
                            vec![1_000_000, 0],
                        ),
                        draw(
                            "draw-01",
                            250_000,
                            0,
                            1_000,
                            vec![1_000_000, 0],
                            vec![0, 1_000_000],
                        ),
                        draw(
                            "draw-10",
                            250_000,
                            1_000,
                            0,
                            vec![0, 1_000_000],
                            vec![1_000_000, 0],
                        ),
                        draw(
                            "draw-11",
                            250_000,
                            1_000,
                            1_000,
                            vec![0, 1_000_000],
                            vec![0, 1_000_000],
                        ),
                    ],
                )
            } else {
                (
                    vec![
                        candidate("a-primary", "axis-a"),
                        candidate("b-orthogonal", "axis-b"),
                    ],
                    vec![
                        draw(
                            "updated-00",
                            500_000,
                            0,
                            0,
                            vec![500_000, 500_000],
                            vec![1_000_000, 0],
                        ),
                        draw(
                            "updated-01",
                            500_000,
                            0,
                            1_000,
                            vec![500_000, 500_000],
                            vec![0, 1_000_000],
                        ),
                    ],
                )
            };
            Ok(PosteriorBatchModelState {
                candidates,
                targets: targets(),
                posterior_draws,
            })
        }
    }

    #[derive(Default)]
    struct ReturningAssays {
        calls: Vec<String>,
    }

    impl PosteriorBatchCampaignExecutor for ReturningAssays {
        fn execute_assay(
            &mut self,
            candidate: &PosteriorBatchCandidate,
            round: u16,
        ) -> Result<PosteriorBatchAssayOutcome, PosteriorBatchProviderFailure> {
            self.calls.push(candidate.candidate_id.clone());
            let observation_id = format!("obs-{round}-{}", candidate.candidate_id);
            Ok(PosteriorBatchAssayOutcome {
                observation_id: observation_id.clone(),
                candidate_id: candidate.candidate_id.clone(),
                // Null/negative outcomes still update the next posterior.
                outcome_bin: 0,
                artifact: LocalArtifactRef {
                    artifact_id: observation_id.clone(),
                    content_hash: ContentHash::of_bytes(observation_id.as_bytes()),
                    content_type: candidate.output_schema.clone(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
        }
    }

    #[test]
    fn updates_posterior_after_each_returned_outcome_and_selects_new_axis() {
        let mut model = UpdatingModel::default();
        let mut executor = ReturningAssays::default();
        let campaign =
            execute_glioma_posterior_batch_campaign(&request(), &mut model, &mut executor)
                .expect("closed-loop assay campaign");

        assert_eq!(model.observations_seen, vec![0, 1]);
        assert_eq!(executor.calls, vec!["a-primary", "b-orthogonal"]);
        assert_eq!(campaign.observations.len(), 2);
        assert_eq!(campaign.rounds.len(), 2);
        assert_eq!(
            campaign.stop_reason,
            PosteriorBatchCampaignStopReason::BudgetExhausted
        );
        assert_eq!(campaign.budget_spent_units, 2);
        campaign.validate().expect("sealed campaign validates");
    }

    struct FailingAssay;

    impl PosteriorBatchCampaignExecutor for FailingAssay {
        fn execute_assay(
            &mut self,
            _candidate: &PosteriorBatchCandidate,
            _round: u16,
        ) -> Result<PosteriorBatchAssayOutcome, PosteriorBatchProviderFailure> {
            Err(PosteriorBatchProviderFailure {
                reason: "assay gateway lost acknowledgement".into(),
            })
        }
    }

    #[test]
    fn ambiguous_failure_is_charged_and_not_retried() {
        let mut model = UpdatingModel::default();
        let campaign =
            execute_glioma_posterior_batch_campaign(&request(), &mut model, &mut FailingAssay)
                .expect("executor failure is a terminal campaign result");

        assert_eq!(
            campaign.stop_reason,
            PosteriorBatchCampaignStopReason::ExecutorFailed
        );
        assert_eq!(
            campaign.disposition,
            PosteriorBatchCampaignDisposition::Failed
        );
        assert_eq!(campaign.budget_spent_units, 1);
        assert_eq!(campaign.rounds[0].attempted_order.len(), 1);
        assert!(campaign.rounds[0].observed_order.is_empty());
        campaign.validate().expect("failure stop validates");
    }
}
