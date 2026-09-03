//! Integer-only Bayesian information design for preclinical glioma assays.
//!
//! This feature chooses the next assay batch by expected reduction in mechanism uncertainty. Each
//! candidate declares a discrete outcome distribution under every competing mechanism. The
//! planner computes a Gini-information reduction, then applies feasibility, risk, cost, and
//! budget gates. It is deliberately one-step and model-declared: it does not invent outcomes,
//! assume causal effects, dispatch an assay, or make a clinical recommendation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaInformationDesign1@1";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_OUTCOMES_PER_ACTION: usize = 128;
pub const MAX_PRIOR_MILLI: u16 = 1_000;
pub const SCORE_SCALE: u64 = 1_000;

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
    pub mechanism_order: Vec<String>,
    pub action_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub scores: Vec<InformationDesignActionScore>,
    pub prior_gini_milli: u64,
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
        "mechanism_order": output.mechanism_order,
        "action_order": output.action_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "scores": output.scores,
        "prior_gini_milli": output.prior_gini_milli,
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
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.feature_id.trim().is_empty()
                    || score.label.trim().is_empty()
                    || score.prior_gini_milli > SCORE_SCALE
                    || score.expected_posterior_gini_milli > SCORE_SCALE
                    || score.expected_information_gain_milli > SCORE_SCALE
                    || score.feasibility_milli > 1_000
                    || score.risk_milli > 1_000
                    || score.cost_units == 0
                    || score.utility_milli > SCORE_SCALE
                    || score.rationale.trim().is_empty()
            })
            || self.scores.windows(2).any(|pair| {
                pair[0].utility_milli < pair[1].utility_milli
                    || (pair[0].utility_milli == pair[1].utility_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(InformationDesignError::InvalidOutput(
                "identity, ordering, score bounds, or rationale is invalid".into(),
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
            "objective, positive budget/selection bounds, information threshold, and score weights are required".into(),
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

/// Plan the most informative bounded assay batch using deterministic posterior-separation scores.
pub fn plan_glioma_information_design(
    request: &InformationDesignRequest,
    mechanisms: &[DesignMechanism],
    actions: &[DesignAction],
) -> Result<InformationDesignPlan, InformationDesignError> {
    validate_inputs(request, mechanisms, actions)?;
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
    let mut scores = ordered_actions
        .iter()
        .map(|action| {
            let (prior, posterior, predictive_outcome_count) =
                expected_information(&ordered_mechanisms, action);
            let information = prior.saturating_sub(posterior);
            let weighted = u128::from(request.information_weight_milli)
                .saturating_mul(u128::from(information))
                .saturating_add(
                    u128::from(request.feasibility_weight_milli)
                        .saturating_mul(u128::from(action.feasibility_milli)),
                )
                .checked_div(1_000)
                .unwrap_or(0);
            let risk_penalty = u128::from(request.risk_penalty_milli)
                .saturating_mul(u128::from(action.risk_milli))
                .checked_div(1_000)
                .unwrap_or(0);
            let cost_penalty = u128::from(request.cost_penalty_milli)
                .saturating_mul(u128::from(action.cost_units))
                .min(u128::from(SCORE_SCALE));
            let utility = weighted
                .saturating_sub(risk_penalty.saturating_add(cost_penalty))
                .min(u128::from(SCORE_SCALE)) as u64;
            let rationale = if action.risk_milli > request.risk_ceiling_milli {
                "information-bearing assay is held above the declared preclinical risk ceiling"
            } else if information < request.min_information_gain_milli {
                "outcome distributions do not clear the declared mechanism-information gate"
            } else {
                "assay ranked by expected mechanism-information reduction with feasibility, risk, and cost bounds"
            };
            InformationDesignActionScore {
                action_id: action.action_id.clone(),
                feature_id: action.feature_id.clone(),
                label: action.label.clone(),
                prior_gini_milli: prior,
                expected_posterior_gini_milli: posterior,
                expected_information_gain_milli: information,
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
                right
                    .expected_information_gain_milli
                    .cmp(&left.expected_information_gain_milli)
            })
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut remaining_budget = request.budget_units;
    let mut selected_order = Vec::new();
    for score in scores.iter_mut() {
        if selected_order.len() >= request.max_selected_actions
            || score.expected_information_gain_milli < request.min_information_gain_milli
            || score.risk_milli > request.risk_ceiling_milli
            || u64::from(score.cost_units) > remaining_budget
        {
            continue;
        }
        let action = ordered_actions
            .iter()
            .find(|action| action.action_id == score.action_id)
            .expect("validated action exists");
        let replicates = action
            .max_replicates
            .min((remaining_budget / u64::from(action.cost_units)) as u16)
            .max(1);
        let spend = u64::from(replicates).saturating_mul(u64::from(action.cost_units));
        if spend > remaining_budget {
            continue;
        }
        score.selected_replicates = replicates;
        remaining_budget = remaining_budget.saturating_sub(spend);
        selected_order.push(score.action_id.clone());
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
                right
                    .expected_information_gain_milli
                    .cmp(&left.expected_information_gain_milli)
            })
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative_evidence: BTreeSet<String> = BTreeSet::new();
    if selected_order.is_empty() {
        if scores.iter().any(|score| {
            score.expected_information_gain_milli >= request.min_information_gain_milli
                && score.risk_milli <= request.risk_ceiling_milli
        }) {
            uncertainty.insert("budget-cannot-fund-an-information-bearing-action".into());
        } else {
            negative_evidence.insert("no-action-clears-mechanism-information-gate".into());
        }
    }
    if scores
        .iter()
        .any(|score| score.expected_posterior_gini_milli > prior_gini)
    {
        uncertainty.insert("posterior-gini-rounding-bound-is-conservative".into());
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
        mechanism_order,
        action_order,
        selected_order,
        deferred_order,
        scores,
        prior_gini_milli: prior_gini,
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
}
