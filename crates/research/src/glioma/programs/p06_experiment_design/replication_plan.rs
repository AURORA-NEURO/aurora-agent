//! Multi-site replication topology planning for preclinical glioma experiments.
//!
//! A local power calculation can look strong while one site, batch, or model system carries the
//! entire effect. This feature estimates site-level contrasts, between-site heterogeneity, a
//! conservative power proxy, and leave-one-site-out sensitivity, then allocates additional local
//! replicates under explicit budget, risk, and site-diversity gates. It plans only; it never
//! dispatches an assay or makes a clinical decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationPlan1@1";
pub const MAX_SITES: usize = 128;
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_REPLICATES_PER_SITE: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationObservation {
    pub site_id: String,
    pub arm_id: String,
    pub label: String,
    pub artifact: LocalArtifactRef,
    pub model_system: GliomaModelSystem,
    pub mean_response_milli: i32,
    pub variance_milli2: u64,
    pub observations: u32,
    pub cost_units_per_replicate: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationPlanRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub treatment_arm_id: String,
    pub target_effect_milli: i32,
    pub alpha_total_milli: u16,
    pub power_target_milli: u16,
    pub min_sites: usize,
    pub max_sites: usize,
    pub min_replicates_per_site: u32,
    pub max_replicates_per_site: u32,
    pub budget_units: u64,
    pub max_total_replicates: u32,
    pub max_site_heterogeneity_milli: u16,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationSiteAction {
    Replicate,
    HoldUnderpowered,
    Heterogeneous,
    RiskBlocked,
    BudgetBlocked,
    InsufficientPair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationSitePlan {
    pub site_id: String,
    pub effect_vs_control_milli: i32,
    pub within_variance_milli2: u64,
    pub current_replicates: u32,
    pub planned_replicates: u32,
    pub projected_cost_units: u64,
    pub leave_one_out_shift_milli: u16,
    pub risk_milli: u16,
    pub action: ReplicationSiteAction,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationPlanDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Heterogeneous,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub control_arm_id: String,
    pub treatment_arm_id: String,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub risk_blocked_site_order: Vec<String>,
    pub budget_blocked_site_order: Vec<String>,
    pub plans: Vec<ReplicationSitePlan>,
    pub pooled_effect_milli: i32,
    pub heterogeneity_milli: u16,
    pub power_proxy_milli: u16,
    pub required_replicates_per_site: u32,
    pub total_planned_replicates: u32,
    pub total_projected_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReplicationPlanDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationPlanError {
    #[error("replication plan request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("replication plan output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication plan digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
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

fn digest_input(plan: &ReplicationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "endpoint": plan.endpoint,
        "control_arm_id": plan.control_arm_id,
        "treatment_arm_id": plan.treatment_arm_id,
        "site_order": plan.site_order,
        "selected_site_order": plan.selected_site_order,
        "risk_blocked_site_order": plan.risk_blocked_site_order,
        "budget_blocked_site_order": plan.budget_blocked_site_order,
        "plans": plan.plans,
        "pooled_effect_milli": plan.pooled_effect_milli,
        "heterogeneity_milli": plan.heterogeneity_milli,
        "power_proxy_milli": plan.power_proxy_milli,
        "required_replicates_per_site": plan.required_replicates_per_site,
        "total_planned_replicates": plan.total_planned_replicates,
        "total_projected_cost_units": plan.total_projected_cost_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
        "next_action": plan.next_action,
    })
}

fn validate_request(request: &ReplicationPlanRequest) -> Result<(), ReplicationPlanError> {
    if request.objective.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.control_arm_id.trim().is_empty()
        || request.treatment_arm_id.trim().is_empty()
        || request.control_arm_id == request.treatment_arm_id
        || request.target_effect_milli == 0
        || request.target_effect_milli.abs() > 1_000
        || request.alpha_total_milli == 0
        || request.alpha_total_milli > 500
        || request.power_target_milli < 500
        || request.power_target_milli > 1_000
        || request.min_sites == 0
        || request.max_sites < request.min_sites
        || request.max_sites > MAX_SITES
        || request.min_replicates_per_site == 0
        || request.max_replicates_per_site < request.min_replicates_per_site
        || request.max_replicates_per_site > MAX_REPLICATES_PER_SITE
        || request.budget_units == 0
        || request.max_total_replicates < request.min_sites as u32 * request.min_replicates_per_site
        || request.max_total_replicates > MAX_REPLICATES_PER_SITE * MAX_SITES as u32
        || request.max_site_heterogeneity_milli > 1_000
        || request.risk_ceiling_milli > 1_000
    {
        return Err(ReplicationPlanError::InvalidRequest(
            "objective, endpoint, arm identity, effect, alpha/power, site, replicate, budget, heterogeneity, and risk bounds are required".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    observation: &ReplicationObservation,
    request: &ReplicationPlanRequest,
) -> Result<(), ReplicationPlanError> {
    observation
        .artifact
        .validate()
        .map_err(|error| ReplicationPlanError::InvalidObservation(error.to_string()))?;
    if observation.site_id.trim().is_empty()
        || observation.arm_id != request.control_arm_id
            && observation.arm_id != request.treatment_arm_id
        || observation.label.trim().is_empty()
        || observation.model_system != request.model_system
        || observation.mean_response_milli.abs() > 1_000
        || observation.variance_milli2 == 0
        || observation.observations == 0
        || observation.cost_units_per_replicate == 0
        || observation.risk_milli > 1_000
    {
        return Err(ReplicationPlanError::InvalidObservation(
            "observations require local artifacts, known arms, matching model system, bounded means, positive variance/count/cost, and bounded risk".into(),
        ));
    }
    Ok(())
}

fn validate_output(plan: &ReplicationPlan) -> Result<(), ReplicationPlanError> {
    if plan.feature_id != FEATURE_ID
        || plan.output_schema != OUTPUT_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.endpoint.trim().is_empty()
        || !canonical(&plan.site_order)
        || !canonical(&plan.selected_site_order)
        || !canonical(&plan.risk_blocked_site_order)
        || !canonical(&plan.budget_blocked_site_order)
        || !canonical(&plan.negative_evidence)
        || !canonical(&plan.uncertainty)
        || plan.plans.len() != plan.site_order.len()
        || plan.heterogeneity_milli > 1_000
        || plan.power_proxy_milli > 1_000
        || plan.plans.iter().any(|item| {
            item.site_id.trim().is_empty()
                || item.current_replicates == 0
                || item.planned_replicates < item.current_replicates
                || item.projected_cost_units == 0
                || item.leave_one_out_shift_milli > 1_000
                || item.risk_milli > 1_000
                || item.rationale.trim().is_empty()
        })
    {
        return Err(ReplicationPlanError::InvalidOutput(
            "identity, ordering, site plan, replicate, cost, heterogeneity, power, or uncertainty bounds are invalid".into(),
        ));
    }
    let site_ids = plan.site_order.iter().cloned().collect::<BTreeSet<_>>();
    let output_ids = plan
        .plans
        .iter()
        .map(|item| item.site_id.clone())
        .collect::<BTreeSet<_>>();
    if site_ids != output_ids
        || plan
            .selected_site_order
            .iter()
            .chain(plan.risk_blocked_site_order.iter())
            .chain(plan.budget_blocked_site_order.iter())
            .any(|site| !site_ids.contains(site))
    {
        return Err(ReplicationPlanError::InvalidOutput(
            "site plan partitions do not reconcile".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(plan))
        .map_err(|error| ReplicationPlanError::Digest(error.to_string()))?;
    if expected != plan.digest {
        return Err(ReplicationPlanError::InvalidOutput(
            "replication plan digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ReplicationPlan {
    pub fn validate(&self) -> Result<(), ReplicationPlanError> {
        validate_output(self)
    }
}

/// Compile a multi-site replication topology from local arm summaries.
pub fn plan_glioma_replication(
    request: &ReplicationPlanRequest,
    observations: &[ReplicationObservation],
) -> Result<ReplicationPlan, ReplicationPlanError> {
    validate_request(request)?;
    if observations.is_empty() || observations.len() > MAX_OBSERVATIONS {
        return Err(ReplicationPlanError::InvalidObservation(
            "a bounded non-empty observation set is required".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for observation in observations {
        validate_observation(observation, request)?;
        if !keys.insert((observation.site_id.clone(), observation.arm_id.clone())) {
            return Err(ReplicationPlanError::InvalidObservation(
                "each site/arm pair must be unique".into(),
            ));
        }
    }
    let mut by_site = BTreeMap::<String, BTreeMap<String, &ReplicationObservation>>::new();
    for observation in observations {
        by_site
            .entry(observation.site_id.clone())
            .or_default()
            .insert(observation.arm_id.clone(), observation);
    }
    let mut sites = Vec::new();
    for (site_id, arms) in by_site {
        let Some(control) = arms.get(&request.control_arm_id) else {
            continue;
        };
        let Some(treatment) = arms.get(&request.treatment_arm_id) else {
            continue;
        };
        let effect = treatment.mean_response_milli - control.mean_response_milli;
        let variance = treatment
            .variance_milli2
            .saturating_div(u64::from(treatment.observations))
            .saturating_add(
                control
                    .variance_milli2
                    .saturating_div(u64::from(control.observations)),
            )
            .max(1);
        let current_replicates = control.observations.min(treatment.observations);
        let cost = u64::from(
            control
                .cost_units_per_replicate
                .max(treatment.cost_units_per_replicate),
        );
        let risk = control.risk_milli.max(treatment.risk_milli);
        sites.push((site_id, effect, variance, current_replicates, cost, risk));
    }
    sites.sort_by(|left, right| left.0.cmp(&right.0));
    if sites.len() > request.max_sites {
        sites.truncate(request.max_sites);
    }
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    if sites.len() < request.min_sites {
        uncertainty.insert(format!(
            "paired-sites:{}<{}",
            sites.len(),
            request.min_sites
        ));
    }
    let total_weight = sites
        .iter()
        .map(|(_, _, variance, _, _, _)| 1_000_000_u64 / variance.max(&1))
        .sum::<u64>()
        .max(1);
    let pooled_effect = (sites
        .iter()
        .map(|(_, effect, variance, _, _, _)| {
            i128::from(*effect) * i128::from(1_000_000_u64 / variance.max(&1))
        })
        .sum::<i128>()
        / i128::from(total_weight)) as i32;
    let heterogeneity = (sites
        .iter()
        .map(|(_, effect, _, _, _, _)| {
            (i64::from(*effect) - i64::from(pooled_effect)).unsigned_abs()
        })
        .sum::<u64>()
        / sites.len().max(1) as u64)
        .min(1_000) as u16;
    if heterogeneity > request.max_site_heterogeneity_milli {
        uncertainty.insert(format!("heterogeneity:{heterogeneity}"));
    }
    let pooled_variance = sites
        .iter()
        .map(|(_, _, variance, _, _, _)| *variance)
        .sum::<u64>()
        .saturating_div(sites.len().max(1) as u64)
        .max(1);
    let alpha_penalty =
        1_000_u64 + u64::from(500_u16.saturating_sub(request.alpha_total_milli)) * 2;
    let power_penalty = u64::from(request.power_target_milli);
    let numerator = alpha_penalty
        .saturating_add(power_penalty)
        .saturating_mul(alpha_penalty.saturating_add(power_penalty))
        .saturating_mul(pooled_variance);
    let denominator = u64::from(request.target_effect_milli.unsigned_abs())
        .saturating_mul(u64::from(request.target_effect_milli.unsigned_abs()).max(1));
    let required_total = numerator
        .saturating_div(denominator.max(1))
        .max(u64::from(request.min_replicates_per_site) * sites.len().max(1) as u64)
        .min(u64::from(request.max_replicates_per_site) * sites.len().max(1) as u64);
    let required_per_site = (required_total
        .saturating_add(sites.len().max(1) as u64 - 1)
        .saturating_div(sites.len().max(1) as u64))
    .clamp(
        u64::from(request.min_replicates_per_site),
        u64::from(request.max_replicates_per_site),
    ) as u32;
    let mut plans = Vec::new();
    let mut selected = BTreeSet::new();
    let mut risk_blocked = BTreeSet::new();
    let mut budget_blocked = BTreeSet::new();
    let mut total_planned_replicates = 0_u32;
    let mut total_cost = 0_u64;
    for (site_id, effect, variance, current, cost, risk) in &sites {
        let planned = if *risk > request.risk_ceiling_milli {
            risk_blocked.insert(site_id.clone());
            *current
        } else {
            let desired = required_per_site.max(*current);
            let available_by_budget = request
                .budget_units
                .saturating_sub(total_cost)
                .saturating_div(*cost)
                .min(u64::from(
                    request
                        .max_total_replicates
                        .saturating_sub(total_planned_replicates),
                ));
            let increment = desired
                .saturating_sub(*current)
                .min(available_by_budget as u32);
            if increment < desired.saturating_sub(*current) {
                budget_blocked.insert(site_id.clone());
            }
            let selected_replicates = current.saturating_add(increment);
            if selected_replicates >= required_per_site {
                selected.insert(site_id.clone());
            }
            selected_replicates
        };
        let projected_cost = planned as u64 * *cost;
        total_planned_replicates = total_planned_replicates.saturating_add(planned);
        total_cost = total_cost.saturating_add(projected_cost);
        let loo_effect = if sites.len() > 1 {
            let remainder = sites
                .iter()
                .filter(|candidate| candidate.0 != *site_id)
                .map(|(_, site_effect, site_variance, _, _, _)| {
                    (i128::from(*site_effect) * i128::from(1_000_000_u64 / site_variance.max(&1)))
                        as i128
                })
                .sum::<i128>();
            let remainder_weight = sites
                .iter()
                .filter(|candidate| candidate.0 != *site_id)
                .map(|(_, _, site_variance, _, _, _)| 1_000_000_u64 / site_variance.max(&1))
                .sum::<u64>()
                .max(1);
            (remainder
                .saturating_div(i128::from(remainder_weight))
                .saturating_sub(i128::from(pooled_effect)))
            .unsigned_abs()
            .min(1_000) as u16
        } else {
            0
        };
        let action = if *risk > request.risk_ceiling_milli {
            ReplicationSiteAction::RiskBlocked
        } else if budget_blocked.contains(site_id) {
            ReplicationSiteAction::BudgetBlocked
        } else if loo_effect > request.max_site_heterogeneity_milli {
            ReplicationSiteAction::Heterogeneous
        } else if planned >= required_per_site {
            ReplicationSiteAction::Replicate
        } else {
            ReplicationSiteAction::HoldUnderpowered
        };
        if action == ReplicationSiteAction::Heterogeneous {
            negative_evidence.insert(format!("site:{site_id}:leave-one-out-shift-{loo_effect}"));
        }
        plans.push(ReplicationSitePlan {
            site_id: site_id.clone(),
            effect_vs_control_milli: *effect,
            within_variance_milli2: *variance,
            current_replicates: *current,
            planned_replicates: planned,
            projected_cost_units: projected_cost,
            leave_one_out_shift_milli: loo_effect,
            risk_milli: *risk,
            action,
            rationale: format!(
                "site effect {effect}, pooled effect {pooled_effect}, planned replicates {planned}, required per site {required_per_site}"
            ),
        });
    }
    let se_proxy = integer_sqrt(
        u128::from(pooled_variance).saturating_mul(u128::from(total_planned_replicates.max(1))),
    )
    .max(1) as u64;
    let power_proxy = (u64::from(pooled_effect.unsigned_abs())
        .saturating_mul(u64::from(total_planned_replicates.max(1)))
        .saturating_mul(1_000)
        .saturating_div(se_proxy.saturating_mul(10).max(1)))
    .min(1_000) as u16;
    let budget_remaining = request.budget_units.saturating_sub(total_cost);
    if power_proxy < request.power_target_milli {
        uncertainty.insert(format!(
            "power-proxy:{power_proxy}<{}",
            request.power_target_milli
        ));
    }
    let disposition = if sites.is_empty() {
        ReplicationPlanDisposition::Unresolved
    } else if sites.len() < request.min_sites {
        ReplicationPlanDisposition::Unresolved
    } else if total_cost >= request.budget_units && selected.len() < request.min_sites {
        ReplicationPlanDisposition::BudgetBlocked
    } else if heterogeneity > request.max_site_heterogeneity_milli {
        ReplicationPlanDisposition::Heterogeneous
    } else if power_proxy >= request.power_target_milli && selected.len() >= request.min_sites {
        ReplicationPlanDisposition::Qualified
    } else {
        ReplicationPlanDisposition::Partial
    };
    let next_action = match disposition {
        ReplicationPlanDisposition::Qualified => {
            "execute the balanced site replication plan locally, then re-estimate with observed site summaries"
        }
        ReplicationPlanDisposition::Partial => {
            "add planned independent replicates and re-estimate power before promoting the effect"
        }
        ReplicationPlanDisposition::BudgetBlocked => {
            "increase the replication budget or reduce per-site assay cost"
        }
        ReplicationPlanDisposition::Heterogeneous => {
            "inspect site-specific protocol, model-system, and batch causes before pooling"
        }
        ReplicationPlanDisposition::Unresolved => {
            "provide paired control/treatment observations from the minimum number of independent sites"
        }
    }
    .to_string();
    let mut plan = ReplicationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        endpoint: request.endpoint.clone(),
        control_arm_id: request.control_arm_id.clone(),
        treatment_arm_id: request.treatment_arm_id.clone(),
        site_order: sites.iter().map(|site| site.0.clone()).collect(),
        selected_site_order: selected.into_iter().collect(),
        risk_blocked_site_order: risk_blocked.into_iter().collect(),
        budget_blocked_site_order: budget_blocked.into_iter().collect(),
        plans,
        pooled_effect_milli: pooled_effect,
        heterogeneity_milli: heterogeneity,
        power_proxy_milli: power_proxy,
        required_replicates_per_site: required_per_site,
        total_planned_replicates,
        total_projected_cost_units: total_cost,
        budget_remaining_units: budget_remaining,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ReplicationPlanError::Digest(error.to_string()))?;
    validate_output(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn observation(site: &str, arm: &str, mean: i32) -> ReplicationObservation {
        ReplicationObservation {
            site_id: site.into(),
            arm_id: arm.into(),
            label: format!("{site}-{arm}"),
            artifact: artifact(&format!("{site}-{arm}")),
            model_system: GliomaModelSystem::Organoid,
            mean_response_milli: mean,
            variance_milli2: 100,
            observations: 2,
            cost_units_per_replicate: 1,
            risk_milli: 100,
        }
    }

    fn request() -> ReplicationPlanRequest {
        ReplicationPlanRequest {
            objective: "replicate glioma invasion effect".into(),
            model_system: GliomaModelSystem::Organoid,
            endpoint: "invasion".into(),
            control_arm_id: "control".into(),
            treatment_arm_id: "treated".into(),
            target_effect_milli: 100,
            alpha_total_milli: 50,
            power_target_milli: 700,
            min_sites: 2,
            max_sites: 4,
            min_replicates_per_site: 2,
            max_replicates_per_site: 20,
            budget_units: 100,
            max_total_replicates: 80,
            max_site_heterogeneity_milli: 200,
            risk_ceiling_milli: 500,
        }
    }

    #[test]
    fn replication_plan_is_replay_stable_and_tracks_power() {
        let observations = vec![
            observation("site-a", "control", 100),
            observation("site-a", "treated", 220),
            observation("site-b", "control", 110),
            observation("site-b", "treated", 230),
        ];
        let first = plan_glioma_replication(&request(), &observations).unwrap();
        let replay = plan_glioma_replication(&request(), &observations).unwrap();
        first.validate().unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.selected_site_order.len(), 2);
        assert!(first.pooled_effect_milli >= 100);
    }

    #[test]
    fn risk_blocked_site_is_explicit() {
        let mut request = request();
        request.risk_ceiling_milli = 100;
        let mut high_risk = observation("site-b", "treated", 230);
        high_risk.risk_milli = 900;
        let result = plan_glioma_replication(
            &request,
            &[
                observation("site-a", "control", 100),
                observation("site-a", "treated", 220),
                observation("site-b", "control", 110),
                high_risk,
            ],
        )
        .unwrap();
        assert!(result.risk_blocked_site_order.contains(&"site-b".into()));
        assert!(result
            .plans
            .iter()
            .any(|plan| plan.action == ReplicationSiteAction::RiskBlocked));
    }

    #[test]
    fn heterogeneous_site_remains_negative() {
        let result = plan_glioma_replication(
            &request(),
            &[
                observation("site-a", "control", 100),
                observation("site-a", "treated", 220),
                observation("site-b", "control", 110),
                observation("site-b", "treated", 800),
            ],
        )
        .unwrap();
        assert_eq!(
            result.disposition,
            ReplicationPlanDisposition::Heterogeneous
        );
        assert!(!result.negative_evidence.is_empty());
    }
}
