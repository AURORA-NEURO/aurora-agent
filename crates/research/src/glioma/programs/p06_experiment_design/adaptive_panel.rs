//! Mechanism-aware adaptive assay-panel design for preclinical glioma research.
//!
//! Existing information design ranks one candidate at a time. This product compiles a bounded
//! multi-assay panel: it greedily chooses complementary assays using expected Gini-information
//! reduction, discounts correlated independence groups, and applies explicit feasibility, risk,
//! cost, and budget gates. Candidate outcome distributions are planning declarations; no outcome
//! is invented, observed, or dispatched by this module.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptivePanelDesign1@1";
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_OUTCOMES: usize = 128;
pub const SCORE_SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelMechanism {
    pub mechanism_id: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelOutcome {
    pub outcome_id: String,
    pub probability_milli_by_mechanism: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelAction {
    pub action_id: String,
    pub feature_id: String,
    pub label: String,
    pub independence_group: String,
    pub outcomes: Vec<PanelOutcome>,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub max_replicates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptivePanelRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<PanelMechanism>,
    pub actions: Vec<PanelAction>,
    pub budget_units: u64,
    pub max_selected_actions: usize,
    pub min_information_gain_milli: u64,
    pub min_feasibility_milli: u16,
    pub risk_ceiling_milli: u16,
    pub diversity_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptivePanelActionKind {
    Selected,
    Deferred,
    RiskBlocked,
    FeasibilityBlocked,
    BudgetBlocked,
    Uninformative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptivePanelSelection {
    pub action_id: String,
    pub feature_id: String,
    pub independence_group: String,
    pub expected_information_gain_milli: u64,
    pub diversity_bonus_milli: u64,
    pub utility_milli: u64,
    pub cost_units: u32,
    pub allocated_replicates: u16,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub action: AdaptivePanelActionKind,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptivePanelDisposition {
    Qualified,
    Partial,
    NoInformativeActions,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptivePanelDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub action_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selections: Vec<AdaptivePanelSelection>,
    pub prior_gini_milli: u64,
    pub planned_final_gini_milli: u64,
    pub total_information_gain_milli: u64,
    pub budget_remaining_units: u64,
    pub selected_group_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: AdaptivePanelDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptivePanelError {
    #[error("adaptive panel request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive panel input is invalid: {0}")]
    InvalidInput(String),
    #[error("adaptive panel output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive panel digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AdaptivePanelDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "action_order": output.action_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "selections": output.selections,
        "prior_gini_milli": output.prior_gini_milli,
        "planned_final_gini_milli": output.planned_final_gini_milli,
        "total_information_gain_milli": output.total_information_gain_milli,
        "budget_remaining_units": output.budget_remaining_units,
        "selected_group_order": output.selected_group_order,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &AdaptivePanelRequest) -> Result<(), AdaptivePanelError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.len() < 2
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.budget_units == 0
        || request.max_selected_actions == 0
        || request.max_selected_actions > request.actions.len()
        || request.min_information_gain_milli > SCORE_SCALE
        || request.min_feasibility_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.diversity_weight_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || request.cost_penalty_milli > 1_000
    {
        return Err(AdaptivePanelError::InvalidRequest(
            "objective, mechanism/action bounds, budget, selection, and score bounds are invalid"
                .into(),
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
        return Err(AdaptivePanelError::InvalidInput(
            "mechanisms require unique identifiers and positive priors summing to 1000".into(),
        ));
    }
    let mut action_ids = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || action.feature_id.trim().is_empty()
            || action.label.trim().is_empty()
            || action.independence_group.trim().is_empty()
            || action.outcomes.is_empty()
            || action.outcomes.len() > MAX_OUTCOMES
            || action.feasibility_milli > 1_000
            || action.risk_milli > 1_000
            || action.cost_units == 0
            || action.max_replicates == 0
            || !action_ids.insert(action.action_id.clone())
        {
            return Err(AdaptivePanelError::InvalidInput(
                "actions require unique ids, labels, independence groups, bounded gates, cost, replicates, and outcomes".into(),
            ));
        }
        let mut outcome_ids = BTreeSet::new();
        for outcome in &action.outcomes {
            if outcome.outcome_id.trim().is_empty()
                || !outcome_ids.insert(outcome.outcome_id.clone())
                || outcome.probability_milli_by_mechanism.len() != mechanism_ids.len()
                || outcome
                    .probability_milli_by_mechanism
                    .keys()
                    .any(|mechanism| !mechanism_ids.contains(mechanism))
                || outcome
                    .probability_milli_by_mechanism
                    .values()
                    .any(|probability| *probability > 1_000)
            {
                return Err(AdaptivePanelError::InvalidInput(
                    "outcomes require unique ids, complete mechanism probability rows, and bounded probabilities".into(),
                ));
            }
        }
        for mechanism in &mechanism_ids {
            let sum = action
                .outcomes
                .iter()
                .map(|outcome| {
                    u32::from(
                        outcome
                            .probability_milli_by_mechanism
                            .get(mechanism)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .sum::<u32>();
            if sum != 1_000 {
                return Err(AdaptivePanelError::InvalidInput(
                    "each action must provide an outcome distribution summing to 1000 per mechanism".into(),
                ));
            }
        }
    }
    Ok(())
}

fn gini(probabilities: &[u16]) -> u64 {
    1_000_u64.saturating_sub(
        probabilities
            .iter()
            .map(|probability| u64::from(*probability) * u64::from(*probability) / 1_000)
            .sum::<u64>(),
    )
}

fn expected_information(
    prior: &[u16],
    mechanisms: &[PanelMechanism],
    action: &PanelAction,
) -> (u64, u64) {
    let prior_gini = gini(prior);
    let mut expected_posterior = 0_u64;
    for outcome in &action.outcomes {
        let predictive = mechanisms
            .iter()
            .enumerate()
            .map(|(index, mechanism)| {
                u64::from(prior[index]).saturating_mul(u64::from(
                    outcome
                        .probability_milli_by_mechanism
                        .get(&mechanism.mechanism_id)
                        .copied()
                        .unwrap_or(0),
                )) / 1_000
            })
            .sum::<u64>();
        if predictive == 0 {
            continue;
        }
        let posterior = mechanisms
            .iter()
            .enumerate()
            .map(|(index, mechanism)| {
                u64::from(prior[index]).saturating_mul(u64::from(
                    outcome
                        .probability_milli_by_mechanism
                        .get(&mechanism.mechanism_id)
                        .copied()
                        .unwrap_or(0),
                )) / predictive
            })
            .map(|value| value.min(1_000) as u16)
            .collect::<Vec<_>>();
        expected_posterior =
            expected_posterior.saturating_add(predictive.saturating_mul(gini(&posterior)) / 1_000);
    }
    (prior_gini, expected_posterior.min(SCORE_SCALE))
}

fn validate_output(output: &AdaptivePanelDesign) -> Result<(), AdaptivePanelError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.mechanism_order.len() < 2
        || !canonical(&output.mechanism_order)
        || !canonical(&output.action_order)
        || !canonical(&output.selected_order)
        || !canonical(&output.deferred_order)
        || !canonical(&output.blocked_order)
        || !canonical(&output.selected_group_order)
        || !canonical(&output.uncertainty)
        || !canonical(&output.negative_evidence)
        || output.selections.len() != output.action_order.len()
        || output.total_information_gain_milli > SCORE_SCALE
        || output.planned_final_gini_milli > SCORE_SCALE
        || output.prior_gini_milli > SCORE_SCALE
        || output.selections.iter().any(|selection| {
            selection.action_id.trim().is_empty()
                || selection.feature_id.trim().is_empty()
                || selection.independence_group.trim().is_empty()
                || selection.expected_information_gain_milli > SCORE_SCALE
                || selection.diversity_bonus_milli > SCORE_SCALE
                || selection.utility_milli > SCORE_SCALE
                || selection.cost_units == 0
                || selection.allocated_replicates == 0
                || selection.feasibility_milli > 1_000
                || selection.risk_milli > 1_000
                || selection.rationale.trim().is_empty()
        })
    {
        return Err(AdaptivePanelError::InvalidOutput(
            "identity, ordering, panel score, allocation, or uncertainty bounds are invalid".into(),
        ));
    }
    let action_ids = output.action_order.iter().cloned().collect::<BTreeSet<_>>();
    let selection_ids = output
        .selections
        .iter()
        .map(|selection| selection.action_id.clone())
        .collect::<BTreeSet<_>>();
    if action_ids != selection_ids
        || output
            .selected_order
            .iter()
            .chain(output.deferred_order.iter())
            .chain(output.blocked_order.iter())
            .collect::<BTreeSet<_>>()
            != action_ids.iter().collect::<BTreeSet<_>>()
    {
        return Err(AdaptivePanelError::InvalidOutput(
            "action, selection, deferred, and blocked partitions do not reconcile".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| AdaptivePanelError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(AdaptivePanelError::InvalidOutput(
            "adaptive panel digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl AdaptivePanelDesign {
    pub fn validate(&self) -> Result<(), AdaptivePanelError> {
        validate_output(self)
    }
}

/// Compile a multi-assay panel using greedy information gain with diversity and safety gates.
pub fn plan_glioma_adaptive_panel(
    request: &AdaptivePanelRequest,
) -> Result<AdaptivePanelDesign, AdaptivePanelError> {
    validate_request(request)?;
    let mut mechanisms = request.mechanisms.clone();
    mechanisms.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<Vec<_>>();
    let prior = mechanisms
        .iter()
        .map(|mechanism| mechanism.prior_milli)
        .collect::<Vec<_>>();
    let prior_gini = gini(&prior);
    let mut remaining_budget = request.budget_units;
    let mut selected = BTreeSet::new();
    let mut selected_groups = BTreeSet::new();
    let mut candidate_actions = request.actions.clone();
    candidate_actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut selections = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    while selected.len() < request.max_selected_actions {
        let mut best = None::<(u64, String, u64, u64)>;
        for action in &candidate_actions {
            if selected.contains(&action.action_id) {
                continue;
            }
            if action.feasibility_milli < request.min_feasibility_milli {
                continue;
            }
            if action.risk_milli > request.risk_ceiling_milli {
                continue;
            }
            if u64::from(action.cost_units) > remaining_budget {
                continue;
            }
            let (prior_gini_for_action, posterior_gini) =
                expected_information(&prior, &mechanisms, action);
            let information_gain = prior_gini_for_action.saturating_sub(posterior_gini);
            let repeat_count = candidate_actions
                .iter()
                .filter(|candidate| {
                    selected.contains(&candidate.action_id)
                        && candidate.independence_group == action.independence_group
                })
                .count() as u64;
            let diversity_bonus = if repeat_count == 0 {
                u64::from(request.diversity_weight_milli)
            } else {
                u64::from(request.diversity_weight_milli) / (repeat_count + 1)
            };
            let risk_penalty = u64::from(action.risk_milli)
                .saturating_mul(u64::from(request.risk_penalty_milli))
                / 1_000;
            let cost_penalty = u64::from(action.cost_units)
                .saturating_mul(u64::from(request.cost_penalty_milli))
                / 1_000;
            let utility = information_gain
                .saturating_add(diversity_bonus)
                .saturating_add(u64::from(action.feasibility_milli) / 10)
                .saturating_sub(risk_penalty)
                .saturating_sub(cost_penalty)
                .min(SCORE_SCALE);
            if information_gain < request.min_information_gain_milli {
                continue;
            }
            let candidate = (
                utility,
                action.action_id.clone(),
                information_gain,
                diversity_bonus,
            );
            if best.as_ref().is_none_or(|current| {
                candidate.0 > current.0 || (candidate.0 == current.0 && candidate.1 < current.1)
            }) {
                best = Some(candidate);
            }
        }
        let Some((utility, action_id, information_gain, diversity_bonus)) = best else {
            break;
        };
        let action = candidate_actions
            .iter()
            .find(|candidate| candidate.action_id == action_id)
            .expect("selected action exists");
        let replicate_cap = (remaining_budget / u64::from(action.cost_units))
            .min(u64::from(action.max_replicates))
            .max(1) as u16;
        let allocated_replicates = 1_u16.min(replicate_cap);
        let projected_cost = u64::from(action.cost_units) * u64::from(allocated_replicates);
        remaining_budget = remaining_budget.saturating_sub(projected_cost);
        selected.insert(action.action_id.clone());
        selected_groups.insert(action.independence_group.clone());
        selections.push(AdaptivePanelSelection {
            action_id: action.action_id.clone(),
            feature_id: action.feature_id.clone(),
            independence_group: action.independence_group.clone(),
            expected_information_gain_milli: information_gain,
            diversity_bonus_milli: diversity_bonus,
            utility_milli: utility,
            cost_units: action.cost_units,
            allocated_replicates,
            feasibility_milli: action.feasibility_milli,
            risk_milli: action.risk_milli,
            action: AdaptivePanelActionKind::Selected,
            rationale: format!(
                "selected for expected information gain {information_gain}, diversity bonus {diversity_bonus}, and utility {utility}"
            ),
        });
    }
    let action_order = candidate_actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let mut deferred_order = Vec::new();
    let mut blocked_order = Vec::new();
    for action in &candidate_actions {
        if selected.contains(&action.action_id) {
            continue;
        }
        let kind = if action.feasibility_milli < request.min_feasibility_milli {
            AdaptivePanelActionKind::FeasibilityBlocked
        } else if action.risk_milli > request.risk_ceiling_milli {
            AdaptivePanelActionKind::RiskBlocked
        } else if u64::from(action.cost_units) > request.budget_units {
            AdaptivePanelActionKind::BudgetBlocked
        } else {
            AdaptivePanelActionKind::Deferred
        };
        if matches!(kind, AdaptivePanelActionKind::Deferred) {
            deferred_order.push(action.action_id.clone());
        } else {
            blocked_order.push(action.action_id.clone());
        }
        let (_, expected_gini) = expected_information(&prior, &mechanisms, action);
        if expected_gini >= prior_gini {
            negative_evidence.insert(format!("action:{}:uninformative", action.action_id));
        }
        if kind == AdaptivePanelActionKind::Deferred {
            uncertainty.insert(format!("action:{}:deferred", action.action_id));
        }
        selections.push(AdaptivePanelSelection {
            action_id: action.action_id.clone(),
            feature_id: action.feature_id.clone(),
            independence_group: action.independence_group.clone(),
            expected_information_gain_milli: prior_gini.saturating_sub(expected_gini),
            diversity_bonus_milli: 0,
            utility_milli: 0,
            cost_units: action.cost_units,
            allocated_replicates: 1,
            feasibility_milli: action.feasibility_milli,
            risk_milli: action.risk_milli,
            action: kind,
            rationale:
                "not selected in the bounded planning pass; acquire outcomes before replanning"
                    .into(),
        });
    }
    selections.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let total_information_gain = selections
        .iter()
        .filter(|selection| selection.action == AdaptivePanelActionKind::Selected)
        .map(|selection| selection.expected_information_gain_milli)
        .sum::<u64>()
        .min(SCORE_SCALE);
    let planned_final_gini = prior_gini.saturating_sub(total_information_gain);
    if selected.is_empty() {
        uncertainty.insert("no-action-selected".into());
    }
    if remaining_budget == 0 && selected.len() < request.max_selected_actions {
        uncertainty.insert("budget-exhausted-before-panel-completion".into());
    }
    let disposition = if selected.is_empty() {
        if request
            .actions
            .iter()
            .all(|action| action.cost_units as u64 > request.budget_units)
        {
            AdaptivePanelDisposition::BudgetBlocked
        } else {
            AdaptivePanelDisposition::NoInformativeActions
        }
    } else if total_information_gain >= request.min_information_gain_milli {
        AdaptivePanelDisposition::Qualified
    } else {
        AdaptivePanelDisposition::Partial
    };
    let next_action = match disposition {
        AdaptivePanelDisposition::Qualified => {
            "execute the selected panel locally, record typed outcomes, then replan with observed posteriors"
        }
        AdaptivePanelDisposition::Partial => {
            "collect outcomes from the selected assays and re-run panel planning before promoting a mechanism"
        }
        AdaptivePanelDisposition::NoInformativeActions => {
            "add a candidate assay with mechanism-separated outcome distributions"
        }
        AdaptivePanelDisposition::BudgetBlocked => {
            "increase the local assay budget or provide lower-cost candidates"
        }
        AdaptivePanelDisposition::Unresolved => {
            "provide competing mechanism priors and typed candidate outcome distributions"
        }
    }
    .to_string();
    let mut result = AdaptivePanelDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        action_order,
        selected_order,
        deferred_order,
        blocked_order,
        selections,
        prior_gini_milli: prior_gini,
        planned_final_gini_milli: planned_final_gini,
        total_information_gain_milli: total_information_gain,
        budget_remaining_units: remaining_budget,
        selected_group_order: selected_groups.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-panel-design"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| AdaptivePanelError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, group: &str, growth: u16, stress: u16, cost: u32) -> PanelAction {
        PanelAction {
            action_id: id.into(),
            feature_id: format!("feature-{id}"),
            label: format!("{id} assay"),
            independence_group: group.into(),
            outcomes: vec![
                PanelOutcome {
                    outcome_id: "high".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("growth".into(), growth),
                        ("stress".into(), stress),
                    ]),
                },
                PanelOutcome {
                    outcome_id: "low".into(),
                    probability_milli_by_mechanism: BTreeMap::from([
                        ("growth".into(), 1_000 - growth),
                        ("stress".into(), 1_000 - stress),
                    ]),
                },
            ],
            feasibility_milli: 900,
            risk_milli: 100,
            cost_units: cost,
            max_replicates: 2,
        }
    }

    fn request(actions: Vec<PanelAction>) -> AdaptivePanelRequest {
        AdaptivePanelRequest {
            objective: "select a glioma mechanism panel".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![
                PanelMechanism {
                    mechanism_id: "growth".into(),
                    prior_milli: 500,
                },
                PanelMechanism {
                    mechanism_id: "stress".into(),
                    prior_milli: 500,
                },
            ],
            actions,
            budget_units: 2,
            max_selected_actions: 2,
            min_information_gain_milli: 10,
            min_feasibility_milli: 700,
            risk_ceiling_milli: 500,
            diversity_weight_milli: 100,
            risk_penalty_milli: 100,
            cost_penalty_milli: 10,
        }
    }

    #[test]
    fn panel_selects_complementary_information_and_replays() {
        let request = request(vec![
            action("imaging", "imaging", 900, 100, 1),
            action("pathway", "pathway", 800, 200, 1),
            action("redundant", "imaging", 700, 300, 1),
        ]);
        let first = plan_glioma_adaptive_panel(&request).unwrap();
        let replay = plan_glioma_adaptive_panel(&request).unwrap();
        first.validate().unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.disposition, AdaptivePanelDisposition::Qualified);
        assert_eq!(first.selected_order.len(), 2);
        assert_eq!(first.selected_group_order, vec!["imaging", "pathway"]);
    }

    #[test]
    fn safety_and_budget_gates_remain_visible() {
        let mut unsafe_action = action("unsafe", "robotics", 900, 100, 1);
        unsafe_action.risk_milli = 900;
        let result = plan_glioma_adaptive_panel(&AdaptivePanelRequest {
            risk_ceiling_milli: 500,
            budget_units: 1,
            max_selected_actions: 1,
            ..request(vec![unsafe_action])
        })
        .unwrap();
        assert_eq!(
            result.disposition,
            AdaptivePanelDisposition::NoInformativeActions
        );
        assert!(result.blocked_order.contains(&"unsafe".into()));
    }

    #[test]
    fn uninformative_candidates_are_negative_not_promoted() {
        let result = plan_glioma_adaptive_panel(&AdaptivePanelRequest {
            max_selected_actions: 1,
            ..request(vec![action("uninformative", "single", 500, 500, 1)])
        })
        .unwrap();
        assert_eq!(
            result.disposition,
            AdaptivePanelDisposition::NoInformativeActions
        );
        assert!(!result.negative_evidence.is_empty());
    }
}
