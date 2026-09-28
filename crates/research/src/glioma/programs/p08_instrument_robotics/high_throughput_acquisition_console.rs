//! Read-only high-throughput acquisition operations for preclinical glioma campaigns.
//!
//! The console is an operational decision surface for the autonomous research engine.  It turns
//! bounded queue and instrument summaries into a deterministic snapshot of backlog, preflight and
//! calibration blockers, operator load, deadline risk, and fairness-preserving reorder proposals.
//! It never rewrites signed schedules, changes sample assignments, or dispatches hardware.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaAcquisitionOperationsSnapshot1@2";
pub const MAX_ITEMS: usize = 1_024;
pub const MAX_DEVICES: usize = 256;
pub const MAX_PROPOSALS: usize = 256;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightState {
    Ready,
    Failed,
    Pending,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionQueueItem {
    pub acquisition_id: String,
    pub campaign_id: String,
    pub priority_milli: u32,
    pub fairness_weight_milli: u32,
    pub submitted_tick: u64,
    pub deadline_tick: u64,
    pub requested_units: u64,
    pub completed_units: u64,
    pub assigned_device_id: Option<String>,
    pub preflight_state: PreflightState,
    pub estimated_duration_ticks: u64,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionDeviceSummary {
    pub device_id: String,
    pub next_free_tick: u64,
    pub calibration_valid_until_tick: u64,
    pub operator_load_units: u32,
    pub operator_capacity_units: u32,
    pub last_observed_tick: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionOperationsRequest {
    pub items: Vec<AcquisitionQueueItem>,
    pub devices: Vec<AcquisitionDeviceSummary>,
    pub current_tick: u64,
    pub horizon_ticks: u64,
    pub telemetry_stale_after_ticks: u64,
    pub max_reorder_proposals: usize,
    pub minimum_fairness_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueRisk {
    Healthy,
    AtRisk,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionAssignmentView {
    pub acquisition_id: String,
    pub device_id: Option<String>,
    pub remaining_units: u64,
    pub projected_finish_tick: Option<u64>,
    pub risk: QueueRisk,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderProposal {
    pub acquisition_id: String,
    pub proposed_rank: usize,
    pub score_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionOperationsSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub current_tick: u64,
    pub horizon_end_tick: u64,
    pub backlog_units: u64,
    pub approved_backlog_units: u64,
    pub completed_units: u64,
    pub preflight_failure_count: usize,
    pub calibration_risk_count: usize,
    pub stale_device_order: Vec<String>,
    pub overloaded_device_order: Vec<String>,
    pub assignment_order: Vec<AcquisitionAssignmentView>,
    /// All approved, incomplete, preflight-ready items eligible for advisory reordering.
    pub reorder_candidate_order: Vec<String>,
    pub reorder_proposals: Vec<ReorderProposal>,
    /// The explicit candidate complement omitted by the proposal cap. These items remain
    /// routable in later rounds and are not silently conflated with blocked or ineligible work.
    pub deferred_reorder_order: Vec<String>,
    pub alerts: Vec<String>,
    pub queue_risk: QueueRisk,
    pub dispatch_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AcquisitionOperationsError {
    #[error("acquisition operations request is invalid: {0}")]
    InvalidRequest(String),
    #[error("acquisition operations output is invalid: {0}")]
    InvalidOutput(String),
    #[error("acquisition operations digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_id(value: &Option<String>) -> bool {
    value.as_ref().map_or(true, |item| safe_text(item))
}

fn snapshot_body(snapshot: &AcquisitionOperationsSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "current_tick": snapshot.current_tick,
        "horizon_end_tick": snapshot.horizon_end_tick,
        "backlog_units": snapshot.backlog_units,
        "approved_backlog_units": snapshot.approved_backlog_units,
        "completed_units": snapshot.completed_units,
        "preflight_failure_count": snapshot.preflight_failure_count,
        "calibration_risk_count": snapshot.calibration_risk_count,
        "stale_device_order": snapshot.stale_device_order,
        "overloaded_device_order": snapshot.overloaded_device_order,
        "assignment_order": snapshot.assignment_order,
        "reorder_candidate_order": snapshot.reorder_candidate_order,
        "reorder_proposals": snapshot.reorder_proposals,
        "deferred_reorder_order": snapshot.deferred_reorder_order,
        "alerts": snapshot.alerts,
        "queue_risk": snapshot.queue_risk,
        "dispatch_permitted": snapshot.dispatch_permitted,
    })
}

fn validate_request(
    request: &AcquisitionOperationsRequest,
) -> Result<(), AcquisitionOperationsError> {
    if request.items.is_empty()
        || request.items.len() > MAX_ITEMS
        || request.devices.is_empty()
        || request.devices.len() > MAX_DEVICES
        || request.current_tick == 0
        || request.horizon_ticks == 0
        || request.telemetry_stale_after_ticks == 0
        || request.max_reorder_proposals == 0
        || request.max_reorder_proposals > MAX_PROPOSALS
        || request.minimum_fairness_milli > 1_000_000
    {
        return Err(AcquisitionOperationsError::InvalidRequest(
            "bounded items/devices, positive horizon, telemetry age, proposal cap, and fairness floor are required".into(),
        ));
    }
    let item_ids = request
        .items
        .iter()
        .map(|item| item.acquisition_id.clone())
        .collect::<Vec<_>>();
    let device_ids = request
        .devices
        .iter()
        .map(|device| device.device_id.clone())
        .collect::<Vec<_>>();
    if item_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || item_ids.iter().collect::<BTreeSet<_>>().len() != item_ids.len()
        || device_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || device_ids.iter().collect::<BTreeSet<_>>().len() != device_ids.len()
    {
        return Err(AcquisitionOperationsError::InvalidRequest(
            "items and devices must be unique and canonically ordered".into(),
        ));
    }
    let known_devices = device_ids.iter().collect::<BTreeSet<_>>();
    for item in &request.items {
        if !safe_text(&item.acquisition_id)
            || !safe_text(&item.campaign_id)
            || item.priority_milli > 1_000_000
            || item.fairness_weight_milli == 0
            || item.fairness_weight_milli > 1_000_000
            || item.submitted_tick == 0
            || item.deadline_tick < request.current_tick
            || item.deadline_tick > request.current_tick.saturating_add(request.horizon_ticks)
            || item.requested_units == 0
            || item.completed_units > item.requested_units
            || item.estimated_duration_ticks == 0
            || !valid_id(&item.assigned_device_id)
            || item
                .assigned_device_id
                .as_ref()
                .is_some_and(|id| !known_devices.contains(id))
        {
            return Err(AcquisitionOperationsError::InvalidRequest(format!(
                "acquisition {} has invalid bounds or unknown device",
                item.acquisition_id
            )));
        }
    }
    for device in &request.devices {
        if !safe_text(&device.device_id)
            || device.next_free_tick == 0
            || device.calibration_valid_until_tick == 0
            || device.operator_capacity_units == 0
            || device.operator_load_units > device.operator_capacity_units
            || device.last_observed_tick == 0
        {
            return Err(AcquisitionOperationsError::InvalidRequest(format!(
                "device {} has invalid telemetry bounds",
                device.device_id
            )));
        }
    }
    Ok(())
}

impl AcquisitionOperationsSnapshot {
    pub fn validate(&self) -> Result<(), AcquisitionOperationsError> {
        if self.feature_id != FEATURE_ID || self.output_schema != OUTPUT_SCHEMA {
            return Err(AcquisitionOperationsError::InvalidOutput(
                "feature or output schema identity is incorrect".into(),
            ));
        }
        if self.current_tick == 0
            || self.horizon_end_tick < self.current_tick
            || self.assignment_order.is_empty()
            || self
                .assignment_order
                .windows(2)
                .any(|pair| pair[0].acquisition_id >= pair[1].acquisition_id)
            || self.reorder_proposals.len() > MAX_PROPOSALS
            || self
                .reorder_proposals
                .windows(2)
                .any(|pair| pair[0].proposed_rank >= pair[1].proposed_rank)
            || !self
                .reorder_candidate_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || !self
                .deferred_reorder_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(AcquisitionOperationsError::InvalidOutput(
                "snapshot bounds or canonical ordering are invalid".into(),
            ));
        }
        if self.dispatch_permitted {
            return Err(AcquisitionOperationsError::InvalidOutput(
                "operations console must remain read-only".into(),
            ));
        }
        let assignment_ids = self
            .assignment_order
            .iter()
            .map(|assignment| assignment.acquisition_id.as_str())
            .collect::<BTreeSet<_>>();
        let candidate_ids = self
            .reorder_candidate_order
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let proposal_ids = self
            .reorder_proposals
            .iter()
            .map(|proposal| proposal.acquisition_id.as_str())
            .collect::<BTreeSet<_>>();
        let deferred_ids = self
            .deferred_reorder_order
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if self.reorder_candidate_order.len() != candidate_ids.len()
            || self.deferred_reorder_order.len() != deferred_ids.len()
            || !candidate_ids.is_subset(&assignment_ids)
            || proposal_ids.len() != self.reorder_proposals.len()
            || !proposal_ids.is_subset(&candidate_ids)
            || !deferred_ids.is_subset(&candidate_ids)
            || proposal_ids.intersection(&deferred_ids).next().is_some()
            || proposal_ids
                .union(&deferred_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_ids
            || self
                .reorder_proposals
                .iter()
                .enumerate()
                .any(|(index, proposal)| proposal.proposed_rank != index + 1)
            || self
                .reorder_proposals
                .iter()
                .any(|proposal| proposal.rationale.trim().is_empty())
        {
            return Err(AcquisitionOperationsError::InvalidOutput(
                "reorder candidates, proposals, and deferred work must form a canonical partition"
                    .into(),
            ));
        }
        let digest = ContentHash::of_value(&snapshot_body(self))
            .map_err(|error| AcquisitionOperationsError::Digest(error.to_string()))?;
        if digest != self.digest {
            return Err(AcquisitionOperationsError::InvalidOutput(
                "snapshot digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn score(item: &AcquisitionQueueItem, current_tick: u64) -> u64 {
    let remaining = item.requested_units.saturating_sub(item.completed_units);
    let urgency = item.deadline_tick.saturating_sub(current_tick).max(1);
    let urgency_milli = 1_000_000u64 / urgency.min(1_000_000);
    u64::from(item.priority_milli)
        .saturating_mul(remaining.min(1_000_000))
        .saturating_add(u64::from(item.fairness_weight_milli).saturating_mul(urgency_milli))
}

pub fn plan_glioma_acquisition_operations(
    request: &AcquisitionOperationsRequest,
) -> Result<AcquisitionOperationsSnapshot, AcquisitionOperationsError> {
    validate_request(request)?;
    let devices = request
        .devices
        .iter()
        .map(|device| (device.device_id.as_str(), device))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut assignments = Vec::with_capacity(request.items.len());
    let mut preflight_failure_count = 0usize;
    let mut calibration_risk_count = 0usize;
    let mut stale_device_order = Vec::new();
    let mut overloaded_device_order = Vec::new();
    for device in &request.devices {
        if request
            .current_tick
            .saturating_sub(device.last_observed_tick)
            > request.telemetry_stale_after_ticks
        {
            stale_device_order.push(device.device_id.clone());
        }
        if device.operator_load_units >= device.operator_capacity_units {
            overloaded_device_order.push(device.device_id.clone());
        }
    }
    for item in &request.items {
        let remaining_units = item.requested_units.saturating_sub(item.completed_units);
        let mut reasons: Vec<String> = Vec::new();
        let device = item
            .assigned_device_id
            .as_ref()
            .and_then(|id| devices.get(id.as_str()).copied());
        if matches!(
            item.preflight_state,
            PreflightState::Failed | PreflightState::Blocked
        ) {
            preflight_failure_count += 1;
            reasons.push("preflight-is-not-admitted".into());
        } else if item.preflight_state == PreflightState::Pending {
            reasons.push("preflight-is-pending".into());
        } else if item.preflight_state == PreflightState::Unknown {
            reasons.push("preflight-state-is-unknown".into());
        }
        let mut projected_finish_tick = None;
        if let Some(device) = device {
            projected_finish_tick = Some(
                device
                    .next_free_tick
                    .saturating_add(item.estimated_duration_ticks),
            );
            if request.current_tick > device.calibration_valid_until_tick {
                calibration_risk_count += 1;
                reasons.push("device-calibration-is-expired".into());
            } else if projected_finish_tick.unwrap_or(0) > device.calibration_valid_until_tick {
                calibration_risk_count += 1;
                reasons.push("device-calibration-expires-before-finish".into());
            }
            if request
                .current_tick
                .saturating_sub(device.last_observed_tick)
                > request.telemetry_stale_after_ticks
            {
                reasons.push("device-telemetry-is-stale".into());
            }
            if !device.enabled {
                reasons.push("device-is-disabled".into());
            }
            if device.operator_load_units >= device.operator_capacity_units {
                reasons.push("operator-capacity-is-full".into());
            }
        } else {
            reasons.push("no-device-assignment".into());
        }
        let finish_misses_deadline = projected_finish_tick
            .map(|finish| finish > item.deadline_tick)
            .unwrap_or(true);
        if finish_misses_deadline {
            reasons.push("deadline-is-at-risk".into());
        }
        let risk = if matches!(
            item.preflight_state,
            PreflightState::Failed | PreflightState::Blocked
        ) || reasons.iter().any(|reason| {
            reason.contains("calibration")
                || reason.contains("expired")
                || reason.contains("disabled")
        }) {
            QueueRisk::Blocked
        } else if item.preflight_state == PreflightState::Unknown
            || device.is_none()
            || reasons.iter().any(|reason| reason.contains("stale"))
        {
            QueueRisk::Unresolved
        } else if !reasons.is_empty() {
            QueueRisk::AtRisk
        } else {
            QueueRisk::Healthy
        };
        assignments.push(AcquisitionAssignmentView {
            acquisition_id: item.acquisition_id.clone(),
            device_id: item.assigned_device_id.clone(),
            remaining_units,
            projected_finish_tick,
            risk,
            reason_order: reasons,
        });
    }
    let mut candidates = request
        .items
        .iter()
        .filter(|item| {
            item.approved
                && item.completed_units < item.requested_units
                && item.preflight_state == PreflightState::Ready
        })
        .map(|item| {
            (
                item.acquisition_id.clone(),
                score(item, request.current_tick),
            )
        })
        .collect::<Vec<_>>();
    let reorder_candidate_order = candidates
        .iter()
        .map(|(acquisition_id, _)| acquisition_id.clone())
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let reorder_proposals = candidates
        .into_iter()
        .take(request.max_reorder_proposals)
        .enumerate()
        .map(|(index, (acquisition_id, score_milli))| ReorderProposal {
            acquisition_id,
            proposed_rank: index + 1,
            score_milli,
            rationale: "approved ready demand ranked by priority, remaining work, urgency, and fairness weight; signed schedule remains unchanged".into(),
        })
        .collect::<Vec<_>>();
    let proposed_ids = reorder_proposals
        .iter()
        .map(|proposal| proposal.acquisition_id.as_str())
        .collect::<BTreeSet<_>>();
    let deferred_reorder_order = reorder_candidate_order
        .iter()
        .filter(|acquisition_id| !proposed_ids.contains(acquisition_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let backlog_units = request
        .items
        .iter()
        .map(|item| item.requested_units.saturating_sub(item.completed_units))
        .sum();
    let approved_backlog_units = request
        .items
        .iter()
        .filter(|item| item.approved)
        .map(|item| item.requested_units.saturating_sub(item.completed_units))
        .sum();
    let completed_units = request.items.iter().map(|item| item.completed_units).sum();
    let mut alerts = Vec::new();
    if !stale_device_order.is_empty() {
        alerts.push("stale-device-telemetry-limits-autonomous-reordering".into());
    }
    if preflight_failure_count > 0 {
        alerts.push("preflight-failures-require-investigation-before-dispatch".into());
    }
    if calibration_risk_count > 0 {
        alerts.push("calibration-window-risk-reduces-available-throughput".into());
    }
    let at_risk_count = assignments
        .iter()
        .filter(|assignment| assignment.risk == QueueRisk::AtRisk)
        .count();
    let queue_risk = if assignments
        .iter()
        .any(|assignment| assignment.risk == QueueRisk::Blocked)
    {
        QueueRisk::Blocked
    } else if assignments
        .iter()
        .any(|assignment| assignment.risk == QueueRisk::Unresolved)
    {
        QueueRisk::Unresolved
    } else if at_risk_count > 0 || !overloaded_device_order.is_empty() {
        QueueRisk::AtRisk
    } else {
        QueueRisk::Healthy
    };
    if request.minimum_fairness_milli > 0 && reorder_proposals.is_empty() {
        alerts.push("no-ready-approved-demand-is-eligible-for-reordering".into());
    }
    let snapshot = AcquisitionOperationsSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        current_tick: request.current_tick,
        horizon_end_tick: request.current_tick.saturating_add(request.horizon_ticks),
        backlog_units,
        approved_backlog_units,
        completed_units,
        preflight_failure_count,
        calibration_risk_count,
        stale_device_order,
        overloaded_device_order,
        assignment_order: assignments,
        reorder_candidate_order,
        reorder_proposals,
        deferred_reorder_order,
        alerts,
        queue_risk,
        dispatch_permitted: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-acquisition-operations"),
    };
    let mut sealed = snapshot;
    sealed.digest = ContentHash::of_value(&snapshot_body(&sealed))
        .map_err(|error| AcquisitionOperationsError::Digest(error.to_string()))?;
    sealed.validate()?;
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> AcquisitionDeviceSummary {
        AcquisitionDeviceSummary {
            device_id: id.into(),
            next_free_tick: 10,
            calibration_valid_until_tick: 40,
            operator_load_units: 1,
            operator_capacity_units: 4,
            last_observed_tick: 10,
            enabled: true,
        }
    }

    fn item(id: &str, priority_milli: u32) -> AcquisitionQueueItem {
        AcquisitionQueueItem {
            acquisition_id: id.into(),
            campaign_id: "campaign-a".into(),
            priority_milli,
            fairness_weight_milli: 1_000,
            submitted_tick: 1,
            deadline_tick: 30,
            requested_units: 5,
            completed_units: 0,
            assigned_device_id: Some("device-a".into()),
            preflight_state: PreflightState::Ready,
            estimated_duration_ticks: 4,
            approved: true,
        }
    }

    fn request(
        items: Vec<AcquisitionQueueItem>,
        devices: Vec<AcquisitionDeviceSummary>,
    ) -> AcquisitionOperationsRequest {
        AcquisitionOperationsRequest {
            items,
            devices,
            current_tick: 10,
            horizon_ticks: 30,
            telemetry_stale_after_ticks: 5,
            max_reorder_proposals: 4,
            minimum_fairness_milli: 500,
        }
    }

    #[test]
    fn healthy_queue_emits_deterministic_read_only_snapshot() {
        let output = plan_glioma_acquisition_operations(&request(
            vec![item("item-a", 900)],
            vec![device("device-a")],
        ))
        .unwrap();
        assert_eq!(output.queue_risk, QueueRisk::Healthy);
        assert_eq!(output.reorder_proposals[0].acquisition_id, "item-a");
        assert!(!output.dispatch_permitted);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn priority_and_urgency_drive_stable_reorder_without_mutation() {
        let mut low = item("item-a", 100);
        low.deadline_tick = 39;
        let high = item("item-b", 900);
        let output =
            plan_glioma_acquisition_operations(&request(vec![low, high], vec![device("device-a")]))
                .unwrap();
        assert_eq!(output.reorder_proposals[0].acquisition_id, "item-b");
        assert_eq!(output.reorder_candidate_order, vec!["item-a", "item-b"]);
        assert_eq!(output.assignment_order[0].acquisition_id, "item-a");
    }

    #[test]
    fn proposal_cap_preserves_deferred_ready_demand_for_the_next_round() {
        let mut request = request(
            vec![item("item-a", 100), item("item-b", 900)],
            vec![device("device-a")],
        );
        request.max_reorder_proposals = 1;
        let output = plan_glioma_acquisition_operations(&request).unwrap();
        assert_eq!(output.reorder_proposals.len(), 1);
        assert_eq!(output.reorder_proposals[0].acquisition_id, "item-b");
        assert_eq!(output.deferred_reorder_order, vec!["item-a"]);
        output.validate().unwrap();
    }

    #[test]
    fn stale_telemetry_and_preflight_failure_are_explicit_risks() {
        let mut stale = device("device-a");
        stale.last_observed_tick = 1;
        let mut blocked = item("item-a", 900);
        blocked.preflight_state = PreflightState::Failed;
        let output =
            plan_glioma_acquisition_operations(&request(vec![blocked], vec![stale])).unwrap();
        assert_eq!(output.queue_risk, QueueRisk::Blocked);
        assert_eq!(output.preflight_failure_count, 1);
        assert!(output.stale_device_order.contains(&"device-a".to_string()));
    }

    #[test]
    fn calibration_expiry_blocks_finish_and_unknown_device_is_rejected() {
        let mut expiring = device("device-a");
        expiring.calibration_valid_until_tick = 12;
        let output =
            plan_glioma_acquisition_operations(&request(vec![item("item-a", 900)], vec![expiring]))
                .unwrap();
        assert_eq!(output.queue_risk, QueueRisk::Blocked);
        assert_eq!(output.calibration_risk_count, 1);
        let mut bad = item("item-a", 900);
        bad.assigned_device_id = Some("missing".into());
        assert!(
            plan_glioma_acquisition_operations(&request(vec![bad], vec![device("device-a")]))
                .is_err()
        );
    }
}
