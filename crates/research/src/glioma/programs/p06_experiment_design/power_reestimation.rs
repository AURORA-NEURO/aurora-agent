//! Adaptive power re-estimation and group-sequential boundaries for preclinical glioma assays.
//!
//! This is a deterministic planning algorithm for a local experimental workbench.  It estimates
//! the next replicate count from observed arm variance, spends a declared one-sided error budget
//! across interim looks, and emits efficacy, futility, hold, risk, and budget decisions.  It does
//! not claim a validated statistical guarantee, choose a clinical dose, or dispatch an assay: all
//! integer boundaries and limitations are returned as part of the product artifact.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaPowerReestimation1@1";
pub const MAX_ARMS: usize = 256;
pub const MAX_LOOKS: u16 = 128;
pub const MAX_REPLICATES_PER_ARM: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerReestimationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub target_effect_milli: i32,
    pub alpha_total_milli: u16,
    pub power_target_milli: u16,
    pub current_look: u16,
    pub max_looks: u16,
    pub min_replicates_per_arm: u32,
    pub max_replicates_per_arm: u32,
    pub max_new_replicates_per_arm: u32,
    pub budget_units: u64,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerArmObservation {
    pub arm_id: String,
    pub label: String,
    pub artifact: LocalArtifactRef,
    pub model_system: GliomaModelSystem,
    pub mean_response_milli: i32,
    pub variance_milli2: u64,
    pub observations: u32,
    pub risk_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerDecisionKind {
    EfficacyStop,
    FutilityStop,
    Continue,
    HoldUnderpowered,
    RiskBlocked,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerArmDecision {
    pub arm_id: String,
    pub label: String,
    pub is_control: bool,
    pub effect_vs_control_milli: i32,
    pub standard_error_milli: u32,
    pub alpha_spent_milli: u16,
    pub efficacy_boundary_milli: i32,
    pub futility_boundary_milli: i32,
    pub power_proxy_milli: u16,
    pub information_utility_milli: u16,
    pub required_replicates: u32,
    pub planned_replicates: u32,
    pub projected_cost_units: u64,
    pub decision: PowerDecisionKind,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerReestimationDisposition {
    Efficacy,
    Futility,
    Continue,
    Hold,
    RiskBlocked,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerReestimationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub current_look: u16,
    pub max_looks: u16,
    pub alpha_total_milli: u16,
    pub alpha_spent_milli: u16,
    pub arm_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub efficacy_stop_order: Vec<String>,
    pub futility_stop_order: Vec<String>,
    pub continue_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub required_replicates_by_arm: BTreeMap<String, u32>,
    pub decisions: Vec<PowerArmDecision>,
    pub total_projected_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: PowerReestimationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PowerReestimationError {
    #[error("power re-estimation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("power re-estimation arm is invalid: {0}")]
    InvalidArm(String),
    #[error("power re-estimation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("power re-estimation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut low = 1_u128;
    let mut high = value.min(u128::from(u64::MAX));
    while low <= high {
        let middle = low + (high - low) / 2;
        if middle <= value / middle {
            low = middle + 1;
        } else {
            high = middle - 1;
        }
    }
    high
}

fn digest_input(plan: &PowerReestimationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "endpoint": plan.endpoint,
        "control_arm_id": plan.control_arm_id,
        "current_look": plan.current_look,
        "max_looks": plan.max_looks,
        "alpha_total_milli": plan.alpha_total_milli,
        "alpha_spent_milli": plan.alpha_spent_milli,
        "arm_order": plan.arm_order,
        "selected_order": plan.selected_order,
        "efficacy_stop_order": plan.efficacy_stop_order,
        "futility_stop_order": plan.futility_stop_order,
        "continue_order": plan.continue_order,
        "hold_order": plan.hold_order,
        "risk_blocked_order": plan.risk_blocked_order,
        "budget_blocked_order": plan.budget_blocked_order,
        "required_replicates_by_arm": plan.required_replicates_by_arm,
        "decisions": plan.decisions,
        "total_projected_cost_units": plan.total_projected_cost_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

fn validate_request(request: &PowerReestimationRequest) -> Result<(), PowerReestimationError> {
    if request.objective.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.control_arm_id.trim().is_empty()
        || request.target_effect_milli.abs() > 1_000
        || request.alpha_total_milli == 0
        || request.alpha_total_milli > 500
        || request.power_target_milli < 500
        || request.power_target_milli > 1_000
        || request.current_look == 0
        || request.max_looks == 0
        || request.current_look > request.max_looks
        || request.max_looks > MAX_LOOKS
        || request.min_replicates_per_arm == 0
        || request.max_replicates_per_arm < request.min_replicates_per_arm
        || request.max_replicates_per_arm > MAX_REPLICATES_PER_ARM
        || request.max_new_replicates_per_arm == 0
        || request.budget_units == 0
        || request.risk_ceiling_milli > 1_000
    {
        return Err(PowerReestimationError::InvalidRequest(
            "objective, endpoint, bounded one-sided alpha/power, ordered interim look bounds, replicate limits, positive budget, and risk ceiling are required".into(),
        ));
    }
    Ok(())
}

fn validate_arm(
    arm: &PowerArmObservation,
    request: &PowerReestimationRequest,
) -> Result<(), PowerReestimationError> {
    arm.artifact
        .validate()
        .map_err(|error| PowerReestimationError::InvalidArm(error.to_string()))?;
    if arm.arm_id.trim().is_empty()
        || arm.label.trim().is_empty()
        || arm.model_system != request.model_system
        || arm.mean_response_milli.abs() > 1_000
        || arm.variance_milli2 == 0
        || arm.observations == 0
        || arm.risk_milli > 1_000
        || arm.cost_units == 0
    {
        return Err(PowerReestimationError::InvalidArm(format!(
            "arm {} has invalid identity, model, response, variance, observation, risk, or cost fields",
            arm.arm_id
        )));
    }
    Ok(())
}

impl PowerReestimationPlan {
    pub fn validate(&self) -> Result<(), PowerReestimationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.control_arm_id.trim().is_empty()
            || self.current_look == 0
            || self.current_look > self.max_looks
            || self.max_looks > MAX_LOOKS
            || self.alpha_total_milli == 0
            || self.alpha_spent_milli > self.alpha_total_milli
            || !canonical(&self.arm_order)
            || !canonical(&self.efficacy_stop_order)
            || !canonical(&self.futility_stop_order)
            || !canonical(&self.continue_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.risk_blocked_order)
            || !canonical(&self.budget_blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.decisions.len() != self.arm_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.arm_id.clone())
                .collect::<Vec<_>>()
                != self.arm_order
            || self.decisions.iter().any(|decision| {
                decision.arm_id.trim().is_empty()
                    || decision.label.trim().is_empty()
                    || decision.standard_error_milli == 0
                    || decision.alpha_spent_milli > self.alpha_total_milli
                    || decision.power_proxy_milli > 1_000
                    || decision.information_utility_milli > 1_000
                    || decision.rationale.trim().is_empty()
            })
        {
            return Err(PowerReestimationError::InvalidOutput(
                "identity, look/alpha, ordering, decision, or bounded score invariants are invalid"
                    .into(),
            ));
        }
        let mut partition = BTreeSet::new();
        for order in [
            &self.efficacy_stop_order,
            &self.futility_stop_order,
            &self.continue_order,
            &self.hold_order,
            &self.risk_blocked_order,
            &self.budget_blocked_order,
        ] {
            for id in order {
                if !self.arm_order.binary_search(id).is_ok() || !partition.insert(id) {
                    return Err(PowerReestimationError::InvalidOutput(
                        "decision indexes do not partition the arm order".into(),
                    ));
                }
            }
        }
        if partition.len() != self.arm_order.len()
            || self
                .selected_order
                .iter()
                .any(|id| !self.arm_order.contains(id))
            || self.selected_order.len() > self.arm_order.len()
            || self
                .selected_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(PowerReestimationError::InvalidOutput(
                "decision partition or selected order is incomplete".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PowerReestimationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(PowerReestimationError::InvalidOutput(
                "digest is not bound to the power-re-estimation plan".into(),
            ));
        }
        Ok(())
    }
}

/// Re-estimate the next preclinical replicate batch with integer group-sequential boundaries.
pub fn plan_glioma_power_reestimation(
    request: &PowerReestimationRequest,
    arms: &[PowerArmObservation],
) -> Result<PowerReestimationPlan, PowerReestimationError> {
    validate_request(request)?;
    if arms.is_empty() || arms.len() > MAX_ARMS {
        return Err(PowerReestimationError::InvalidArm(
            "at least one and at most MAX_ARMS arms are required".into(),
        ));
    }
    let mut sorted = arms.to_vec();
    sorted.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    if sorted
        .windows(2)
        .any(|pair| pair[0].arm_id == pair[1].arm_id)
    {
        return Err(PowerReestimationError::InvalidArm(
            "arm identifiers must be unique".into(),
        ));
    }
    for arm in &sorted {
        validate_arm(arm, request)?;
    }
    let Some(control) = sorted
        .iter()
        .find(|arm| arm.arm_id == request.control_arm_id)
    else {
        return Err(PowerReestimationError::InvalidArm(
            "control arm is missing from the observation set".into(),
        ));
    };
    let look = u128::from(request.current_look);
    let max_looks = u128::from(request.max_looks);
    // Quadratic spending is an integer Lan-DeMets-style proxy: early looks spend less alpha,
    // while the final look spends the complete declared one-sided budget.
    let alpha_spent_milli =
        ((u128::from(request.alpha_total_milli) * look * look) / (max_looks * max_looks)) as u16;
    let alpha_spent_milli = alpha_spent_milli.max(1).min(request.alpha_total_milli);
    let control_variance_per_observation =
        u128::from(control.variance_milli2) / u128::from(control.observations.max(1));
    let mut decisions = Vec::with_capacity(sorted.len());
    let mut required_replicates_by_arm = BTreeMap::new();
    let mut efficacy_stop_order = Vec::new();
    let mut futility_stop_order = Vec::new();
    let mut continue_order = Vec::new();
    let mut hold_order = Vec::new();
    let mut risk_blocked_order = Vec::new();
    let mut budget_blocked_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut planned = Vec::<(String, u32, u32, u16)>::new();
    for arm in &sorted {
        let is_control = arm.arm_id == request.control_arm_id;
        let effect = if is_control {
            0
        } else {
            arm.mean_response_milli - control.mean_response_milli
        };
        let variance = control_variance_per_observation
            .saturating_add(u128::from(arm.variance_milli2) / u128::from(arm.observations.max(1)));
        let standard_error = integer_sqrt(variance).max(1).min(u128::from(u32::MAX)) as u32;
        // More conservative early looks have a larger boundary inflation term.
        let inflation = ((u128::from(standard_error)
            * u128::from(
                request
                    .alpha_total_milli
                    .saturating_sub(alpha_spent_milli)
                    .max(1),
            ))
            / u128::from(request.alpha_total_milli.max(1))) as i32;
        let efficacy_boundary = request.target_effect_milli.saturating_add(inflation);
        let futility_boundary = request
            .target_effect_milli
            .saturating_sub(standard_error as i32);
        let excess = effect.saturating_sub(efficacy_boundary);
        let power_proxy = if excess <= 0 {
            0
        } else {
            ((u128::from(excess as u32) * 1_000)
                / (u128::from(excess as u32) + u128::from(standard_error)))
            .min(1_000) as u16
        };
        let variance_term = variance.saturating_mul(u128::from(
            request.target_effect_milli.unsigned_abs().max(1),
        ));
        let required = if is_control || effect <= request.target_effect_milli {
            request.min_replicates_per_arm
        } else {
            let gap = u128::from(effect.saturating_sub(request.target_effect_milli) as u32).max(1);
            let estimate = variance_term
                .saturating_mul(u128::from(request.power_target_milli.max(1)))
                .saturating_div(gap.saturating_mul(gap).max(1))
                .saturating_add(u128::from(request.min_replicates_per_arm));
            estimate.min(u128::from(request.max_replicates_per_arm)) as u32
        };
        required_replicates_by_arm.insert(arm.arm_id.clone(), required);
        let decision = if arm.risk_milli > request.risk_ceiling_milli {
            PowerDecisionKind::RiskBlocked
        } else if is_control {
            PowerDecisionKind::Continue
        } else if arm.observations >= request.min_replicates_per_arm
            && effect >= efficacy_boundary
            && power_proxy >= request.power_target_milli
        {
            PowerDecisionKind::EfficacyStop
        } else if arm.observations >= request.min_replicates_per_arm && effect <= futility_boundary
        {
            PowerDecisionKind::FutilityStop
        } else if arm.observations < request.min_replicates_per_arm {
            PowerDecisionKind::HoldUnderpowered
        } else {
            PowerDecisionKind::Continue
        };
        let remaining = request
            .max_replicates_per_arm
            .saturating_sub(arm.observations);
        let requested_new = required
            .saturating_sub(arm.observations)
            .max(
                if matches!(
                    decision,
                    PowerDecisionKind::Continue | PowerDecisionKind::HoldUnderpowered
                ) {
                    request
                        .min_replicates_per_arm
                        .saturating_sub(arm.observations)
                } else {
                    0
                },
            )
            .min(request.max_new_replicates_per_arm)
            .min(remaining);
        let projected_cost = u64::from(requested_new).saturating_mul(u64::from(arm.cost_units));
        if matches!(
            decision,
            PowerDecisionKind::Continue | PowerDecisionKind::HoldUnderpowered
        ) && requested_new > 0
        {
            planned.push((
                arm.arm_id.clone(),
                requested_new,
                arm.cost_units,
                power_proxy,
            ));
        }
        let rationale = match decision {
            PowerDecisionKind::EfficacyStop => {
                "observed effect clears the look-adjusted efficacy boundary".into()
            }
            PowerDecisionKind::FutilityStop => {
                "upper uncertainty boundary remains below the declared target effect".into()
            }
            PowerDecisionKind::HoldUnderpowered => {
                "replicate floor is not met; retain the arm as an explicit hold".into()
            }
            PowerDecisionKind::RiskBlocked => {
                "arm risk exceeds the local preclinical risk ceiling".into()
            }
            PowerDecisionKind::BudgetBlocked => {
                "the next information-bearing batch cannot fit the declared budget".into()
            }
            PowerDecisionKind::Continue => {
                "re-estimated information gain supports another bounded interim batch".into()
            }
        };
        decisions.push(PowerArmDecision {
            arm_id: arm.arm_id.clone(),
            label: arm.label.clone(),
            is_control,
            effect_vs_control_milli: effect,
            standard_error_milli: standard_error,
            alpha_spent_milli,
            efficacy_boundary_milli: efficacy_boundary,
            futility_boundary_milli: futility_boundary,
            power_proxy_milli: power_proxy,
            information_utility_milli: power_proxy
                .saturating_add(1_000u16.saturating_sub(arm.risk_milli))
                .min(1_000),
            required_replicates: required,
            planned_replicates: requested_new,
            projected_cost_units: projected_cost,
            decision,
            rationale,
        });
    }
    planned.sort_by(|left, right| right.3.cmp(&left.3).then_with(|| left.0.cmp(&right.0)));
    let mut remaining_budget = request.budget_units;
    let mut selected_order = Vec::new();
    let mut selected_ids = BTreeSet::new();
    for (arm_id, replicates, cost, _) in &planned {
        let projected = u64::from(*replicates).saturating_mul(u64::from(*cost));
        if projected <= remaining_budget {
            remaining_budget -= projected;
            selected_order.push(arm_id.clone());
            selected_ids.insert(arm_id.clone());
        }
    }
    for decision in &mut decisions {
        if (matches!(
            decision.decision,
            PowerDecisionKind::Continue | PowerDecisionKind::HoldUnderpowered
        ) && decision.planned_replicates > 0)
            && !selected_ids.contains(&decision.arm_id)
        {
            decision.decision = PowerDecisionKind::BudgetBlocked;
            decision.rationale =
                "the information-bearing batch was ranked below the remaining budget".into();
            decision.planned_replicates = 0;
            decision.projected_cost_units = 0;
        }
        match decision.decision {
            PowerDecisionKind::EfficacyStop => efficacy_stop_order.push(decision.arm_id.clone()),
            PowerDecisionKind::FutilityStop => {
                futility_stop_order.push(decision.arm_id.clone());
                negative_evidence.push(format!("futility:{}", decision.arm_id));
            }
            PowerDecisionKind::HoldUnderpowered => {
                hold_order.push(decision.arm_id.clone());
                uncertainty.push(format!("underpowered:{}", decision.arm_id));
            }
            PowerDecisionKind::RiskBlocked => {
                risk_blocked_order.push(decision.arm_id.clone());
                uncertainty.push(format!("risk:{}", decision.arm_id));
            }
            PowerDecisionKind::BudgetBlocked => {
                budget_blocked_order.push(decision.arm_id.clone());
                uncertainty.push(format!("budget:{}", decision.arm_id));
            }
            PowerDecisionKind::Continue => {
                continue_order.push(decision.arm_id.clone());
                uncertainty.push(format!("interim:{}", decision.arm_id));
            }
        }
    }
    let total_projected_cost_units = decisions
        .iter()
        .map(|decision| decision.projected_cost_units)
        .sum::<u64>();
    let disposition = if !efficacy_stop_order.is_empty() {
        PowerReestimationDisposition::Efficacy
    } else if !futility_stop_order.is_empty() && selected_order.is_empty() {
        PowerReestimationDisposition::Futility
    } else if !budget_blocked_order.is_empty() && selected_order.is_empty() {
        PowerReestimationDisposition::BudgetBlocked
    } else if !risk_blocked_order.is_empty() && selected_order.is_empty() && hold_order.is_empty() {
        PowerReestimationDisposition::RiskBlocked
    } else if !selected_order.is_empty() {
        PowerReestimationDisposition::Continue
    } else if !hold_order.is_empty() {
        PowerReestimationDisposition::Hold
    } else {
        PowerReestimationDisposition::Unresolved
    };
    negative_evidence.sort();
    uncertainty.sort();
    let arm_order = sorted
        .iter()
        .map(|arm| arm.arm_id.clone())
        .collect::<Vec<_>>();
    let mut plan = PowerReestimationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        endpoint: request.endpoint.clone(),
        control_arm_id: request.control_arm_id.clone(),
        current_look: request.current_look,
        max_looks: request.max_looks,
        alpha_total_milli: request.alpha_total_milli,
        alpha_spent_milli,
        arm_order,
        selected_order,
        efficacy_stop_order,
        futility_stop_order,
        continue_order,
        hold_order,
        risk_blocked_order,
        budget_blocked_order,
        required_replicates_by_arm,
        decisions,
        total_projected_cost_units,
        budget_remaining_units: remaining_budget,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-power-reestimation"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| PowerReestimationError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request(budget_units: u64) -> PowerReestimationRequest {
        PowerReestimationRequest {
            objective: "test invasion assay power".into(),
            model_system: GliomaModelSystem::Organoid,
            endpoint: "invasion_fraction".into(),
            control_arm_id: "control".into(),
            target_effect_milli: 100,
            alpha_total_milli: 50,
            power_target_milli: 700,
            current_look: 1,
            max_looks: 4,
            min_replicates_per_arm: 4,
            max_replicates_per_arm: 24,
            max_new_replicates_per_arm: 8,
            budget_units,
            risk_ceiling_milli: 300,
        }
    }

    fn arms() -> Vec<PowerArmObservation> {
        vec![
            PowerArmObservation {
                arm_id: "control".into(),
                label: "vehicle".into(),
                artifact: artifact("control"),
                model_system: GliomaModelSystem::Organoid,
                mean_response_milli: 200,
                variance_milli2: 100,
                observations: 6,
                risk_milli: 50,
                cost_units: 2,
            },
            PowerArmObservation {
                arm_id: "perturbation".into(),
                label: "egfr perturbation".into(),
                artifact: artifact("perturbation"),
                model_system: GliomaModelSystem::Organoid,
                mean_response_milli: 310,
                variance_milli2: 100,
                observations: 6,
                risk_milli: 100,
                cost_units: 2,
            },
        ]
    }

    #[test]
    fn spends_less_alpha_at_early_looks_and_reestimates_a_batch() {
        let plan = plan_glioma_power_reestimation(&request(100), &arms()).unwrap();
        assert!(plan.alpha_spent_milli < plan.alpha_total_milli);
        assert_eq!(plan.disposition, PowerReestimationDisposition::Continue);
        assert_eq!(plan.selected_order, vec!["perturbation"]);
        assert!(plan.decisions[1].required_replicates > plan.decisions[1].planned_replicates);
        plan.validate().unwrap();
    }

    #[test]
    fn budget_and_risk_holds_are_not_promoted_to_execution() {
        let mut observed = arms();
        observed[1].risk_milli = 900;
        let plan = plan_glioma_power_reestimation(&request(1), &observed).unwrap();
        assert!(plan
            .decisions
            .iter()
            .any(|decision| decision.decision == PowerDecisionKind::RiskBlocked));
        assert!(plan.selected_order.is_empty());
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item == "risk:perturbation"));
    }

    #[test]
    fn arm_permutation_preserves_the_content_digest() {
        let mut reversed = arms();
        reversed.reverse();
        let first = plan_glioma_power_reestimation(&request(100), &arms()).unwrap();
        let second = plan_glioma_power_reestimation(&request(100), &reversed).unwrap();
        assert_eq!(first.digest, second.digest);
    }
}
