//! Held-out evaluation for the autonomous glioma research engine.
//!
//! This is deliberately a policy evaluator, not another execution receipt. It compiles the
//! same dependency-safe director frontier used by the engine, compares that frontier with
//! deterministic greedy/coverage baselines, and computes a bounded dependency-aware oracle on
//! caller-supplied held-out utility. Held-out values are never passed to the planner and never
//! become biological evidence. The evaluator is useful for choosing an autonomous policy before
//! enabling an institution-local worker on a real preclinical campaign.

use super::action_execution::{
    ActionExecutionDisposition, ActionExecutionFailure, ActionExecutionResult,
    DryRunGliomaActionExecutor, GliomaActionExecutionContext, GliomaActionExecutor,
};
use super::autonomous_engine::{
    execute_glioma_autonomous_research_engine, GliomaAutonomousResearchEngineDisposition,
    GliomaAutonomousResearchEngineRequest, GliomaAutonomousResearchEngineStopReason, MAX_CYCLES,
};
use super::director::{
    plan_glioma_research_director, GliomaDirectorCheckpoint, GliomaDirectorFocus,
    GliomaResearchDirectorRequest,
};
use crate::glioma_engine::{GliomaActionCandidate, GliomaSelectionWeights};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::autonomous_engine::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchEngineEvaluation1@1";
pub const STRESS_OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchEngineStressEvaluation1@1";
pub const TRACE_OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchEngineTraceEvaluation1@1";
const ORACLE_BEAM_WIDTH: usize = 256;
const VALUE_LIMIT: i64 = 1_000_000;
const MAX_SCENARIOS: usize = 32;

/// Policies compared on the same compiled glioma action frontier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEngineEvaluationPolicy {
    AuroraEngine,
    ScoreGreedy,
    CoverageFirst,
    OracleBeam,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineEvaluationMetric {
    pub policy: GliomaEngineEvaluationPolicy,
    pub selected_order: Vec<String>,
    pub cost_units: u32,
    pub held_out_utility_milli: i64,
    pub regret_to_oracle_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineEvaluation {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub held_out_truth_digest: ContentHash,
    pub oracle_selected_order: Vec<String>,
    pub oracle_utility_milli: i64,
    pub metrics: Vec<GliomaEngineEvaluationMetric>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

/// One named held-out scenario in a policy stress evaluation. Scenario truth remains external to
/// planning and is summarized only by content digests and the bounded oracle utility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineStressScenarioSummary {
    pub scenario_id: String,
    pub evaluation_digest: ContentHash,
    pub held_out_truth_digest: ContentHash,
    pub oracle_utility_milli: i64,
}

/// Aggregate policy behavior across several held-out utility worlds. Stability is the mean
/// pairwise Jaccard overlap of selected action sets; it describes policy sensitivity, not
/// scientific reproducibility of a biological result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineStressPolicyMetric {
    pub policy: GliomaEngineEvaluationPolicy,
    pub scenario_count: u16,
    pub mean_utility_milli: i64,
    pub lower_quartile_utility_milli: i64,
    pub worst_utility_milli: i64,
    pub mean_regret_to_oracle_milli: i64,
    pub selection_stability_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineStressEvaluation {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub scenario_order: Vec<String>,
    pub scenarios: Vec<GliomaEngineStressScenarioSummary>,
    pub metrics: Vec<GliomaEngineStressPolicyMetric>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

/// A synthetic, replayable provider outcome used only to evaluate the closed-loop engine. The
/// trace is never treated as a biological observation and never authorizes an external effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEngineTraceDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineTraceOutcome {
    pub disposition: GliomaEngineTraceDisposition,
    #[serde(default)]
    pub retryable: bool,
}

/// One policy/scenario replay of the real adaptive engine loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineTraceScenarioSummary {
    pub scenario_id: String,
    pub policy: GliomaDirectorFocus,
    pub engine_digest: ContentHash,
    pub disposition: GliomaAutonomousResearchEngineDisposition,
    pub stop_reason: GliomaAutonomousResearchEngineStopReason,
    pub completed_stage_count: u16,
    pub budget_spent_units: u32,
    pub negative_result_count: u16,
    pub failed_result_count: u16,
}

/// Aggregate end-to-end workflow behavior over named provider-outcome traces. This evaluates
/// autonomous replanning, not a static action score, while keeping all traces synthetic and
/// outside biological evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaEngineTracePolicyMetric {
    pub policy: GliomaDirectorFocus,
    pub scenario_count: u16,
    pub mean_completed_stage_milli: i64,
    pub worst_completed_stage: u16,
    pub qualified_rate_milli: u16,
    pub mean_budget_spent_units: u32,
    pub mean_negative_result_count: i64,
    pub mean_failed_result_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineTraceEvaluation {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub scenario_order: Vec<String>,
    pub policy_order: Vec<GliomaDirectorFocus>,
    pub scenarios: Vec<GliomaEngineTraceScenarioSummary>,
    pub metrics: Vec<GliomaEngineTracePolicyMetric>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEngineEvaluationError {
    #[error("glioma engine evaluation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma engine evaluation planning failed: {0}")]
    Planning(String),
    #[error("glioma engine evaluation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma engine evaluation digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &GliomaAutonomousResearchEngineEvaluation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "held_out_truth_digest": output.held_out_truth_digest,
        "oracle_selected_order": output.oracle_selected_order,
        "oracle_utility_milli": output.oracle_utility_milli,
        "metrics": output.metrics,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
    })
}

fn stress_digest_input(
    output: &GliomaAutonomousResearchEngineStressEvaluation,
) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "scenario_order": output.scenario_order,
        "scenarios": output.scenarios,
        "metrics": output.metrics,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
    })
}

fn trace_digest_input(output: &GliomaAutonomousResearchEngineTraceEvaluation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "scenario_order": output.scenario_order,
        "policy_order": output.policy_order,
        "scenarios": output.scenarios,
        "metrics": output.metrics,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl GliomaAutonomousResearchEngineEvaluation {
    pub fn validate(&self) -> Result<(), GliomaEngineEvaluationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.plan_digest.as_str().len() != 64
            || self.held_out_truth_digest.as_str().len() != 64
            || self
                .oracle_selected_order
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.oracle_selected_order.len()
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.metrics.is_empty()
            || self
                .metrics
                .windows(2)
                .any(|pair| pair[0].policy >= pair[1].policy)
            || self.metrics.iter().any(|metric| {
                metric.selected_order.iter().collect::<BTreeSet<_>>().len()
                    != metric.selected_order.len()
                    || (!metric.selected_order.is_empty() && metric.cost_units == 0)
            })
        {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "evaluation identity, ordering, metric, or digest bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "evaluation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

impl GliomaAutonomousResearchEngineStressEvaluation {
    pub fn validate(&self) -> Result<(), GliomaEngineEvaluationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != STRESS_OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.plan_digest.as_str().len() != 64
            || self.scenarios.is_empty()
            || self.scenarios.len() > MAX_SCENARIOS
            || self.scenario_order
                != self
                    .scenarios
                    .iter()
                    .map(|s| s.scenario_id.clone())
                    .collect::<Vec<_>>()
            || !canonical(&self.scenario_order)
            || self.scenarios.iter().any(|scenario| {
                scenario.scenario_id.trim().is_empty()
                    || scenario.evaluation_digest.as_str().len() != 64
                    || scenario.held_out_truth_digest.as_str().len() != 64
            })
            || self
                .scenarios
                .windows(2)
                .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
            || self.metrics.len() != 4
            || self
                .metrics
                .windows(2)
                .any(|pair| pair[0].policy >= pair[1].policy)
            || self.metrics.iter().any(|metric| {
                metric.scenario_count != self.scenarios.len() as u16
                    || metric.selection_stability_milli > 1_000
            })
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
        {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "stress-evaluation identity, scenario ordering, policy metrics, or digest bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&stress_digest_input(self))
            .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "stress evaluation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

impl GliomaAutonomousResearchEngineTraceEvaluation {
    pub fn validate(&self) -> Result<(), GliomaEngineEvaluationError> {
        let scenario_ids = self
            .scenarios
            .iter()
            .map(|scenario| scenario.scenario_id.clone())
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != TRACE_OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.scenarios.is_empty()
            || scenario_ids.len() > MAX_SCENARIOS
            || self.scenario_order != scenario_ids.iter().cloned().collect::<Vec<_>>()
            || !canonical(&self.scenario_order)
            || self.scenarios.windows(2).any(|pair| {
                (pair[0].scenario_id.as_str(), pair[0].policy)
                    >= (pair[1].scenario_id.as_str(), pair[1].policy)
            })
            || self.policy_order.is_empty()
            || !self.policy_order.windows(2).all(|pair| pair[0] < pair[1])
            || self.scenarios.len() != scenario_ids.len() * self.policy_order.len()
            || self.metrics.len() != self.policy_order.len()
            || self.metrics.iter().enumerate().any(|(index, metric)| {
                metric.policy != self.policy_order[index]
                    || metric.scenario_count
                        != self
                            .scenarios
                            .iter()
                            .filter(|row| row.policy == metric.policy)
                            .count() as u16
                    || metric.qualified_rate_milli > 1_000
                    || metric.worst_completed_stage > 14
            })
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
        {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "trace-evaluation identity, ordering, policy metrics, or bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&trace_digest_input(self))
            .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEngineEvaluationError::InvalidOutput(
                "trace evaluation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn director_request(
    request: &GliomaAutonomousResearchEngineRequest,
) -> GliomaResearchDirectorRequest {
    GliomaResearchDirectorRequest {
        intent: request.intent.clone(),
        focus: request.focus,
        completed_checkpoints: request.completed_checkpoints.clone(),
        budget_units: request.budget_units,
        max_actions: request.max_actions,
        approval_granted: request.approval_granted,
        allow_instrument_execution: request.allow_instrument_execution,
        allow_federation: request.allow_federation,
        selection_weights: request.selection_weights,
        max_retries: request.max_retries,
        require_artifacts: request.require_artifacts,
        outcome_summaries: request.outcome_summaries.clone(),
    }
}

fn weighted_score(candidate: &GliomaActionCandidate, weights: GliomaSelectionWeights) -> u64 {
    u64::from(candidate.information_gain_milli) * u64::from(weights.information_gain)
        + u64::from(candidate.frontier_novelty_milli) * u64::from(weights.frontier_novelty)
        + u64::from(candidate.workflow_leverage_milli) * u64::from(weights.workflow_leverage)
        + u64::from(candidate.cross_stage_unlock_milli) * u64::from(weights.cross_stage_unlock)
        + u64::from(candidate.reproducibility_safety_milli)
            * u64::from(weights.reproducibility_safety)
        + u64::from(candidate.federation_value_milli) * u64::from(weights.federation_value)
        + u64::from(candidate.feasibility_milli) * u64::from(weights.feasibility)
}

fn usable_candidates(
    candidates: &[GliomaActionCandidate],
    blocked: &BTreeSet<String>,
    completed: &BTreeSet<String>,
) -> Vec<GliomaActionCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            !blocked.contains(&candidate.action_id) && !completed.contains(&candidate.action_id)
        })
        .cloned()
        .collect()
}

fn select_ranked(
    request: &GliomaAutonomousResearchEngineRequest,
    candidates: &[GliomaActionCandidate],
    completed: &BTreeSet<String>,
    ranking: &[String],
) -> Vec<String> {
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.action_id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut selected = completed.clone();
    let mut order = Vec::new();
    let mut spent = 0_u32;
    for id in ranking {
        let Some(candidate) = by_id.get(id.as_str()) else {
            continue;
        };
        if order.len() >= usize::from(request.max_actions)
            || spent.saturating_add(candidate.cost_units) > request.budget_units
            || candidate
                .depends_on
                .iter()
                .any(|dependency| !selected.contains(dependency))
        {
            continue;
        }
        order.push(id.clone());
        selected.insert(id.clone());
        spent = spent.saturating_add(candidate.cost_units);
    }
    order
}

#[derive(Clone)]
struct OracleState {
    selected: Vec<String>,
    selected_set: BTreeSet<String>,
    spent: u32,
    utility: i128,
}

fn oracle_better(left: &OracleState, right: &OracleState) -> bool {
    left.utility > right.utility
        || (left.utility == right.utility
            && (left.selected.len() > right.selected.len()
                || (left.selected.len() == right.selected.len()
                    && (left.spent < right.spent
                        || (left.spent == right.spent && left.selected < right.selected)))))
}

fn oracle_beam(
    request: &GliomaAutonomousResearchEngineRequest,
    candidates: &[GliomaActionCandidate],
    completed: &BTreeSet<String>,
    held_out: &BTreeMap<String, i64>,
) -> OracleState {
    let mut beam = vec![OracleState {
        selected: Vec::new(),
        selected_set: completed.clone(),
        spent: 0,
        utility: 0,
    }];
    for candidate in candidates {
        let mut expanded = beam.clone();
        for state in &beam {
            if state.selected.len() >= usize::from(request.max_actions)
                || state.spent.saturating_add(candidate.cost_units) > request.budget_units
                || candidate
                    .depends_on
                    .iter()
                    .any(|dependency| !state.selected_set.contains(dependency))
            {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(candidate.action_id.clone());
            let mut selected_set = state.selected_set.clone();
            selected_set.insert(candidate.action_id.clone());
            expanded.push(OracleState {
                selected,
                selected_set,
                spent: state.spent.saturating_add(candidate.cost_units),
                utility: state.utility.saturating_add(i128::from(
                    held_out
                        .get(&candidate.action_id)
                        .copied()
                        .unwrap_or_default(),
                )),
            });
        }
        expanded.sort_by(|left, right| {
            if oracle_better(left, right) {
                std::cmp::Ordering::Less
            } else if oracle_better(right, left) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let mut seen = BTreeSet::new();
        expanded.retain(|state| seen.insert(state.selected.clone()));
        expanded.truncate(ORACLE_BEAM_WIDTH);
        beam = expanded;
    }
    beam.into_iter()
        .max_by(|left, right| {
            if oracle_better(left, right) {
                std::cmp::Ordering::Greater
            } else if oracle_better(right, left) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .unwrap_or(OracleState {
            selected: Vec::new(),
            selected_set: completed.clone(),
            spent: 0,
            utility: 0,
        })
}

fn metric(
    policy: GliomaEngineEvaluationPolicy,
    selected_order: Vec<String>,
    candidates: &BTreeMap<String, GliomaActionCandidate>,
    held_out: &BTreeMap<String, i64>,
    oracle_utility: i64,
) -> GliomaEngineEvaluationMetric {
    let cost_units = selected_order
        .iter()
        .filter_map(|id| candidates.get(id))
        .map(|candidate| candidate.cost_units)
        .sum();
    let utility = selected_order
        .iter()
        .filter_map(|id| held_out.get(id))
        .copied()
        .sum::<i64>();
    GliomaEngineEvaluationMetric {
        policy,
        selected_order,
        cost_units,
        held_out_utility_milli: utility,
        regret_to_oracle_milli: oracle_utility.saturating_sub(utility),
    }
}

fn mean_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let total = values
        .iter()
        .fold(0_i128, |sum, value| sum.saturating_add(i128::from(*value)));
    (total / i128::try_from(values.len()).unwrap_or(1))
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn jaccard_milli(left: &[String], right: &[String]) -> u16 {
    let left = left.iter().collect::<BTreeSet<_>>();
    let right = right.iter().collect::<BTreeSet<_>>();
    let union = left.union(&right).count();
    if union == 0 {
        return 1_000;
    }
    (left.intersection(&right).count() as u64 * 1_000 / union as u64) as u16
}

/// Evaluate policy robustness across named held-out utility scenarios without invoking a
/// provider. Every scenario is evaluated against the same compiled frontier and bounded oracle;
/// no scenario truth enters planning.
pub fn evaluate_glioma_autonomous_research_engine_scenarios(
    request: &GliomaAutonomousResearchEngineRequest,
    scenarios: &BTreeMap<String, BTreeMap<String, i64>>,
) -> Result<GliomaAutonomousResearchEngineStressEvaluation, GliomaEngineEvaluationError> {
    if scenarios.is_empty()
        || scenarios.len() > MAX_SCENARIOS
        || scenarios.keys().any(|id| id.trim().is_empty())
        || scenarios.values().any(|values| values.is_empty())
    {
        return Err(GliomaEngineEvaluationError::InvalidRequest(
            "stress evaluation requires a bounded non-empty set of named held-out scenarios".into(),
        ));
    }
    let mut scenario_outputs = Vec::with_capacity(scenarios.len());
    for (scenario_id, held_out) in scenarios {
        let evaluation = evaluate_glioma_autonomous_research_engine(request, held_out)?;
        scenario_outputs.push((scenario_id.clone(), evaluation));
    }
    let plan_digest = scenario_outputs[0].1.plan_digest.clone();
    if scenario_outputs
        .iter()
        .any(|(_, evaluation)| evaluation.plan_digest != plan_digest)
    {
        return Err(GliomaEngineEvaluationError::Planning(
            "stress scenarios compiled different plan digests".into(),
        ));
    }
    let scenario_order = scenario_outputs
        .iter()
        .map(|(scenario_id, _)| scenario_id.clone())
        .collect::<Vec<_>>();
    let scenarios_summary = scenario_outputs
        .iter()
        .map(
            |(scenario_id, evaluation)| GliomaEngineStressScenarioSummary {
                scenario_id: scenario_id.clone(),
                evaluation_digest: evaluation.digest.clone(),
                held_out_truth_digest: evaluation.held_out_truth_digest.clone(),
                oracle_utility_milli: evaluation.oracle_utility_milli,
            },
        )
        .collect::<Vec<_>>();
    let policies = [
        GliomaEngineEvaluationPolicy::AuroraEngine,
        GliomaEngineEvaluationPolicy::ScoreGreedy,
        GliomaEngineEvaluationPolicy::CoverageFirst,
        GliomaEngineEvaluationPolicy::OracleBeam,
    ];
    let mut metrics = Vec::with_capacity(policies.len());
    let mut negative_evidence = BTreeSet::new();
    for policy in policies {
        let rows = scenario_outputs
            .iter()
            .map(|(_, evaluation)| {
                evaluation
                    .metrics
                    .iter()
                    .find(|metric| metric.policy == policy)
                    .expect("every evaluation emits all policies")
            })
            .collect::<Vec<_>>();
        let mut utilities = rows
            .iter()
            .map(|metric| metric.held_out_utility_milli)
            .collect::<Vec<_>>();
        utilities.sort();
        let regrets = rows
            .iter()
            .map(|metric| metric.regret_to_oracle_milli)
            .collect::<Vec<_>>();
        let stability = if rows.len() < 2 {
            1_000
        } else {
            let mut total = 0_u64;
            let mut pairs = 0_u64;
            for left in 0..rows.len() {
                for right in (left + 1)..rows.len() {
                    total = total.saturating_add(u64::from(jaccard_milli(
                        &rows[left].selected_order,
                        &rows[right].selected_order,
                    )));
                    pairs = pairs.saturating_add(1);
                }
            }
            (total / pairs.max(1)) as u16
        };
        let mean_utility = mean_i64(&utilities);
        let worst_utility = *utilities.first().unwrap_or(&0);
        if mean_utility < 0 {
            negative_evidence.insert(format!("stress:{policy:?}:negative-mean-held-out-utility"));
        }
        if worst_utility < 0 {
            negative_evidence.insert(format!(
                "stress:{policy:?}:negative-worst-case-held-out-utility"
            ));
        }
        metrics.push(GliomaEngineStressPolicyMetric {
            policy,
            scenario_count: rows.len() as u16,
            mean_utility_milli: mean_utility,
            lower_quartile_utility_milli: utilities[(utilities.len() - 1) / 4],
            worst_utility_milli: worst_utility,
            mean_regret_to_oracle_milli: mean_i64(&regrets),
            selection_stability_milli: stability,
        });
    }
    let mut output = GliomaAutonomousResearchEngineStressEvaluation {
        feature_id: FEATURE_ID.into(),
        output_schema: STRESS_OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        plan_digest,
        scenario_order,
        scenarios: scenarios_summary,
        metrics,
        uncertainty: vec![
            "each-scenario-oracle-is-a-bounded-dependency-aware-beam-upper-bound".into(),
            "held-out-scenario-utilities-are-evaluation-only-not-biological-evidence".into(),
            "selection-stability-is-set-overlap-not-biological-reproducibility".into(),
            "stress-evaluation-plans-only-and-does-not-invoke-a-provider".into(),
        ],
        negative_evidence: negative_evidence.into_iter().collect(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-engine-stress-evaluation"),
    };
    output.digest = ContentHash::of_value(&stress_digest_input(&output))
        .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

struct TraceExecutor<'a> {
    outcomes: &'a BTreeMap<String, GliomaEngineTraceOutcome>,
}

impl GliomaActionExecutor for TraceExecutor<'_> {
    fn execute_action(
        &mut self,
        candidate: &GliomaActionCandidate,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        let outcome = self
            .outcomes
            .get(&candidate.action_id)
            .or_else(|| self.outcomes.get(candidate.stage_kind.stage_id()))
            .copied()
            .unwrap_or(GliomaEngineTraceOutcome {
                disposition: GliomaEngineTraceDisposition::Failed,
                retryable: false,
            });
        if outcome.disposition == GliomaEngineTraceDisposition::Failed {
            return Err(ActionExecutionFailure {
                reason: format!(
                    "trace outcome is failed for {}{}",
                    candidate.action_id,
                    if self.outcomes.contains_key(&candidate.action_id)
                        || self.outcomes.contains_key(candidate.stage_kind.stage_id())
                    {
                        ""
                    } else {
                        " (unmapped action)"
                    }
                ),
                retryable: outcome.retryable,
            });
        }
        let mut dry_run = DryRunGliomaActionExecutor;
        let mut result = dry_run
            .execute_action(candidate, attempt)
            .map_err(|failure| ActionExecutionFailure {
                reason: failure.reason,
                retryable: false,
            })?;
        result.disposition = match outcome.disposition {
            GliomaEngineTraceDisposition::Completed => ActionExecutionDisposition::Completed,
            GliomaEngineTraceDisposition::Negative => ActionExecutionDisposition::Negative,
            GliomaEngineTraceDisposition::Partial => ActionExecutionDisposition::Partial,
            GliomaEngineTraceDisposition::Failed => unreachable!("failed traces return above"),
        };
        result.note = format!(
            "synthetic trace replay: {:?}; no provider or biological effect occurred",
            outcome.disposition
        );
        result
            .uncertainty
            .push("synthetic-trace-evaluation-only".into());
        if outcome.disposition == GliomaEngineTraceDisposition::Negative {
            result
                .negative_evidence
                .push(format!("trace-negative-result:{}", candidate.action_id));
        }
        Ok(result)
    }

    fn execute_action_with_context(
        &mut self,
        candidate: &GliomaActionCandidate,
        _context: &GliomaActionExecutionContext,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        self.execute_action(candidate, attempt)
    }
}

/// Replay the actual adaptive engine loop over named synthetic provider-outcome traces. The same
/// trace is run through every closed focus policy; no static utility oracle is consulted and no
/// provider is invoked. Missing action outcomes fail closed instead of being treated as success.
pub fn evaluate_glioma_autonomous_research_engine_traces(
    request: &GliomaAutonomousResearchEngineRequest,
    scenarios: &BTreeMap<String, BTreeMap<String, GliomaEngineTraceOutcome>>,
) -> Result<GliomaAutonomousResearchEngineTraceEvaluation, GliomaEngineEvaluationError> {
    if scenarios.is_empty()
        || scenarios.len() > MAX_SCENARIOS
        || scenarios.keys().any(|id| id.trim().is_empty())
        || scenarios.values().any(|trace| trace.is_empty())
        || request.max_cycles == 0
        || request.max_cycles > MAX_CYCLES
    {
        return Err(GliomaEngineEvaluationError::InvalidRequest(
            "trace evaluation requires bounded non-empty named traces and a valid cycle bound"
                .into(),
        ));
    }
    let policies = [
        GliomaDirectorFocus::Adaptive,
        GliomaDirectorFocus::EvidenceFirst,
        GliomaDirectorFocus::MechanismFirst,
        GliomaDirectorFocus::ExperimentFirst,
        GliomaDirectorFocus::ComputationFirst,
        GliomaDirectorFocus::ReplicationFirst,
        GliomaDirectorFocus::FullProgram,
    ];
    let mut summaries = Vec::with_capacity(scenarios.len() * policies.len());
    let mut negative_evidence = BTreeSet::new();
    for (scenario_id, trace) in scenarios {
        for policy in policies {
            let mut policy_request = request.clone();
            policy_request.focus = policy;
            let mut executor = TraceExecutor { outcomes: trace };
            let run = execute_glioma_autonomous_research_engine(&policy_request, &mut executor)
                .map_err(|error| {
                    GliomaEngineEvaluationError::Planning(format!(
                        "trace {scenario_id} policy {policy:?} failed: {error}"
                    ))
                })?;
            let negative_result_count = run
                .cycles
                .iter()
                .flat_map(|cycle| cycle.director.execution.as_ref())
                .flat_map(|execution| execution.results.iter())
                .filter(|result| result.disposition == ActionExecutionDisposition::Negative)
                .count() as u16;
            let failed_result_count = run
                .cycles
                .iter()
                .flat_map(|cycle| cycle.director.execution.as_ref())
                .flat_map(|execution| execution.results.iter())
                .filter(|result| {
                    matches!(
                        result.disposition,
                        ActionExecutionDisposition::Failed | ActionExecutionDisposition::Skipped
                    )
                })
                .count() as u16;
            if negative_result_count > 0 {
                negative_evidence.insert(format!(
                    "trace:{scenario_id}:{policy:?}:negative-result-retained"
                ));
            }
            if failed_result_count > 0 {
                negative_evidence.insert(format!(
                    "trace:{scenario_id}:{policy:?}:failed-or-skipped-work"
                ));
            }
            if run.disposition != GliomaAutonomousResearchEngineDisposition::Completed {
                negative_evidence.insert(format!("trace:{scenario_id}:{policy:?}:not-qualified"));
            }
            summaries.push(GliomaEngineTraceScenarioSummary {
                scenario_id: scenario_id.clone(),
                policy,
                engine_digest: run.digest,
                disposition: run.disposition,
                stop_reason: run.stop_reason,
                completed_stage_count: run.completed_stage_order.len() as u16,
                budget_spent_units: run.budget_spent_units,
                negative_result_count,
                failed_result_count,
            });
        }
    }
    let mut metrics = Vec::with_capacity(policies.len());
    for policy in policies {
        let rows = summaries
            .iter()
            .filter(|row| row.policy == policy)
            .collect::<Vec<_>>();
        let progress = rows
            .iter()
            .map(|row| i64::from(row.completed_stage_count) * 1_000 / 14)
            .collect::<Vec<_>>();
        let worst_completed_stage = rows
            .iter()
            .map(|row| row.completed_stage_count)
            .min()
            .unwrap_or(0);
        let qualified_count = rows
            .iter()
            .filter(|row| row.disposition == GliomaAutonomousResearchEngineDisposition::Completed)
            .count();
        metrics.push(GliomaEngineTracePolicyMetric {
            policy,
            scenario_count: rows.len() as u16,
            mean_completed_stage_milli: mean_i64(&progress),
            worst_completed_stage,
            qualified_rate_milli: (qualified_count * 1_000 / rows.len().max(1)) as u16,
            mean_budget_spent_units: (rows
                .iter()
                .map(|row| u64::from(row.budget_spent_units))
                .sum::<u64>()
                / rows.len().max(1) as u64) as u32,
            mean_negative_result_count: mean_i64(
                &rows
                    .iter()
                    .map(|row| i64::from(row.negative_result_count))
                    .collect::<Vec<_>>(),
            ),
            mean_failed_result_count: mean_i64(
                &rows
                    .iter()
                    .map(|row| i64::from(row.failed_result_count))
                    .collect::<Vec<_>>(),
            ),
        });
    }
    let mut output = GliomaAutonomousResearchEngineTraceEvaluation {
        feature_id: FEATURE_ID.into(),
        output_schema: TRACE_OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        scenario_order: scenarios.keys().cloned().collect(),
        policy_order: policies.to_vec(),
        scenarios: summaries,
        metrics,
        uncertainty: vec![
            "all-trace-outcomes-are-synthetic-evaluation-inputs-not-biological-evidence".into(),
            "missing-action-outcomes-fail-closed-instead-of-defaulting-to-success".into(),
            "policy-comparisons-replay-the-real-adaptive-engine-loop".into(),
            "trace-evaluation-invokes-no-provider-or-external-effect".into(),
        ],
        negative_evidence: negative_evidence.into_iter().collect(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-engine-trace-evaluation"),
    };
    output.digest = ContentHash::of_value(&trace_digest_input(&output))
        .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Evaluate the autonomous engine's action policy without executing any provider.
pub fn evaluate_glioma_autonomous_research_engine(
    request: &GliomaAutonomousResearchEngineRequest,
    held_out_utility_milli: &BTreeMap<String, i64>,
) -> Result<GliomaAutonomousResearchEngineEvaluation, GliomaEngineEvaluationError> {
    if request.mission_id.trim().is_empty()
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_cycles == 0
        || request.max_cycles > MAX_CYCLES
        || held_out_utility_milli.is_empty()
        || held_out_utility_milli
            .values()
            .any(|value| value.unsigned_abs() > VALUE_LIMIT as u64)
    {
        return Err(GliomaEngineEvaluationError::InvalidRequest(
            "engine identity and bounded budget/action/cycle limits plus non-empty held-out utility are required".into(),
        ));
    }
    let director = plan_glioma_research_director(&director_request(request))
        .map_err(|error| GliomaEngineEvaluationError::Planning(error.to_string()))?;
    let candidates = director
        .actions
        .iter()
        .map(|action| action.candidate.clone())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(GliomaEngineEvaluationError::InvalidRequest(
            "the request compiles no runnable glioma actions".into(),
        ));
    }
    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if candidate_ids
        != held_out_utility_milli
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
    {
        return Err(GliomaEngineEvaluationError::InvalidRequest(
            "held-out utility keys must match the compiled action frontier".into(),
        ));
    }
    let completed = request
        .completed_checkpoints
        .iter()
        .map(|checkpoint: &GliomaDirectorCheckpoint| checkpoint.stage_kind.stage_id().to_string())
        .collect::<BTreeSet<_>>();
    let blocked = director
        .selection
        .as_ref()
        .map(|selection| {
            selection
                .blocked_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let candidates = usable_candidates(&candidates, &blocked, &completed);
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut score_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    score_order.sort_by(|left, right| {
        let l = &by_id[left];
        let r = &by_id[right];
        (weighted_score(r, request.selection_weights) / u64::from(r.cost_units))
            .cmp(&(weighted_score(l, request.selection_weights) / u64::from(l.cost_units)))
            .then_with(|| left.cmp(right))
    });
    let mut coverage_order = candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    coverage_order.sort_by(|left, right| {
        by_id[left]
            .stage_kind
            .cmp(&by_id[right].stage_kind)
            .then_with(|| left.cmp(right))
    });
    let aurora_order = director
        .selection
        .as_ref()
        .map(|selection| {
            selection
                .selected_order
                .iter()
                .filter(|id| by_id.contains_key(*id))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let oracle = oracle_beam(request, &candidates, &completed, held_out_utility_milli);
    let oracle_utility = i64::try_from(oracle.utility).unwrap_or_else(|_| {
        if oracle.utility.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    });
    let mut metrics = vec![
        metric(
            GliomaEngineEvaluationPolicy::AuroraEngine,
            aurora_order,
            &by_id,
            held_out_utility_milli,
            oracle_utility,
        ),
        metric(
            GliomaEngineEvaluationPolicy::ScoreGreedy,
            select_ranked(request, &candidates, &completed, &score_order),
            &by_id,
            held_out_utility_milli,
            oracle_utility,
        ),
        metric(
            GliomaEngineEvaluationPolicy::CoverageFirst,
            select_ranked(request, &candidates, &completed, &coverage_order),
            &by_id,
            held_out_utility_milli,
            oracle_utility,
        ),
        metric(
            GliomaEngineEvaluationPolicy::OracleBeam,
            oracle.selected.clone(),
            &by_id,
            held_out_utility_milli,
            oracle_utility,
        ),
    ];
    metrics.sort_by_key(|metric| metric.policy);
    let held_out_truth_digest = ContentHash::of_value(
        &serde_json::to_value(held_out_utility_milli)
            .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?,
    )
    .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
    let mut uncertainty = BTreeSet::from([
        "held-out-utility-is-evaluation-only-not-biological-evidence".to_string(),
        "oracle-is-a-bounded-dependency-aware-beam-upper-bound-not-a-global-optimum".to_string(),
        "evaluation-plans-only-and-does-not-invoke-a-provider".to_string(),
    ]);
    if !director.hold_order.is_empty() || !director.approval_order.is_empty() {
        uncertainty.insert("compiled-frontier-has-held-or-approval-gated-stages".into());
    }
    let negative_evidence = if metrics
        .iter()
        .find(|metric| metric.policy == GliomaEngineEvaluationPolicy::AuroraEngine)
        .is_some_and(|metric| metric.held_out_utility_milli < 0)
    {
        vec!["autonomous-engine-negative-held-out-utility".to_string()]
    } else {
        Vec::new()
    };
    let mut output = GliomaAutonomousResearchEngineEvaluation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        plan_digest: director.workflow_plan.plan_digest,
        held_out_truth_digest,
        oracle_selected_order: oracle.selected,
        oracle_utility_milli: oracle_utility,
        metrics,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence,
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-engine-evaluation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaEngineEvaluationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::autonomous_engine::tests::request;
    use std::collections::BTreeMap;

    #[test]
    fn compares_engine_policy_with_baselines_and_replays() {
        let request = request();
        let director = plan_glioma_research_director(&director_request(&request)).unwrap();
        let utilities = director
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                (
                    action.candidate.action_id.clone(),
                    1000 - (index as i64 * 17),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let first = evaluate_glioma_autonomous_research_engine(&request, &utilities).unwrap();
        let second = evaluate_glioma_autonomous_research_engine(&request, &utilities).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.metrics.len(), 4);
        assert!(first
            .metrics
            .iter()
            .any(|metric| metric.policy == GliomaEngineEvaluationPolicy::OracleBeam));
        first.validate().unwrap();
    }

    #[test]
    fn refuses_truth_that_does_not_match_compiled_frontier() {
        let request = request();
        let error = evaluate_glioma_autonomous_research_engine(
            &request,
            &BTreeMap::from([(String::from("unknown-action"), 1)]),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            GliomaEngineEvaluationError::Planning(_)
                | GliomaEngineEvaluationError::InvalidRequest(_)
        ));
    }

    #[test]
    fn stress_evaluation_reports_worst_case_and_selection_stability() {
        let request = request();
        let director = plan_glioma_research_director(&director_request(&request)).unwrap();
        let baseline = director
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| (action.candidate.action_id.clone(), 900 - index as i64 * 11))
            .collect::<BTreeMap<_, _>>();
        let stress = baseline
            .iter()
            .map(|(action_id, utility)| (action_id.clone(), -*utility))
            .collect::<BTreeMap<_, _>>();
        let scenarios = BTreeMap::from([
            ("baseline".into(), baseline),
            ("stress-negative".into(), stress),
        ]);
        let first =
            evaluate_glioma_autonomous_research_engine_scenarios(&request, &scenarios).unwrap();
        let second =
            evaluate_glioma_autonomous_research_engine_scenarios(&request, &scenarios).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.scenario_order, vec!["baseline", "stress-negative"]);
        assert_eq!(first.metrics.len(), 4);
        assert!(first
            .metrics
            .iter()
            .all(|metric| metric.scenario_count == 2));
        assert!(first
            .metrics
            .iter()
            .any(|metric| metric.worst_utility_milli < 0));
        first.validate().unwrap();
    }

    #[test]
    fn trace_evaluation_replays_adaptive_engine_and_retains_negative_results() {
        let request = request();
        let director = plan_glioma_research_director(&director_request(&request)).unwrap();
        let complete = director
            .actions
            .iter()
            .map(|action| {
                (
                    action.candidate.action_id.clone(),
                    GliomaEngineTraceOutcome {
                        disposition: GliomaEngineTraceDisposition::Completed,
                        retryable: false,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut negative = complete.clone();
        let negative_action = director.actions[0].candidate.action_id.clone();
        negative.insert(
            negative_action,
            GliomaEngineTraceOutcome {
                disposition: GliomaEngineTraceDisposition::Negative,
                retryable: false,
            },
        );
        let scenarios = BTreeMap::from([
            ("complete-world".into(), complete),
            ("negative-world".into(), negative),
        ]);
        let first =
            evaluate_glioma_autonomous_research_engine_traces(&request, &scenarios).unwrap();
        let second =
            evaluate_glioma_autonomous_research_engine_traces(&request, &scenarios).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.scenarios.len(), 14);
        assert_eq!(first.metrics.len(), 7);
        assert!(first
            .scenarios
            .iter()
            .any(|row| row.negative_result_count > 0));
        assert!(first
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-result-retained")));
        first.validate().unwrap();
    }
}
