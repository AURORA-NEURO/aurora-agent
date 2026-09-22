//! Scenario stress surface for preclinical glioma power planning.
//!
//! Interim power re-estimation (P06-F04) reacts to observed arm summaries. This feature is a
//! prospective design stress test: before a protocol is admitted, it evaluates every declared
//! effect/variance/attrition world, computes an integer replicate requirement, and reports the
//! worst-case power proxy and budget exposure. It never treats the proxy as a validated clinical
//! guarantee and never dispatches an assay.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaPowerStressSurface1@1";
pub const MAX_SCENARIOS: usize = 512;
pub const MAX_ARMS: usize = 256;
pub const MAX_REPLICATES_PER_ARM: u32 = 1_000_000;
pub const SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressScenario {
    pub scenario_id: String,
    pub label: String,
    pub weight_milli: u16,
    pub target_effect_milli: u32,
    pub variance_milli2: u64,
    pub attrition_milli: u16,
    pub feasibility_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressArm {
    pub arm_id: String,
    pub label: String,
    pub feature_id: String,
    pub cost_units_per_replicate: u32,
    pub risk_milli: u16,
    pub max_replicates: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressSurfaceRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub scenarios: Vec<PowerStressScenario>,
    pub arms: Vec<PowerStressArm>,
    pub critical_quantile_milli: u32,
    pub power_quantile_milli: u32,
    pub target_power_milli: u16,
    pub min_replicates_per_arm: u32,
    pub max_total_replicates: u32,
    pub budget_units: u64,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressScenarioResult {
    pub scenario_id: String,
    pub required_replicates: u32,
    pub planned_replicates: u32,
    pub achieved_power_milli: u16,
    pub projected_cost_units: u64,
    pub attrition_adjusted: bool,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressArmResult {
    pub arm_id: String,
    pub label: String,
    pub feature_id: String,
    pub worst_case_required_replicates: u32,
    pub planned_replicates: u32,
    pub worst_case_power_milli: u16,
    pub weighted_power_milli: u16,
    pub projected_cost_units: u64,
    pub scenario_results: Vec<PowerStressScenarioResult>,
    pub action: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerStressDisposition {
    Qualified,
    Partial,
    RiskBlocked,
    BudgetBlocked,
    NoEligibleArms,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerStressSurface {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub scenario_order: Vec<String>,
    pub arm_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub results: Vec<PowerStressArmResult>,
    pub target_power_milli: u16,
    pub budget_remaining_units: u64,
    pub total_projected_cost_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: PowerStressDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PowerStressError {
    #[error("power stress request is invalid: {0}")]
    InvalidRequest(String),
    #[error("power stress input is invalid: {0}")]
    InvalidInput(String),
    #[error("power stress output is invalid: {0}")]
    InvalidOutput(String),
    #[error("power stress digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &PowerStressSurface) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "scenario_order": output.scenario_order,
        "arm_order": output.arm_order,
        "selected_order": output.selected_order,
        "risk_blocked_order": output.risk_blocked_order,
        "budget_blocked_order": output.budget_blocked_order,
        "results": output.results,
        "target_power_milli": output.target_power_milli,
        "budget_remaining_units": output.budget_remaining_units,
        "total_projected_cost_units": output.total_projected_cost_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &PowerStressSurfaceRequest) -> Result<(), PowerStressError> {
    if request.objective.trim().is_empty()
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || request.arms.is_empty()
        || request.arms.len() > MAX_ARMS
        || request.critical_quantile_milli == 0
        || request.power_quantile_milli == 0
        || request.target_power_milli == 0
        || request.target_power_milli > 1_000
        || request.min_replicates_per_arm == 0
        || request.max_total_replicates < request.min_replicates_per_arm
        || request.max_total_replicates > MAX_REPLICATES_PER_ARM
        || request.budget_units == 0
        || request.risk_ceiling_milli > 1_000
    {
        return Err(PowerStressError::InvalidRequest(
            "objective, bounded scenarios/arms, positive quantiles/power/replicate/budget limits, and finite risk gates are required".into(),
        ));
    }
    let mut scenario_ids = BTreeSet::new();
    let mut weight = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || scenario.label.trim().is_empty()
            || scenario.weight_milli == 0
            || scenario.target_effect_milli == 0
            || scenario.variance_milli2 == 0
            || scenario.attrition_milli >= 1_000
            || scenario.feasibility_milli > 1_000
            || !scenario_ids.insert(scenario.scenario_id.clone())
        {
            return Err(PowerStressError::InvalidInput(
                "scenario identity, positive effect/variance, sub-1000 attrition, bounded feasibility, and uniqueness are required".into(),
            ));
        }
        weight = weight.saturating_add(u32::from(scenario.weight_milli));
    }
    if weight != 1_000 {
        return Err(PowerStressError::InvalidInput(
            "scenario weights must sum to exactly 1000 milli-units".into(),
        ));
    }
    let mut arm_ids = BTreeSet::new();
    for arm in &request.arms {
        if arm.arm_id.trim().is_empty()
            || arm.label.trim().is_empty()
            || arm.feature_id.trim().is_empty()
            || arm.cost_units_per_replicate == 0
            || arm.risk_milli > 1_000
            || arm.max_replicates < request.min_replicates_per_arm
            || !arm_ids.insert(arm.arm_id.clone())
        {
            return Err(PowerStressError::InvalidInput(
                "arm identity, feature, positive cost, bounded risk, capacity, and uniqueness are required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &PowerStressSurface) -> Result<(), PowerStressError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.scenario_order)
        || !canonical(&output.arm_order)
        || !canonical(&output.selected_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.budget_blocked_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .results
            .windows(2)
            .any(|pair| pair[0].arm_id >= pair[1].arm_id)
        || output.results.iter().any(|result| {
            result.arm_id.trim().is_empty()
                || result.label.trim().is_empty()
                || result.feature_id.trim().is_empty()
                || result.scenario_results.is_empty()
                || result.worst_case_power_milli > 1_000
                || result.weighted_power_milli > 1_000
                || result.action.trim().is_empty()
                || result.rationale.trim().is_empty()
        })
    {
        return Err(PowerStressError::InvalidOutput(
            "identity, canonical ordering, scenario-result, power, and rationale invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| PowerStressError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(PowerStressError::InvalidOutput(
            "digest is not bound to the power stress surface".into(),
        ));
    }
    Ok(())
}

impl PowerStressSurface {
    pub fn validate(&self) -> Result<(), PowerStressError> {
        validate_output(self)
    }
}

fn required_replicates(
    scenario: &PowerStressScenario,
    critical_quantile_milli: u32,
    power_quantile_milli: u32,
) -> u32 {
    let z = u128::from(critical_quantile_milli.saturating_add(power_quantile_milli));
    let numerator = z
        .saturating_mul(z)
        .saturating_mul(u128::from(scenario.variance_milli2));
    let denominator = u128::from(scenario.target_effect_milli)
        .saturating_mul(u128::from(scenario.target_effect_milli))
        .max(1);
    let base = numerator.saturating_add(denominator.saturating_sub(1)) / denominator;
    let attrition_denominator = u128::from(1_000_u16.saturating_sub(scenario.attrition_milli));
    let inflated = base
        .saturating_mul(1_000)
        .saturating_add(attrition_denominator.saturating_sub(1))
        / attrition_denominator.max(1);
    inflated.max(1).min(u128::from(MAX_REPLICATES_PER_ARM)) as u32
}

fn achieved_power(planned: u32, required: u32) -> u16 {
    if required == 0 {
        return 1_000;
    }
    u64::from(planned)
        .saturating_mul(SCALE)
        .checked_div(u64::from(required))
        .unwrap_or(0)
        .min(SCALE) as u16
}

/// Stress-test every eligible arm against a declared prospective power surface.
pub fn plan_glioma_power_stress_surface(
    request: &PowerStressSurfaceRequest,
) -> Result<PowerStressSurface, PowerStressError> {
    validate_request(request)?;
    let scenario_order = request
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let arm_order = request
        .arms
        .iter()
        .map(|arm| arm.arm_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let risk_blocked_order = request
        .arms
        .iter()
        .filter(|arm| arm.risk_milli > request.risk_ceiling_milli)
        .map(|arm| arm.arm_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let eligible = request
        .arms
        .iter()
        .filter(|arm| arm.risk_milli <= request.risk_ceiling_milli)
        .collect::<Vec<_>>();
    let eligible_count = eligible.len();
    let mut results = Vec::new();
    let mut selected_order = Vec::new();
    let mut budget_blocked_order = Vec::new();
    let mut total_cost = 0_u64;
    let mut negative_evidence = risk_blocked_order
        .iter()
        .map(|id| format!("risk-gate-blocked:{id}"))
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    for arm in eligible {
        let mut scenario_results = Vec::new();
        let mut worst_required = 0_u32;
        let mut weighted_power = 0_u64;
        for scenario in &request.scenarios {
            let required = required_replicates(
                scenario,
                request.critical_quantile_milli,
                request.power_quantile_milli,
            );
            worst_required = worst_required.max(required);
            let remaining_replicate_budget = request
                .max_total_replicates
                .saturating_sub(request.min_replicates_per_arm);
            let planned = worst_required
                .max(request.min_replicates_per_arm)
                .min(arm.max_replicates)
                .min(remaining_replicate_budget.max(request.min_replicates_per_arm));
            let power = achieved_power(planned, required);
            weighted_power = weighted_power
                .saturating_add(u64::from(power).saturating_mul(u64::from(scenario.weight_milli)));
            scenario_results.push(PowerStressScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                required_replicates: required,
                planned_replicates: planned,
                achieved_power_milli: power,
                projected_cost_units: u64::from(planned)
                    .saturating_mul(u64::from(arm.cost_units_per_replicate)),
                attrition_adjusted: scenario.attrition_milli > 0,
                action: if power >= request.target_power_milli {
                    "power-qualified".into()
                } else {
                    "underpowered-under-scenario".into()
                },
            });
        }
        let planned = worst_required
            .max(request.min_replicates_per_arm)
            .min(arm.max_replicates)
            .min(request.max_total_replicates);
        let projected_cost =
            u64::from(planned).saturating_mul(u64::from(arm.cost_units_per_replicate));
        let worst_power = scenario_results
            .iter()
            .map(|result| result.achieved_power_milli)
            .min()
            .unwrap_or(0);
        let weighted_power = (weighted_power / u64::from(SCALE as u16)).min(SCALE) as u16;
        let action = if worst_power < request.target_power_milli {
            "underpowered".to_string()
        } else if total_cost.saturating_add(projected_cost) > request.budget_units {
            budget_blocked_order.push(arm.arm_id.clone());
            "budget-blocked".to_string()
        } else {
            selected_order.push(arm.arm_id.clone());
            total_cost = total_cost.saturating_add(projected_cost);
            "qualified".to_string()
        };
        if action == "underpowered" {
            negative_evidence.push(format!("underpowered:{arm_id}", arm_id = arm.arm_id));
        }
        results.push(PowerStressArmResult {
            arm_id: arm.arm_id.clone(),
            label: arm.label.clone(),
            feature_id: arm.feature_id.clone(),
            worst_case_required_replicates: worst_required,
            planned_replicates: planned,
            worst_case_power_milli: worst_power,
            weighted_power_milli: weighted_power,
            projected_cost_units: projected_cost,
            scenario_results,
            action,
            rationale: if worst_power < request.target_power_milli {
                "the declared worst-case scenario remains below the target power proxy".into()
            } else {
                "all declared scenarios meet the target power proxy within arm limits".into()
            },
        });
    }
    results.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    selected_order.sort();
    budget_blocked_order.sort();
    negative_evidence.sort();
    if !risk_blocked_order.is_empty() || !budget_blocked_order.is_empty() {
        uncertainty.push("eligible-arm-set-is-constrained-by-risk-or-budget".into());
    }
    if results
        .iter()
        .any(|result| result.worst_case_power_milli < request.target_power_milli)
    {
        uncertainty.push("target-power-proxy-fails-in-at-least-one-declared-world".into());
    }
    uncertainty.sort();
    let disposition = if eligible_count == 0 {
        PowerStressDisposition::NoEligibleArms
    } else if selected_order.is_empty() {
        if !budget_blocked_order.is_empty() {
            PowerStressDisposition::BudgetBlocked
        } else {
            PowerStressDisposition::Partial
        }
    } else if results
        .iter()
        .any(|result| result.worst_case_power_milli < request.target_power_milli)
        || !budget_blocked_order.is_empty()
    {
        PowerStressDisposition::Partial
    } else {
        PowerStressDisposition::Qualified
    };
    let mut output = PowerStressSurface {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        scenario_order,
        arm_order,
        selected_order,
        risk_blocked_order,
        budget_blocked_order,
        results,
        target_power_milli: request.target_power_milli,
        budget_remaining_units: request.budget_units.saturating_sub(total_cost),
        total_projected_cost_units: total_cost,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| PowerStressError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PowerStressSurfaceRequest {
        PowerStressSurfaceRequest {
            objective: "stress invasion assay power".into(),
            model_system: GliomaModelSystem::Organoid,
            scenarios: vec![
                PowerStressScenario {
                    scenario_id: "nominal".into(),
                    label: "nominal effect".into(),
                    weight_milli: 600,
                    target_effect_milli: 500,
                    variance_milli2: 100,
                    attrition_milli: 100,
                    feasibility_milli: 900,
                },
                PowerStressScenario {
                    scenario_id: "stress".into(),
                    label: "attenuated effect".into(),
                    weight_milli: 400,
                    target_effect_milli: 350,
                    variance_milli2: 180,
                    attrition_milli: 250,
                    feasibility_milli: 750,
                },
            ],
            arms: vec![
                PowerStressArm {
                    arm_id: "arm-a".into(),
                    label: "candidate A".into(),
                    feature_id: "assay-a".into(),
                    cost_units_per_replicate: 3,
                    risk_milli: 100,
                    max_replicates: 200,
                },
                PowerStressArm {
                    arm_id: "arm-b".into(),
                    label: "candidate B".into(),
                    feature_id: "assay-b".into(),
                    cost_units_per_replicate: 4,
                    risk_milli: 100,
                    max_replicates: 200,
                },
            ],
            critical_quantile_milli: 1_960,
            power_quantile_milli: 840,
            target_power_milli: 800,
            min_replicates_per_arm: 2,
            max_total_replicates: 1_000,
            budget_units: 10_000,
            risk_ceiling_milli: 500,
        }
    }

    #[test]
    fn stress_surface_is_deterministic_and_valid() {
        let first = plan_glioma_power_stress_surface(&request()).expect("surface");
        let second = plan_glioma_power_stress_surface(&request()).expect("surface");
        assert_eq!(first, second);
        assert_eq!(first.results.len(), 2);
        assert!(first
            .results
            .iter()
            .all(|result| result.scenario_results.len() == 2));
        first.validate().expect("valid output");
    }

    #[test]
    fn retains_risk_and_underpower_negative_evidence() {
        let mut input = request();
        input.arms[1].risk_milli = 900;
        input.scenarios[1].target_effect_milli = 10;
        input.target_power_milli = 999;
        let output = plan_glioma_power_stress_surface(&input).expect("surface");
        assert_eq!(output.risk_blocked_order, vec!["arm-b"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "risk-gate-blocked:arm-b"));
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.starts_with("underpowered:")));
    }
}
