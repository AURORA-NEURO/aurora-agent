//! Robust branch planning for autonomous preclinical glioma research.
//!
//! A single greedy action batch is not enough when competing mechanisms, missing measurements,
//! or contradictory model systems make the next workflow uncertain. This feature evaluates a
//! bounded beam of dependency-closed action portfolios across explicit scenario outcomes, then
//! returns a Pareto frontier over expected value, worst-case value, uncertainty, failure risk, and
//! cost. It is a planning product: a caller-owned selector or executor still controls every
//! assay, instrument, computation, and federation effect.

use super::context_compiler::DecisionContext;
use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionWeights};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionBranchPlan1@1";
pub const MAX_CANDIDATES: usize = 128;
pub const MAX_SCENARIOS: usize = 32;
pub const MAX_OUTCOMES_PER_SCENARIO: usize = 128;
pub const MAX_BRANCHES: usize = 64;
pub const MAX_BEAM_WIDTH: usize = 256;

/// A scenario is a declared, bounded view of how candidate actions may score. It is not an
/// observed result: scenario values are forecasts supplied by a local model or researcher and
/// stay visibly separate from returned biological evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionScenarioOutcome {
    pub action_id: String,
    pub value_milli: i32,
    pub uncertainty_milli: u16,
    pub failure_probability_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionScenario {
    pub scenario_id: String,
    pub probability_milli: u16,
    pub outcomes: Vec<DecisionScenarioOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchPlannerRequest {
    pub objective: String,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_action_order: Vec<String>,
    pub scenarios: Vec<DecisionScenario>,
    pub budget_units: u32,
    pub max_actions_per_branch: u16,
    pub max_branches: u16,
    pub beam_width: u16,
    pub minimum_robustness_milli: i64,
    pub uncertainty_penalty_milli: u16,
    pub failure_penalty_milli: u16,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionScenarioScore {
    pub scenario_id: String,
    pub value_milli: i64,
    pub uncertainty_milli: u32,
    pub failure_risk_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchPortfolio {
    pub branch_id: String,
    pub selected_order: Vec<String>,
    pub cost_units: u32,
    pub expected_value_milli: i64,
    pub worst_case_value_milli: i64,
    pub uncertainty_milli: u32,
    pub failure_risk_milli: u32,
    pub robustness_milli: i64,
    pub scenario_scores: Vec<DecisionScenarioScore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBranchPlanDisposition {
    Qualified,
    Partial,
    Unresolved,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBranchPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub branch_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub portfolios: Vec<DecisionBranchPortfolio>,
    pub selected_branch_id: Option<String>,
    pub omitted_action_order: Vec<String>,
    pub budget_block_order: Vec<String>,
    pub unresolved_scenario_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionBranchPlanDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionBranchPlannerError {
    #[error("decision branch planner request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision branch planner context is invalid: {0}")]
    InvalidContext(String),
    #[error("decision branch planner graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("decision branch planner output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision branch planner digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty()) && canonical(values)
}

fn digest_input(output: &DecisionBranchPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "branch_order": output.branch_order,
        "frontier_order": output.frontier_order,
        "portfolios": output.portfolios,
        "selected_branch_id": output.selected_branch_id,
        "omitted_action_order": output.omitted_action_order,
        "budget_block_order": output.budget_block_order,
        "unresolved_scenario_order": output.unresolved_scenario_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

impl DecisionBranchPlan {
    pub fn validate(&self) -> Result<(), DecisionBranchPlannerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.context_digest.as_str().len() != 64
            || !unique_nonempty(&self.branch_order)
            || !unique_nonempty(&self.frontier_order)
            || !unique_nonempty(&self.omitted_action_order)
            || !unique_nonempty(&self.budget_block_order)
            || !unique_nonempty(&self.unresolved_scenario_order)
            || !unique_nonempty(&self.negative_evidence_order)
            || !unique_nonempty(&self.uncertainty_order)
            || self.portfolios.len() != self.branch_order.len()
            || self
                .portfolios
                .windows(2)
                .any(|pair| pair[0].branch_id >= pair[1].branch_id)
            || self.portfolios.iter().any(|portfolio| {
                portfolio.branch_id.trim().is_empty()
                    || !unique_nonempty(&portfolio.selected_order)
                    || portfolio.selected_order.is_empty()
                    || portfolio.cost_units == 0
                    || portfolio.scenario_scores.is_empty()
                    || portfolio
                        .scenario_scores
                        .windows(2)
                        .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
                    || portfolio.scenario_scores.iter().any(|score| {
                        score.scenario_id.trim().is_empty()
                            || score.uncertainty_milli > 1_000
                            || score.failure_risk_milli > 1_000
                    })
            })
        {
            return Err(DecisionBranchPlannerError::InvalidOutput(
                "identity, canonical partitions, portfolio ordering, and scenario score invariants are invalid".into(),
            ));
        }
        let branch_ids = self
            .portfolios
            .iter()
            .map(|portfolio| portfolio.branch_id.clone())
            .collect::<BTreeSet<_>>();
        if branch_ids != self.branch_order.iter().cloned().collect::<BTreeSet<_>>()
            || self
                .frontier_order
                .iter()
                .any(|branch| !branch_ids.contains(branch))
            || self
                .selected_branch_id
                .as_ref()
                .is_some_and(|branch| !branch_ids.contains(branch))
        {
            return Err(DecisionBranchPlannerError::InvalidOutput(
                "branch identity or selected/frontier partition does not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionBranchPlannerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionBranchPlannerError::InvalidOutput(
                "decision branch plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_weights(weights: GliomaSelectionWeights) -> Result<(), DecisionBranchPlannerError> {
    let total = u32::from(weights.information_gain)
        + u32::from(weights.frontier_novelty)
        + u32::from(weights.workflow_leverage)
        + u32::from(weights.cross_stage_unlock)
        + u32::from(weights.reproducibility_safety)
        + u32::from(weights.federation_value)
        + u32::from(weights.feasibility);
    if total != 100
        || [
            weights.information_gain,
            weights.frontier_novelty,
            weights.workflow_leverage,
            weights.cross_stage_unlock,
            weights.reproducibility_safety,
            weights.federation_value,
            weights.feasibility,
        ]
        .iter()
        .any(|value| *value > 1_000)
    {
        return Err(DecisionBranchPlannerError::InvalidRequest(
            "selection weights must sum to 100 and remain bounded".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &DecisionBranchPlannerRequest,
) -> Result<(), DecisionBranchPlannerError> {
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
        || request.scenarios.len() < 2
        || request.scenarios.len() > MAX_SCENARIOS
        || request.budget_units == 0
        || request.max_actions_per_branch == 0
        || usize::from(request.max_actions_per_branch) > MAX_CANDIDATES
        || request.max_branches == 0
        || usize::from(request.max_branches) > MAX_BRANCHES
        || request.beam_width == 0
        || usize::from(request.beam_width) > MAX_BEAM_WIDTH
        || request.uncertainty_penalty_milli > 1_000
        || request.failure_penalty_milli > 1_000
    {
        return Err(DecisionBranchPlannerError::InvalidRequest(
            "objective, candidates, canonical completed ids, at least two bounded scenarios, positive budget, and bounded beam/branch limits are required".into(),
        ));
    }
    let mut completed_ids = BTreeSet::new();
    if request
        .completed_action_order
        .iter()
        .any(|id| !completed_ids.insert(id.clone()))
    {
        return Err(DecisionBranchPlannerError::InvalidRequest(
            "completed action ids must be unique".into(),
        ));
    }
    validate_weights(request.selection_weights)?;
    let mut scenario_ids = BTreeSet::new();
    let mut probability_total = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !scenario_ids.insert(scenario.scenario_id.clone())
            || scenario.probability_milli == 0
            || scenario.outcomes.is_empty()
            || scenario.outcomes.len() > MAX_OUTCOMES_PER_SCENARIO
            || scenario.outcomes.iter().any(|outcome| {
                outcome.action_id.trim().is_empty()
                    || outcome.uncertainty_milli > 1_000
                    || outcome.failure_probability_milli > 1_000
            })
        {
            return Err(DecisionBranchPlannerError::InvalidRequest(
                "scenarios require unique ids, positive probabilities, canonical outcomes, and bounded uncertainty/failure values".into(),
            ));
        }
        let mut outcome_ids = BTreeSet::new();
        if scenario
            .outcomes
            .iter()
            .any(|outcome| !outcome_ids.insert(outcome.action_id.clone()))
        {
            return Err(DecisionBranchPlannerError::InvalidRequest(
                "scenario outcome action ids must be unique".into(),
            ));
        }
        probability_total = probability_total.saturating_add(u32::from(scenario.probability_milli));
    }
    if probability_total != 1_000 {
        return Err(DecisionBranchPlannerError::InvalidRequest(
            "scenario probabilities must sum to 1000 milli-probability".into(),
        ));
    }
    Ok(())
}

fn validate_graph(
    request: &DecisionBranchPlannerRequest,
) -> Result<BTreeMap<String, GliomaActionCandidate>, DecisionBranchPlannerError> {
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.effects.is_empty()
            || [
                candidate.information_gain_milli,
                candidate.frontier_novelty_milli,
                candidate.workflow_leverage_milli,
                candidate.cross_stage_unlock_milli,
                candidate.reproducibility_safety_milli,
                candidate.federation_value_milli,
                candidate.feasibility_milli,
            ]
            .iter()
            .any(|value| *value > 1_000)
            || candidate.depends_on.iter().any(|dependency| {
                dependency == &candidate.action_id || dependency.trim().is_empty()
            })
            || candidates
                .insert(candidate.action_id.clone(), candidate.clone())
                .is_some()
        {
            return Err(DecisionBranchPlannerError::InvalidGraph(
                "candidate identities, positive costs, effects, bounded scores, and canonical non-self dependencies are required".into(),
            ));
        }
        let mut dependencies = BTreeSet::new();
        if candidate
            .depends_on
            .iter()
            .any(|dependency| !dependencies.insert(dependency.clone()))
        {
            return Err(DecisionBranchPlannerError::InvalidGraph(
                "candidate dependencies must be unique".into(),
            ));
        }
    }
    if request
        .completed_action_order
        .iter()
        .any(|id| !candidates.contains_key(id))
    {
        return Err(DecisionBranchPlannerError::InvalidGraph(
            "completed actions must resolve to declared candidates".into(),
        ));
    }
    for candidate in candidates.values() {
        if candidate.depends_on.iter().any(|dependency| {
            !completed.contains(dependency) && !candidates.contains_key(dependency)
        }) {
            return Err(DecisionBranchPlannerError::InvalidGraph(
                "candidate dependency references an unknown action".into(),
            ));
        }
    }
    let mut indegree = candidates
        .iter()
        .map(|(id, candidate)| {
            (
                id.clone(),
                candidate
                    .depends_on
                    .iter()
                    .filter(|dependency| !completed.contains(*dependency))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for candidate in candidates.values() {
        for dependency in &candidate.depends_on {
            if !completed.contains(dependency) {
                outgoing
                    .entry(dependency.clone())
                    .or_default()
                    .push(candidate.action_id.clone());
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    ready.sort();
    let mut visited = 0_usize;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(children) = outgoing.get(&id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("graph child exists");
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.push(child.clone());
                }
            }
            ready.sort();
        }
    }
    if visited != candidates.len() {
        return Err(DecisionBranchPlannerError::InvalidGraph(
            "candidate dependency graph contains a cycle".into(),
        ));
    }
    for scenario in &request.scenarios {
        for outcome in &scenario.outcomes {
            if !candidates.contains_key(&outcome.action_id) {
                return Err(DecisionBranchPlannerError::InvalidGraph(
                    "scenario outcome references an unknown candidate".into(),
                ));
            }
        }
    }
    Ok(candidates)
}

fn base_score(candidate: &GliomaActionCandidate, weights: GliomaSelectionWeights) -> i64 {
    (u64::from(candidate.information_gain_milli) * u64::from(weights.information_gain)
        + u64::from(candidate.frontier_novelty_milli) * u64::from(weights.frontier_novelty)
        + u64::from(candidate.workflow_leverage_milli) * u64::from(weights.workflow_leverage)
        + u64::from(candidate.cross_stage_unlock_milli) * u64::from(weights.cross_stage_unlock)
        + u64::from(candidate.reproducibility_safety_milli)
            * u64::from(weights.reproducibility_safety)
        + u64::from(candidate.federation_value_milli) * u64::from(weights.federation_value)
        + u64::from(candidate.feasibility_milli) * u64::from(weights.feasibility))
    .saturating_div(100) as i64
}

fn add_dependency_closure(
    action_id: &str,
    candidates: &BTreeMap<String, GliomaActionCandidate>,
    completed: &BTreeSet<String>,
    selected: &mut BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
) -> Result<(), DecisionBranchPlannerError> {
    if completed.contains(action_id) || selected.contains(action_id) {
        return Ok(());
    }
    if !visiting.insert(action_id.to_string()) {
        return Err(DecisionBranchPlannerError::InvalidGraph(
            "dependency closure encountered a cycle".into(),
        ));
    }
    let candidate = candidates.get(action_id).ok_or_else(|| {
        DecisionBranchPlannerError::InvalidGraph(
            "dependency closure references an unknown action".into(),
        )
    })?;
    for dependency in &candidate.depends_on {
        add_dependency_closure(dependency, candidates, completed, selected, visiting)?;
    }
    visiting.remove(action_id);
    selected.insert(action_id.to_string());
    Ok(())
}

fn selected_cost(
    selected: &BTreeSet<String>,
    candidates: &BTreeMap<String, GliomaActionCandidate>,
) -> u32 {
    selected
        .iter()
        .filter_map(|id| candidates.get(id))
        .map(|candidate| candidate.cost_units)
        .fold(0_u32, u32::saturating_add)
}

fn scenario_outcomes(scenario: &DecisionScenario) -> BTreeMap<&str, &DecisionScenarioOutcome> {
    scenario
        .outcomes
        .iter()
        .map(|outcome| (outcome.action_id.as_str(), outcome))
        .collect()
}

fn branch_id(selected_order: &[String]) -> String {
    let digest = ContentHash::of_value(&serde_json::json!(selected_order))
        .expect("canonical branch ids are hashable");
    format!("branch-{}", &digest.as_str()[..16])
}

fn evaluate_portfolio(
    selected: &BTreeSet<String>,
    candidates: &BTreeMap<String, GliomaActionCandidate>,
    request: &DecisionBranchPlannerRequest,
) -> DecisionBranchPortfolio {
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let cost_units = selected_cost(selected, candidates);
    let mut scores = Vec::with_capacity(request.scenarios.len());
    let mut expected_numerator = 0_i64;
    let mut worst_case = i64::MAX;
    let mut uncertainty = 0_u32;
    let mut failure_risk = 0_u32;
    for scenario in &request.scenarios {
        let outcomes = scenario_outcomes(scenario);
        let mut value = 0_i64;
        let mut uncertainty_sum = 0_u32;
        let mut survival_milli = 1_000_u32;
        for action_id in &selected_order {
            let base = base_score(&candidates[action_id], request.selection_weights);
            if let Some(outcome) = outcomes.get(action_id.as_str()) {
                value = value.saturating_add(base.saturating_add(i64::from(outcome.value_milli)));
                uncertainty_sum =
                    uncertainty_sum.saturating_add(u32::from(outcome.uncertainty_milli));
                survival_milli = survival_milli
                    .saturating_mul(
                        1_000_u32.saturating_sub(u32::from(outcome.failure_probability_milli)),
                    )
                    .saturating_div(1_000);
            } else {
                value = value.saturating_add(base);
                uncertainty_sum = uncertainty_sum.saturating_add(1_000);
                survival_milli = 0;
            }
        }
        let scenario_uncertainty = if selected_order.is_empty() {
            0
        } else {
            (uncertainty_sum / selected_order.len() as u32).min(1_000)
        };
        let scenario_failure = 1_000_u32.saturating_sub(survival_milli);
        expected_numerator = expected_numerator
            .saturating_add(value.saturating_mul(i64::from(scenario.probability_milli)));
        worst_case = worst_case.min(value);
        uncertainty = uncertainty.max(scenario_uncertainty);
        failure_risk = failure_risk.max(scenario_failure);
        scores.push(DecisionScenarioScore {
            scenario_id: scenario.scenario_id.clone(),
            value_milli: value,
            uncertainty_milli: scenario_uncertainty,
            failure_risk_milli: scenario_failure,
        });
    }
    let expected_value = expected_numerator / 1_000;
    let robustness = worst_case
        .saturating_sub(
            i64::from(uncertainty).saturating_mul(i64::from(request.uncertainty_penalty_milli))
                / 1_000,
        )
        .saturating_sub(
            i64::from(failure_risk).saturating_mul(i64::from(request.failure_penalty_milli))
                / 1_000,
        );
    DecisionBranchPortfolio {
        branch_id: branch_id(&selected_order),
        selected_order,
        cost_units,
        expected_value_milli: expected_value,
        worst_case_value_milli: if worst_case == i64::MAX {
            0
        } else {
            worst_case
        },
        uncertainty_milli: uncertainty,
        failure_risk_milli: failure_risk,
        robustness_milli: robustness,
        scenario_scores: scores,
    }
}

fn rank_portfolio(
    left: &DecisionBranchPortfolio,
    right: &DecisionBranchPortfolio,
) -> std::cmp::Ordering {
    right
        .robustness_milli
        .cmp(&left.robustness_milli)
        .then_with(|| right.expected_value_milli.cmp(&left.expected_value_milli))
        .then_with(|| left.uncertainty_milli.cmp(&right.uncertainty_milli))
        .then_with(|| left.failure_risk_milli.cmp(&right.failure_risk_milli))
        .then_with(|| left.cost_units.cmp(&right.cost_units))
        .then_with(|| left.branch_id.cmp(&right.branch_id))
}

fn dominates(left: &DecisionBranchPortfolio, right: &DecisionBranchPortfolio) -> bool {
    let no_worse = left.expected_value_milli >= right.expected_value_milli
        && left.worst_case_value_milli >= right.worst_case_value_milli
        && left.uncertainty_milli <= right.uncertainty_milli
        && left.failure_risk_milli <= right.failure_risk_milli
        && left.cost_units <= right.cost_units;
    let strictly_better = left.expected_value_milli > right.expected_value_milli
        || left.worst_case_value_milli > right.worst_case_value_milli
        || left.uncertainty_milli < right.uncertainty_milli
        || left.failure_risk_milli < right.failure_risk_milli
        || left.cost_units < right.cost_units;
    no_worse && strictly_better
}

fn state_key(state: &BTreeSet<String>) -> String {
    state.iter().cloned().collect::<Vec<_>>().join("\u{1f}")
}

/// Plan robust, dependency-closed alternatives for a typed preclinical glioma decision context.
pub fn plan_glioma_decision_branches(
    input: &DecisionBranchPlannerRequest,
    context: &DecisionContext,
) -> Result<DecisionBranchPlan, DecisionBranchPlannerError> {
    let mut normalized = input.clone();
    normalized.completed_action_order.sort();
    normalized
        .candidates
        .sort_by(|left, right| left.action_id.cmp(&right.action_id));
    for candidate in &mut normalized.candidates {
        candidate.depends_on.sort();
    }
    normalized
        .scenarios
        .sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    for scenario in &mut normalized.scenarios {
        scenario
            .outcomes
            .sort_by(|left, right| left.action_id.cmp(&right.action_id));
    }
    let request = &normalized;
    validate_request(request)?;
    context
        .validate()
        .map_err(|error| DecisionBranchPlannerError::InvalidContext(error.to_string()))?;
    if request.objective.trim() != context.objective.trim() {
        return Err(DecisionBranchPlannerError::InvalidRequest(
            "branch-planner objective must match the compiled decision context".into(),
        ));
    }
    let candidates = validate_graph(request)?;
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut ranking = candidates.values().collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        let left_ratio =
            base_score(left, request.selection_weights) as i128 * i128::from(right.cost_units);
        let right_ratio =
            base_score(right, request.selection_weights) as i128 * i128::from(left.cost_units);
        right_ratio
            .cmp(&left_ratio)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });

    let mut states = vec![BTreeSet::<String>::new()];
    for candidate in ranking {
        let mut expanded = states.clone();
        for state in &states {
            let mut next = state.clone();
            add_dependency_closure(
                &candidate.action_id,
                &candidates,
                &completed,
                &mut next,
                &mut BTreeSet::new(),
            )?;
            if next.len() > usize::from(request.max_actions_per_branch)
                || selected_cost(&next, &candidates) > request.budget_units
                || next == *state
            {
                continue;
            }
            expanded.push(next);
        }
        let mut unique = BTreeMap::<String, BTreeSet<String>>::new();
        for state in expanded {
            unique.entry(state_key(&state)).or_insert(state);
        }
        let mut ranked_states = unique.into_values().collect::<Vec<_>>();
        ranked_states.sort_by(|left, right| {
            let left_portfolio = evaluate_portfolio(left, &candidates, request);
            let right_portfolio = evaluate_portfolio(right, &candidates, request);
            rank_portfolio(&left_portfolio, &right_portfolio)
        });
        ranked_states.truncate(usize::from(request.beam_width));
        states = ranked_states;
    }

    let mut all_portfolios = states
        .iter()
        .filter(|state| !state.is_empty())
        .map(|state| evaluate_portfolio(state, &candidates, request))
        .collect::<Vec<_>>();
    all_portfolios.sort_by(rank_portfolio);
    all_portfolios.dedup_by(|left, right| left.branch_id == right.branch_id);
    let frontier = all_portfolios
        .iter()
        .filter(|portfolio| {
            !all_portfolios
                .iter()
                .any(|other| other.branch_id != portfolio.branch_id && dominates(other, portfolio))
        })
        .map(|portfolio| portfolio.branch_id.clone())
        .collect::<BTreeSet<_>>();
    let mut chosen = all_portfolios
        .iter()
        .filter(|portfolio| frontier.contains(&portfolio.branch_id))
        .cloned()
        .collect::<Vec<_>>();
    for portfolio in &all_portfolios {
        if chosen.len() >= usize::from(request.max_branches) {
            break;
        }
        if !chosen
            .iter()
            .any(|existing| existing.branch_id == portfolio.branch_id)
        {
            chosen.push(portfolio.clone());
        }
    }
    chosen.truncate(usize::from(request.max_branches));
    chosen.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let branch_order = chosen
        .iter()
        .map(|portfolio| portfolio.branch_id.clone())
        .collect::<Vec<_>>();
    let frontier_order = frontier
        .iter()
        .filter(|branch| branch_order.binary_search(branch).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    let selected_branch_id = all_portfolios
        .iter()
        .find(|portfolio| branch_order.binary_search(&portfolio.branch_id).is_ok())
        .map(|portfolio| portfolio.branch_id.clone());
    let represented_actions = chosen
        .iter()
        .flat_map(|portfolio| portfolio.selected_order.iter().cloned())
        .collect::<BTreeSet<_>>();
    let omitted_action_order = candidates
        .keys()
        .filter(|id| !represented_actions.contains(*id) && !completed.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut budget_block_order = BTreeSet::new();
    for candidate in candidates.values() {
        let mut closure = BTreeSet::new();
        add_dependency_closure(
            &candidate.action_id,
            &candidates,
            &completed,
            &mut closure,
            &mut BTreeSet::new(),
        )?;
        if selected_cost(&closure, &candidates) > request.budget_units {
            budget_block_order.insert(candidate.action_id.clone());
        }
    }
    let mut unresolved_scenarios = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for portfolio in &chosen {
        for score in &portfolio.scenario_scores {
            if score.uncertainty_milli > 0 {
                uncertainty.insert(format!("{}:uncertainty", score.scenario_id));
            }
            if score.uncertainty_milli >= 1_000 || score.failure_risk_milli >= 1_000 {
                unresolved_scenarios.insert(score.scenario_id.clone());
            }
        }
        for scenario in &request.scenarios {
            let outcomes = scenario_outcomes(scenario);
            for action_id in &portfolio.selected_order {
                match outcomes.get(action_id.as_str()) {
                    Some(outcome) if outcome.value_milli < 0 => {
                        negative_evidence.insert(format!(
                            "{}:{}:{}",
                            portfolio.branch_id, scenario.scenario_id, action_id
                        ));
                    }
                    None => {
                        unresolved_scenarios.insert(scenario.scenario_id.clone());
                        uncertainty.insert(format!(
                            "{}:{}:missing-outcome",
                            scenario.scenario_id, action_id
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    let best = selected_branch_id
        .as_ref()
        .and_then(|id| chosen.iter().find(|portfolio| &portfolio.branch_id == id));
    let disposition = if chosen.is_empty() {
        if budget_block_order.len() == candidates.len() {
            DecisionBranchPlanDisposition::BudgetBlocked
        } else {
            DecisionBranchPlanDisposition::Unresolved
        }
    } else if best.is_some_and(|portfolio| {
        portfolio.robustness_milli >= request.minimum_robustness_milli
            && unresolved_scenarios.is_empty()
    }) {
        DecisionBranchPlanDisposition::Qualified
    } else {
        DecisionBranchPlanDisposition::Partial
    };
    let mut output = DecisionBranchPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_digest: context.digest.clone(),
        branch_order,
        frontier_order,
        portfolios: chosen,
        selected_branch_id,
        omitted_action_order,
        budget_block_order: budget_block_order.into_iter().collect(),
        unresolved_scenario_order: unresolved_scenarios.into_iter().collect(),
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-branch-plan"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionBranchPlannerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::{
        compile_decision_context, DecisionContextRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_foundation::{AutonomyTier, Effect};
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn context() -> DecisionContext {
        let record = EvidenceRecord {
            evidence_id: "e1".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "a1".into(),
                content_hash: hash("a1"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "rank invasion mechanisms".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap();
        compile_decision_context(
            &DecisionContextRequest {
                objective: "rank invasion mechanisms".into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge,
        )
        .unwrap()
    }

    fn candidate(id: &str, cost: u32, gain: u16, depends_on: Vec<String>) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind: crate::glioma_engine::GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on,
            cost_units: cost,
            information_gain_milli: gain,
            frontier_novelty_milli: gain,
            workflow_leverage_milli: gain,
            cross_stage_unlock_milli: gain,
            reproducibility_safety_milli: 900,
            federation_value_milli: 500,
            feasibility_milli: 800,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation]),
        }
    }

    fn request() -> DecisionBranchPlannerRequest {
        DecisionBranchPlannerRequest {
            objective: "rank invasion mechanisms".into(),
            candidates: vec![
                candidate("a", 4, 900, Vec::new()),
                candidate("b", 5, 850, vec!["a".into()]),
                candidate("c", 4, 700, Vec::new()),
            ],
            completed_action_order: Vec::new(),
            scenarios: vec![
                DecisionScenario {
                    scenario_id: "invasion-high".into(),
                    probability_milli: 600,
                    outcomes: vec![
                        DecisionScenarioOutcome {
                            action_id: "a".into(),
                            value_milli: 800,
                            uncertainty_milli: 100,
                            failure_probability_milli: 50,
                        },
                        DecisionScenarioOutcome {
                            action_id: "b".into(),
                            value_milli: 900,
                            uncertainty_milli: 150,
                            failure_probability_milli: 50,
                        },
                        DecisionScenarioOutcome {
                            action_id: "c".into(),
                            value_milli: -200,
                            uncertainty_milli: 400,
                            failure_probability_milli: 300,
                        },
                    ],
                },
                DecisionScenario {
                    scenario_id: "invasion-low".into(),
                    probability_milli: 400,
                    outcomes: vec![
                        DecisionScenarioOutcome {
                            action_id: "a".into(),
                            value_milli: 300,
                            uncertainty_milli: 200,
                            failure_probability_milli: 100,
                        },
                        DecisionScenarioOutcome {
                            action_id: "b".into(),
                            value_milli: -100,
                            uncertainty_milli: 600,
                            failure_probability_milli: 250,
                        },
                        DecisionScenarioOutcome {
                            action_id: "c".into(),
                            value_milli: 500,
                            uncertainty_milli: 150,
                            failure_probability_milli: 100,
                        },
                    ],
                },
            ],
            budget_units: 9,
            max_actions_per_branch: 3,
            max_branches: 8,
            beam_width: 32,
            minimum_robustness_milli: 0,
            uncertainty_penalty_milli: 400,
            failure_penalty_milli: 400,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn planner_returns_replay_stable_dependency_closed_frontier() {
        let context = context();
        let first = plan_glioma_decision_branches(&request(), &context).unwrap();
        let mut reversed = request();
        reversed.candidates.reverse();
        reversed.scenarios.reverse();
        for scenario in &mut reversed.scenarios {
            scenario.outcomes.reverse();
        }
        let second = plan_glioma_decision_branches(&reversed, &context).unwrap();
        assert_eq!(first, second);
        assert!(!first.portfolios.is_empty());
        assert!(!first.frontier_order.is_empty());
        for portfolio in &first.portfolios {
            if portfolio.selected_order.contains(&"b".into()) {
                assert!(portfolio.selected_order.contains(&"a".into()));
            }
        }
        first.validate().unwrap();
    }

    #[test]
    fn planner_preserves_missing_scenarios_and_negative_outcomes() {
        let context = context();
        let mut request = request();
        request.scenarios[1]
            .outcomes
            .retain(|outcome| outcome.action_id != "b");
        let output = plan_glioma_decision_branches(&request, &context).unwrap();
        assert!(!output.unresolved_scenario_order.is_empty());
        assert!(!output.uncertainty_order.is_empty());
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains(":c")));
    }

    #[test]
    fn planner_exposes_budget_block_without_fabricating_a_branch() {
        let context = context();
        let mut request = request();
        request.budget_units = 2;
        let output = plan_glioma_decision_branches(&request, &context).unwrap();
        assert_eq!(
            output.disposition,
            DecisionBranchPlanDisposition::BudgetBlocked
        );
        assert!(output.portfolios.is_empty());
        assert_eq!(output.budget_block_order.len(), 3);
        output.validate().unwrap();
    }
}
