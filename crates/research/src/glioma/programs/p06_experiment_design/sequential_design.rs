//! Sequential Bayesian stopping and allocation for preclinical glioma assays.
//!
//! This feature is the interim-decision layer between arm allocation and protocol execution. It
//! maintains independent Beta posteriors for each declared arm, compares each arm with a local
//! control using an integer Cantelli bound, and emits a conservative next-round plan. A result can
//! stop for a declared success or futility gate only when the replicate floor is met; otherwise it
//! remains a hold/continue decision. Risk, budget, model-system, artifact-locality, and negative
//! evidence gates are explicit. The planner never chooses a clinical dose, infers human outcomes,
//! or dispatches an assay.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaSequentialDesign1@1";
pub const MAX_ARMS: usize = 256;
pub const MAX_ROUNDS: u16 = 128;
pub const MAX_NEW_REPLICATES: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialDesignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub target_effect_milli: i32,
    pub success_probability_milli: u16,
    pub futility_probability_milli: u16,
    pub min_replicates_per_arm: u32,
    pub max_new_replicates_per_round: u32,
    pub max_rounds: u16,
    pub max_selected_arms: usize,
    pub budget_units: u64,
    pub risk_ceiling_milli: u16,
    pub exploration_weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialArmObservation {
    pub arm_id: String,
    pub label: String,
    pub artifact: LocalArtifactRef,
    pub model_system: GliomaModelSystem,
    pub successes: u32,
    pub failures: u32,
    pub prior_alpha: u32,
    pub prior_beta: u32,
    pub risk_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequentialDecisionKind {
    SuccessStop,
    FutilityStop,
    Continue,
    HoldUnderpowered,
    RiskBlocked,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialArmDecision {
    pub arm_id: String,
    pub label: String,
    pub is_control: bool,
    pub alpha: u64,
    pub beta: u64,
    pub observations: u32,
    pub posterior_mean_milli: u32,
    pub posterior_variance_milli2: u64,
    pub uncertainty_milli: u32,
    pub effect_vs_control_milli: i32,
    pub probability_exceeds_target_milli: u16,
    pub probability_futile_milli: u16,
    pub information_utility_milli: u16,
    pub cost_units: u32,
    pub planned_replicates: u32,
    pub projected_cost_units: u64,
    pub decision: SequentialDecisionKind,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialDesignRound {
    pub round: u16,
    pub arm_order: Vec<String>,
    pub planned_replicates: u32,
    pub cost_units: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequentialDesignDisposition {
    Success,
    Futility,
    Continue,
    Hold,
    RiskBlocked,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialDesignPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub arm_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub rounds: Vec<SequentialDesignRound>,
    pub decisions: Vec<SequentialArmDecision>,
    pub budget_remaining_units: u64,
    pub success_stop_order: Vec<String>,
    pub futility_stop_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SequentialDesignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SequentialDesignError {
    #[error("sequential design request is invalid: {0}")]
    InvalidRequest(String),
    #[error("sequential design arm is invalid: {0}")]
    InvalidArm(String),
    #[error("sequential design output is invalid: {0}")]
    InvalidOutput(String),
    #[error("sequential design digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &SequentialDesignPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "endpoint": plan.endpoint,
        "control_arm_id": plan.control_arm_id,
        "arm_order": plan.arm_order,
        "selected_order": plan.selected_order,
        "rounds": plan.rounds,
        "decisions": plan.decisions,
        "budget_remaining_units": plan.budget_remaining_units,
        "success_stop_order": plan.success_stop_order,
        "futility_stop_order": plan.futility_stop_order,
        "hold_order": plan.hold_order,
        "risk_blocked_order": plan.risk_blocked_order,
        "budget_blocked_order": plan.budget_blocked_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value.min(u128::from(u64::MAX));
    while low <= high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    high
}

fn posterior(
    arm: &SequentialArmObservation,
) -> Result<(u64, u64, u32, u32, u64, u32), SequentialDesignError> {
    let alpha = u64::from(arm.prior_alpha) + u64::from(arm.successes);
    let beta = u64::from(arm.prior_beta) + u64::from(arm.failures);
    let observations = arm.successes.saturating_add(arm.failures);
    let total = alpha.saturating_add(beta);
    if alpha == 0 || beta == 0 || total == 0 {
        return Err(SequentialDesignError::InvalidArm(
            "posterior alpha/beta and total trials must be positive".into(),
        ));
    }
    let mean = (u128::from(alpha) * 1_000 / u128::from(total)) as u32;
    let denominator = u128::from(total)
        .saturating_mul(u128::from(total))
        .saturating_mul(u128::from(total.saturating_add(1)));
    let variance = u128::from(alpha)
        .saturating_mul(u128::from(beta))
        .saturating_mul(1_000_000)
        / denominator.max(1);
    let uncertainty = integer_sqrt(variance).min(u128::from(u32::MAX)) as u32;
    Ok((
        alpha,
        beta,
        observations,
        mean,
        variance.min(u128::from(u64::MAX)) as u64,
        uncertainty,
    ))
}

fn one_sided_probability(effect: i32, threshold: i32, variance: u128) -> u16 {
    let excess = i128::from(effect) - i128::from(threshold);
    if excess <= 0 {
        return 0;
    }
    let squared = (excess as u128).saturating_mul(excess as u128);
    (squared.saturating_mul(1_000) / squared.saturating_add(variance)).min(1_000) as u16
}

fn futility_probability(effect: i32, variance: u128) -> u16 {
    if effect >= 0 {
        return 0;
    }
    let magnitude = u128::from(effect.unsigned_abs());
    let squared = magnitude.saturating_mul(magnitude);
    (squared.saturating_mul(1_000) / squared.saturating_add(variance)).min(1_000) as u16
}

fn validate_request(request: &SequentialDesignRequest) -> Result<(), SequentialDesignError> {
    if request.objective.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.control_arm_id.trim().is_empty()
        || request.target_effect_milli < -1_000
        || request.target_effect_milli > 1_000
        || request.success_probability_milli > 1_000
        || request.futility_probability_milli > 1_000
        || request.futility_probability_milli == 0
        || request.success_probability_milli <= request.futility_probability_milli
        || request.min_replicates_per_arm == 0
        || request.max_new_replicates_per_round == 0
        || request.max_new_replicates_per_round > MAX_NEW_REPLICATES
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_selected_arms == 0
        || request.budget_units == 0
        || request.risk_ceiling_milli > 1_000
        || request.exploration_weight_milli > 1_000
    {
        return Err(SequentialDesignError::InvalidRequest(
            "objective, endpoint, control, bounded effect/probability gates, positive replicate/round/budget bounds, and risk limits are required".into(),
        ));
    }
    Ok(())
}

impl SequentialDesignPlan {
    pub fn validate(&self) -> Result<(), SequentialDesignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.control_arm_id.trim().is_empty()
            || !canonical(&self.arm_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.success_stop_order)
            || !canonical(&self.futility_stop_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.risk_blocked_order)
            || !canonical(&self.budget_blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.decisions.len() != self.arm_order.len()
            || self.decisions.iter().any(|decision| {
                decision.arm_id.trim().is_empty()
                    || decision.label.trim().is_empty()
                    || decision.alpha == 0
                    || decision.beta == 0
                    || decision.posterior_mean_milli > 1_000
                    || decision.uncertainty_milli > 1_000
                    || decision.probability_exceeds_target_milli > 1_000
                    || decision.probability_futile_milli > 1_000
                    || decision.information_utility_milli > 1_000
                    || decision.cost_units == 0
                    || decision.rationale.trim().is_empty()
            })
            || self
                .decisions
                .windows(2)
                .any(|pair| pair[0].arm_id >= pair[1].arm_id)
            || self
                .rounds
                .windows(2)
                .any(|pair| pair[0].round >= pair[1].round)
            || self.rounds.iter().any(|round| {
                !canonical(&round.arm_order)
                    || round.planned_replicates == 0
                    || round.rationale.trim().is_empty()
            })
        {
            return Err(SequentialDesignError::InvalidOutput(
                "identity, ordering, posterior bounds, decision partitions, or round invariants are invalid".into(),
            ));
        }
        let all = self.arm_order.iter().cloned().collect::<BTreeSet<_>>();
        let decision_ids = self
            .decisions
            .iter()
            .map(|decision| decision.arm_id.clone())
            .collect::<BTreeSet<_>>();
        if all != decision_ids {
            return Err(SequentialDesignError::InvalidOutput(
                "arm order and decision identities do not partition".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SequentialDesignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SequentialDesignError::InvalidOutput(
                "sequential design digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Plan a bounded interim decision for each local preclinical glioma assay arm.
pub fn plan_glioma_sequential_design(
    request: &SequentialDesignRequest,
    observations: &[SequentialArmObservation],
) -> Result<SequentialDesignPlan, SequentialDesignError> {
    validate_request(request)?;
    if observations.len() < 2 || observations.len() > MAX_ARMS {
        return Err(SequentialDesignError::InvalidArm(
            "a control plus at least one candidate and at most the bounded number of arms are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut by_id = BTreeMap::new();
    for arm in observations {
        arm.artifact
            .validate()
            .map_err(|error| SequentialDesignError::InvalidArm(error.to_string()))?;
        if arm.arm_id.trim().is_empty()
            || arm.label.trim().is_empty()
            || arm.model_system != request.model_system
            || arm.prior_alpha == 0
            || arm.prior_beta == 0
            || arm.cost_units == 0
            || arm.risk_milli > 1_000
            || !ids.insert(arm.arm_id.clone())
        {
            return Err(SequentialDesignError::InvalidArm(
                "arm identity, model system, positive priors/cost, risk bounds, and uniqueness are required".into(),
            ));
        }
        let stats = posterior(arm)?;
        by_id.insert(arm.arm_id.clone(), (arm.clone(), stats));
    }
    let Some((control, control_stats)) = by_id.get(&request.control_arm_id) else {
        return Err(SequentialDesignError::InvalidArm(
            "declared control arm is missing".into(),
        ));
    };
    let control = control.clone();
    let (_, _, control_observations, control_mean, control_variance, _) = *control_stats;
    let arm_order = by_id.keys().cloned().collect::<Vec<_>>();
    let mut decisions = Vec::new();
    let mut selected = BTreeSet::new();
    let mut success_stop = BTreeSet::new();
    let mut futility_stop = BTreeSet::new();
    let mut hold = BTreeSet::new();
    let mut risk_blocked = BTreeSet::new();
    let mut budget_blocked = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut candidate_utilities = Vec::new();

    for arm_id in &arm_order {
        let (arm, stats) = by_id.get(arm_id).expect("arm order comes from map");
        let (alpha, beta, observations_count, mean, variance, uncertainty_milli) = *stats;
        let is_control = arm_id == &request.control_arm_id;
        let combined_variance = u128::from(variance).saturating_add(u128::from(control_variance));
        let effect = if is_control {
            0
        } else {
            i64::from(mean).saturating_sub(i64::from(control_mean)) as i32
        };
        let probability_exceeds = if is_control {
            1_000
        } else {
            one_sided_probability(effect, request.target_effect_milli, combined_variance)
        };
        let probability_futile = if is_control {
            0
        } else {
            futility_probability(effect, combined_variance)
        };
        let replicate_floor = request
            .min_replicates_per_arm
            .saturating_sub(observations_count);
        let risk_ok = arm.risk_milli <= request.risk_ceiling_milli;
        let success_gate = !is_control
            && observations_count >= request.min_replicates_per_arm
            && probability_exceeds >= request.success_probability_milli;
        let futility_gate = !is_control
            && observations_count >= request.min_replicates_per_arm
            && probability_futile >= request.futility_probability_milli;
        let mut planned = replicate_floor.min(request.max_new_replicates_per_round);
        let decision = if success_gate {
            success_stop.insert(arm_id.clone());
            SequentialDecisionKind::SuccessStop
        } else if futility_gate {
            futility_stop.insert(arm_id.clone());
            negative_evidence.insert(format!("{arm_id}:posterior-futility-gate"));
            SequentialDecisionKind::FutilityStop
        } else if !risk_ok {
            risk_blocked.insert(arm_id.clone());
            uncertainty.insert(format!("{arm_id}:risk-exceeds-ceiling"));
            planned = 0;
            SequentialDecisionKind::RiskBlocked
        } else if observations_count < request.min_replicates_per_arm {
            hold.insert(arm_id.clone());
            uncertainty.insert(format!("{arm_id}:replicate-floor-incomplete"));
            SequentialDecisionKind::HoldUnderpowered
        } else {
            SequentialDecisionKind::Continue
        };
        // Budget is allocated only after utilities are ranked. Applying it here would make the
        // input ordering decide which arms appear budget-blocked before the best candidates are
        // known. Keep every eligible arm's provisional batch intact until the bounded selection
        // pass below, which is deterministic by utility and arm id.
        let projected_cost = u64::from(planned).saturating_mul(u64::from(arm.cost_units));
        if matches!(
            decision,
            SequentialDecisionKind::Continue | SequentialDecisionKind::HoldUnderpowered
        ) && planned > 0
            && (!is_control || observations_count < request.min_replicates_per_arm)
        {
            selected.insert(arm_id.clone());
            candidate_utilities.push((
                arm_id.clone(),
                (u64::from(uncertainty_milli)
                    .saturating_mul(u64::from(request.exploration_weight_milli))
                    .saturating_add(u64::from(probability_exceeds.max(probability_futile)) * 100))
                    / u64::from(arm.cost_units.max(1)),
            ));
        }
        let rationale = match decision {
            SequentialDecisionKind::SuccessStop => "replicate floor met and target-effect posterior clears the success gate".to_string(),
            SequentialDecisionKind::FutilityStop => "replicate floor met and the posterior futility probability clears the negative gate".to_string(),
            SequentialDecisionKind::RiskBlocked => "arm exceeds the declared preclinical risk ceiling".to_string(),
            SequentialDecisionKind::BudgetBlocked => "no affordable next replicate remains in the bounded budget".to_string(),
            SequentialDecisionKind::HoldUnderpowered => "continue only to close the declared replicate floor; no scientific promotion is made".to_string(),
            SequentialDecisionKind::Continue => "posterior remains decision-relevant; allocate a bounded information-bearing next batch".to_string(),
        };
        decisions.push(SequentialArmDecision {
            arm_id: arm_id.clone(),
            label: arm.label.clone(),
            is_control,
            alpha,
            beta,
            observations: observations_count,
            posterior_mean_milli: mean,
            posterior_variance_milli2: variance,
            uncertainty_milli,
            effect_vs_control_milli: effect,
            probability_exceeds_target_milli: probability_exceeds,
            probability_futile_milli: probability_futile,
            information_utility_milli: 0,
            cost_units: arm.cost_units,
            planned_replicates: planned,
            projected_cost_units: projected_cost,
            decision,
            rationale,
        });
    }
    candidate_utilities
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut selected_ids = candidate_utilities
        .iter()
        .take(request.max_selected_arms)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut remaining_budget = request.budget_units;
    for decision in &mut decisions {
        let utility = candidate_utilities
            .iter()
            .find(|(id, _)| id == &decision.arm_id)
            .map(|(_, utility)| (*utility).min(1_000) as u16)
            .unwrap_or(0);
        decision.information_utility_milli = utility;
        if selected_ids.contains(&decision.arm_id) {
            let affordable = remaining_budget / u64::from(decision.cost_units);
            let planned = decision
                .planned_replicates
                .min(affordable.min(u64::from(u32::MAX)) as u32);
            decision.planned_replicates = planned;
            decision.projected_cost_units =
                u64::from(planned).saturating_mul(u64::from(decision.cost_units));
            if planned == 0 {
                selected_ids.remove(&decision.arm_id);
                budget_blocked.insert(decision.arm_id.clone());
                uncertainty.insert(format!(
                    "{}:budget-insufficient-for-selected-next-replicate",
                    decision.arm_id
                ));
                decision.decision = SequentialDecisionKind::BudgetBlocked;
            } else {
                remaining_budget = remaining_budget.saturating_sub(decision.projected_cost_units);
            }
        } else if decision.planned_replicates > 0 {
            decision.planned_replicates = 0;
            decision.projected_cost_units = 0;
        }
    }
    selected = selected_ids;
    let mut rounds = Vec::new();
    let mut round_arm_order = selected.iter().cloned().collect::<Vec<_>>();
    round_arm_order.sort();
    if !round_arm_order.is_empty() {
        let planned_replicates = decisions
            .iter()
            .filter(|decision| selected.contains(&decision.arm_id))
            .map(|decision| decision.planned_replicates)
            .sum::<u32>();
        let cost_units = decisions
            .iter()
            .filter(|decision| selected.contains(&decision.arm_id))
            .map(|decision| decision.projected_cost_units)
            .sum::<u64>();
        rounds.push(SequentialDesignRound {
            round: 1,
            arm_order: round_arm_order,
            planned_replicates,
            cost_units,
            rationale: "one bounded interim batch; recompute posteriors before any subsequent round".into(),
        });
    }
    let has_success = !success_stop.is_empty();
    let all_non_control_stopped = decisions
        .iter()
        .filter(|decision| !decision.is_control)
        .all(|decision| {
            matches!(
                decision.decision,
                SequentialDecisionKind::SuccessStop
                    | SequentialDecisionKind::FutilityStop
                    | SequentialDecisionKind::RiskBlocked
                    | SequentialDecisionKind::BudgetBlocked
            )
        });
    let disposition = if has_success && all_non_control_stopped {
        SequentialDesignDisposition::Success
    } else if !futility_stop.is_empty() && all_non_control_stopped {
        SequentialDesignDisposition::Futility
    } else if !risk_blocked.is_empty() && selected.is_empty() {
        SequentialDesignDisposition::RiskBlocked
    } else if !budget_blocked.is_empty() && selected.is_empty() {
        SequentialDesignDisposition::BudgetBlocked
    } else if !selected.is_empty() {
        SequentialDesignDisposition::Continue
    } else if !hold.is_empty() {
        SequentialDesignDisposition::Hold
    } else {
        SequentialDesignDisposition::Unresolved
    };
    if control_observations < request.min_replicates_per_arm {
        uncertainty.insert(format!(
            "{}:control-replicate-floor-incomplete",
            control.arm_id
        ));
    }
    let mut plan = SequentialDesignPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        endpoint: request.endpoint.clone(),
        control_arm_id: request.control_arm_id.clone(),
        arm_order,
        selected_order: selected.into_iter().collect(),
        rounds,
        decisions,
        budget_remaining_units: remaining_budget,
        success_stop_order: success_stop.into_iter().collect(),
        futility_stop_order: futility_stop.into_iter().collect(),
        hold_order: hold.into_iter().collect(),
        risk_blocked_order: risk_blocked.into_iter().collect(),
        budget_blocked_order: budget_blocked.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-sequential-design"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| SequentialDesignError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-sequential+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> SequentialDesignRequest {
        SequentialDesignRequest {
            objective: "identify a reproducible invasion signal in organoids".into(),
            model_system: GliomaModelSystem::Organoid,
            endpoint: "invasion_fraction".into(),
            control_arm_id: "control".into(),
            target_effect_milli: 150,
            success_probability_milli: 750,
            futility_probability_milli: 700,
            min_replicates_per_arm: 3,
            max_new_replicates_per_round: 3,
            max_rounds: 4,
            max_selected_arms: 2,
            budget_units: 20,
            risk_ceiling_milli: 800,
            exploration_weight_milli: 400,
        }
    }

    fn arm(id: &str, successes: u32, failures: u32, risk: u16) -> SequentialArmObservation {
        SequentialArmObservation {
            arm_id: id.into(),
            label: id.into(),
            artifact: artifact(id),
            model_system: GliomaModelSystem::Organoid,
            successes,
            failures,
            prior_alpha: 1,
            prior_beta: 1,
            risk_milli: risk,
            cost_units: 2,
        }
    }

    #[test]
    fn success_gate_requires_replicate_floor_and_target_probability() {
        let output = plan_glioma_sequential_design(
            &request(),
            &[arm("control", 3, 7, 200), arm("strong", 10, 0, 200)],
        )
        .unwrap();
        assert!(output.success_stop_order.contains(&"strong".to_string()));
        assert!(output
            .decisions
            .iter()
            .any(|decision| decision.arm_id == "strong"
                && decision.decision == SequentialDecisionKind::SuccessStop));
        output.validate().unwrap();
    }

    #[test]
    fn futility_and_negative_evidence_are_first_class() {
        let output = plan_glioma_sequential_design(
            &request(),
            &[arm("control", 7, 3, 200), arm("weak", 0, 10, 200)],
        )
        .unwrap();
        assert!(output.futility_stop_order.contains(&"weak".to_string()));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("weak")));
    }

    #[test]
    fn underpowered_arm_is_held_not_promoted() {
        let output = plan_glioma_sequential_design(
            &request(),
            &[arm("control", 1, 1, 200), arm("candidate", 2, 0, 200)],
        )
        .unwrap();
        assert!(output.hold_order.contains(&"candidate".to_string()));
        assert!(!output.success_stop_order.contains(&"candidate".to_string()));
        assert!(output.uncertainty.iter().any(|item| item.contains("floor")));
    }

    #[test]
    fn control_is_not_reallocated_after_its_replicate_floor_is_met() {
        let output = plan_glioma_sequential_design(
            &request(),
            &[arm("control", 3, 7, 200), arm("candidate", 2, 0, 200)],
        )
        .unwrap();
        assert_eq!(output.selected_order, vec!["candidate"]);
        assert_eq!(output.disposition, SequentialDesignDisposition::Continue);
        assert!(!output
            .decisions
            .iter()
            .any(|decision| decision.arm_id == "control" && decision.planned_replicates > 0));
    }

    #[test]
    fn replay_is_stable_under_input_permutation_and_risk_blocks() {
        let mut blocked = request();
        blocked.risk_ceiling_milli = 100;
        let first = plan_glioma_sequential_design(
            &blocked,
            &[arm("candidate", 4, 4, 500), arm("control", 4, 4, 50)],
        )
        .unwrap();
        let second = plan_glioma_sequential_design(
            &blocked,
            &[arm("control", 4, 4, 50), arm("candidate", 4, 4, 500)],
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.risk_blocked_order.contains(&"candidate".to_string()));
        assert_eq!(first.digest, second.digest);
    }
}
