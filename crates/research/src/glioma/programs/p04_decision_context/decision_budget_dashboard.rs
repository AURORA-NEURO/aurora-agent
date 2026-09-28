//! Prospective research-budget accounting and rebalancing for autonomous glioma programs.
//!
//! This feature is the control surface between a decision-context plan and local execution. It
//! reconciles actual event consumption with in-flight quotations and prospective branch
//! forecasts, then emits deterministic alerts and approval-bound reallocation proposals. It does
//! not authorize spending or execute a study; a hard cap is never treated as advisory.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionBudgetSnapshot1@1";
pub const MAX_ITEMS: usize = 1_024;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetResource {
    Assay,
    Compute,
    Time,
    Review,
}

impl BudgetResource {
    fn all() -> [Self; 4] {
        [Self::Assay, Self::Compute, Self::Time, Self::Review]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetBucket {
    pub resource: BudgetResource,
    pub approved_units: u64,
    pub alert_threshold_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetEventState {
    Completed,
    Running,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetEvent {
    pub event_id: String,
    pub branch_id: String,
    pub resource: BudgetResource,
    pub consumed_units: u64,
    pub quoted_units: u64,
    pub state: BudgetEventState,
    pub event_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetForecast {
    pub branch_id: String,
    pub resource: BudgetResource,
    pub additional_units: u64,
    pub confidence_milli: u16,
    pub required: bool,
    pub forecast_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetBranchPlan {
    pub branch_id: String,
    pub resource: BudgetResource,
    pub planned_units: u64,
    pub priority_milli: u16,
    pub mandatory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBudgetRequest {
    pub campaign_id: String,
    pub objective: String,
    pub budgets: Vec<BudgetBucket>,
    pub events: Vec<BudgetEvent>,
    pub forecasts: Vec<BudgetForecast>,
    pub branch_plans: Vec<BudgetBranchPlan>,
    pub current_tick: u64,
    pub forecast_horizon_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetAlertSeverity {
    Warning,
    ApprovalRequired,
    HardStop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub resource: BudgetResource,
    pub severity: BudgetAlertSeverity,
    pub consumed_units: u64,
    pub projected_units: u64,
    pub approved_units: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetReallocationProposal {
    pub proposal_id: String,
    pub resource: BudgetResource,
    pub from_branch_id: String,
    pub to_branch_id: String,
    pub units: u64,
    pub from_priority_milli: u16,
    pub to_priority_milli: u16,
    pub approval_required: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBudgetBucketSnapshot {
    pub resource: BudgetResource,
    pub approved_units: u64,
    pub consumed_units: u64,
    pub reserved_units: u64,
    pub projected_units: u64,
    pub remaining_units: u64,
    pub utilization_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionBudgetDisposition {
    Balanced,
    Warning,
    ApprovalRequired,
    HardStop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionBudgetSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub campaign_id: String,
    pub objective: String,
    pub current_tick: u64,
    pub forecast_horizon_ticks: u64,
    pub buckets: Vec<DecisionBudgetBucketSnapshot>,
    pub events_applied_order: Vec<String>,
    pub forecast_order: Vec<String>,
    pub alerts: Vec<BudgetAlert>,
    pub reallocation_proposals: Vec<BudgetReallocationProposal>,
    pub hard_stop_branch_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionBudgetDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionBudgetError {
    #[error("decision-budget request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-budget snapshot is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-budget digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_text(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= MAX_ITEMS
        && values
            .iter()
            .all(|value| safe_text(value) && seen.insert(value.clone()))
}

fn digest_input(snapshot: &DecisionBudgetSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "campaign_id": snapshot.campaign_id,
        "objective": snapshot.objective,
        "current_tick": snapshot.current_tick,
        "forecast_horizon_ticks": snapshot.forecast_horizon_ticks,
        "buckets": snapshot.buckets,
        "events_applied_order": snapshot.events_applied_order,
        "forecast_order": snapshot.forecast_order,
        "alerts": snapshot.alerts,
        "reallocation_proposals": snapshot.reallocation_proposals,
        "hard_stop_branch_order": snapshot.hard_stop_branch_order,
        "negative_evidence_order": snapshot.negative_evidence_order,
        "uncertainty_order": snapshot.uncertainty_order,
        "disposition": snapshot.disposition,
    })
}

fn validate_request(request: &DecisionBudgetRequest) -> Result<(), DecisionBudgetError> {
    if !safe_text(&request.campaign_id)
        || !safe_text(&request.objective)
        || request.budgets.len() != BudgetResource::all().len()
        || request.events.len() > MAX_ITEMS
        || request.forecasts.len() > MAX_ITEMS
        || request.branch_plans.len() > MAX_ITEMS
        || request.current_tick == 0
        || request.forecast_horizon_ticks == 0
    {
        return Err(DecisionBudgetError::InvalidRequest(
            "bounded campaign identity, complete four-resource budget, event/forecast limits, and positive time bounds are required".into(),
        ));
    }
    let mut budget_resources = BTreeSet::new();
    for budget in &request.budgets {
        if budget.approved_units == 0
            || budget.alert_threshold_milli < 500
            || budget.alert_threshold_milli > 1_000
            || !budget_resources.insert(budget.resource)
        {
            return Err(DecisionBudgetError::InvalidRequest(
                "each resource needs one positive approved cap and a 500..=1000 alert threshold"
                    .into(),
            ));
        }
    }
    if budget_resources.len() != BudgetResource::all().len() {
        return Err(DecisionBudgetError::InvalidRequest(
            "assay, compute, time, and review budgets are all required".into(),
        ));
    }
    let mut event_ids = BTreeSet::new();
    for event in &request.events {
        if !safe_text(&event.event_id)
            || !safe_text(&event.branch_id)
            || !event_ids.insert(event.event_id.clone())
            || event.consumed_units > event.quoted_units
            || event.quoted_units == 0
            || event.event_tick == 0
            || event.event_tick > request.current_tick
        {
            return Err(DecisionBudgetError::InvalidRequest(
                "events require unique bounded identities, nonnegative reconciled consumption, and nonfuture ticks".into(),
            ));
        }
    }
    let mut forecast_keys = BTreeSet::new();
    for forecast in &request.forecasts {
        if !safe_text(&forecast.branch_id)
            || forecast.additional_units == 0
            || forecast.confidence_milli > 1_000
            || forecast.forecast_tick == 0
            || forecast.forecast_tick > request.current_tick
            || !forecast_keys.insert((forecast.branch_id.clone(), forecast.resource))
        {
            return Err(DecisionBudgetError::InvalidRequest(
                "forecasts require unique branch/resource rows, positive units, bounded confidence, and current ticks".into(),
            ));
        }
    }
    let mut plan_keys = BTreeSet::new();
    for plan in &request.branch_plans {
        if !safe_text(&plan.branch_id)
            || plan.planned_units == 0
            || plan.priority_milli > 1_000
            || !plan_keys.insert((plan.branch_id.clone(), plan.resource))
        {
            return Err(DecisionBudgetError::InvalidRequest(
                "branch plans require unique branch/resource rows, positive units, and bounded priority".into(),
            ));
        }
    }
    Ok(())
}

fn bucket_for(budgets: &[BudgetBucket], resource: BudgetResource) -> &BudgetBucket {
    budgets
        .iter()
        .find(|budget| budget.resource == resource)
        .expect("validated resource budget exists")
}

fn utilization_milli(projected: u64, approved: u64) -> u16 {
    projected
        .saturating_mul(1_000)
        .checked_div(approved)
        .unwrap_or(1_000)
        .min(1_000) as u16
}

impl DecisionBudgetSnapshot {
    pub fn validate(&self) -> Result<(), DecisionBudgetError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.campaign_id)
            || !safe_text(&self.objective)
            || self.current_tick == 0
            || self.forecast_horizon_ticks == 0
            || self.buckets.len() != BudgetResource::all().len()
            || self
                .buckets
                .windows(2)
                .any(|pair| pair[0].resource >= pair[1].resource)
            || !unique_text(&self.events_applied_order)
            || !canonical(&self.events_applied_order)
            || !unique_text(&self.forecast_order)
            || !canonical(&self.forecast_order)
            || !unique_text(&self.hard_stop_branch_order)
            || !canonical(&self.hard_stop_branch_order)
            || !unique_text(&self.negative_evidence_order)
            || !canonical(&self.negative_evidence_order)
            || !unique_text(&self.uncertainty_order)
            || !canonical(&self.uncertainty_order)
            || !valid_hash(&self.digest)
        {
            return Err(DecisionBudgetError::InvalidOutput(
                "budget snapshot identity, resource ordering, partitions, or digest is invalid"
                    .into(),
            ));
        }
        for bucket in &self.buckets {
            if bucket.approved_units == 0
                || bucket.projected_units < bucket.consumed_units
                || bucket.remaining_units
                    != bucket.approved_units.saturating_sub(bucket.projected_units)
                || bucket.utilization_milli > 1_000
            {
                return Err(DecisionBudgetError::InvalidOutput(
                    "budget bucket reconciliation or utilization bounds are invalid".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionBudgetError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionBudgetError::InvalidOutput(
                "budget snapshot digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Reconcile a prospective glioma campaign budget and emit approval-bound control signals.
pub fn compile_glioma_decision_budget_snapshot(
    request: &DecisionBudgetRequest,
) -> Result<DecisionBudgetSnapshot, DecisionBudgetError> {
    validate_request(request)?;
    let mut actual = BTreeMap::<BudgetResource, u64>::new();
    let mut reserved = BTreeMap::<BudgetResource, u64>::new();
    let mut events_applied_order = Vec::new();
    for event in &request.events {
        *actual.entry(event.resource).or_default() = actual
            .get(&event.resource)
            .copied()
            .unwrap_or(0)
            .saturating_add(event.consumed_units);
        if matches!(event.state, BudgetEventState::Running) {
            *reserved.entry(event.resource).or_default() = reserved
                .get(&event.resource)
                .copied()
                .unwrap_or(0)
                .saturating_add(event.quoted_units.saturating_sub(event.consumed_units));
        }
        events_applied_order.push(event.event_id.clone());
    }
    events_applied_order.sort();
    let mut forecast_order = request
        .forecasts
        .iter()
        .map(|forecast| format!("{}:{:?}", forecast.branch_id, forecast.resource))
        .collect::<Vec<_>>();
    forecast_order.sort();
    let mut forecasts_by_resource = BTreeMap::<BudgetResource, u64>::new();
    for forecast in &request.forecasts {
        *forecasts_by_resource.entry(forecast.resource).or_default() = forecasts_by_resource
            .get(&forecast.resource)
            .copied()
            .unwrap_or(0)
            .saturating_add(forecast.additional_units);
    }
    let mut buckets = Vec::new();
    let mut alerts = Vec::new();
    let mut negative = Vec::new();
    let mut uncertainty = Vec::new();
    for resource in BudgetResource::all() {
        let budget = bucket_for(&request.budgets, resource);
        let consumed = actual.get(&resource).copied().unwrap_or(0);
        let reserved_units = reserved.get(&resource).copied().unwrap_or(0);
        let projected = consumed
            .saturating_add(reserved_units)
            .saturating_add(forecasts_by_resource.get(&resource).copied().unwrap_or(0));
        let utilization = utilization_milli(projected, budget.approved_units);
        let remaining = budget.approved_units.saturating_sub(projected);
        buckets.push(DecisionBudgetBucketSnapshot {
            resource,
            approved_units: budget.approved_units,
            consumed_units: consumed,
            reserved_units,
            projected_units: projected,
            remaining_units: remaining,
            utilization_milli: utilization,
        });
        let severity = if consumed >= budget.approved_units {
            Some(BudgetAlertSeverity::HardStop)
        } else if projected > budget.approved_units {
            Some(BudgetAlertSeverity::ApprovalRequired)
        } else if utilization >= budget.alert_threshold_milli {
            Some(BudgetAlertSeverity::Warning)
        } else {
            None
        };
        if let Some(severity) = severity {
            let reason = match severity {
                BudgetAlertSeverity::HardStop => "actual consumption reached or exceeded hard cap",
                BudgetAlertSeverity::ApprovalRequired => {
                    "in-flight plus forecast demand exceeds hard cap"
                }
                BudgetAlertSeverity::Warning => "projected utilization crossed alert threshold",
            };
            alerts.push(BudgetAlert {
                resource,
                severity,
                consumed_units: consumed,
                projected_units: projected,
                approved_units: budget.approved_units,
                reason: reason.into(),
            });
            if matches!(severity, BudgetAlertSeverity::HardStop) {
                negative.push(format!("{:?}:hard-cap-reached", resource));
            } else if matches!(severity, BudgetAlertSeverity::ApprovalRequired) {
                negative.push(format!("{:?}:forecast-exceeds-hard-cap", resource));
            }
        }
    }
    for forecast in &request.forecasts {
        if forecast.confidence_milli < 700 {
            uncertainty.push(format!(
                "{}:{:?}:low-confidence-forecast",
                forecast.branch_id, forecast.resource
            ));
        }
    }
    let mut branch_plan = BTreeMap::<(String, BudgetResource), BudgetBranchPlan>::new();
    for plan in &request.branch_plans {
        branch_plan.insert((plan.branch_id.clone(), plan.resource), plan.clone());
    }
    let mut forecast_by_branch = BTreeMap::<(String, BudgetResource), u64>::new();
    for forecast in &request.forecasts {
        *forecast_by_branch
            .entry((forecast.branch_id.clone(), forecast.resource))
            .or_default() = forecast_by_branch
            .get(&(forecast.branch_id.clone(), forecast.resource))
            .copied()
            .unwrap_or(0)
            .saturating_add(forecast.additional_units);
    }
    let mut proposals = Vec::new();
    for resource in BudgetResource::all() {
        let projected = buckets
            .iter()
            .find(|bucket| bucket.resource == resource)
            .expect("bucket exists")
            .projected_units;
        let cap = bucket_for(&request.budgets, resource).approved_units;
        if projected <= cap {
            continue;
        }
        let deficit = projected - cap;
        let mut recipients = request
            .branch_plans
            .iter()
            .filter(|plan| plan.resource == resource)
            .filter_map(|plan| {
                let forecast = forecast_by_branch
                    .get(&(plan.branch_id.clone(), resource))
                    .copied()
                    .unwrap_or(0);
                if forecast > 0 {
                    Some((
                        plan.branch_id.clone(),
                        plan.priority_milli,
                        plan.mandatory,
                        forecast,
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut donors = request
            .branch_plans
            .iter()
            .filter(|plan| plan.resource == resource)
            .filter_map(|plan| {
                let forecast = forecast_by_branch
                    .get(&(plan.branch_id.clone(), resource))
                    .copied()
                    .unwrap_or(0);
                let available = plan.planned_units.saturating_sub(forecast);
                (available > 0).then(|| (plan.branch_id.clone(), plan.priority_milli, available))
            })
            .collect::<Vec<_>>();
        recipients.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        donors.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        if let (
            Some((to_branch, to_priority, mandatory, _)),
            Some((from_branch, from_priority, available)),
        ) = (recipients.first(), donors.first())
        {
            let units = deficit.min(*available);
            if units > 0 && from_branch != to_branch {
                proposals.push(BudgetReallocationProposal {
                    proposal_id: format!("reallocate-{:?}-{}-{}", resource, from_branch, to_branch),
                    resource,
                    from_branch_id: from_branch.clone(),
                    to_branch_id: to_branch.clone(),
                    units,
                    from_priority_milli: *from_priority,
                    to_priority_milli: *to_priority,
                    approval_required: true,
                    rationale: if *mandatory {
                        "mandatory branch demand exceeds the current cap; transfer requires explicit grant".into()
                    } else {
                        "priority-weighted branch demand exceeds the current cap; transfer requires explicit grant".into()
                    },
                });
            }
        }
    }
    let mut hard_stop_branch_order = request
        .branch_plans
        .iter()
        .filter(|plan| plan.mandatory)
        .filter(|plan| {
            alerts.iter().any(|alert| {
                matches!(
                    alert.severity,
                    BudgetAlertSeverity::HardStop | BudgetAlertSeverity::ApprovalRequired
                ) && alert.resource == plan.resource
            })
        })
        .map(|plan| plan.branch_id.clone())
        .collect::<Vec<_>>();
    hard_stop_branch_order.sort();
    hard_stop_branch_order.dedup();
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    alerts.sort_by(|left, right| {
        left.resource
            .cmp(&right.resource)
            .then(left.severity.cmp(&right.severity))
    });
    proposals.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    let disposition = if alerts
        .iter()
        .any(|alert| matches!(alert.severity, BudgetAlertSeverity::HardStop))
    {
        DecisionBudgetDisposition::HardStop
    } else if alerts
        .iter()
        .any(|alert| matches!(alert.severity, BudgetAlertSeverity::ApprovalRequired))
    {
        DecisionBudgetDisposition::ApprovalRequired
    } else if !alerts.is_empty() {
        DecisionBudgetDisposition::Warning
    } else {
        DecisionBudgetDisposition::Balanced
    };
    let mut snapshot = DecisionBudgetSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        campaign_id: request.campaign_id.clone(),
        objective: request.objective.clone(),
        current_tick: request.current_tick,
        forecast_horizon_ticks: request.forecast_horizon_ticks,
        buckets,
        events_applied_order,
        forecast_order,
        alerts,
        reallocation_proposals: proposals,
        hard_stop_branch_order,
        negative_evidence_order: negative,
        uncertainty_order: uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-budget-snapshot"),
    };
    snapshot.digest = ContentHash::of_value(&digest_input(&snapshot))
        .map_err(|error| DecisionBudgetError::Digest(error.to_string()))?;
    snapshot.validate()?;
    let _ = branch_plan;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DecisionBudgetRequest {
        DecisionBudgetRequest {
            campaign_id: "campaign-1".into(),
            objective: "allocate preclinical glioma research capacity".into(),
            budgets: BudgetResource::all()
                .into_iter()
                .map(|resource| BudgetBucket {
                    resource,
                    approved_units: 100,
                    alert_threshold_milli: 800,
                })
                .collect(),
            events: vec![],
            forecasts: vec![],
            branch_plans: vec![
                BudgetBranchPlan {
                    branch_id: "branch-a".into(),
                    resource: BudgetResource::Compute,
                    planned_units: 80,
                    priority_milli: 900,
                    mandatory: true,
                },
                BudgetBranchPlan {
                    branch_id: "branch-b".into(),
                    resource: BudgetResource::Compute,
                    planned_units: 20,
                    priority_milli: 400,
                    mandatory: false,
                },
            ],
            current_tick: 10,
            forecast_horizon_ticks: 50,
        }
    }

    #[test]
    fn reconciles_actual_and_reserved_consumption() {
        let mut request = request();
        request.events = vec![BudgetEvent {
            event_id: "event-1".into(),
            branch_id: "branch-a".into(),
            resource: BudgetResource::Compute,
            consumed_units: 30,
            quoted_units: 50,
            state: BudgetEventState::Running,
            event_tick: 9,
        }];
        let snapshot = compile_glioma_decision_budget_snapshot(&request).expect("snapshot");
        let compute = snapshot
            .buckets
            .iter()
            .find(|bucket| bucket.resource == BudgetResource::Compute)
            .unwrap();
        assert_eq!(compute.consumed_units, 30);
        assert_eq!(compute.reserved_units, 20);
        assert_eq!(compute.projected_units, 50);
    }

    #[test]
    fn hard_cap_crossing_requires_stop_and_preserves_mandatory_branch() {
        let mut request = request();
        request.events = vec![BudgetEvent {
            event_id: "event-1".into(),
            branch_id: "branch-a".into(),
            resource: BudgetResource::Compute,
            consumed_units: 100,
            quoted_units: 100,
            state: BudgetEventState::Completed,
            event_tick: 9,
        }];
        let snapshot = compile_glioma_decision_budget_snapshot(&request).expect("snapshot");
        assert_eq!(snapshot.disposition, DecisionBudgetDisposition::HardStop);
        assert_eq!(snapshot.hard_stop_branch_order, vec!["branch-a"]);
        assert!(snapshot
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("hard-cap")));
    }

    #[test]
    fn low_confidence_forecast_is_explicit_and_warns_before_cap() {
        let mut request = request();
        request.forecasts = vec![BudgetForecast {
            branch_id: "branch-a".into(),
            resource: BudgetResource::Assay,
            additional_units: 85,
            confidence_milli: 400,
            required: true,
            forecast_tick: 10,
        }];
        let snapshot = compile_glioma_decision_budget_snapshot(&request).expect("snapshot");
        assert_eq!(snapshot.disposition, DecisionBudgetDisposition::Warning);
        assert!(snapshot
            .uncertainty_order
            .iter()
            .any(|item| item.contains("low-confidence")));
    }

    #[test]
    fn over_cap_forecast_emits_approval_bound_reallocation() {
        let mut request = request();
        request.forecasts = vec![BudgetForecast {
            branch_id: "branch-a".into(),
            resource: BudgetResource::Compute,
            additional_units: 120,
            confidence_milli: 900,
            required: true,
            forecast_tick: 10,
        }];
        let snapshot = compile_glioma_decision_budget_snapshot(&request).expect("snapshot");
        assert_eq!(
            snapshot.disposition,
            DecisionBudgetDisposition::ApprovalRequired
        );
        assert_eq!(snapshot.reallocation_proposals.len(), 1);
        assert!(snapshot.reallocation_proposals[0].approval_required);
    }

    #[test]
    fn duplicate_events_are_rejected_before_accounting() {
        let mut request = request();
        let event = BudgetEvent {
            event_id: "duplicate".into(),
            branch_id: "branch-a".into(),
            resource: BudgetResource::Assay,
            consumed_units: 1,
            quoted_units: 1,
            state: BudgetEventState::Completed,
            event_tick: 9,
        };
        request.events = vec![event.clone(), event];
        assert!(matches!(
            compile_glioma_decision_budget_snapshot(&request),
            Err(DecisionBudgetError::InvalidRequest(_))
        ));
    }
}
