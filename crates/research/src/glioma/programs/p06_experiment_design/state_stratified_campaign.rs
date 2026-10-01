//! State- and clone-stratified adaptive assay campaigns for preclinical glioma research.
//!
//! A separate mechanism posterior is maintained for each declared biological stratum (such as a
//! clone, cell state, or model). The campaign selects and runs one assay at a time, then updates
//! only the stratum that produced it. Nulls are valid measurements. An outcome assigned zero
//! probability by every declared mechanism is retained as out-of-model evidence and stops the
//! campaign rather than being smoothed into false support.

use super::adaptive_information_campaign::{
    plan_glioma_adaptive_information_campaign, AdaptiveInformationCampaignDisposition,
    AdaptiveInformationCampaignError, AdaptiveInformationCampaignRequest,
    AdaptiveInformationObservation, AdaptiveMechanismPosterior,
};
use super::information_design::{DesignAction, DesignMechanism};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

// This is a state-stratified execution extension of the existing P06-F19 adaptive information
// campaign, not a second claim on a portfolio slot.
const FEATURE_ID: &str = super::adaptive_information_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaStateStratifiedCampaign1@4";
pub const MAX_STRATA: usize = 128;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_ROUNDS: u16 = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaResearchStratum {
    pub stratum_id: String,
    pub label: String,
    /// Predeclared share of scientific priority; positive weights sum to 1,000.
    pub priority_weight_milli: u16,
    /// Priors are local to this stratum and are never inferred from another stratum's outcome.
    pub mechanism_priors: Vec<DesignMechanism>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratifiedAssayCandidate {
    pub stratum_id: String,
    pub action: DesignAction,
    /// These declared null/negative bins remain valid outcomes and update the posterior normally.
    pub negative_outcome_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratifiedAssayObservation {
    pub stratum_id: String,
    pub action_id: String,
    pub outcome_id: String,
    pub replicate_index: u16,
    pub artifact: crate::glioma_engine::LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedStratifiedObservation {
    pub stratum_id: String,
    pub action_id: String,
    pub outcome_id: String,
    pub replicate_index: u16,
    pub negative_result: bool,
    pub out_of_model: bool,
    pub artifact: crate::glioma_engine::LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateStratifiedCampaignRequest {
    pub campaign: AdaptiveInformationCampaignRequest,
    /// Hard exploration floor before optimizing weighted information gain; not a replication claim.
    pub min_assays_per_stratum: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateStratifiedStopReason {
    AllStrataConverged,
    OutOfModelOutcome,
    CandidateModelUnresolved,
    CoverageBlocked,
    BudgetExhausted,
    NoInformativeAssays,
    MaxRounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateStratifiedDisposition {
    Completed,
    Partial,
    EvidenceBlocked,
    BudgetExhausted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateStratifiedRound {
    pub round: u16,
    pub stratum_id: String,
    pub action_id: String,
    pub replicate_index: u16,
    /// True when this assay was admitted to meet the declared per-stratum coverage floor,
    /// including when the mechanism posterior was already concentrated.
    pub coverage_floor_assay: bool,
    pub expected_information_gain_milli: u64,
    /// Expected realized operator-variance reduction across the recorded acquisition horizon.
    pub expected_operator_variance_reduction_milli: Option<u16>,
    pub operator_reduction_per_cost_million: Option<u64>,
    pub minimum_predicted_effective_sample_fraction_milli: Option<u16>,
    /// Number of sequential assays represented by the acquisition selector's horizon.
    pub acquisition_horizon_assays: Option<u8>,
    pub posterior_before: Vec<AdaptiveMechanismPosterior>,
    pub posterior_after: Vec<AdaptiveMechanismPosterior>,
    pub outcome_id: String,
    pub negative_result: bool,
    pub out_of_model: bool,
    pub candidate_model_update_issue: Option<String>,
    pub cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateStratumPosterior {
    pub stratum_id: String,
    pub label: String,
    pub priority_weight_milli: u16,
    pub valid_assay_count: u32,
    pub posterior: Vec<AdaptiveMechanismPosterior>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateSpecificAcquisitionPriority {
    /// Number of sequential assays represented by this score (1 for immediate-greedy policies).
    pub horizon_assays: u8,
    /// Expected variance reduction as a milli-fraction of the selector's complete objective.
    pub expected_variance_reduction_milli: u16,
    /// Expected realized variance reduction per expected total cost, feasibility adjusted.
    pub reduction_per_cost_million: u64,
    pub minimum_predicted_effective_sample_fraction_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateSpecificOutcomeUpdate {
    Updated,
    Unresolved { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateStratifiedCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: crate::glioma_engine::GliomaModelSystem,
    pub stratum_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub rounds: Vec<StateStratifiedRound>,
    pub observations: Vec<RecordedStratifiedObservation>,
    pub final_posteriors: Vec<StateStratumPosterior>,
    pub negative_result_order: Vec<String>,
    pub out_of_model_order: Vec<String>,
    pub budget_units: u64,
    pub budget_spent_units: u64,
    pub budget_remaining_units: u64,
    pub stop_reason: StateStratifiedStopReason,
    pub disposition: StateStratifiedDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateStratifiedExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay gateway or deterministic preclinical simulator.
/// The campaign calls once per selected assay and never blindly retries a scientific measurement.
pub trait GliomaStateStratifiedAssayExecutor {
    fn execute_action(
        &mut self,
        stratum_id: &str,
        action: &DesignAction,
        round: u16,
    ) -> Result<AdaptiveInformationObservation, StateStratifiedExecutionFailure>;

    /// Opt into a calibrated, candidate-specific second objective. Defaults preserve the existing
    /// mechanism-information selector for every existing executor.
    fn uses_candidate_specific_acquisition(&self) -> bool {
        false
    }

    fn candidate_specific_acquisition_priority(
        &self,
        _stratum_id: &str,
        _action: &DesignAction,
    ) -> Option<CandidateSpecificAcquisitionPriority> {
        None
    }

    /// Score a feasible set in one pass so bounded adaptive rollout can share its work across
    /// candidate choices. Generic executors retain their one-step behavior by default.
    fn candidate_specific_acquisition_priorities(
        &self,
        candidates: &[StratifiedAssayCandidate],
        _remaining_budget_units: u64,
        _remaining_rounds: u16,
    ) -> Result<BTreeMap<(String, String), CandidateSpecificAcquisitionPriority>, String> {
        Ok(candidates
            .iter()
            .filter_map(|candidate| {
                self.candidate_specific_acquisition_priority(
                    &candidate.stratum_id,
                    &candidate.action,
                )
                .map(|priority| {
                    (
                        (
                            candidate.stratum_id.clone(),
                            candidate.action.action_id.clone(),
                        ),
                        priority,
                    )
                })
            })
            .collect())
    }

    /// Assimilate the exact outcome after the ordinary P06 posterior has recorded it. A failed
    /// secondary update preserves the measured result but stops automation without mislabeling the
    /// observation as impossible under the P06 mechanism model.
    fn assimilate_candidate_specific_outcome(
        &mut self,
        _stratum_id: &str,
        _action: &DesignAction,
        _outcome_id: &str,
    ) -> Result<CandidateSpecificOutcomeUpdate, String> {
        Ok(CandidateSpecificOutcomeUpdate::Updated)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateStratifiedCampaignError {
    #[error("state-stratified campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("state-stratified campaign input is invalid: {0}")]
    InvalidInput(String),
    #[error("state-stratified campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("state-stratified assay executor failed: {0}")]
    Executor(String),
    #[error("state-stratified campaign digest failed: {0}")]
    Digest(String),
}

fn digest_input(run: &StateStratifiedCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id, "output_schema": run.output_schema,
        "objective": run.objective, "model_system": run.model_system,
        "stratum_order": run.stratum_order, "candidate_order": run.candidate_order,
        "selected_order": run.selected_order, "rounds": run.rounds,
        "observations": run.observations, "final_posteriors": run.final_posteriors,
        "negative_result_order": run.negative_result_order,
        "out_of_model_order": run.out_of_model_order,
        "budget_units": run.budget_units, "budget_spent_units": run.budget_spent_units,
        "budget_remaining_units": run.budget_remaining_units, "stop_reason": run.stop_reason,
        "disposition": run.disposition, "next_step": run.next_step,
    })
}

impl StateStratifiedCampaign {
    pub fn validate(&self) -> Result<(), StateStratifiedCampaignError> {
        let selected = self
            .rounds
            .iter()
            .map(|round| round.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.stratum_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .candidate_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || selected != self.selected_order
            || self.final_posteriors.len() != self.stratum_order.len()
            || self
                .final_posteriors
                .iter()
                .map(|item| item.stratum_id.clone())
                .collect::<Vec<_>>()
                != self.stratum_order
            || self.rounds.len() > MAX_ROUNDS as usize
            || self
                .budget_spent_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
            || self
                .negative_result_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .out_of_model_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.rounds.iter().any(|round| {
                round.stratum_id.trim().is_empty()
                    || round.action_id.trim().is_empty()
                    || round.outcome_id.trim().is_empty()
                    || round
                        .expected_operator_variance_reduction_milli
                        .is_some_and(|score| score > 1_000)
                    || round.expected_operator_variance_reduction_milli.is_some()
                        != round.operator_reduction_per_cost_million.is_some()
                    || round.expected_operator_variance_reduction_milli.is_some()
                        != round
                            .minimum_predicted_effective_sample_fraction_milli
                            .is_some()
                    || round.expected_operator_variance_reduction_milli.is_some()
                        != round.acquisition_horizon_assays.is_some()
                    || round
                        .acquisition_horizon_assays
                        .is_some_and(|horizon| !(1..=2).contains(&horizon))
                    || round
                        .minimum_predicted_effective_sample_fraction_milli
                        .is_some_and(|score| score > 1_000)
                    || round
                        .candidate_model_update_issue
                        .as_ref()
                        .is_some_and(|issue| issue.trim().is_empty())
            })
            || self.next_step.trim().is_empty()
        {
            return Err(StateStratifiedCampaignError::InvalidOutput(
                "identity, ordering, posterior partitions, budget, or campaign bounds are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| StateStratifiedCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(StateStratifiedCampaignError::InvalidOutput(
                "campaign digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

type StateStratumIndex = BTreeMap<String, usize>;

fn validate_inputs(
    request: &StateStratifiedCampaignRequest,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial: &[StratifiedAssayObservation],
) -> Result<(StateStratumIndex, StateStratumIndex), StateStratifiedCampaignError> {
    if request.min_assays_per_stratum == 0
        || request.min_assays_per_stratum > request.campaign.max_rounds
        || request.campaign.max_rounds > MAX_ROUNDS
        || strata.is_empty()
        || strata.len() > MAX_STRATA
        || candidates.is_empty()
        || candidates.len() > MAX_ACTIONS
    {
        return Err(StateStratifiedCampaignError::InvalidRequest(
            "positive coverage floor, bounded campaign, strata, and assay candidates are required"
                .into(),
        ));
    }
    let mut stratum_map = BTreeMap::new();
    let mut priority_sum = 0_u32;
    for (index, stratum) in strata.iter().enumerate() {
        if stratum.stratum_id.trim().is_empty()
            || stratum.label.trim().is_empty()
            || stratum.priority_weight_milli == 0
            || stratum.mechanism_priors.len() < 2
            || stratum_map
                .insert(stratum.stratum_id.clone(), index)
                .is_some()
        {
            return Err(StateStratifiedCampaignError::InvalidInput(
                "strata require unique ids, labels, positive priority, and at least two mechanisms"
                    .into(),
            ));
        }
        priority_sum += u32::from(stratum.priority_weight_milli);
    }
    if priority_sum != 1_000 {
        return Err(StateStratifiedCampaignError::InvalidInput(
            "stratum priority weights must sum to 1,000".into(),
        ));
    }
    let mut action_map = BTreeMap::new();
    let mut action_strata = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if !stratum_map.contains_key(&candidate.stratum_id)
            || candidate.action.action_id.trim().is_empty()
            || action_map
                .insert(candidate.action.action_id.clone(), index)
                .is_some()
            || candidate
                .negative_outcome_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(StateStratifiedCampaignError::InvalidInput(
                "assay ids must be unique, belong to a declared stratum, and use canonical null-outcome ids".into(),
            ));
        }
        let outcome_ids = candidate
            .action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate
            .negative_outcome_order
            .iter()
            .any(|id| !outcome_ids.contains(id))
        {
            return Err(StateStratifiedCampaignError::InvalidInput(
                "a declared null/negative outcome is absent from its assay model".into(),
            ));
        }
        action_strata.insert(
            candidate.action.action_id.clone(),
            candidate.stratum_id.clone(),
        );
    }
    if strata.iter().any(|stratum| {
        !candidates
            .iter()
            .any(|candidate| candidate.stratum_id == stratum.stratum_id)
    }) {
        return Err(StateStratifiedCampaignError::InvalidInput(
            "each biological stratum needs at least one executable assay candidate".into(),
        ));
    }
    let mut observation_keys = BTreeSet::new();
    for observation in initial {
        if action_strata.get(&observation.action_id) != Some(&observation.stratum_id)
            || observation.replicate_index == 0
            || !observation_keys
                .insert((observation.action_id.clone(), observation.replicate_index))
        {
            return Err(StateStratifiedCampaignError::InvalidInput(
                "initial observations must reference their action's stratum and have unique positive replicate indexes".into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| StateStratifiedCampaignError::InvalidInput(error.to_string()))?;
    }
    Ok((stratum_map, action_map))
}

fn recorded_observation(
    observation: StratifiedAssayObservation,
    candidate: &StratifiedAssayCandidate,
    stratum: &GliomaResearchStratum,
) -> Result<RecordedStratifiedObservation, StateStratifiedCampaignError> {
    let outcome = candidate
        .action
        .outcomes
        .iter()
        .find(|outcome| outcome.outcome_id == observation.outcome_id)
        .ok_or_else(|| {
            StateStratifiedCampaignError::InvalidInput(format!(
                "assay {} returned undeclared outcome {}",
                observation.action_id, observation.outcome_id
            ))
        })?;
    let out_of_model = stratum.mechanism_priors.iter().all(|mechanism| {
        outcome
            .probability_milli_by_mechanism
            .get(&mechanism.mechanism_id)
            .copied()
            .unwrap_or(0)
            == 0
    });
    Ok(RecordedStratifiedObservation {
        stratum_id: observation.stratum_id,
        action_id: observation.action_id,
        negative_result: candidate
            .negative_outcome_order
            .binary_search(&observation.outcome_id)
            .is_ok(),
        outcome_id: observation.outcome_id,
        replicate_index: observation.replicate_index,
        out_of_model,
        artifact: observation.artifact,
    })
}

fn as_campaign_observations(
    observations: &[RecordedStratifiedObservation],
    stratum_id: &str,
) -> Vec<AdaptiveInformationObservation> {
    observations
        .iter()
        .filter(|observation| observation.stratum_id == stratum_id && !observation.out_of_model)
        .map(|observation| AdaptiveInformationObservation {
            action_id: observation.action_id.clone(),
            outcome_id: observation.outcome_id.clone(),
            replicate_index: observation.replicate_index,
            artifact: observation.artifact.clone(),
        })
        .collect()
}

fn local_plan(
    request: &StateStratifiedCampaignRequest,
    stratum: &GliomaResearchStratum,
    candidates: &[StratifiedAssayCandidate],
    observations: &[RecordedStratifiedObservation],
    spent_units: u64,
) -> Result<
    super::adaptive_information_campaign::AdaptiveInformationCampaignPlan,
    StateStratifiedCampaignError,
> {
    let actions = candidates
        .iter()
        .filter(|candidate| candidate.stratum_id == stratum.stratum_id)
        .map(|candidate| candidate.action.clone())
        .collect::<Vec<_>>();
    let observed = as_campaign_observations(observations, &stratum.stratum_id);
    let candidate_costs = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.action.action_id.as_str(),
                u64::from(candidate.action.cost_units),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let local_spent = observed
        .iter()
        .map(|observation| {
            candidate_costs
                .get(observation.action_id.as_str())
                .copied()
                .unwrap_or(0)
        })
        .sum::<u64>();
    let mut local_request = request.campaign.clone();
    local_request.max_actions_per_round = 1;
    // The one-stratum planner sees the same remaining budget as the whole campaign.
    local_request.budget_units =
        local_spent.saturating_add(request.campaign.budget_units.saturating_sub(spent_units));
    plan_glioma_adaptive_information_campaign(
        &local_request,
        &stratum.mechanism_priors,
        &actions,
        &observed,
    )
    .map_err(|error: AdaptiveInformationCampaignError| {
        StateStratifiedCampaignError::InvalidInput(error.to_string())
    })
}

/// Run a bounded adaptive assay campaign without pooling posteriors across glioma states/clones.
pub fn execute_glioma_state_stratified_campaign<E: GliomaStateStratifiedAssayExecutor>(
    request: &StateStratifiedCampaignRequest,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial_observations: &[StratifiedAssayObservation],
    executor: &mut E,
) -> Result<StateStratifiedCampaign, StateStratifiedCampaignError> {
    let (stratum_indices, action_indices) =
        validate_inputs(request, strata, candidates, initial_observations)?;
    let strata_by_id = strata
        .iter()
        .map(|item| (item.stratum_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let candidates_by_id = candidates
        .iter()
        .map(|item| (item.action.action_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut spent = initial_observations
        .iter()
        .map(|item| u64::from(candidates_by_id[item.action_id.as_str()].action.cost_units))
        .sum::<u64>();
    if spent > request.campaign.budget_units {
        return Err(StateStratifiedCampaignError::InvalidInput(
            "initial assay outcomes exceed the campaign-wide budget".into(),
        ));
    }
    let mut observations = initial_observations
        .iter()
        .map(|item| {
            let candidate = candidates_by_id[item.action_id.as_str()];
            let stratum = strata_by_id[item.stratum_id.as_str()];
            recorded_observation(item.clone(), candidate, stratum)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Validate each stratum's categorical likelihood model and its valid historical outcomes.
    for stratum in strata {
        let local = as_campaign_observations(&observations, &stratum.stratum_id);
        let local_cost = local
            .iter()
            .map(|item| u64::from(candidates_by_id[item.action_id.as_str()].action.cost_units))
            .sum::<u64>();
        let mut probe = request.clone();
        probe.campaign.budget_units = local_cost
            .saturating_add(request.campaign.budget_units.saturating_sub(spent))
            .max(1);
        let actions = candidates
            .iter()
            .filter(|item| item.stratum_id == stratum.stratum_id)
            .map(|item| item.action.clone())
            .collect::<Vec<_>>();
        plan_glioma_adaptive_information_campaign(
            &probe.campaign,
            &stratum.mechanism_priors,
            &actions,
            &local,
        )
        .map_err(|error| StateStratifiedCampaignError::InvalidInput(error.to_string()))?;
    }

    let mut rounds = Vec::new();
    let mut selected_order = Vec::new();
    let mut stop_reason = StateStratifiedStopReason::MaxRounds;
    if observations.iter().any(|item| item.out_of_model) {
        stop_reason = StateStratifiedStopReason::OutOfModelOutcome;
    } else {
        for round_number in 1..=request.campaign.max_rounds {
            if spent >= request.campaign.budget_units {
                stop_reason = StateStratifiedStopReason::BudgetExhausted;
                break;
            }
            let counts = strata
                .iter()
                .map(|stratum| {
                    (
                        stratum.stratum_id.clone(),
                        observations
                            .iter()
                            .filter(|item| {
                                item.stratum_id == stratum.stratum_id && !item.out_of_model
                            })
                            .count(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let floor_active = counts
                .values()
                .any(|count| *count < usize::from(request.min_assays_per_stratum));
            let least_covered = counts.values().copied().min().unwrap_or(0);
            let mut choices = Vec::new();
            let mut all_converged = true;
            let acquisition_mode = executor.uses_candidate_specific_acquisition();
            let remaining_budget = request.campaign.budget_units.saturating_sub(spent);
            let acquisition_priorities = if acquisition_mode {
                let acquisition_candidates = candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.action.risk_milli <= request.campaign.risk_ceiling_milli
                            && u64::from(candidate.action.cost_units) <= remaining_budget
                            && observations
                                .iter()
                                .filter(|item| item.action_id == candidate.action.action_id)
                                .count()
                                < usize::from(candidate.action.max_replicates)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                // During minimum-coverage enforcement, do not plan a speculative follow-up that
                // could violate the campaign's least-covered-stratum rule. Once every stratum
                // meets its floor, score the full cross-stratum candidate set together.
                let remaining_rounds = if floor_active {
                    1
                } else {
                    request.campaign.max_rounds.saturating_sub(round_number - 1)
                };
                executor
                    .candidate_specific_acquisition_priorities(
                        &acquisition_candidates,
                        remaining_budget,
                        remaining_rounds,
                    )
                    .map_err(|reason| {
                        StateStratifiedCampaignError::InvalidRequest(format!(
                            "candidate-specific acquisition could not score the bounded rollout: {reason}"
                        ))
                    })?
            } else {
                BTreeMap::new()
            };
            for stratum in strata {
                let plan = local_plan(request, stratum, candidates, &observations, spent)?;
                all_converged &=
                    plan.disposition == AdaptiveInformationCampaignDisposition::Converged;
                if floor_active && counts[&stratum.stratum_id] != least_covered {
                    continue;
                }
                if acquisition_mode {
                    let choices_before_stratum = choices.len();
                    let mut state_has_acquisition_value = false;
                    for score in &plan.scores {
                        let Some(&candidate_index) = action_indices.get(&score.action_id) else {
                            return Err(StateStratifiedCampaignError::InvalidOutput(
                                "adaptive planner scored an unknown assay".into(),
                            ));
                        };
                        let candidate = &candidates[candidate_index];
                        if candidate.stratum_id != stratum.stratum_id
                            || candidate.action.risk_milli > request.campaign.risk_ceiling_milli
                            || u64::from(candidate.action.cost_units)
                                > request.campaign.budget_units.saturating_sub(spent)
                            || observations
                                .iter()
                                .filter(|item| item.action_id == candidate.action.action_id)
                                .count()
                                >= usize::from(candidate.action.max_replicates)
                        {
                            continue;
                        }
                        let Some(acquisition) = acquisition_priorities
                            .get(&(
                                stratum.stratum_id.clone(),
                                candidate.action.action_id.clone(),
                            ))
                            .copied()
                        else {
                            continue;
                        };
                        if acquisition.expected_variance_reduction_milli > 1_000
                            || acquisition.minimum_predicted_effective_sample_fraction_milli > 1_000
                        {
                            return Err(StateStratifiedCampaignError::InvalidOutput(
                                "candidate-specific acquisition score is outside its declared scale".into(),
                            ));
                        }
                        if acquisition.expected_variance_reduction_milli > 0 {
                            state_has_acquisition_value = true;
                        }
                        if !floor_active && acquisition.expected_variance_reduction_milli == 0 {
                            continue;
                        }
                        choices.push((
                            stratum,
                            candidate,
                            acquisition.reduction_per_cost_million,
                            score.expected_information_gain_milli,
                            plan.posterior_order.clone(),
                            floor_active,
                            Some(acquisition),
                            score.utility_milli,
                        ));
                    }
                    if state_has_acquisition_value {
                        all_converged = false;
                    }
                    if floor_active && choices.len() == choices_before_stratum {
                        let coverage_candidate = plan
                            .action_order
                            .iter()
                            .filter_map(|action_id| {
                                candidates_by_id.get(action_id.as_str()).copied()
                            })
                            .filter(|candidate| {
                                candidate.stratum_id == stratum.stratum_id
                                    && candidate.action.risk_milli
                                        <= request.campaign.risk_ceiling_milli
                                    && u64::from(candidate.action.cost_units)
                                        <= request.campaign.budget_units.saturating_sub(spent)
                                    && observations
                                        .iter()
                                        .filter(|item| item.action_id == candidate.action.action_id)
                                        .count()
                                        < usize::from(candidate.action.max_replicates)
                                    && acquisition_priorities.contains_key(&(
                                        stratum.stratum_id.clone(),
                                        candidate.action.action_id.clone(),
                                    ))
                            })
                            .min_by(|left, right| {
                                left.action
                                    .risk_milli
                                    .cmp(&right.action.risk_milli)
                                    .then_with(|| {
                                        right
                                            .action
                                            .feasibility_milli
                                            .cmp(&left.action.feasibility_milli)
                                    })
                                    .then_with(|| {
                                        left.action.cost_units.cmp(&right.action.cost_units)
                                    })
                                    .then_with(|| {
                                        left.action.action_id.cmp(&right.action.action_id)
                                    })
                            });
                        if let Some(candidate) = coverage_candidate {
                            let coverage_acquisition_priority = acquisition_priorities
                                .get(&(
                                    stratum.stratum_id.clone(),
                                    candidate.action.action_id.clone(),
                                ))
                                .copied();
                            choices.push((
                                stratum,
                                candidate,
                                u64::from(candidate.action.feasibility_milli),
                                0,
                                plan.posterior_order.clone(),
                                true,
                                coverage_acquisition_priority,
                                u64::from(candidate.action.feasibility_milli),
                            ));
                        }
                    }
                } else if let Some(action_id) = plan.next_action_order.first() {
                    let score = plan
                        .scores
                        .iter()
                        .find(|score| score.action_id == *action_id)
                        .ok_or_else(|| {
                            StateStratifiedCampaignError::InvalidOutput(
                                "adaptive planner omitted the selected assay score".into(),
                            )
                        })?;
                    let candidate_index = *action_indices.get(action_id).ok_or_else(|| {
                        StateStratifiedCampaignError::InvalidOutput(
                            "adaptive planner selected an unknown assay".into(),
                        )
                    })?;
                    choices.push((
                        stratum,
                        &candidates[candidate_index],
                        score.utility_milli,
                        score.expected_information_gain_milli,
                        plan.posterior_order,
                        floor_active,
                        None,
                        score.utility_milli,
                    ));
                } else if floor_active {
                    // Convergence answers the mechanism-discrimination question; it does not
                    // satisfy the separate requirement to observe every declared stratum. When
                    // the coverage floor is still active, admit the safest, most feasible,
                    // lowest-cost eligible local assay even if its expected information gain is
                    // below the normal optimization threshold. This is explicitly labeled in the
                    // run so coverage evidence cannot be mistaken for an information-optimal pick.
                    let coverage_candidate = plan
                        .action_order
                        .iter()
                        .filter_map(|action_id| candidates_by_id.get(action_id.as_str()).copied())
                        .filter(|candidate| {
                            candidate.action.risk_milli <= request.campaign.risk_ceiling_milli
                                && u64::from(candidate.action.cost_units)
                                    <= request.campaign.budget_units.saturating_sub(spent)
                        })
                        .min_by(|left, right| {
                            left.action
                                .risk_milli
                                .cmp(&right.action.risk_milli)
                                .then_with(|| {
                                    right
                                        .action
                                        .feasibility_milli
                                        .cmp(&left.action.feasibility_milli)
                                })
                                .then_with(|| left.action.cost_units.cmp(&right.action.cost_units))
                                .then_with(|| left.action.action_id.cmp(&right.action.action_id))
                        });
                    if let Some(candidate) = coverage_candidate {
                        choices.push((
                            stratum,
                            candidate,
                            u64::from(candidate.action.feasibility_milli),
                            0,
                            plan.posterior_order,
                            true,
                            None,
                            u64::from(candidate.action.feasibility_milli),
                        ));
                    }
                }
            }
            if all_converged && !floor_active {
                stop_reason = StateStratifiedStopReason::AllStrataConverged;
                break;
            }
            if choices.is_empty() {
                stop_reason = if floor_active {
                    StateStratifiedStopReason::CoverageBlocked
                } else {
                    StateStratifiedStopReason::NoInformativeAssays
                };
                break;
            }
            choices.sort_by(|left, right| {
                let left_value = u128::from(left.2) * u128::from(left.0.priority_weight_milli);
                let right_value = u128::from(right.2) * u128::from(right.0.priority_weight_milli);
                right_value
                    .cmp(&left_value)
                    .then_with(|| right.3.cmp(&left.3))
                    .then_with(|| right.7.cmp(&left.7))
                    .then_with(|| left.1.action.action_id.cmp(&right.1.action.action_id))
                    .then_with(|| left.0.stratum_id.cmp(&right.0.stratum_id))
            });
            let (
                stratum,
                candidate,
                _,
                expected_information_gain_milli,
                posterior_before,
                coverage_floor_assay,
                candidate_acquisition_priority,
                _,
            ) = choices.remove(0);
            let replicate_index = observations
                .iter()
                .filter(|item| item.action_id == candidate.action.action_id)
                .count()
                .saturating_add(1) as u16;
            let raw = executor
                .execute_action(&stratum.stratum_id, &candidate.action, round_number)
                .map_err(|failure| {
                    StateStratifiedCampaignError::Executor(if failure.retryable {
                        format!(
                            "{} (adapter marked retryable; campaign did not auto-retry)",
                            failure.reason
                        )
                    } else {
                        failure.reason
                    })
                })?;
            if raw.action_id != candidate.action.action_id || raw.replicate_index != replicate_index
            {
                return Err(StateStratifiedCampaignError::InvalidOutput(
                    "executor returned a different action or non-contiguous replicate index".into(),
                ));
            }
            raw.artifact
                .validate()
                .map_err(|error| StateStratifiedCampaignError::InvalidOutput(error.to_string()))?;
            let record = recorded_observation(
                StratifiedAssayObservation {
                    stratum_id: stratum.stratum_id.clone(),
                    action_id: raw.action_id.clone(),
                    outcome_id: raw.outcome_id.clone(),
                    replicate_index: raw.replicate_index,
                    artifact: raw.artifact.clone(),
                },
                candidate,
                stratum,
            )?;
            let candidate_model_update_issue =
                if !record.out_of_model && executor.uses_candidate_specific_acquisition() {
                    match executor.assimilate_candidate_specific_outcome(
                        &stratum.stratum_id,
                        &candidate.action,
                        &record.outcome_id,
                    ) {
                        Ok(CandidateSpecificOutcomeUpdate::Updated) => None,
                        Ok(CandidateSpecificOutcomeUpdate::Unresolved { reason }) => Some(reason),
                        Err(reason) => Some(reason),
                    }
                } else {
                    None
                };
            spent = spent.saturating_add(u64::from(candidate.action.cost_units));
            selected_order.push(candidate.action.action_id.clone());
            observations.push(record.clone());
            let posterior_after = if record.out_of_model {
                posterior_before.clone()
            } else {
                local_plan(request, stratum, candidates, &observations, spent)?.posterior_order
            };
            rounds.push(StateStratifiedRound {
                round: round_number,
                stratum_id: stratum.stratum_id.clone(),
                action_id: candidate.action.action_id.clone(),
                replicate_index,
                coverage_floor_assay,
                expected_information_gain_milli,
                expected_operator_variance_reduction_milli: candidate_acquisition_priority
                    .map(|priority| priority.expected_variance_reduction_milli),
                operator_reduction_per_cost_million: candidate_acquisition_priority
                    .map(|priority| priority.reduction_per_cost_million),
                minimum_predicted_effective_sample_fraction_milli: candidate_acquisition_priority
                    .map(|priority| priority.minimum_predicted_effective_sample_fraction_milli),
                acquisition_horizon_assays: candidate_acquisition_priority
                    .map(|priority| priority.horizon_assays),
                posterior_before,
                posterior_after,
                outcome_id: record.outcome_id.clone(),
                negative_result: record.negative_result,
                out_of_model: record.out_of_model,
                candidate_model_update_issue: candidate_model_update_issue.clone(),
                cost_units: candidate.action.cost_units,
            });
            if record.out_of_model {
                stop_reason = StateStratifiedStopReason::OutOfModelOutcome;
                break;
            }
            if candidate_model_update_issue.is_some() {
                stop_reason = StateStratifiedStopReason::CandidateModelUnresolved;
                break;
            }
            if round_number == request.campaign.max_rounds {
                stop_reason = StateStratifiedStopReason::MaxRounds;
            }
        }
    }

    if stop_reason == StateStratifiedStopReason::MaxRounds
        && strata.iter().any(|stratum| {
            observations
                .iter()
                .filter(|item| item.stratum_id == stratum.stratum_id && !item.out_of_model)
                .count()
                < usize::from(request.min_assays_per_stratum)
        })
    {
        stop_reason = StateStratifiedStopReason::CoverageBlocked;
    }

    let mut final_posteriors = Vec::new();
    for stratum in strata {
        let plan = local_plan(request, stratum, candidates, &observations, spent)?;
        final_posteriors.push(StateStratumPosterior {
            stratum_id: stratum.stratum_id.clone(),
            label: stratum.label.clone(),
            priority_weight_milli: stratum.priority_weight_milli,
            valid_assay_count: observations
                .iter()
                .filter(|item| item.stratum_id == stratum.stratum_id && !item.out_of_model)
                .count() as u32,
            posterior: plan.posterior_order,
            uncertainty: plan.uncertainty,
        });
    }
    final_posteriors.sort_by(|left, right| left.stratum_id.cmp(&right.stratum_id));
    let mut negative_result_order = observations
        .iter()
        .filter(|item| item.negative_result)
        .map(|item| {
            format!(
                "{}:{}:{}",
                item.stratum_id, item.action_id, item.replicate_index
            )
        })
        .collect::<Vec<_>>();
    let mut out_of_model_order = observations
        .iter()
        .filter(|item| item.out_of_model)
        .map(|item| {
            format!(
                "{}:{}:{}",
                item.stratum_id, item.action_id, item.replicate_index
            )
        })
        .collect::<Vec<_>>();
    negative_result_order.sort();
    out_of_model_order.sort();
    let disposition = match stop_reason {
        StateStratifiedStopReason::AllStrataConverged => StateStratifiedDisposition::Completed,
        StateStratifiedStopReason::OutOfModelOutcome
        | StateStratifiedStopReason::CandidateModelUnresolved
        | StateStratifiedStopReason::CoverageBlocked => StateStratifiedDisposition::EvidenceBlocked,
        StateStratifiedStopReason::BudgetExhausted => StateStratifiedDisposition::BudgetExhausted,
        StateStratifiedStopReason::NoInformativeAssays | StateStratifiedStopReason::MaxRounds => {
            StateStratifiedDisposition::Partial
        }
    };
    let next_step = match stop_reason {
        StateStratifiedStopReason::AllStrataConverged => "route stratum-specific mechanism findings to independent preclinical validation",
        StateStratifiedStopReason::OutOfModelOutcome => "revise the mechanism/outcome model; the observed result was impossible under every declared mechanism",
        StateStratifiedStopReason::CandidateModelUnresolved => "preserve the assay result, expand or recalibrate the candidate-specific model ensemble, and resume only after its posterior update is stable",
        StateStratifiedStopReason::CoverageBlocked => "add a safe, informative assay for each under-covered glioma stratum before pooling conclusions",
        StateStratifiedStopReason::BudgetExhausted => "resume only with an explicit additional local assay budget",
        StateStratifiedStopReason::NoInformativeAssays => "expand or recalibrate competing mechanism predictions; do not interpret a tie as evidence of equivalence",
        StateStratifiedStopReason::MaxRounds => "continue the bounded campaign or route unresolved strata to orthogonal validation",
    }.to_owned();
    let mut output = StateStratifiedCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.campaign.objective.clone(),
        model_system: request.campaign.model_system,
        stratum_order: stratum_indices.keys().cloned().collect(),
        candidate_order: action_indices.keys().cloned().collect(),
        selected_order,
        rounds,
        observations,
        final_posteriors,
        negative_result_order,
        out_of_model_order,
        budget_units: request.campaign.budget_units,
        budget_spent_units: spent,
        budget_remaining_units: request.campaign.budget_units.saturating_sub(spent),
        stop_reason,
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-state-stratified-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| StateStratifiedCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::information_design::DesignOutcome;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.preclinical-assay+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn stratum(id: &str, weight: u16) -> GliomaResearchStratum {
        GliomaResearchStratum {
            stratum_id: id.into(),
            label: format!("{id} glioma model stratum"),
            priority_weight_milli: weight,
            mechanism_priors: vec![
                DesignMechanism {
                    mechanism_id: "m-a".into(),
                    prior_milli: 500,
                },
                DesignMechanism {
                    mechanism_id: "m-b".into(),
                    prior_milli: 500,
                },
            ],
        }
    }

    fn assay(id: &str, stratum_id: &str, null_a: u16, null_b: u16) -> StratifiedAssayCandidate {
        let outcome = |outcome_id: &str, a: u16, b: u16| DesignOutcome {
            outcome_id: outcome_id.into(),
            label: outcome_id.into(),
            probability_milli_by_mechanism: [("m-a".into(), a), ("m-b".into(), b)].into(),
        };
        StratifiedAssayCandidate {
            stratum_id: stratum_id.into(),
            action: DesignAction {
                action_id: id.into(),
                feature_id: format!("feature-{id}"),
                label: format!("{id} orthogonal perturbation assay"),
                outcomes: vec![
                    outcome("null", null_a, null_b),
                    outcome("signal", 1_000 - null_a, 1_000 - null_b),
                ],
                feasibility_milli: 1_000,
                risk_milli: 0,
                cost_units: 1,
                max_replicates: 1,
            },
            negative_outcome_order: vec!["null".into()],
        }
    }

    fn request(max_rounds: u16, min_assays_per_stratum: u16) -> StateStratifiedCampaignRequest {
        StateStratifiedCampaignRequest {
            campaign: AdaptiveInformationCampaignRequest {
                objective:
                    "distinguish preclinical glioma mechanisms without averaging cell states".into(),
                model_system: GliomaModelSystem::Organoid,
                max_rounds,
                max_actions_per_round: 4,
                budget_units: 16,
                min_information_gain_milli: 1,
                information_weight_milli: 1_000,
                feasibility_weight_milli: 0,
                risk_penalty_milli: 0,
                cost_penalty_milli: 0,
                risk_ceiling_milli: 1_000,
                stop_concentration_milli: 1_000,
            },
            min_assays_per_stratum,
        }
    }

    struct QueueExecutor {
        outcomes: VecDeque<String>,
        observed_strata: Vec<String>,
    }

    impl GliomaStateStratifiedAssayExecutor for QueueExecutor {
        fn execute_action(
            &mut self,
            stratum_id: &str,
            action: &DesignAction,
            _: u16,
        ) -> Result<AdaptiveInformationObservation, StateStratifiedExecutionFailure> {
            self.observed_strata.push(stratum_id.into());
            Ok(AdaptiveInformationObservation {
                action_id: action.action_id.clone(),
                outcome_id: self.outcomes.pop_front().expect("scripted assay outcome"),
                replicate_index: 1,
                artifact: artifact(&action.action_id),
            })
        }
    }

    struct AcquisitionBatchProbe {
        outcomes: VecDeque<String>,
        candidate_batches: RefCell<Vec<Vec<(String, String)>>>,
    }

    impl GliomaStateStratifiedAssayExecutor for AcquisitionBatchProbe {
        fn execute_action(
            &mut self,
            stratum_id: &str,
            action: &DesignAction,
            _: u16,
        ) -> Result<AdaptiveInformationObservation, StateStratifiedExecutionFailure> {
            Ok(AdaptiveInformationObservation {
                action_id: action.action_id.clone(),
                outcome_id: self.outcomes.pop_front().expect("scripted assay outcome"),
                replicate_index: 1,
                artifact: artifact(&format!("{stratum_id}:{}", action.action_id)),
            })
        }

        fn uses_candidate_specific_acquisition(&self) -> bool {
            true
        }

        fn candidate_specific_acquisition_priorities(
            &self,
            candidates: &[StratifiedAssayCandidate],
            _: u64,
            _: u16,
        ) -> Result<BTreeMap<(String, String), CandidateSpecificAcquisitionPriority>, String>
        {
            self.candidate_batches.borrow_mut().push(
                candidates
                    .iter()
                    .map(|candidate| {
                        (
                            candidate.stratum_id.clone(),
                            candidate.action.action_id.clone(),
                        )
                    })
                    .collect(),
            );
            Ok(candidates
                .iter()
                .map(|candidate| {
                    (
                        (
                            candidate.stratum_id.clone(),
                            candidate.action.action_id.clone(),
                        ),
                        CandidateSpecificAcquisitionPriority {
                            horizon_assays: 1,
                            expected_variance_reduction_milli: 500,
                            reduction_per_cost_million: 500_000_000,
                            minimum_predicted_effective_sample_fraction_milli: 1_000,
                        },
                    )
                })
                .collect())
        }
    }

    #[test]
    fn candidate_specific_selector_receives_all_state_strata_together() {
        let strata = vec![stratum("mes_like", 500), stratum("npc_like", 500)];
        let candidates = vec![
            assay("assay-mes", "mes_like", 500, 500),
            assay("assay-npc", "npc_like", 500, 500),
        ];
        let mut executor = AcquisitionBatchProbe {
            outcomes: ["signal", "signal"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            candidate_batches: RefCell::new(Vec::new()),
        };

        let run = execute_glioma_state_stratified_campaign(
            &request(2, 1),
            &strata,
            &candidates,
            &[],
            &mut executor,
        )
        .unwrap();

        let batches = executor.candidate_batches.borrow();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 2);
        assert!(batches[0].contains(&("mes_like".into(), "assay-mes".into())));
        assert!(batches[0].contains(&("npc_like".into(), "assay-npc".into())));
        assert_eq!(run.selected_order.len(), 2);
        assert!(run
            .rounds
            .iter()
            .take(2)
            .all(|round| round.coverage_floor_assay));
        assert_eq!(run.stop_reason, StateStratifiedStopReason::MaxRounds);
    }

    struct UnresolvedAcquisitionExecutor {
        inner: QueueExecutor,
    }

    impl GliomaStateStratifiedAssayExecutor for UnresolvedAcquisitionExecutor {
        fn execute_action(
            &mut self,
            stratum_id: &str,
            action: &DesignAction,
            round: u16,
        ) -> Result<AdaptiveInformationObservation, StateStratifiedExecutionFailure> {
            self.inner.execute_action(stratum_id, action, round)
        }

        fn uses_candidate_specific_acquisition(&self) -> bool {
            true
        }

        fn candidate_specific_acquisition_priority(
            &self,
            _stratum_id: &str,
            _action: &DesignAction,
        ) -> Option<CandidateSpecificAcquisitionPriority> {
            Some(CandidateSpecificAcquisitionPriority {
                horizon_assays: 1,
                expected_variance_reduction_milli: 500,
                reduction_per_cost_million: 500_000_000,
                minimum_predicted_effective_sample_fraction_milli: 400,
            })
        }

        fn assimilate_candidate_specific_outcome(
            &mut self,
            _stratum_id: &str,
            _action: &DesignAction,
            _outcome_id: &str,
        ) -> Result<CandidateSpecificOutcomeUpdate, String> {
            Ok(CandidateSpecificOutcomeUpdate::Unresolved {
                reason: "candidate posterior effective sample size fell below the release gate"
                    .into(),
            })
        }
    }

    #[test]
    fn valid_assay_with_depleted_secondary_posterior_is_preserved_as_unresolved_not_out_of_model() {
        let strata = vec![stratum("npc-state", 1_000)];
        let candidates = vec![assay("npc-assay", "npc-state", 900, 100)];
        let mut executor = UnresolvedAcquisitionExecutor {
            inner: QueueExecutor {
                outcomes: ["signal"].into_iter().map(str::to_owned).collect(),
                observed_strata: Vec::new(),
            },
        };
        let run = execute_glioma_state_stratified_campaign(
            &request(1, 1),
            &strata,
            &candidates,
            &[],
            &mut executor,
        )
        .unwrap();

        assert_eq!(
            run.stop_reason,
            StateStratifiedStopReason::CandidateModelUnresolved
        );
        assert_eq!(run.disposition, StateStratifiedDisposition::EvidenceBlocked);
        assert_eq!(run.observations.len(), 1);
        assert!(!run.observations[0].out_of_model);
        assert!(!run.rounds[0].out_of_model);
        assert!(run.rounds[0]
            .candidate_model_update_issue
            .as_deref()
            .is_some_and(|issue| issue.contains("effective sample size")));
        assert_eq!(
            run.rounds[0].minimum_predicted_effective_sample_fraction_milli,
            Some(400)
        );
        run.validate().unwrap();
    }

    #[test]
    fn candidate_specific_objective_can_select_an_assay_with_zero_mechanism_information_gain() {
        let strata = vec![stratum("npc-state", 1_000)];
        let candidates = vec![
            assay("historical", "npc-state", 900, 100),
            assay("npc-assay", "npc-state", 500, 500),
        ];
        let initial = vec![StratifiedAssayObservation {
            stratum_id: "npc-state".into(),
            action_id: "historical".into(),
            outcome_id: "signal".into(),
            replicate_index: 1,
            artifact: artifact("historical"),
        }];
        let mut executor = UnresolvedAcquisitionExecutor {
            inner: QueueExecutor {
                outcomes: ["signal"].into_iter().map(str::to_owned).collect(),
                observed_strata: Vec::new(),
            },
        };
        let run = execute_glioma_state_stratified_campaign(
            &request(2, 1),
            &strata,
            &candidates,
            &initial,
            &mut executor,
        )
        .unwrap();

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.rounds[0].action_id, "npc-assay");
        assert_eq!(run.rounds[0].expected_information_gain_milli, 0);
        assert_eq!(
            run.stop_reason,
            StateStratifiedStopReason::CandidateModelUnresolved
        );
        run.validate().unwrap();
    }

    #[test]
    fn null_updates_its_own_stratum_but_does_not_change_an_unmeasured_stratum() {
        let strata = vec![stratum("mes-state", 700), stratum("npc-state", 300)];
        let candidates = vec![
            assay("mes-screen", "mes-state", 900, 100),
            assay("mes-followup", "mes-state", 800, 200),
            assay("npc-screen", "npc-state", 900, 100),
        ];
        let mut executor = QueueExecutor {
            outcomes: ["null"].into_iter().map(str::to_owned).collect(),
            observed_strata: Vec::new(),
        };
        let run = execute_glioma_state_stratified_campaign(
            &request(1, 1),
            &strata,
            &candidates,
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.rounds[0].stratum_id, "mes-state");
        assert!(run.rounds[0].negative_result);
        assert_eq!(run.rounds[0].posterior_after[0].mechanism_id, "m-a");
        assert_eq!(run.rounds[0].posterior_after[0].posterior_milli, 900);
        assert_eq!(run.final_posteriors[0].posterior[0].posterior_milli, 900);
        assert_eq!(run.final_posteriors[1].posterior[0].posterior_milli, 500);
        assert_eq!(run.negative_result_order.len(), 1);
        run.validate().unwrap();
    }

    #[test]
    fn coverage_floor_samples_low_priority_strata_before_optimizing_gain() {
        let strata = vec![stratum("high-priority", 999), stratum("low-priority", 1)];
        let candidates = vec![
            assay("high-assay", "high-priority", 900, 100),
            assay("low-assay", "low-priority", 900, 100),
        ];
        let mut executor = QueueExecutor {
            outcomes: ["signal", "null"].into_iter().map(str::to_owned).collect(),
            observed_strata: Vec::new(),
        };
        let run = execute_glioma_state_stratified_campaign(
            &request(2, 1),
            &strata,
            &candidates,
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(executor.observed_strata.len(), 2);
        assert_eq!(
            executor
                .observed_strata
                .iter()
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        assert_eq!(
            run.final_posteriors
                .iter()
                .map(|state| state.valid_assay_count)
                .collect::<Vec<_>>(),
            vec![1, 1]
        );
        assert!(run.rounds.iter().all(|round| round.coverage_floor_assay));
    }

    #[test]
    fn coverage_floor_runs_a_local_assay_even_when_prior_is_already_concentrated() {
        let mut confident = stratum("mes-state", 1_000);
        confident.mechanism_priors[0].prior_milli = 900;
        confident.mechanism_priors[1].prior_milli = 100;
        let mut campaign_request = request(2, 1);
        campaign_request.campaign.stop_concentration_milli = 700;
        let mut executor = QueueExecutor {
            outcomes: ["null"].into_iter().map(str::to_owned).collect(),
            observed_strata: Vec::new(),
        };

        let run = execute_glioma_state_stratified_campaign(
            &campaign_request,
            &[confident],
            &[assay("mes-validation", "mes-state", 900, 100)],
            &[],
            &mut executor,
        )
        .unwrap();

        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].coverage_floor_assay);
        assert_eq!(run.rounds[0].expected_information_gain_milli, 0);
        assert_eq!(
            run.stop_reason,
            StateStratifiedStopReason::AllStrataConverged
        );
        assert_eq!(run.final_posteriors[0].valid_assay_count, 1);
        run.validate().unwrap();
    }

    #[test]
    fn unmet_coverage_at_the_round_limit_is_reported_as_blocked() {
        let strata = vec![stratum("mes-state", 999), stratum("npc-state", 1)];
        let mut executor = QueueExecutor {
            outcomes: ["signal"].into_iter().map(str::to_owned).collect(),
            observed_strata: Vec::new(),
        };

        let run = execute_glioma_state_stratified_campaign(
            &request(1, 1),
            &strata,
            &[
                assay("mes-screen", "mes-state", 900, 100),
                assay("npc-screen", "npc-state", 900, 100),
            ],
            &[],
            &mut executor,
        )
        .unwrap();

        assert_eq!(run.stop_reason, StateStratifiedStopReason::CoverageBlocked);
        assert_eq!(run.disposition, StateStratifiedDisposition::EvidenceBlocked);
        assert!(run.next_step.contains("under-covered"));
    }

    #[test]
    fn impossible_outcome_stops_and_preserves_prior_instead_of_smoothing() {
        let strata = vec![stratum("clone-a", 1_000)];
        let mut candidate = assay("unexpected-assay", "clone-a", 0, 0);
        candidate.action.outcomes.push(DesignOutcome {
            outcome_id: "other-model".into(),
            label: "other model".into(),
            probability_milli_by_mechanism: [("m-a".into(), 1_000), ("m-b".into(), 0)].into(),
        });
        candidate.action.outcomes[1]
            .probability_milli_by_mechanism
            .insert("m-a".into(), 0);
        candidate.action.outcomes[1]
            .probability_milli_by_mechanism
            .insert("m-b".into(), 1_000);
        let mut executor = QueueExecutor {
            outcomes: ["null"].into_iter().map(str::to_owned).collect(),
            observed_strata: Vec::new(),
        };
        let run = execute_glioma_state_stratified_campaign(
            &request(4, 1),
            &strata,
            &[candidate],
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            run.stop_reason,
            StateStratifiedStopReason::OutOfModelOutcome
        );
        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].out_of_model);
        assert_eq!(
            run.final_posteriors[0]
                .posterior
                .iter()
                .map(|item| item.posterior_milli)
                .collect::<Vec<_>>(),
            vec![500, 500]
        );
    }
}
