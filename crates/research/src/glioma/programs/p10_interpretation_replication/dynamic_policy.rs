//! Deterministic longitudinal policy evaluation for preclinical glioma workflows.
//!
//! A research engine needs to compare *sequences* of local perturbations, not only a single
//! endpoint contrast. This module evaluates declared finite-horizon experiment policies with
//! self-normalized inverse-propensity weighting. It keeps positivity failures, support coverage,
//! weight clipping, effective sample size, and leave-one-trajectory-out instability explicit.
//! The actions are preclinical assay or computation actions; this is not a clinical treatment
//! policy and never makes a patient-facing decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaDynamicPolicyEvaluation1@1";
pub const MAX_POLICIES: usize = 64;
pub const MAX_RULES_PER_POLICY: usize = 512;
pub const MAX_TRAJECTORIES: usize = 4_096;
pub const MAX_OBSERVATIONS_PER_TRAJECTORY: usize = 64;
pub const MAX_HORIZON: u16 = 32;
const SCALE: u128 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub horizon: u16,
    pub reference_policy_id: String,
    pub maximize_effect: bool,
    pub min_trajectories: usize,
    pub min_coverage_milli: u16,
    pub min_propensity_milli: u16,
    pub max_weight_milli: u64,
    pub min_effect_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyRule {
    pub time_step: u16,
    pub state_key: String,
    pub action_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyCandidate {
    pub policy_id: String,
    pub label: String,
    pub rules: Vec<DynamicPolicyRule>,
    pub risk_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyObservation {
    pub observation_id: String,
    pub time_step: u16,
    pub state_key: String,
    pub action_id: String,
    pub outcome_milli: i64,
    pub propensity_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyTrajectory {
    pub trajectory_id: String,
    pub unit_id: String,
    pub model_system: GliomaModelSystem,
    pub observations: Vec<DynamicPolicyObservation>,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyContribution {
    pub trajectory_id: String,
    pub included: bool,
    pub weight_milli: u64,
    pub return_milli: i64,
    pub matched_steps: u16,
    pub mismatch_steps: u16,
    pub positivity_violation: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPolicyScoreDisposition {
    Qualified,
    Negative,
    Unresolved,
    PositivityBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyScore {
    pub policy_id: String,
    pub label: String,
    pub weighted_return_milli: i64,
    pub reference_return_milli: i64,
    pub effect_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub uncertainty_milli: u64,
    pub effective_sample_size_milli: u64,
    pub coverage_milli: u16,
    pub max_importance_weight_milli: u64,
    pub leave_one_out_shift_milli: u64,
    pub included_trajectory_count: usize,
    pub positivity_violation_count: usize,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub contributions: Vec<DynamicPolicyContribution>,
    pub eligible: bool,
    pub disposition: DynamicPolicyScoreDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPolicyDisposition {
    Qualified,
    Negative,
    Unresolved,
    PositivityBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicPolicyEvaluation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub reference_policy_id: String,
    pub policy_order: Vec<String>,
    pub ranking_order: Vec<String>,
    pub selected_policy_id: Option<String>,
    pub scores: Vec<DynamicPolicyScore>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: DynamicPolicyDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DynamicPolicyError {
    #[error("dynamic policy request is invalid: {0}")]
    InvalidRequest(String),
    #[error("dynamic policy candidate is invalid: {0}")]
    InvalidPolicy(String),
    #[error("dynamic policy trajectory is invalid: {0}")]
    InvalidTrajectory(String),
    #[error("dynamic policy output is invalid: {0}")]
    InvalidOutput(String),
    #[error("dynamic policy digest failed: {0}")]
    Digest(String),
}

fn unique_sorted(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_any_order(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn integer_sqrt(value: u128) -> u128 {
    if value <= 1 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value;
    while low <= high {
        let mid = low.saturating_add(high).saturating_div(2);
        if mid <= value / mid {
            low = mid.saturating_add(1);
        } else {
            high = mid.saturating_sub(1);
        }
    }
    high
}

fn digest_input(output: &DynamicPolicyEvaluation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "reference_policy_id": output.reference_policy_id,
        "policy_order": output.policy_order,
        "ranking_order": output.ranking_order,
        "selected_policy_id": output.selected_policy_id,
        "scores": output.scores,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl DynamicPolicyEvaluation {
    pub fn validate(&self) -> Result<(), DynamicPolicyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.reference_policy_id.trim().is_empty()
            || !unique_sorted(&self.policy_order)
            || !unique_any_order(&self.ranking_order)
            || self
                .ranking_order
                .iter()
                .any(|id| !self.policy_order.contains(id))
            || !unique_sorted(&self.negative_evidence)
            || !unique_sorted(&self.uncertainty)
            || self.scores.len() != self.policy_order.len()
            || self
                .scores
                .windows(2)
                .any(|pair| pair[0].policy_id >= pair[1].policy_id)
            || self.scores.iter().any(|score| {
                score.policy_id.trim().is_empty()
                    || score.label.trim().is_empty()
                    || score.interval_low_milli > score.interval_high_milli
                    || score.coverage_milli > 1_000
                    || score.risk_milli > 1_000
                    || !unique_sorted(
                        &score
                            .contributions
                            .iter()
                            .map(|contribution| contribution.trajectory_id.clone())
                            .collect::<Vec<_>>(),
                    )
            })
        {
            return Err(DynamicPolicyError::InvalidOutput(
                "identity, policy ranking, score partition, interval, coverage, or contribution ordering is invalid".into(),
            ));
        }
        let ids = self
            .scores
            .iter()
            .map(|score| score.policy_id.clone())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.scores.len()
            || ids != self.policy_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.reference_policy_id.is_empty()
            || !ids.contains(&self.reference_policy_id)
            || self
                .selected_policy_id
                .as_ref()
                .is_some_and(|id| !ids.contains(id))
        {
            return Err(DynamicPolicyError::InvalidOutput(
                "policy identities, reference policy, or selected policy do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DynamicPolicyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DynamicPolicyError::InvalidOutput(
                "dynamic policy digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &DynamicPolicyRequest) -> Result<(), DynamicPolicyError> {
    if request.objective.trim().is_empty()
        || request.horizon == 0
        || request.horizon > MAX_HORIZON
        || request.reference_policy_id.trim().is_empty()
        || request.min_trajectories == 0
        || request.min_coverage_milli == 0
        || request.min_coverage_milli > 1_000
        || request.min_propensity_milli == 0
        || request.min_propensity_milli > 1_000
        || request.max_weight_milli < SCALE as u64
        || request.min_effect_milli > i64::MAX as u64
        || request.max_leave_one_out_shift_milli > i64::MAX as u64
    {
        return Err(DynamicPolicyError::InvalidRequest(
            "objective, horizon, reference policy, support floors, propensity floor, and bounded weighting gates are required".into(),
        ));
    }
    Ok(())
}

fn validate_policies(
    policies: &[DynamicPolicyCandidate],
    request: &DynamicPolicyRequest,
) -> Result<BTreeMap<String, DynamicPolicyCandidate>, DynamicPolicyError> {
    if policies.is_empty() || policies.len() > MAX_POLICIES {
        return Err(DynamicPolicyError::InvalidPolicy(
            "at least one and at most the bounded number of policies are required".into(),
        ));
    }
    let mut result = BTreeMap::new();
    for policy in policies {
        if policy.policy_id.trim().is_empty()
            || policy.label.trim().is_empty()
            || policy.rules.is_empty()
            || policy.rules.len() > MAX_RULES_PER_POLICY
            || policy.risk_milli > 1_000
            || policy.cost_units == 0
            || result
                .insert(policy.policy_id.clone(), policy.clone())
                .is_some()
        {
            return Err(DynamicPolicyError::InvalidPolicy(
                "policy identity, label, rules, positive cost, bounded risk, and unique identities are required".into(),
            ));
        }
        let mut keys = BTreeSet::new();
        for rule in &policy.rules {
            if rule.time_step >= request.horizon
                || rule.state_key.trim().is_empty()
                || rule.action_id.trim().is_empty()
                || !keys.insert((rule.time_step, rule.state_key.clone()))
            {
                return Err(DynamicPolicyError::InvalidPolicy(
                    "policy rules require bounded time, state, action, and unique state-time keys"
                        .into(),
                ));
            }
        }
    }
    if !result.contains_key(&request.reference_policy_id) {
        return Err(DynamicPolicyError::InvalidRequest(
            "reference_policy_id must name one supplied policy".into(),
        ));
    }
    Ok(result)
}

fn validate_trajectories(
    trajectories: &[DynamicPolicyTrajectory],
    request: &DynamicPolicyRequest,
) -> Result<Vec<DynamicPolicyTrajectory>, DynamicPolicyError> {
    if trajectories.is_empty() || trajectories.len() > MAX_TRAJECTORIES {
        return Err(DynamicPolicyError::InvalidTrajectory(
            "at least one and at most the bounded number of trajectories are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut units = BTreeSet::new();
    let mut result = trajectories.to_vec();
    result.sort_by(|left, right| left.trajectory_id.cmp(&right.trajectory_id));
    for trajectory in &result {
        if trajectory.trajectory_id.trim().is_empty()
            || trajectory.unit_id.trim().is_empty()
            || trajectory.observations.is_empty()
            || trajectory.observations.len() > MAX_OBSERVATIONS_PER_TRAJECTORY
            || !ids.insert(trajectory.trajectory_id.clone())
            || !units.insert(trajectory.unit_id.clone())
            || trajectory.model_system != request.model_system
            || trajectory.artifact.validate().is_err()
            || !trajectory.artifact.local_only
            || trajectory.artifact.contains_human_data
            || trajectory.artifact.contains_direct_identifiers
        {
            return Err(DynamicPolicyError::InvalidTrajectory(
                "trajectory identity, unique unit, model binding, local non-human artifact, and bounded observations are required".into(),
            ));
        }
        let mut observation_ids = BTreeSet::new();
        let mut previous_step = None;
        for observation in &trajectory.observations {
            if observation.observation_id.trim().is_empty()
                || !observation_ids.insert(observation.observation_id.clone())
                || observation.time_step >= request.horizon
                || observation.state_key.trim().is_empty()
                || observation.action_id.trim().is_empty()
                || observation.propensity_milli == 0
                || observation.propensity_milli > 1_000
                || previous_step.is_some_and(|step| step >= observation.time_step)
            {
                return Err(DynamicPolicyError::InvalidTrajectory(
                    "observations require unique identities, bounded increasing time, state/action, and propensity bounds".into(),
                ));
            }
            previous_step = Some(observation.time_step);
        }
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct PolicyAggregate {
    contributions: Vec<DynamicPolicyContribution>,
    weighted_return_milli: i64,
    effective_sample_size_milli: u64,
    coverage_milli: u16,
    max_weight_milli: u64,
    leave_one_out_shift_milli: u64,
    included_count: usize,
    positivity_count: usize,
    uncertainty_milli: u64,
    interval_low_milli: i64,
    interval_high_milli: i64,
}

fn weight_for(
    observations: &[DynamicPolicyObservation],
    policy: &DynamicPolicyCandidate,
    request: &DynamicPolicyRequest,
) -> (u64, u16, u16, bool, bool) {
    let by_key = policy
        .rules
        .iter()
        .map(|rule| {
            (
                (rule.time_step, rule.state_key.as_str()),
                rule.action_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut weight = SCALE as u64;
    let mut matched = 0_u16;
    let mut mismatch = 0_u16;
    let mut positivity = false;
    for observation in observations {
        let expected = by_key.get(&(observation.time_step, observation.state_key.as_str()));
        if expected.is_some_and(|action| *action == observation.action_id.as_str()) {
            matched = matched.saturating_add(1);
            if observation.propensity_milli < request.min_propensity_milli {
                positivity = true;
            }
            let factor = SCALE
                .saturating_mul(SCALE)
                .checked_div(u128::from(observation.propensity_milli.max(1)))
                .unwrap_or(u128::MAX);
            weight = u128::from(weight)
                .saturating_mul(factor)
                .checked_div(SCALE)
                .unwrap_or(u128::MAX)
                .min(u128::from(request.max_weight_milli)) as u64;
        } else {
            mismatch = mismatch.saturating_add(1);
        }
    }
    (
        weight,
        matched,
        mismatch,
        positivity,
        mismatch == 0 && !positivity,
    )
}

fn aggregate_policy(
    policy: &DynamicPolicyCandidate,
    trajectories: &[DynamicPolicyTrajectory],
    request: &DynamicPolicyRequest,
) -> PolicyAggregate {
    let mut contributions = Vec::with_capacity(trajectories.len());
    let mut sum_weight = 0_u128;
    let mut sum_weight_squared = 0_u128;
    let mut weighted_sum = 0_i128;
    let mut included_returns = Vec::new();
    let mut max_weight = 0_u64;
    let mut positivity_count = 0_usize;
    for trajectory in trajectories {
        let (weight, matched, mismatch, positivity, included) =
            weight_for(&trajectory.observations, policy, request);
        let return_milli = trajectory
            .observations
            .iter()
            .map(|observation| i128::from(observation.outcome_milli))
            .sum::<i128>()
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
        let reason = if positivity {
            positivity_count = positivity_count.saturating_add(1);
            Some("propensity-below-floor".into())
        } else if mismatch != 0 {
            Some("policy-action-mismatch".into())
        } else {
            None
        };
        contributions.push(DynamicPolicyContribution {
            trajectory_id: trajectory.trajectory_id.clone(),
            included,
            weight_milli: if included { weight } else { 0 },
            return_milli,
            matched_steps: matched,
            mismatch_steps: mismatch,
            positivity_violation: positivity,
            exclusion_reason: reason,
        });
        if included {
            let weight_i = u128::from(weight.max(1));
            sum_weight = sum_weight.saturating_add(weight_i);
            sum_weight_squared =
                sum_weight_squared.saturating_add(weight_i.saturating_mul(weight_i));
            weighted_sum = weighted_sum
                .saturating_add(i128::from(return_milli).saturating_mul(weight_i as i128));
            included_returns.push((trajectory.trajectory_id.clone(), return_milli, weight));
            max_weight = max_weight.max(weight);
        }
    }
    let weighted_return = if sum_weight == 0 {
        0
    } else {
        (weighted_sum / sum_weight as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    };
    let effective_sample_size_milli = if sum_weight_squared == 0 {
        0
    } else {
        sum_weight
            .saturating_mul(sum_weight)
            .saturating_mul(SCALE)
            .checked_div(sum_weight_squared)
            .unwrap_or(0)
            .min(u128::from(trajectories.len() as u64).saturating_mul(SCALE)) as u64
    };
    let included_count = included_returns.len();
    let coverage_milli =
        ((included_count as u128).saturating_mul(SCALE) / trajectories.len().max(1) as u128) as u16;
    let uncertainty_milli = if sum_weight == 0 {
        0
    } else {
        let deviation = included_returns
            .iter()
            .map(|(_, value, weight)| {
                u128::from(value.saturating_sub(weighted_return).unsigned_abs())
                    .saturating_mul(u128::from(*weight))
            })
            .sum::<u128>()
            .checked_div(sum_weight)
            .unwrap_or(0);
        let ess = (effective_sample_size_milli / SCALE as u64).max(1);
        (deviation / integer_sqrt(u128::from(ess)).max(1)) as u64
    };
    let interval_low =
        weighted_return.saturating_sub(uncertainty_milli.min(i64::MAX as u64) as i64);
    let interval_high =
        weighted_return.saturating_add(uncertainty_milli.min(i64::MAX as u64) as i64);
    let mut leave_one_out_shift = 0_u64;
    if included_returns.len() > 1 {
        for (removed_id, removed_return, removed_weight) in &included_returns {
            let remaining_weight = sum_weight.saturating_sub(u128::from(*removed_weight));
            if remaining_weight == 0 {
                continue;
            }
            let remaining_sum = weighted_sum.saturating_sub(
                i128::from(*removed_return).saturating_mul(i128::from(*removed_weight)),
            );
            let remaining_mean = (remaining_sum / remaining_weight as i128) as i64;
            leave_one_out_shift = leave_one_out_shift.max(
                remaining_mean
                    .saturating_sub(weighted_return)
                    .unsigned_abs(),
            );
            let _ = removed_id;
        }
    }
    PolicyAggregate {
        contributions,
        weighted_return_milli: weighted_return,
        effective_sample_size_milli,
        coverage_milli,
        max_weight_milli: max_weight,
        leave_one_out_shift_milli: leave_one_out_shift,
        included_count,
        positivity_count,
        uncertainty_milli,
        interval_low_milli: interval_low,
        interval_high_milli: interval_high,
    }
}

fn score_policy(
    policy: &DynamicPolicyCandidate,
    aggregate: PolicyAggregate,
    reference_return_milli: i64,
    request: &DynamicPolicyRequest,
) -> DynamicPolicyScore {
    let effect = aggregate
        .weighted_return_milli
        .saturating_sub(reference_return_milli);
    let support_ok = aggregate.included_count >= request.min_trajectories
        && aggregate.coverage_milli >= request.min_coverage_milli
        && aggregate.positivity_count == 0;
    let stable = aggregate.leave_one_out_shift_milli <= request.max_leave_one_out_shift_milli;
    let direction_ok = if request.maximize_effect {
        effect >= 0
    } else {
        effect <= 0
    };
    let effect_large = effect.unsigned_abs() >= request.min_effect_milli;
    let eligible = support_ok && stable;
    let disposition = if !support_ok {
        DynamicPolicyScoreDisposition::PositivityBlocked
    } else if !stable || !effect_large {
        DynamicPolicyScoreDisposition::Unresolved
    } else if direction_ok
        && (aggregate.interval_low_milli > reference_return_milli
            || aggregate.interval_high_milli < reference_return_milli)
    {
        DynamicPolicyScoreDisposition::Qualified
    } else {
        DynamicPolicyScoreDisposition::Negative
    };
    let rationale = match disposition {
        DynamicPolicyScoreDisposition::Qualified => {
            "supported policy effect clears the threshold with stable weighted evidence".into()
        }
        DynamicPolicyScoreDisposition::Negative => {
            "policy effect does not support the declared optimization direction".into()
        }
        DynamicPolicyScoreDisposition::Unresolved => {
            "support, effect, uncertainty, or leave-one-out stability remains insufficient".into()
        }
        DynamicPolicyScoreDisposition::PositivityBlocked => {
            "policy lacks enough overlap or violates the declared propensity floor".into()
        }
    };
    DynamicPolicyScore {
        policy_id: policy.policy_id.clone(),
        label: policy.label.clone(),
        weighted_return_milli: aggregate.weighted_return_milli,
        reference_return_milli,
        effect_milli: effect,
        interval_low_milli: aggregate.interval_low_milli,
        interval_high_milli: aggregate.interval_high_milli,
        uncertainty_milli: aggregate.uncertainty_milli,
        effective_sample_size_milli: aggregate.effective_sample_size_milli,
        coverage_milli: aggregate.coverage_milli,
        max_importance_weight_milli: aggregate.max_weight_milli,
        leave_one_out_shift_milli: aggregate.leave_one_out_shift_milli,
        included_trajectory_count: aggregate.included_count,
        positivity_violation_count: aggregate.positivity_count,
        risk_milli: policy.risk_milli,
        cost_units: policy.cost_units,
        contributions: aggregate.contributions,
        eligible,
        disposition,
        rationale,
    }
}

/// Evaluate competing finite-horizon preclinical policies from local longitudinal trajectories.
/// This is an off-policy research-analysis product: it does not dispatch an assay or recommend a
/// clinical treatment. A caller can hand the selected policy to the P07 mission controller as a
/// bounded next-workflow frontier.
pub fn evaluate_glioma_dynamic_policies(
    request: &DynamicPolicyRequest,
    policies: &[DynamicPolicyCandidate],
    trajectories: &[DynamicPolicyTrajectory],
) -> Result<DynamicPolicyEvaluation, DynamicPolicyError> {
    validate_request(request)?;
    let policies = validate_policies(policies, request)?;
    let trajectories = validate_trajectories(trajectories, request)?;
    let mut aggregates = policies
        .values()
        .map(|policy| {
            (
                policy.policy_id.clone(),
                aggregate_policy(policy, &trajectories, request),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let reference_return = aggregates
        .get(&request.reference_policy_id)
        .map(|aggregate| aggregate.weighted_return_milli)
        .unwrap_or_default();
    let mut scores = policies
        .values()
        .map(|policy| {
            score_policy(
                policy,
                aggregates
                    .remove(&policy.policy_id)
                    .expect("aggregate was created"),
                reference_return,
                request,
            )
        })
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| left.policy_id.cmp(&right.policy_id));
    let mut ranking = scores
        .iter()
        .filter(|score| score.eligible)
        .map(|score| score.policy_id.clone())
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        let l = scores
            .iter()
            .find(|score| score.policy_id == *left)
            .expect("score exists");
        let r = scores
            .iter()
            .find(|score| score.policy_id == *right)
            .expect("score exists");
        let l_value = if request.maximize_effect {
            l.effect_milli
        } else {
            l.effect_milli.saturating_neg()
        };
        let r_value = if request.maximize_effect {
            r.effect_milli
        } else {
            r.effect_milli.saturating_neg()
        };
        r_value
            .cmp(&l_value)
            .then_with(|| l.risk_milli.cmp(&r.risk_milli))
            .then_with(|| l.cost_units.cmp(&r.cost_units))
            .then_with(|| l.policy_id.cmp(&r.policy_id))
    });
    let selected_policy_id = ranking.first().cloned();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for score in &scores {
        if score.positivity_violation_count != 0 {
            negative_evidence.insert(format!("{}:positivity-violations", score.policy_id));
        }
        if score.coverage_milli < request.min_coverage_milli {
            uncertainty.insert(format!("{}:support-below-coverage-floor", score.policy_id));
        }
        if score.leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
            uncertainty.insert(format!("{}:leave-one-out-instability", score.policy_id));
        }
        if score.effect_milli.unsigned_abs() < request.min_effect_milli {
            negative_evidence.insert(format!("{}:effect-below-threshold", score.policy_id));
        }
    }
    let disposition = if let Some(selected) = selected_policy_id.as_ref() {
        match scores
            .iter()
            .find(|score| &score.policy_id == selected)
            .map(|score| score.disposition)
        {
            Some(DynamicPolicyScoreDisposition::Qualified) => DynamicPolicyDisposition::Qualified,
            Some(DynamicPolicyScoreDisposition::Negative) => DynamicPolicyDisposition::Negative,
            _ => DynamicPolicyDisposition::Unresolved,
        }
    } else if scores
        .iter()
        .all(|score| score.disposition == DynamicPolicyScoreDisposition::PositivityBlocked)
    {
        DynamicPolicyDisposition::PositivityBlocked
    } else {
        DynamicPolicyDisposition::Unresolved
    };
    let mut output = DynamicPolicyEvaluation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        reference_policy_id: request.reference_policy_id.clone(),
        policy_order: scores.iter().map(|score| score.policy_id.clone()).collect(),
        ranking_order: ranking,
        selected_policy_id,
        scores,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-dynamic-policy"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DynamicPolicyError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma.trajectory+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> DynamicPolicyRequest {
        DynamicPolicyRequest {
            objective: "select an organoid invasion perturbation policy".into(),
            model_system: GliomaModelSystem::Organoid,
            horizon: 3,
            reference_policy_id: "reference".into(),
            maximize_effect: true,
            min_trajectories: 2,
            min_coverage_milli: 500,
            min_propensity_milli: 100,
            max_weight_milli: 20_000,
            min_effect_milli: 20,
            max_leave_one_out_shift_milli: 500,
        }
    }

    fn policy(id: &str, action: &str) -> DynamicPolicyCandidate {
        DynamicPolicyCandidate {
            policy_id: id.into(),
            label: id.into(),
            rules: (0..3)
                .map(|time_step| DynamicPolicyRule {
                    time_step,
                    state_key: "baseline".into(),
                    action_id: action.into(),
                })
                .collect(),
            risk_milli: if id == "reference" { 100 } else { 200 },
            cost_units: if id == "reference" { 1 } else { 2 },
        }
    }

    fn trajectories() -> Vec<DynamicPolicyTrajectory> {
        (0..4)
            .map(|index| DynamicPolicyTrajectory {
                trajectory_id: format!("t-{index}"),
                unit_id: format!("unit-{index}"),
                model_system: GliomaModelSystem::Organoid,
                observations: (0..3)
                    .map(|time_step| DynamicPolicyObservation {
                        observation_id: format!("t-{index}-o-{time_step}"),
                        time_step,
                        state_key: "baseline".into(),
                        action_id: if index < 2 {
                            "reference".into()
                        } else {
                            "candidate".into()
                        },
                        outcome_milli: if index < 2 { 10 } else { 80 },
                        propensity_milli: 500,
                    })
                    .collect(),
                artifact: artifact(&format!("artifact-{index}")),
            })
            .collect()
    }

    #[test]
    fn policy_evaluation_ranks_candidate_and_replays_after_permutation() {
        let request = request();
        let policies = vec![
            policy("candidate", "candidate"),
            policy("reference", "reference"),
        ];
        let first = evaluate_glioma_dynamic_policies(&request, &policies, &trajectories()).unwrap();
        let mut reversed = trajectories();
        reversed.reverse();
        let second = evaluate_glioma_dynamic_policies(&request, &policies, &reversed).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_policy_id.as_deref(), Some("candidate"));
        assert_eq!(first.disposition, DynamicPolicyDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn positivity_and_support_failures_remain_explicit() {
        let mut request = request();
        request.min_propensity_milli = 900;
        let trajectories = trajectories()
            .into_iter()
            .map(|mut trajectory| {
                trajectory
                    .observations
                    .iter_mut()
                    .for_each(|observation| observation.propensity_milli = 100);
                trajectory
            })
            .collect::<Vec<_>>();
        let output = evaluate_glioma_dynamic_policies(
            &request,
            &[policy("reference", "reference")],
            &trajectories,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            DynamicPolicyDisposition::PositivityBlocked
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("positivity")));
    }

    #[test]
    fn clipped_weights_and_leave_one_out_uncertainty_are_reported() {
        let mut request = request();
        request.max_weight_milli = 1_000;
        request.max_leave_one_out_shift_milli = 0;
        let mut trajectories = trajectories();
        trajectories[1]
            .observations
            .iter_mut()
            .for_each(|observation| observation.outcome_milli = 800);
        let output = evaluate_glioma_dynamic_policies(
            &request,
            &[policy("reference", "reference")],
            &trajectories,
        )
        .unwrap();
        let score = &output.scores[0];
        assert!(score.max_importance_weight_milli <= 1_000);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("leave-one-out")));
    }
}
