//! Integer-only Bayesian information design with beam-selected assay panels for preclinical glioma assays.
//!
//! This feature chooses the next assay batch from investigator-declared finite outcome models. It
//! supports both expected mechanism-Gini reduction and expected reduction in pairwise predictive
//! disagreement over the declared assay panel. The latter is a one-step finite categorical
//! acquisition score; it is not the published continuous/noisy PDBAL algorithm. The planner then
//! applies feasibility, risk, cost, and budget gates. It does not invent outcomes, assume causal
//! effects, dispatch an assay, or make a clinical recommendation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaInformationDesign1@2";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_OUTCOMES_PER_ACTION: usize = 128;
pub const MAX_PREDICTIVE_DIAMETER_WORK_TERMS: u64 = 10_000_000;
pub const MAX_PRIOR_MILLI: u16 = 1_000;
pub const SCORE_SCALE: u64 = 1_000;
const INFORMATION_BATCH_BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InformationAcquisitionObjective {
    MechanismGini,
    PanelPredictiveDiameter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InformationDesignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u64,
    pub max_selected_actions: usize,
    pub min_information_gain_milli: u64,
    pub information_weight_milli: u16,
    pub feasibility_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignMechanism {
    pub mechanism_id: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignOutcome {
    pub outcome_id: String,
    pub label: String,
    pub probability_milli_by_mechanism: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignAction {
    pub action_id: String,
    pub feature_id: String,
    pub label: String,
    pub outcomes: Vec<DesignOutcome>,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub max_replicates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InformationDesignActionScore {
    pub action_id: String,
    pub feature_id: String,
    pub label: String,
    pub prior_gini_milli: u64,
    pub expected_posterior_gini_milli: u64,
    pub expected_information_gain_milli: u64,
    pub prior_predictive_diameter_milli: u64,
    pub expected_posterior_predictive_diameter_milli: u64,
    pub expected_predictive_diameter_reduction_milli: u64,
    pub predictive_metric_truncated_milli: bool,
    pub predictive_outcome_count: usize,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub utility_milli: u64,
    pub selected_replicates: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InformationDesignDisposition {
    Qualified,
    BudgetBlocked,
    NoInformativeActions,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InformationDesignPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub acquisition_objective: InformationAcquisitionObjective,
    pub mechanism_order: Vec<String>,
    pub action_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub scores: Vec<InformationDesignActionScore>,
    pub prior_gini_milli: u64,
    pub prior_predictive_diameter_milli: u64,
    pub predictive_panel_digest: ContentHash,
    pub budget_remaining_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: InformationDesignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InformationDesignError {
    #[error("information design request is invalid: {0}")]
    InvalidRequest(String),
    #[error("information design input is invalid: {0}")]
    InvalidInput(String),
    #[error("information design output is invalid: {0}")]
    InvalidOutput(String),
    #[error(
        "posterior-predictive acquisition requires {required} work terms; configured limit is {limit}"
    )]
    PredictiveWorkBudgetExceeded { required: u64, limit: u64 },
    #[error("information design digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &InformationDesignPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "acquisition_objective": output.acquisition_objective,
        "mechanism_order": output.mechanism_order,
        "action_order": output.action_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "scores": output.scores,
        "prior_gini_milli": output.prior_gini_milli,
        "prior_predictive_diameter_milli": output.prior_predictive_diameter_milli,
        "predictive_panel_digest": output.predictive_panel_digest,
        "budget_remaining_units": output.budget_remaining_units,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

impl InformationDesignPlan {
    pub fn validate(&self) -> Result<(), InformationDesignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.mechanism_order.len() < 2
            || !canonical(&self.mechanism_order)
            || !canonical(&self.action_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.scores.len() != self.action_order.len()
        {
            return Err(InformationDesignError::InvalidOutput(
                "identity, canonical ordering, or action-score cardinality is invalid".into(),
            ));
        }
        if let Some(score) = self.scores.iter().find(|score| {
            score.action_id.trim().is_empty()
                || score.feature_id.trim().is_empty()
                || score.label.trim().is_empty()
                || score.prior_gini_milli > SCORE_SCALE
                || score.expected_posterior_gini_milli > SCORE_SCALE
                || score.expected_information_gain_milli > SCORE_SCALE
                || score.prior_predictive_diameter_milli > SCORE_SCALE
                || score.expected_posterior_predictive_diameter_milli > SCORE_SCALE
                || score.expected_predictive_diameter_reduction_milli > SCORE_SCALE
                || score.feasibility_milli > 1_000
                || score.risk_milli > 1_000
                || score.cost_units == 0
                || score.utility_milli > SCORE_SCALE
                || score.prior_predictive_diameter_milli != self.prior_predictive_diameter_milli
                || score.expected_predictive_diameter_reduction_milli
                    != score
                        .prior_predictive_diameter_milli
                        .saturating_sub(score.expected_posterior_predictive_diameter_milli)
                || score.rationale.trim().is_empty()
        }) {
            return Err(InformationDesignError::InvalidOutput(format!(
                "action score fields are inconsistent or out of bounds: {score:?}"
            )));
        }
        if self.scores.windows(2).any(|pair| {
            pair[0].utility_milli < pair[1].utility_milli
                || (pair[0].utility_milli == pair[1].utility_milli
                    && (acquisition_gain(&pair[0], self.acquisition_objective)
                        < acquisition_gain(&pair[1], self.acquisition_objective)
                        || (acquisition_gain(&pair[0], self.acquisition_objective)
                            == acquisition_gain(&pair[1], self.acquisition_objective)
                            && pair[0].action_id > pair[1].action_id)))
        }) {
            return Err(InformationDesignError::InvalidOutput(
                "action scores are not ordered by utility, selected acquisition score, and action id".into(),
            ));
        }
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<BTreeSet<_>>();
        let action_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        if score_ids != action_ids
            || self
                .selected_order
                .iter()
                .any(|id| !action_ids.contains(id))
            || self
                .deferred_order
                .iter()
                .any(|id| !action_ids.contains(id))
            || self.selected_order.iter().any(|id| {
                self.scores
                    .iter()
                    .find(|score| &score.action_id == id)
                    .is_none_or(|score| score.selected_replicates == 0)
            })
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .collect::<BTreeSet<_>>()
                != action_ids.iter().collect::<BTreeSet<_>>()
        {
            return Err(InformationDesignError::InvalidOutput(
                "action score, selected, and deferred partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InformationDesignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InformationDesignError::InvalidOutput(
                "information design digest is not bound to the ranked plan".into(),
            ));
        }
        Ok(())
    }
}

fn gini_milli(probabilities: impl Iterator<Item = u16>) -> u64 {
    let values = probabilities.map(u64::from).collect::<Vec<_>>();
    let total = values
        .iter()
        .copied()
        .fold(0_u64, |sum, value| sum.saturating_add(value));
    if total == 0 {
        return 0;
    }
    // The caller supplies a complete 1000-milli distribution. Keeping the calculation generic
    // makes the invariant obvious and permits a conservative result if a future caller changes
    // the normalization scale.
    let squared = values.iter().fold(0_u128, |sum, value| {
        sum.saturating_add(u128::from(*value).saturating_mul(u128::from(*value)))
    });
    let total_squared = u128::from(total).saturating_mul(u128::from(total));
    ((total_squared.saturating_sub(squared) * u128::from(SCORE_SCALE)) / total_squared.max(1))
        as u64
}

fn gini_from_mass(masses: &[u64], total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    let total_squared = u128::from(total).saturating_mul(u128::from(total));
    let squared = masses.iter().fold(0_u128, |sum, value| {
        sum.saturating_add(u128::from(*value).saturating_mul(u128::from(*value)))
    });
    ((total_squared.saturating_sub(squared) * u128::from(SCORE_SCALE)) / total_squared.max(1))
        as u64
}

fn expected_information(
    mechanisms: &[DesignMechanism],
    action: &DesignAction,
) -> (u64, u64, usize) {
    let prior_gini = gini_milli(mechanisms.iter().map(|mechanism| mechanism.prior_milli));
    let mut expected_posterior = 0_u64;
    let mut predictive_outcome_count = 0;
    for outcome in &action.outcomes {
        let mut joint = Vec::with_capacity(mechanisms.len());
        for mechanism in mechanisms {
            let conditional = outcome
                .probability_milli_by_mechanism
                .get(&mechanism.mechanism_id)
                .copied()
                .unwrap_or(0);
            joint.push(u64::from(mechanism.prior_milli) * u64::from(conditional));
        }
        let outcome_mass = joint.iter().copied().sum::<u64>();
        if outcome_mass == 0 {
            continue;
        }
        predictive_outcome_count += 1;
        let posterior_gini = gini_from_mass(&joint, outcome_mass);
        expected_posterior = expected_posterior
            .saturating_add(posterior_gini.saturating_mul(outcome_mass) / 1_000_000);
    }
    (
        prior_gini,
        expected_posterior.min(SCORE_SCALE),
        predictive_outcome_count,
    )
}

fn predictive_work_terms(mechanism_count: usize, actions: &[DesignAction]) -> u64 {
    let mechanism_pairs = (mechanism_count * (mechanism_count - 1) / 2) as u64;
    let outcome_count = actions
        .iter()
        .map(|action| action.outcomes.len() as u64)
        .sum::<u64>();
    // One pass builds pairwise assay-panel distances; the second evaluates all posterior
    // predictive diameters over candidate outcomes.
    mechanism_pairs * outcome_count * 2
}

fn panel_pairwise_distances(
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
) -> (Vec<Vec<u16>>, bool) {
    let mut accumulated = vec![vec![0_u64; mechanisms.len()]; mechanisms.len()];
    for action in actions {
        for left in 0..mechanisms.len() {
            for right in (left + 1)..mechanisms.len() {
                let absolute_difference = action
                    .outcomes
                    .iter()
                    .map(|outcome| {
                        let left_probability = u64::from(
                            outcome.probability_milli_by_mechanism[&mechanisms[left].mechanism_id],
                        );
                        let right_probability = u64::from(
                            outcome.probability_milli_by_mechanism[&mechanisms[right].mechanism_id],
                        );
                        left_probability.abs_diff(right_probability)
                    })
                    .sum::<u64>();
                // Total variation is one half of the L1 distance between categorical
                // distributions. Since each distribution sums to 1000, this is integral.
                accumulated[left][right] += absolute_difference / 2;
            }
        }
    }
    let action_count = actions.len() as u64;
    let truncated = (0..mechanisms.len()).any(|left| {
        ((left + 1)..mechanisms.len()).any(|right| accumulated[left][right] % action_count != 0)
    });
    let distances = accumulated
        .into_iter()
        .enumerate()
        .map(|(left, row)| {
            row.into_iter()
                .enumerate()
                .map(|(right, sum)| {
                    if right <= left {
                        0
                    } else {
                        (sum / action_count) as u16
                    }
                })
                .collect()
        })
        .collect();
    (distances, truncated)
}

fn predictive_diameter_from_mass(masses: &[u64], pairwise_distances: &[Vec<u16>]) -> (u64, bool) {
    let total = masses.iter().copied().sum::<u64>();
    if total == 0 {
        return (0, false);
    }
    let mut disagreement_mass = 0_u128;
    for left in 0..masses.len() {
        for right in (left + 1)..masses.len() {
            disagreement_mass += 2
                * u128::from(masses[left])
                * u128::from(masses[right])
                * u128::from(pairwise_distances[left][right]);
        }
    }
    let total_squared = u128::from(total) * u128::from(total);
    // `pairwise_distances` already uses milli units, so this ratio remains milli-scaled.
    (
        (disagreement_mass / total_squared) as u64,
        disagreement_mass % total_squared != 0,
    )
}

fn expected_predictive_diameter(
    mechanisms: &[DesignMechanism],
    action: &DesignAction,
    pairwise_distances: &[Vec<u16>],
) -> (u64, usize, bool) {
    let mut expected_mass = 0_u128;
    let mut predictive_outcome_count = 0;
    let mut truncated = false;
    for outcome in &action.outcomes {
        let joint = mechanisms
            .iter()
            .map(|mechanism| {
                u64::from(mechanism.prior_milli)
                    * u64::from(outcome.probability_milli_by_mechanism[&mechanism.mechanism_id])
            })
            .collect::<Vec<_>>();
        let outcome_mass = joint.iter().copied().sum::<u64>();
        if outcome_mass == 0 {
            continue;
        }
        predictive_outcome_count += 1;
        let (posterior_diameter, posterior_truncated) =
            predictive_diameter_from_mass(&joint, pairwise_distances);
        truncated |= posterior_truncated;
        expected_mass += u128::from(posterior_diameter) * u128::from(outcome_mass);
    }
    (
        (expected_mass / 1_000_000).min(u128::from(SCORE_SCALE)) as u64,
        predictive_outcome_count,
        truncated || expected_mass % 1_000_000 != 0,
    )
}

fn acquisition_gain(
    score: &InformationDesignActionScore,
    objective: InformationAcquisitionObjective,
) -> u64 {
    match objective {
        InformationAcquisitionObjective::MechanismGini => score.expected_information_gain_milli,
        InformationAcquisitionObjective::PanelPredictiveDiameter => {
            score.expected_predictive_diameter_reduction_milli
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InformationBatchState {
    selected: Vec<usize>,
    spent_units: u64,
}

fn outcome_signature_distance(
    left: &DesignAction,
    right: &DesignAction,
    mechanism_order: &[String],
) -> u64 {
    let outcome_order = left
        .outcomes
        .iter()
        .chain(right.outcomes.iter())
        .map(|outcome| outcome.outcome_id.clone())
        .collect::<BTreeSet<_>>();
    let mut distance = 0_u64;
    for mechanism in mechanism_order {
        for outcome_id in &outcome_order {
            let left_probability = left
                .outcomes
                .iter()
                .find(|outcome| &outcome.outcome_id == outcome_id)
                .and_then(|outcome| outcome.probability_milli_by_mechanism.get(mechanism))
                .copied()
                .unwrap_or(0);
            let right_probability = right
                .outcomes
                .iter()
                .find(|outcome| &outcome.outcome_id == outcome_id)
                .and_then(|outcome| outcome.probability_milli_by_mechanism.get(mechanism))
                .copied()
                .unwrap_or(0);
            distance =
                distance.saturating_add(u64::from(left_probability.abs_diff(right_probability)));
        }
    }
    let denominator = (mechanism_order.len() as u64).saturating_mul(2_000).max(1);
    (distance.saturating_mul(SCORE_SCALE) / denominator).min(SCORE_SCALE)
}

fn information_batch_score(
    state: &InformationBatchState,
    eligible: &[(&DesignAction, &InformationDesignActionScore)],
    mechanism_order: &[String],
) -> u64 {
    let base = state
        .selected
        .iter()
        .map(|index| eligible[*index].1.utility_milli.saturating_mul(SCORE_SCALE))
        .sum::<u64>();
    let complementarity = state
        .selected
        .iter()
        .enumerate()
        .flat_map(|(position, left)| {
            state.selected.iter().skip(position + 1).map(|right| {
                outcome_signature_distance(eligible[*left].0, eligible[*right].0, mechanism_order)
                    .saturating_mul(100)
            })
        })
        .sum::<u64>();
    base.saturating_add(complementarity)
        .saturating_sub(state.spent_units.saturating_mul(10))
}

fn select_information_batch(
    eligible: &[(&DesignAction, &InformationDesignActionScore)],
    mechanism_order: &[String],
    budget_units: u64,
    max_selected_actions: usize,
) -> Vec<String> {
    if eligible.is_empty() || max_selected_actions == 0 {
        return Vec::new();
    }
    let mut states = vec![InformationBatchState {
        selected: Vec::new(),
        spent_units: 0,
    }];
    for index in 0..eligible.len() {
        let action = eligible[index].0;
        let mut next = states.clone();
        for state in &states {
            let spent = state
                .spent_units
                .saturating_add(u64::from(action.cost_units));
            if state.selected.len() >= max_selected_actions || spent > budget_units {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(index);
            next.push(InformationBatchState {
                selected,
                spent_units: spent,
            });
        }
        next.sort_by(|left, right| {
            information_batch_score(right, eligible, mechanism_order)
                .cmp(&information_batch_score(left, eligible, mechanism_order))
                .then_with(|| left.spent_units.cmp(&right.spent_units))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(INFORMATION_BATCH_BEAM_WIDTH);
        states = next;
    }
    let chosen = states
        .into_iter()
        .filter(|state| !state.selected.is_empty())
        .max_by(|left, right| {
            information_batch_score(left, eligible, mechanism_order)
                .cmp(&information_batch_score(right, eligible, mechanism_order))
                .then_with(|| right.spent_units.cmp(&left.spent_units))
                .then_with(|| right.selected.cmp(&left.selected))
        });
    chosen
        .map(|state| {
            state
                .selected
                .into_iter()
                .map(|index| eligible[index].0.action_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn validate_inputs(
    request: &InformationDesignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
) -> Result<(), InformationDesignError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selected_actions == 0
        || request.max_selected_actions > MAX_ACTIONS
        || request.min_information_gain_milli > SCORE_SCALE
        || request.information_weight_milli > 1_000
        || request.feasibility_weight_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || request.cost_penalty_milli > 1_000
        || request.risk_ceiling_milli > 1_000
    {
        return Err(InformationDesignError::InvalidRequest(
            "objective, positive budget/selection bounds, acquisition threshold, and score weights are required".into(),
        ));
    }
    if mechanisms.len() < 2 || mechanisms.len() > MAX_MECHANISMS {
        return Err(InformationDesignError::InvalidInput(
            "at least two and at most 256 mechanisms are required".into(),
        ));
    }
    let mechanism_ids = mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanism_ids.len() != mechanisms.len()
        || mechanisms.iter().any(|mechanism| {
            mechanism.mechanism_id.trim().is_empty()
                || mechanism.prior_milli == 0
                || mechanism.prior_milli > MAX_PRIOR_MILLI
        })
        || mechanisms
            .iter()
            .map(|mechanism| u32::from(mechanism.prior_milli))
            .sum::<u32>()
            != 1_000
    {
        return Err(InformationDesignError::InvalidInput(
            "mechanism ids must be unique and positive priors must sum to 1000".into(),
        ));
    }
    if actions.is_empty() || actions.len() > MAX_ACTIONS {
        return Err(InformationDesignError::InvalidInput(
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
                || action.outcomes.len() < 2
                || action.outcomes.len() > MAX_OUTCOMES_PER_ACTION
                || action.feasibility_milli > 1_000
                || action.risk_milli > 1_000
                || action.cost_units == 0
                || action.max_replicates == 0
                || action.outcomes.iter().any(|outcome| {
                    outcome.outcome_id.trim().is_empty()
                        || outcome.label.trim().is_empty()
                        || outcome.probability_milli_by_mechanism.len() != mechanisms.len()
                        || outcome
                            .probability_milli_by_mechanism
                            .iter()
                            .any(|(id, probability)| {
                                !mechanism_ids.contains(id) || *probability > MAX_PRIOR_MILLI
                            })
                })
        })
    {
        return Err(InformationDesignError::InvalidInput(
            "action identity, outcome distributions, feasibility, risk, cost, and replicate bounds are invalid".into(),
        ));
    }
    for action in actions {
        let outcome_ids = action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.clone())
            .collect::<BTreeSet<_>>();
        if outcome_ids.len() != action.outcomes.len()
            || mechanisms.iter().any(|mechanism| {
                action
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.probability_milli_by_mechanism[&mechanism.mechanism_id])
                    .map(u32::from)
                    .sum::<u32>()
                    != 1_000
            })
        {
            return Err(InformationDesignError::InvalidInput(format!(
                "action {} outcome probabilities must be unique and sum to 1000 for every mechanism",
                action.action_id
            )));
        }
    }
    Ok(())
}

/// Plan a bounded assay batch using the selected finite-model acquisition score.
pub fn plan_glioma_information_design(
    request: &InformationDesignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
) -> Result<InformationDesignPlan, InformationDesignError> {
    plan_glioma_information_design_with_objective(
        request,
        InformationAcquisitionObjective::MechanismGini,
        mechanisms,
        actions,
    )
}

/// Plan a bounded assay batch using an explicitly selected acquisition objective.
///
/// The original `plan_glioma_information_design` entry point remains a mechanism-Gini-compatible
/// route for existing callers. New research workflows can opt into panel predictive diameter here.
pub fn plan_glioma_information_design_with_objective(
    request: &InformationDesignRequest,
    acquisition_objective: InformationAcquisitionObjective,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
) -> Result<InformationDesignPlan, InformationDesignError> {
    validate_inputs(request, mechanisms, actions)?;
    let required_work = predictive_work_terms(mechanisms.len(), actions);
    if required_work > MAX_PREDICTIVE_DIAMETER_WORK_TERMS {
        return Err(InformationDesignError::PredictiveWorkBudgetExceeded {
            required: required_work,
            limit: MAX_PREDICTIVE_DIAMETER_WORK_TERMS,
        });
    }
    let mut ordered_mechanisms = mechanisms.to_vec();
    ordered_mechanisms.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = ordered_mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<Vec<_>>();
    let mut ordered_actions = actions.to_vec();
    ordered_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = ordered_actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let prior_gini = gini_milli(
        ordered_mechanisms
            .iter()
            .map(|mechanism| mechanism.prior_milli),
    );
    let (pairwise_distances, panel_average_truncated) =
        panel_pairwise_distances(&ordered_mechanisms, &ordered_actions);
    let (prior_predictive_diameter, prior_diameter_truncated) = predictive_diameter_from_mass(
        &ordered_mechanisms
            .iter()
            .map(|mechanism| u64::from(mechanism.prior_milli))
            .collect::<Vec<_>>(),
        &pairwise_distances,
    );
    let predictive_panel_digest = ContentHash::of_value(&serde_json::json!({
        "mechanisms": ordered_mechanisms,
        "actions": ordered_actions,
    }))
    .map_err(|error| InformationDesignError::Digest(error.to_string()))?;
    let mut scores = ordered_actions
        .iter()
        .map(|action| {
            let (prior, posterior, predictive_outcome_count) =
                expected_information(&ordered_mechanisms, action);
            let information = prior.saturating_sub(posterior);
            let (
                expected_posterior_predictive_diameter,
                _,
                posterior_diameter_truncated,
            ) = expected_predictive_diameter(&ordered_mechanisms, action, &pairwise_distances);
            let predictive_reduction =
                prior_predictive_diameter.saturating_sub(expected_posterior_predictive_diameter);
            let predictive_metric_truncated_milli = panel_average_truncated
                || prior_diameter_truncated
                || posterior_diameter_truncated;
            let acquisition_score = match acquisition_objective {
                InformationAcquisitionObjective::MechanismGini => information,
                InformationAcquisitionObjective::PanelPredictiveDiameter => predictive_reduction,
            };
            let weighted = (u128::from(request.information_weight_milli)
                * u128::from(acquisition_score)
                + u128::from(request.feasibility_weight_milli)
                    * u128::from(action.feasibility_milli))
                / 1_000;
            let risk_penalty =
                u128::from(request.risk_penalty_milli) * u128::from(action.risk_milli) / 1_000;
            let cost_penalty = u128::from(request.cost_penalty_milli)
                * u128::from(action.cost_units)
                .min(u128::from(SCORE_SCALE));
            let utility = weighted
                .saturating_sub(risk_penalty.saturating_add(cost_penalty))
                .min(u128::from(SCORE_SCALE)) as u64;
            let rationale = if action.risk_milli > request.risk_ceiling_milli {
                "information-bearing assay is held above the declared preclinical risk ceiling"
            } else {
                match acquisition_objective {
                    InformationAcquisitionObjective::MechanismGini => {
                        "assay ranked by expected mechanism-Gini reduction with feasibility, risk, and cost bounds"
                    }
                    InformationAcquisitionObjective::PanelPredictiveDiameter => {
                        "assay ranked by expected reduction in pairwise predictive disagreement over the declared panel, with feasibility, risk, and cost bounds"
                    }
                }
            };
            let rationale = if action.risk_milli <= request.risk_ceiling_milli
                && acquisition_score < request.min_information_gain_milli
            {
                match acquisition_objective {
                    InformationAcquisitionObjective::MechanismGini => {
                        "outcome distributions do not clear the declared mechanism-Gini acquisition gate"
                    }
                    InformationAcquisitionObjective::PanelPredictiveDiameter => {
                        "outcome distributions do not clear the declared panel predictive-diameter acquisition gate"
                    }
                }
            } else {
                rationale
            };
            InformationDesignActionScore {
                action_id: action.action_id.clone(),
                feature_id: action.feature_id.clone(),
                label: action.label.clone(),
                prior_gini_milli: prior,
                expected_posterior_gini_milli: posterior,
                expected_information_gain_milli: information,
                prior_predictive_diameter_milli: prior_predictive_diameter,
                expected_posterior_predictive_diameter_milli:
                    expected_posterior_predictive_diameter,
                expected_predictive_diameter_reduction_milli: predictive_reduction,
                predictive_metric_truncated_milli,
                predictive_outcome_count,
                feasibility_milli: action.feasibility_milli,
                risk_milli: action.risk_milli,
                cost_units: action.cost_units,
                utility_milli: utility,
                selected_replicates: 0,
                rationale: rationale.into(),
            }
        })
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| {
                acquisition_gain(right, acquisition_objective)
                    .cmp(&acquisition_gain(left, acquisition_objective))
            })
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut remaining_budget = request.budget_units;
    let candidate_order = {
        let eligible = scores
            .iter()
            .filter(|score| {
                acquisition_gain(score, acquisition_objective) >= request.min_information_gain_milli
                    && score.risk_milli <= request.risk_ceiling_milli
                    && u64::from(score.cost_units) <= remaining_budget
            })
            .filter_map(|score| {
                ordered_actions
                    .iter()
                    .find(|action| action.action_id == score.action_id)
                    .map(|action| (action, score))
            })
            .collect::<Vec<_>>();
        select_information_batch(
            &eligible,
            &mechanism_order,
            remaining_budget,
            request.max_selected_actions,
        )
    };
    let mut selected_order = Vec::new();
    for action_id in candidate_order {
        let Some(score) = scores.iter_mut().find(|score| score.action_id == action_id) else {
            continue;
        };
        let action = ordered_actions
            .iter()
            .find(|action| action.action_id == action_id)
            .expect("validated action exists");
        let spend = u64::from(action.cost_units);
        if spend > remaining_budget {
            continue;
        }
        score.selected_replicates = 1;
        remaining_budget = remaining_budget.saturating_sub(spend);
        selected_order.push(action_id);
    }
    // Give every selected assay one replicate before spending residual capacity. This preserves
    // the beam's complementary panel instead of letting the first action consume the entire
    // budget through its replicate ceiling.
    for action_id in selected_order.clone() {
        let Some(score) = scores.iter_mut().find(|score| score.action_id == action_id) else {
            continue;
        };
        let action = ordered_actions
            .iter()
            .find(|action| action.action_id == action_id)
            .expect("validated action exists");
        let additional = action
            .max_replicates
            .saturating_sub(score.selected_replicates)
            .min((remaining_budget / u64::from(action.cost_units)) as u16);
        score.selected_replicates = score.selected_replicates.saturating_add(additional);
        remaining_budget = remaining_budget
            .saturating_sub(u64::from(additional).saturating_mul(u64::from(action.cost_units)));
    }
    let selected_set = selected_order.iter().collect::<BTreeSet<_>>();
    let deferred_order = action_order
        .iter()
        .filter(|action_id| !selected_set.contains(action_id))
        .cloned()
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| {
                acquisition_gain(right, acquisition_objective)
                    .cmp(&acquisition_gain(left, acquisition_objective))
            })
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative_evidence: BTreeSet<String> = BTreeSet::new();
    if selected_order.is_empty() {
        if scores.iter().any(|score| {
            acquisition_gain(score, acquisition_objective) >= request.min_information_gain_milli
                && score.risk_milli <= request.risk_ceiling_milli
        }) {
            uncertainty.insert("budget-cannot-fund-an-information-bearing-action".into());
        } else {
            negative_evidence.insert("no-action-clears-declared-acquisition-gain-gate".into());
        }
    }
    if scores
        .iter()
        .any(|score| score.expected_posterior_gini_milli > prior_gini)
    {
        uncertainty.insert("posterior-gini-rounding-bound-is-conservative".into());
    }
    if scores.iter().any(|score| {
        score.expected_posterior_predictive_diameter_milli > score.prior_predictive_diameter_milli
    }) {
        uncertainty.insert("predictive-diameter-rounding-bound-is-conservative".into());
    }
    if scores
        .iter()
        .any(|score| score.predictive_metric_truncated_milli)
    {
        uncertainty.insert("predictive-diameter-score-truncated-at-milli-precision".into());
    }
    if scores
        .iter()
        .any(|score| score.risk_milli > request.risk_ceiling_milli)
    {
        negative_evidence
            .insert("one-or-more-information-bearing-actions-exceed-risk-ceiling".into());
    }
    let disposition = if !selected_order.is_empty() {
        InformationDesignDisposition::Qualified
    } else if uncertainty.iter().any(|item| item.contains("budget")) {
        InformationDesignDisposition::BudgetBlocked
    } else if !negative_evidence.is_empty() {
        InformationDesignDisposition::NoInformativeActions
    } else {
        InformationDesignDisposition::Unresolved
    };
    let mut output = InformationDesignPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        acquisition_objective,
        mechanism_order,
        action_order,
        selected_order,
        deferred_order,
        scores,
        prior_gini_milli: prior_gini,
        prior_predictive_diameter_milli: prior_predictive_diameter,
        predictive_panel_digest,
        budget_remaining_units: remaining_budget,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-information-design"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InformationDesignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn action(id: &str, inverse: bool) -> DesignAction {
        let (egfr_a, matrix_a) = if inverse { (900, 100) } else { (500, 500) };
        let (egfr_b, matrix_b) = if inverse { (100, 900) } else { (500, 500) };
        DesignAction {
            action_id: id.into(),
            feature_id: format!("feature-{id}"),
            label: id.into(),
            outcomes: vec![
                DesignOutcome {
                    outcome_id: "low".into(),
                    label: "low invasion".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("egfr".into(), egfr_a),
                        ("matrix".into(), matrix_a),
                    ]),
                },
                DesignOutcome {
                    outcome_id: "high".into(),
                    label: "high invasion".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("egfr".into(), egfr_b),
                        ("matrix".into(), matrix_b),
                    ]),
                },
            ],
            feasibility_milli: 900,
            risk_milli: 100,
            cost_units: 2,
            max_replicates: 1,
        }
    }

    fn request() -> InformationDesignRequest {
        InformationDesignRequest {
            objective: "select an assay that separates EGFR and matrix invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_units: 4,
            max_selected_actions: 1,
            min_information_gain_milli: 10,
            information_weight_milli: 800,
            feasibility_weight_milli: 200,
            risk_penalty_milli: 100,
            cost_penalty_milli: 0,
            risk_ceiling_milli: 700,
        }
    }

    fn isolating_action(action_id: &str, target: &str) -> DesignAction {
        let ids = ["a", "b", "c"];
        let hit = |id: &str| if id == target { 1_000 } else { 0 };
        DesignAction {
            action_id: action_id.into(),
            feature_id: format!("feature-{action_id}"),
            label: format!("isolate {target}"),
            outcomes: vec![
                DesignOutcome {
                    outcome_id: "hit".into(),
                    label: "target-specific response".into(),
                    probability_milli_by_mechanism: ids
                        .iter()
                        .map(|id| ((*id).to_string(), hit(id)))
                        .collect(),
                },
                DesignOutcome {
                    outcome_id: "miss".into(),
                    label: "other response".into(),
                    probability_milli_by_mechanism: ids
                        .iter()
                        .map(|id| ((*id).to_string(), 1_000 - hit(id)))
                        .collect(),
                },
            ],
            feasibility_milli: 1_000,
            risk_milli: 0,
            cost_units: 1,
            max_replicates: 1,
        }
    }

    #[test]
    fn separating_action_is_selected_for_information_gain() {
        let output = plan_glioma_information_design(
            &request(),
            &mechanisms(),
            &[action("uninformative", false), action("separating", true)],
        )
        .unwrap();
        assert_eq!(output.disposition, InformationDesignDisposition::Qualified);
        assert_eq!(output.selected_order, vec!["separating"]);
        let score = output
            .scores
            .iter()
            .find(|score| score.action_id == "separating")
            .unwrap();
        assert!(score.expected_information_gain_milli > 0);
        output.validate().unwrap();
    }

    #[test]
    fn information_panel_beam_prefers_complementary_outcome_partitions() {
        let left = action("left", true);
        let duplicate = action("duplicate", true);
        let mut complementary = action("complementary", true);
        for outcome in &mut complementary.outcomes {
            let egfr = outcome
                .probability_milli_by_mechanism
                .get("egfr")
                .copied()
                .unwrap();
            let matrix = outcome
                .probability_milli_by_mechanism
                .get("matrix")
                .copied()
                .unwrap();
            outcome
                .probability_milli_by_mechanism
                .insert("egfr".into(), matrix);
            outcome
                .probability_milli_by_mechanism
                .insert("matrix".into(), egfr);
        }
        let score = |id: &str, utility: u64| InformationDesignActionScore {
            action_id: id.into(),
            feature_id: format!("feature-{id}"),
            label: id.into(),
            prior_gini_milli: 500,
            expected_posterior_gini_milli: 100,
            expected_information_gain_milli: 400,
            prior_predictive_diameter_milli: 500,
            expected_posterior_predictive_diameter_milli: 100,
            expected_predictive_diameter_reduction_milli: 400,
            predictive_metric_truncated_milli: false,
            predictive_outcome_count: 2,
            feasibility_milli: 900,
            risk_milli: 100,
            cost_units: 1,
            utility_milli: utility,
            selected_replicates: 0,
            rationale: "test score".into(),
        };
        let scores = [
            score("left", 1_000),
            score("duplicate", 1_000),
            score("complementary", 950),
        ];
        let eligible = vec![
            (&left, &scores[0]),
            (&duplicate, &scores[1]),
            (&complementary, &scores[2]),
        ];
        let selected = select_information_batch(&eligible, &["egfr".into(), "matrix".into()], 4, 2);
        assert_eq!(
            selected,
            vec!["left".to_string(), "complementary".to_string()]
        );
    }

    #[test]
    fn permutation_replays_identically() {
        let first = plan_glioma_information_design(
            &request(),
            &mechanisms(),
            &[action("separating", true), action("uninformative", false)],
        )
        .unwrap();
        let second = plan_glioma_information_design(
            &request(),
            &mechanisms().into_iter().rev().collect::<Vec<_>>(),
            &[action("uninformative", false), action("separating", true)],
        )
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn risk_ceiling_withholds_an_informative_action() {
        let mut request = request();
        request.risk_ceiling_milli = 50;
        let output =
            plan_glioma_information_design(&request, &mechanisms(), &[action("separating", true)])
                .unwrap();
        assert_eq!(
            output.disposition,
            InformationDesignDisposition::NoInformativeActions
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("risk-ceiling")));
    }

    #[test]
    fn predictive_panel_objective_breaks_a_gini_tie_by_remaining_assay_disagreement() {
        let mechanisms = vec![
            DesignMechanism {
                mechanism_id: "a".into(),
                prior_milli: 500,
            },
            DesignMechanism {
                mechanism_id: "b".into(),
                prior_milli: 250,
            },
            DesignMechanism {
                mechanism_id: "c".into(),
                prior_milli: 250,
            },
        ];
        let actions = vec![
            isolating_action("b-candidate", "b"),
            isolating_action("c-candidate", "c"),
            isolating_action("panel-reference-c", "c"),
        ];
        let output = plan_glioma_information_design_with_objective(
            &request(),
            InformationAcquisitionObjective::PanelPredictiveDiameter,
            &mechanisms,
            &actions,
        )
        .unwrap();
        let b_score = output
            .scores
            .iter()
            .find(|score| score.action_id == "b-candidate")
            .unwrap();
        let c_score = output
            .scores
            .iter()
            .find(|score| score.action_id == "c-candidate")
            .unwrap();

        assert_eq!(
            b_score.expected_information_gain_milli, c_score.expected_information_gain_milli,
            "isolating either equally weighted mechanism has equal one-step Gini reduction"
        );
        assert!(
            c_score.expected_predictive_diameter_reduction_milli
                > b_score.expected_predictive_diameter_reduction_milli,
            "the C assay should better reduce disagreement over this explicitly asymmetric panel"
        );
        assert_eq!(c_score.expected_predictive_diameter_reduction_milli, 263);
        assert_eq!(b_score.expected_predictive_diameter_reduction_milli, 152);
        assert!(c_score.predictive_metric_truncated_milli);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| { item == "predictive-diameter-score-truncated-at-milli-precision" }));
        assert_eq!(output.selected_order, vec!["c-candidate"]);
        assert_eq!(
            output.predictive_panel_digest,
            ContentHash::of_value(
                &serde_json::json!({ "mechanisms": mechanisms, "actions": actions })
            )
            .expect("canonical declared panel serializes"),
            "the digest is over the canonical id-sorted model inputs"
        );

        let gini_output =
            plan_glioma_information_design(&request(), &mechanisms, &actions).unwrap();
        assert_eq!(gini_output.selected_order, vec!["b-candidate"]);
    }

    #[test]
    fn impossible_declared_outcome_is_not_counted_as_predictive_evidence() {
        let mut deterministic = action("deterministic", false);
        deterministic.outcomes[0]
            .probability_milli_by_mechanism
            .values_mut()
            .for_each(|probability| *probability = 1_000);
        deterministic.outcomes[1]
            .probability_milli_by_mechanism
            .values_mut()
            .for_each(|probability| *probability = 0);

        let output = plan_glioma_information_design_with_objective(
            &request(),
            InformationAcquisitionObjective::PanelPredictiveDiameter,
            &mechanisms(),
            &[deterministic],
        )
        .unwrap();
        assert_eq!(output.scores[0].predictive_outcome_count, 1);
        assert_eq!(
            output.scores[0].expected_predictive_diameter_reduction_milli,
            0
        );
        assert_eq!(
            output.disposition,
            InformationDesignDisposition::NoInformativeActions
        );
    }

    #[test]
    fn predictive_work_budget_exhaustion_is_a_typed_error_not_a_zero_score() {
        let mechanisms = (0..MAX_MECHANISMS)
            .map(|index| DesignMechanism {
                mechanism_id: format!("m{index:03}"),
                prior_milli: if index == MAX_MECHANISMS - 1 {
                    1_000 - (MAX_MECHANISMS as u16 - 1) * 3
                } else {
                    3
                },
            })
            .collect::<Vec<_>>();
        let actions = (0..100)
            .map(|index| {
                let mut action = action(&format!("a{index:03}"), false);
                action.outcomes.iter_mut().for_each(|outcome| {
                    outcome.probability_milli_by_mechanism = mechanisms
                        .iter()
                        .map(|mechanism| (mechanism.mechanism_id.clone(), 500))
                        .collect();
                });
                action
            })
            .collect::<Vec<_>>();

        assert!(matches!(
            plan_glioma_information_design(&request(), &mechanisms, &actions),
            Err(InformationDesignError::PredictiveWorkBudgetExceeded { .. })
        ));
    }
}
