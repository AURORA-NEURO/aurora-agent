//! Prospective acquisition-capacity allocation for high-throughput preclinical glioma studies.
//!
//! This controller is a deterministic max-min/fair-share allocator over approved assay demand.
//! It reserves maintenance and operator capacity before allocation, spends a bounded budget, and
//! allocates one unit at a time using a weighted-deficit utility so low-volume studies cannot be
//! starved by a large campaign.  It emits a plan only: instrument preflight and signed operator
//! authorization remain separate gates.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaAcquisitionCapacityPlan1@2";
pub const MAX_DEMANDS: usize = 2_048;
pub const MAX_RESOURCES: usize = 512;
pub const MAX_HORIZON: u64 = 1_000_000;
const CAPACITY_BEAM_WIDTH: usize = 96;
const CAPACITY_CANDIDATE_WIDTH: usize = 128;
const MAX_BEAM_STEPS: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionDemand {
    pub campaign_id: String,
    pub priority_milli: u16,
    pub fairness_weight_milli: u16,
    pub minimum_units: u32,
    pub target_units: u32,
    pub maximum_units: u32,
    pub cost_per_unit: u32,
    pub latest_tick: u64,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionCapacityResource {
    pub resource_id: String,
    pub capacity_units: u32,
    pub operator_capacity_units: u32,
    pub maintenance_reserved_units: u32,
    pub budget_units: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionCapacityRequest {
    pub objective: String,
    pub current_tick: u64,
    pub horizon_ticks: u64,
    pub total_budget_units: u64,
    pub demands: Vec<AcquisitionDemand>,
    pub resources: Vec<AcquisitionCapacityResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionAllocation {
    pub campaign_id: String,
    pub resource_id: String,
    pub allocated_units: u32,
    pub cost_units: u64,
    pub completion_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionDeferral {
    pub campaign_id: String,
    pub allocated_units: u32,
    pub deferred_units: u32,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionCapacityPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub resource_order: Vec<String>,
    pub campaign_order: Vec<String>,
    pub allocations: Vec<AcquisitionAllocation>,
    pub deferrals: Vec<AcquisitionDeferral>,
    pub total_allocated_units: u64,
    pub total_cost_units: u64,
    pub minimum_fairness_milli: u16,
    pub weighted_jain_milli: u16,
    pub completion_tick: u64,
    pub dispatch_permitted: bool,
    pub negative_evidence_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AcquisitionCapacityError {
    #[error("acquisition capacity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("acquisition capacity plan is invalid: {0}")]
    InvalidOutput(String),
    #[error("acquisition capacity digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_body(plan: &AcquisitionCapacityPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "resource_order": plan.resource_order,
        "campaign_order": plan.campaign_order,
        "allocations": plan.allocations,
        "deferrals": plan.deferrals,
        "total_allocated_units": plan.total_allocated_units,
        "total_cost_units": plan.total_cost_units,
        "minimum_fairness_milli": plan.minimum_fairness_milli,
        "weighted_jain_milli": plan.weighted_jain_milli,
        "completion_tick": plan.completion_tick,
        "dispatch_permitted": plan.dispatch_permitted,
        "negative_evidence_order": plan.negative_evidence_order,
    })
}

fn validate_request(request: &AcquisitionCapacityRequest) -> Result<(), AcquisitionCapacityError> {
    if !safe_text(&request.objective)
        || request.current_tick == 0
        || request.horizon_ticks == 0
        || request.horizon_ticks > MAX_HORIZON
        || request.total_budget_units == 0
        || request.demands.is_empty()
        || request.demands.len() > MAX_DEMANDS
        || request.resources.is_empty()
        || request.resources.len() > MAX_RESOURCES
    {
        return Err(AcquisitionCapacityError::InvalidRequest(
            "objective, positive bounded horizon/budget, and bounded demand/resource sets are required".into(),
        ));
    }
    let mut campaigns = BTreeSet::new();
    for demand in &request.demands {
        if !safe_text(&demand.campaign_id)
            || !campaigns.insert(demand.campaign_id.clone())
            || demand.priority_milli == 0
            || demand.fairness_weight_milli == 0
            || demand.priority_milli > 1_000
            || demand.fairness_weight_milli > 1_000
            || demand.minimum_units > demand.target_units
            || demand.target_units > demand.maximum_units
            || demand.maximum_units == 0
            || demand.cost_per_unit == 0
            || demand.latest_tick < request.current_tick
            || demand.latest_tick > request.current_tick.saturating_add(request.horizon_ticks)
        {
            return Err(AcquisitionCapacityError::InvalidRequest(
                "demands require unique approved-bound identities, ordered unit bounds, cost, and horizon deadlines".into(),
            ));
        }
    }
    let mut resources = BTreeSet::new();
    for resource in &request.resources {
        if !safe_text(&resource.resource_id)
            || !resources.insert(resource.resource_id.clone())
            || resource.capacity_units == 0
            || resource.operator_capacity_units == 0
            || resource.maintenance_reserved_units > resource.capacity_units
            || resource.budget_units == 0
        {
            return Err(AcquisitionCapacityError::InvalidRequest(
                "resources require unique enabled capacity, operator, maintenance, and budget bounds".into(),
            ));
        }
    }
    Ok(())
}

impl AcquisitionCapacityPlan {
    pub fn validate(&self) -> Result<(), AcquisitionCapacityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !canonical(&self.resource_order)
            || !canonical(&self.campaign_order)
            || !unique(&self.resource_order)
            || !unique(&self.campaign_order)
            || self.allocations.iter().any(|allocation| {
                !safe_text(&allocation.campaign_id)
                    || !safe_text(&allocation.resource_id)
                    || allocation.allocated_units == 0
                    || allocation.cost_units == 0
                    || allocation.completion_tick == 0
            })
            || self.deferrals.iter().any(|deferral| {
                !safe_text(&deferral.campaign_id)
                    || !canonical(&deferral.reason_order)
                    || deferral.deferred_units == 0
            })
            || !canonical(&self.negative_evidence_order)
            || self.minimum_fairness_milli > 1_000
            || self.weighted_jain_milli > 1_000
            || self.completion_tick == 0
            || self.dispatch_permitted
            || !valid_hash(&self.digest)
        {
            return Err(AcquisitionCapacityError::InvalidOutput(
                "capacity plan identity, deterministic partitions, safety, fairness, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| AcquisitionCapacityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AcquisitionCapacityError::InvalidOutput(
                "capacity plan digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapacityState {
    allocated: BTreeMap<String, u32>,
    resource_remaining: BTreeMap<String, u32>,
    resource_budget_left: BTreeMap<String, u64>,
    assignment_counts: BTreeMap<(String, String), u32>,
    budget_left: u64,
}

fn capacity_state_score(state: &CapacityState, demands: &[AcquisitionDemand]) -> u128 {
    let mut minimum_covered = 0_u128;
    let mut target_covered_weighted = 0_u128;
    let mut priority_covered = 0_u128;
    let mut minimum_fairness = 1_000_u128;
    let mut approved_count = 0_u128;
    for demand in demands.iter().filter(|demand| demand.approved) {
        let units = u128::from(*state.allocated.get(&demand.campaign_id).unwrap_or(&0));
        let minimum = u128::from(demand.minimum_units);
        let target = u128::from(demand.target_units.max(demand.minimum_units));
        minimum_covered = minimum_covered.saturating_add(units.min(minimum));
        target_covered_weighted = target_covered_weighted.saturating_add(
            u128::from(demand.fairness_weight_milli)
                .saturating_mul(units.min(target))
                .saturating_mul(1_000)
                .checked_div(target.max(1))
                .unwrap_or(0),
        );
        priority_covered = priority_covered
            .saturating_add(u128::from(demand.priority_milli).saturating_mul(units.min(target)));
        if target > 0 {
            minimum_fairness = minimum_fairness.min(
                units
                    .min(target)
                    .saturating_mul(1_000)
                    .checked_div(target)
                    .unwrap_or(0),
            );
        }
        approved_count += 1;
    }
    if approved_count == 0 {
        minimum_fairness = 0;
    }
    minimum_covered
        .saturating_mul(1_000_000_000_000_000)
        .saturating_add(minimum_fairness.saturating_mul(1_000_000_000_000))
        .saturating_add(target_covered_weighted.saturating_mul(1_000_000))
        .saturating_add(priority_covered.saturating_mul(1_000))
        .saturating_add((state.assignment_counts.len() as u128).saturating_mul(10))
        .saturating_sub(state.budget_left as u128)
}

fn capacity_candidates(
    state: &CapacityState,
    demands: &[AcquisitionDemand],
    resources: &[AcquisitionCapacityResource],
    current_tick: u64,
) -> Vec<(usize, usize, u128)> {
    let mut candidates = Vec::new();
    for (demand_index, demand) in demands.iter().enumerate() {
        if !demand.approved || demand.latest_tick < current_tick {
            continue;
        }
        let current = *state.allocated.get(&demand.campaign_id).unwrap_or(&0);
        let limit = demand.target_units.max(demand.minimum_units);
        if current >= limit {
            continue;
        }
        for (resource_index, resource) in resources.iter().enumerate() {
            let remaining = *state
                .resource_remaining
                .get(&resource.resource_id)
                .unwrap_or(&0);
            let cost = u64::from(demand.cost_per_unit);
            if !resource.enabled
                || remaining == 0
                || cost > state.budget_left
                || cost
                    > *state
                        .resource_budget_left
                        .get(&resource.resource_id)
                        .unwrap_or(&0)
            {
                continue;
            }
            let deficit = u128::from(limit.saturating_sub(current));
            let urgency = u128::from(
                MAX_HORIZON
                    .checked_div(request_deadline_slack(demand.latest_tick, current_tick).max(1))
                    .unwrap_or(1)
                    .max(1),
            );
            let utility = u128::from(demand.priority_milli)
                .saturating_mul(u128::from(demand.fairness_weight_milli))
                .saturating_mul(deficit)
                .saturating_mul(1_000_000)
                .saturating_mul(urgency)
                .checked_div(u128::from(current).saturating_add(1))
                .unwrap_or(0)
                .checked_div(u128::from(demand.cost_per_unit).max(1))
                .unwrap_or(0);
            candidates.push((demand_index, resource_index, utility));
        }
    }
    candidates.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    candidates.truncate(CAPACITY_CANDIDATE_WIDTH);
    candidates
}

fn request_deadline_slack(latest_tick: u64, current_tick: u64) -> u64 {
    latest_tick.saturating_sub(current_tick)
}

fn apply_capacity_candidate(
    state: &CapacityState,
    demand: &AcquisitionDemand,
    resource: &AcquisitionCapacityResource,
) -> CapacityState {
    let mut next = state.clone();
    *next
        .allocated
        .entry(demand.campaign_id.clone())
        .or_default() += 1;
    *next
        .resource_remaining
        .entry(resource.resource_id.clone())
        .or_default() -= 1;
    let cost = u64::from(demand.cost_per_unit);
    *next
        .resource_budget_left
        .entry(resource.resource_id.clone())
        .or_default() -= cost;
    next.budget_left = next.budget_left.saturating_sub(cost);
    *next
        .assignment_counts
        .entry((demand.campaign_id.clone(), resource.resource_id.clone()))
        .or_default() += 1;
    next
}

/// Allocate approved acquisition demand across local resources with fairness and hard budgets.
pub fn plan_glioma_acquisition_capacity(
    request: &AcquisitionCapacityRequest,
) -> Result<AcquisitionCapacityPlan, AcquisitionCapacityError> {
    validate_request(request)?;
    let mut demands = request.demands.clone();
    demands.sort_by(|left, right| left.campaign_id.cmp(&right.campaign_id));
    let mut resources = request.resources.clone();
    resources.sort_by(|left, right| left.resource_id.cmp(&right.resource_id));
    let mut allocated = demands
        .iter()
        .map(|demand| (demand.campaign_id.clone(), 0u32))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut allocations = Vec::new();
    let budget_left = request.total_budget_units;
    let mut negative = BTreeSet::new();
    for resource in &resources {
        if !resource.enabled {
            negative.insert(format!("{}:disabled", resource.resource_id));
        }
    }
    let resource_remaining = resources
        .iter()
        .map(|resource| {
            (
                resource.resource_id.clone(),
                resource
                    .capacity_units
                    .saturating_sub(resource.maintenance_reserved_units)
                    .min(resource.operator_capacity_units),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for resource in &resources {
        if resource.enabled && resource.capacity_units <= resource.maintenance_reserved_units {
            negative.insert(format!("{}:maintenance-reserved", resource.resource_id));
        }
    }
    let resource_budget_left = resources
        .iter()
        .map(|resource| (resource.resource_id.clone(), resource.budget_units))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut states = vec![CapacityState {
        allocated: allocated.clone(),
        resource_remaining: resource_remaining.clone(),
        resource_budget_left: resource_budget_left.clone(),
        assignment_counts: BTreeMap::new(),
        budget_left,
    }];
    let mut steps = 0_usize;
    let mut search_bounded = false;
    loop {
        if steps >= MAX_BEAM_STEPS {
            search_bounded = true;
            break;
        }
        let mut expanded = states.clone();
        let mut added = false;
        for state in &states {
            for (demand_index, resource_index, _) in
                capacity_candidates(state, &demands, &resources, request.current_tick)
            {
                let next = apply_capacity_candidate(
                    state,
                    &demands[demand_index],
                    &resources[resource_index],
                );
                expanded.push(next);
                added = true;
            }
        }
        if !added {
            break;
        }
        expanded.sort_by(|left, right| {
            capacity_state_score(right, &demands)
                .cmp(&capacity_state_score(left, &demands))
                .then_with(|| left.budget_left.cmp(&right.budget_left))
                .then_with(|| left.assignment_counts.cmp(&right.assignment_counts))
        });
        expanded.dedup_by(|left, right| left.assignment_counts == right.assignment_counts);
        expanded.truncate(CAPACITY_BEAM_WIDTH);
        states = expanded;
        steps += 1;
    }
    if search_bounded {
        negative.insert("allocation-search-bounded-at-max-steps".into());
    }
    let chosen = states
        .into_iter()
        .max_by(|left, right| {
            capacity_state_score(left, &demands)
                .cmp(&capacity_state_score(right, &demands))
                .then_with(|| right.budget_left.cmp(&left.budget_left))
                .then_with(|| right.assignment_counts.cmp(&left.assignment_counts))
        })
        .expect("capacity beam retains an empty state");
    allocated = chosen.allocated;
    for ((campaign_id, resource_id), units) in chosen.assignment_counts {
        let demand = demands
            .iter()
            .find(|demand| demand.campaign_id == campaign_id)
            .expect("selected demand");
        allocations.push(AcquisitionAllocation {
            campaign_id,
            resource_id,
            allocated_units: units,
            cost_units: u64::from(demand.cost_per_unit).saturating_mul(u64::from(units)),
            completion_tick: request.current_tick + request.horizon_ticks,
        });
    }
    allocations.sort_by(|left, right| {
        left.campaign_id
            .cmp(&right.campaign_id)
            .then_with(|| left.resource_id.cmp(&right.resource_id))
    });
    let mut deferrals = Vec::new();
    let mut ratios = Vec::new();
    let mut weighted_sum = 0u128;
    let mut weighted_square = 0u128;
    let mut total_weight = 0u128;
    for demand in &demands {
        let units = *allocated.get(&demand.campaign_id).unwrap_or(&0);
        if !demand.approved {
            negative.insert(format!("{}:approval-required", demand.campaign_id));
        }
        if units < demand.target_units {
            let reason = if units < demand.minimum_units {
                "capacity-or-budget-shortfall"
            } else {
                "target-deferred"
            };
            deferrals.push(AcquisitionDeferral {
                campaign_id: demand.campaign_id.clone(),
                allocated_units: units,
                deferred_units: demand.target_units - units,
                reason_order: vec![reason.into()],
            });
        }
        let ratio = ((units.min(demand.target_units) as u64) * 1_000
            / demand.target_units.max(1) as u64)
            .min(1_000) as u16;
        ratios.push(ratio);
        let weight = demand.fairness_weight_milli as u128;
        weighted_sum += weight * ratio as u128;
        weighted_square += weight * (ratio as u128) * (ratio as u128);
        total_weight += weight;
    }
    deferrals.sort_by(|left, right| left.campaign_id.cmp(&right.campaign_id));
    let minimum_fairness = ratios.into_iter().min().unwrap_or(0);
    let weighted_jain = if weighted_square == 0 {
        0
    } else {
        ((weighted_sum * weighted_sum * 1_000) / (total_weight * weighted_square)).min(1_000) as u16
    };
    let total_allocated_units = allocations
        .iter()
        .map(|allocation| allocation.allocated_units as u64)
        .sum();
    let total_cost_units = allocations
        .iter()
        .map(|allocation| allocation.cost_units)
        .sum();
    let completion_tick = allocations
        .iter()
        .map(|allocation| allocation.completion_tick)
        .max()
        .unwrap_or(request.current_tick);
    let mut plan = AcquisitionCapacityPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        resource_order: resources
            .iter()
            .map(|resource| resource.resource_id.clone())
            .collect(),
        campaign_order: demands
            .iter()
            .map(|demand| demand.campaign_id.clone())
            .collect(),
        allocations,
        deferrals,
        total_allocated_units,
        total_cost_units,
        minimum_fairness_milli: minimum_fairness,
        weighted_jain_milli: weighted_jain,
        completion_tick,
        dispatch_permitted: false,
        negative_evidence_order: negative.into_iter().collect(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-capacity-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_body(&plan))
        .map_err(|error| AcquisitionCapacityError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(id: &str, priority: u16, target: u32, approved: bool) -> AcquisitionDemand {
        AcquisitionDemand {
            campaign_id: id.into(),
            priority_milli: priority,
            fairness_weight_milli: 1_000,
            minimum_units: target / 2,
            target_units: target,
            maximum_units: target + 2,
            cost_per_unit: 1,
            latest_tick: 11,
            approved,
        }
    }

    fn request(demands: Vec<AcquisitionDemand>) -> AcquisitionCapacityRequest {
        AcquisitionCapacityRequest {
            objective: "allocate glioma organoid acquisition capacity".into(),
            current_tick: 1,
            horizon_ticks: 10,
            total_budget_units: 8,
            demands,
            resources: vec![AcquisitionCapacityResource {
                resource_id: "scope-1".into(),
                capacity_units: 8,
                operator_capacity_units: 8,
                maintenance_reserved_units: 1,
                budget_units: 8,
                enabled: true,
            }],
        }
    }

    #[test]
    fn allocator_meets_minimums_before_priority_weighted_targets() {
        let plan = plan_glioma_acquisition_capacity(&request(vec![
            demand("campaign-a", 1_000, 6, true),
            demand("campaign-b", 100, 6, true),
        ]))
        .unwrap();
        assert_eq!(plan.total_allocated_units, 7);
        assert!(plan.minimum_fairness_milli >= 500);
        assert!(plan
            .allocations
            .iter()
            .any(|allocation| allocation.campaign_id == "campaign-b"));
    }

    #[test]
    fn unapproved_demand_is_deferred_without_consuming_capacity() {
        let plan = plan_glioma_acquisition_capacity(&request(vec![
            demand("approved", 1_000, 2, true),
            demand("pending", 1_000, 2, false),
        ]))
        .unwrap();
        assert!(plan
            .negative_evidence_order
            .iter()
            .any(|value| value == "pending:approval-required"));
        assert!(plan
            .deferrals
            .iter()
            .any(|deferral| deferral.campaign_id == "pending"));
        assert!(plan
            .allocations
            .iter()
            .all(|allocation| allocation.campaign_id != "pending"));
    }

    #[test]
    fn maintenance_and_budget_bounds_are_respected() {
        let mut request = request(vec![demand("campaign-a", 1_000, 20, true)]);
        request.total_budget_units = 2;
        request.resources[0].maintenance_reserved_units = 7;
        let plan = plan_glioma_acquisition_capacity(&request).unwrap();
        assert!(plan.total_allocated_units <= 1);
        assert!(plan.deferrals[0].deferred_units > 0);
    }

    #[test]
    fn tampered_bounds_fail_closed() {
        let mut request = request(vec![demand("campaign-a", 1_000, 2, true)]);
        request.demands[0].target_units = 0;
        assert!(matches!(
            plan_glioma_acquisition_capacity(&request),
            Err(AcquisitionCapacityError::InvalidRequest(_))
        ));
    }
}
