//! Continual capacity and quorum planning for federated preclinical glioma workflows.
//!
//! The planner forecasts bounded site-local capacity and privacy headroom, then proposes a
//! schedule only when quorum, latency, and resource gates are simultaneously visible. It never
//! creates a job, contacts a site, or treats predicted capacity as an observation.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaFederationCapacityPlan1@1";
pub const MAX_OBSERVATIONS: usize = 2_048;
pub const MAX_HORIZON: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacitySiteWindow {
    pub site_id: String,
    pub institution_group: String,
    pub epoch: u64,
    pub capacity_units: u64,
    pub committed_capacity_units: u64,
    pub privacy_budget_milli: u64,
    pub expected_latency_minutes: u64,
    pub active_workflow_count: usize,
    pub availability_milli: u16,
    pub eligible_for_quorum: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationCapacityRequest {
    pub objective: String,
    pub required_quorum_sites: usize,
    pub planning_horizon: usize,
    pub demand_units_per_window: u64,
    pub maximum_schedule_units: u64,
    pub minimum_privacy_budget_milli: u64,
    pub maximum_latency_minutes: u64,
    pub maximum_site_commitment_fraction_milli: u16,
    pub observations: Vec<CapacitySiteWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityForecastWindow {
    pub horizon_index: usize,
    pub forecast_site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub at_risk_site_order: Vec<String>,
    pub projected_capacity_units: u64,
    pub projected_committed_units: u64,
    pub schedulable_units: u64,
    pub projected_privacy_budget_milli: u64,
    pub projected_latency_minutes: u64,
    pub quorum_satisfied: bool,
    pub demand_met: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityPlanDisposition {
    Scheduled,
    Constrained,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationCapacityPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub forecast: Vec<CapacityForecastWindow>,
    pub recommended_horizon_index: Option<usize>,
    pub recommended_site_order: Vec<String>,
    pub recommended_schedule_units: u64,
    pub disposition: CapacityPlanDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederationCapacityError {
    #[error("federation capacity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federation capacity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federation capacity digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value)) && values.windows(2).all(|pair| pair[0] != pair[1])
}

fn output_body(plan: &FederationCapacityPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "forecast": plan.forecast,
        "recommended_horizon_index": plan.recommended_horizon_index,
        "recommended_site_order": plan.recommended_site_order,
        "recommended_schedule_units": plan.recommended_schedule_units,
        "disposition": plan.disposition,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
    })
}

impl FederationCapacityPlan {
    pub fn validate(&self) -> Result<(), FederationCapacityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !canonical(&self.recommended_site_order)
            || self.forecast.is_empty()
            || self.forecast.iter().any(|window| {
                !canonical(&window.forecast_site_order)
                    || !canonical(&window.selected_site_order)
                    || !canonical(&window.at_risk_site_order)
                    || (window.demand_met && !window.quorum_satisfied)
            })
        {
            return Err(FederationCapacityError::InvalidOutput(
                "capacity identity, forecast partitions, or quorum/schedule invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederationCapacityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederationCapacityError::InvalidOutput(
                "capacity plan digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &FederationCapacityRequest) -> Result<(), FederationCapacityError> {
    if !safe_text(&request.objective)
        || request.required_quorum_sites == 0
        || request.planning_horizon == 0
        || request.planning_horizon > MAX_HORIZON
        || request.demand_units_per_window == 0
        || request.maximum_schedule_units == 0
        || request.minimum_privacy_budget_milli == 0
        || request.maximum_latency_minutes == 0
        || request.maximum_site_commitment_fraction_milli == 0
        || request.maximum_site_commitment_fraction_milli > 1_000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(FederationCapacityError::InvalidRequest(
            "objective, quorum/horizon, demand, privacy, latency, commitment, and bounded site history are required".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for observation in &request.observations {
        if !safe_text(&observation.site_id)
            || !safe_text(&observation.institution_group)
            || !keys.insert((observation.site_id.clone(), observation.epoch))
            || observation.capacity_units == 0
            || observation.committed_capacity_units > observation.capacity_units.saturating_mul(100)
            || observation.privacy_budget_milli == 0
            || observation.expected_latency_minutes == 0
            || observation.availability_milli == 0
            || observation.availability_milli > 1_000
            || !observation.local_only
            || observation.contains_human_data
            || observation.contains_direct_identifiers
        {
            return Err(FederationCapacityError::InvalidRequest(format!(
                "site {} epoch {} has invalid capacity, privacy, locality, or duplicate fields",
                observation.site_id, observation.epoch
            )));
        }
    }
    Ok(())
}

pub fn plan_federation_capacity(
    request: &FederationCapacityRequest,
) -> Result<FederationCapacityPlan, FederationCapacityError> {
    validate_request(request)?;
    let mut by_site: BTreeMap<String, Vec<&CapacitySiteWindow>> = BTreeMap::new();
    for observation in &request.observations {
        by_site
            .entry(observation.site_id.clone())
            .or_default()
            .push(observation);
    }
    for history in by_site.values_mut() {
        history.sort_by(|left, right| left.epoch.cmp(&right.epoch));
    }
    let mut forecast = Vec::new();
    let mut negative = Vec::new();
    let mut uncertainty = Vec::new();
    for horizon_index in 0..request.planning_horizon {
        let mut eligible = Vec::new();
        let mut at_risk = Vec::new();
        for (site_id, history) in &by_site {
            let latest = history.last().expect("validated history is non-empty");
            let trend = history
                .windows(2)
                .last()
                .map(|pair| {
                    pair[1]
                        .capacity_units
                        .saturating_sub(pair[0].capacity_units)
                        .min(1_000) as i64
                        - pair[0]
                            .capacity_units
                            .saturating_sub(pair[1].capacity_units)
                            .min(1_000) as i64
                })
                .unwrap_or(0);
            let capacity = if trend >= 0 {
                latest
                    .capacity_units
                    .saturating_add((trend as u64).saturating_mul(horizon_index as u64))
            } else {
                latest
                    .capacity_units
                    .saturating_sub(trend.unsigned_abs().saturating_mul(horizon_index as u64))
            };
            let committed = latest.committed_capacity_units;
            let available = capacity.saturating_sub(committed);
            let usable = available
                .saturating_mul(u64::from(latest.availability_milli))
                .checked_div(1_000)
                .unwrap_or(0);
            let commitment_fraction = latest
                .committed_capacity_units
                .saturating_mul(1_000)
                .checked_div(capacity.max(1))
                .unwrap_or(1_000);
            let qualifies = latest.eligible_for_quorum
                && latest.privacy_budget_milli >= request.minimum_privacy_budget_milli
                && latest.expected_latency_minutes <= request.maximum_latency_minutes
                && commitment_fraction <= u64::from(request.maximum_site_commitment_fraction_milli)
                && usable > 0;
            if qualifies {
                eligible.push((site_id.clone(), latest, usable));
            } else {
                at_risk.push(site_id.clone());
            }
        }
        eligible.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then_with(|| {
                    left.1
                        .expected_latency_minutes
                        .cmp(&right.1.expected_latency_minutes)
                })
                .then_with(|| left.0.cmp(&right.0))
        });
        let quorum_satisfied = eligible.len() >= request.required_quorum_sites;
        let selected = if quorum_satisfied {
            eligible
                .iter()
                .take(request.required_quorum_sites.max(1))
                .map(|(site_id, _, _)| site_id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let projected_capacity = eligible.iter().map(|(_, _, usable)| *usable).sum::<u64>();
        let projected_committed = eligible
            .iter()
            .map(|(_, latest, _)| latest.committed_capacity_units)
            .sum::<u64>();
        let schedulable = if quorum_satisfied {
            projected_capacity.min(request.maximum_schedule_units)
        } else {
            0
        };
        let projected_privacy = eligible
            .iter()
            .map(|(_, latest, _)| latest.privacy_budget_milli)
            .min()
            .unwrap_or(0);
        let projected_latency = eligible
            .iter()
            .map(|(_, latest, _)| latest.expected_latency_minutes)
            .max()
            .unwrap_or(0);
        let demand_met = quorum_satisfied && schedulable >= request.demand_units_per_window;
        if !quorum_satisfied {
            negative.push(format!(
                "horizon {horizon_index} cannot satisfy the required federated quorum"
            ));
        } else if !demand_met {
            negative.push(format!(
                "horizon {horizon_index} cannot meet the declared capacity demand"
            ));
        }
        if !at_risk.is_empty() {
            uncertainty.push(format!(
                "horizon {horizon_index} has at-risk or unavailable sites"
            ));
        }
        let mut forecast_site_order = eligible
            .iter()
            .map(|(site_id, _, _)| site_id.clone())
            .collect::<Vec<_>>();
        forecast_site_order.sort();
        at_risk.sort();
        let mut selected_site_order = selected;
        selected_site_order.sort();
        forecast.push(CapacityForecastWindow {
            horizon_index,
            forecast_site_order,
            selected_site_order,
            at_risk_site_order: at_risk,
            projected_capacity_units: projected_capacity,
            projected_committed_units: projected_committed,
            schedulable_units: schedulable,
            projected_privacy_budget_milli: projected_privacy,
            projected_latency_minutes: projected_latency,
            quorum_satisfied,
            demand_met,
        });
    }
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let recommended = forecast
        .iter()
        .find(|window| window.demand_met)
        .or_else(|| forecast.iter().find(|window| window.quorum_satisfied));
    let (recommended_horizon_index, recommended_site_order, recommended_schedule_units) =
        recommended
            .map(|window| {
                (
                    Some(window.horizon_index),
                    window.selected_site_order.clone(),
                    window.schedulable_units,
                )
            })
            .unwrap_or((None, Vec::new(), 0));
    let disposition = if recommended.is_none() {
        CapacityPlanDisposition::Blocked
    } else if recommended.unwrap().demand_met {
        CapacityPlanDisposition::Scheduled
    } else if recommended.unwrap().quorum_satisfied {
        CapacityPlanDisposition::Constrained
    } else {
        CapacityPlanDisposition::Unresolved
    };
    let mut plan = FederationCapacityPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        forecast,
        recommended_horizon_index,
        recommended_site_order,
        recommended_schedule_units,
        disposition,
        negative_evidence: negative,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-capacity-plan"),
    };
    plan.digest = ContentHash::of_value(&output_body(&plan))
        .map_err(|error| FederationCapacityError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(site_id: &str, epoch: u64, capacity: u64) -> CapacitySiteWindow {
        CapacitySiteWindow {
            site_id: site_id.into(),
            institution_group: format!("group-{site_id}"),
            epoch,
            capacity_units: capacity,
            committed_capacity_units: 10,
            privacy_budget_milli: 800,
            expected_latency_minutes: 20,
            active_workflow_count: 2,
            availability_milli: 900,
            eligible_for_quorum: true,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            source_digest: ContentHash::of_bytes(site_id.as_bytes()),
        }
    }

    fn request(observations: Vec<CapacitySiteWindow>) -> FederationCapacityRequest {
        FederationCapacityRequest {
            objective: "schedule a glioma benchmark federation".into(),
            required_quorum_sites: 2,
            planning_horizon: 3,
            demand_units_per_window: 100,
            maximum_schedule_units: 200,
            minimum_privacy_budget_milli: 500,
            maximum_latency_minutes: 60,
            maximum_site_commitment_fraction_milli: 800,
            observations,
        }
    }

    #[test]
    fn capacity_and_quorum_produce_schedule() {
        let plan = plan_federation_capacity(&request(vec![
            observation("site-a", 1, 100),
            observation("site-b", 1, 100),
        ]))
        .unwrap();
        assert_eq!(plan.disposition, CapacityPlanDisposition::Scheduled);
        assert!(plan.recommended_schedule_units >= 100);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn insufficient_quorum_blocks_without_dispatch() {
        let mut only = observation("site-a", 1, 100);
        only.eligible_for_quorum = false;
        let plan = plan_federation_capacity(&request(vec![only])).unwrap();
        assert_eq!(plan.disposition, CapacityPlanDisposition::Blocked);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("quorum")));
    }

    #[test]
    fn privacy_and_latency_risk_are_visible() {
        let mut risky = observation("site-b", 1, 100);
        risky.privacy_budget_milli = 100;
        risky.expected_latency_minutes = 100;
        let plan =
            plan_federation_capacity(&request(vec![observation("site-a", 1, 100), risky])).unwrap();
        assert!(plan
            .forecast
            .iter()
            .any(|window| window.at_risk_site_order.contains(&"site-b".into())));
        assert!(plan.uncertainty.iter().any(|item| item.contains("at-risk")));
    }

    #[test]
    fn deterministic_forecast_repeats() {
        let input = request(vec![
            observation("site-a", 1, 100),
            observation("site-b", 1, 100),
        ]);
        assert_eq!(
            plan_federation_capacity(&input).unwrap(),
            plan_federation_capacity(&input).unwrap()
        );
    }

    #[test]
    fn protected_capacity_history_is_rejected() {
        let mut protected = observation("site-a", 1, 100);
        protected.contains_human_data = true;
        assert!(plan_federation_capacity(&request(vec![protected])).is_err());
    }
}
