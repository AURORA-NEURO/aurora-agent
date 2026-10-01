//! Fair, gate-preserving high-throughput scheduling for preclinical glioma releases.
//!
//! The scheduler is a planning capability, not a publication side effect. It orders only
//! candidates whose scientific and provenance gates are ready, accounts for reviewer and compute
//! capacity, preserves deadline/risk/fairness evidence, and records every deferral reason.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseQueueSchedule1@1";
pub const MAX_CANDIDATES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueItem {
    pub candidate_id: String,
    pub object_id: String,
    pub gate_ready: bool,
    pub reviewer_ready: bool,
    pub compute_units: u32,
    pub reviewer_units: u16,
    pub risk_milli: u16,
    pub deadline_epoch: u64,
    pub fairness_credit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueCapacity {
    pub now_epoch: u64,
    pub planning_horizon_epochs: u64,
    pub compute_units: u32,
    pub reviewer_units: u16,
    pub max_scheduled: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueScheduleRequest {
    pub candidates: Vec<ReleaseQueueItem>,
    pub capacity: QueueCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueScheduleAction {
    Scheduled,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueScheduleEntry {
    pub candidate_id: String,
    pub rank: usize,
    pub action: QueueScheduleAction,
    pub start_epoch: Option<u64>,
    pub reason_order: Vec<String>,
    pub risk_adjusted_priority_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueScheduleDisposition {
    Scheduled,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub entries: Vec<QueueScheduleEntry>,
    pub scheduled_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compute_used: u32,
    pub reviewer_used: u16,
    pub disposition: QueueScheduleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueSchedulerError {
    #[error("release queue scheduler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release queue scheduler output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release queue scheduler digest failed: {0}")]
    Digest(String),
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}
fn digest_input(schedule: &ReleaseQueueSchedule) -> serde_json::Value {
    serde_json::json!({"feature_id":schedule.feature_id,"output_schema":schedule.output_schema,"entries":schedule.entries,"scheduled_order":schedule.scheduled_order,"deferred_order":schedule.deferred_order,"blocked_order":schedule.blocked_order,"compute_used":schedule.compute_used,"reviewer_used":schedule.reviewer_used,"disposition":schedule.disposition})
}
fn validate_request(request: &ReleaseQueueScheduleRequest) -> Result<(), QueueSchedulerError> {
    if request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.capacity.now_epoch == 0
        || request.capacity.planning_horizon_epochs == 0
        || request.capacity.max_scheduled == 0
        || request.candidates.iter().any(|candidate| {
            !valid_id(&candidate.candidate_id)
                || !valid_id(&candidate.object_id)
                || candidate.compute_units == 0
                || candidate.reviewer_units == 0
                || candidate.deadline_epoch < request.capacity.now_epoch
        })
    {
        return Err(QueueSchedulerError::InvalidRequest(
            "bounded candidates, capacity, deadlines, and typed identities are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if request
        .candidates
        .iter()
        .any(|candidate| !ids.insert(candidate.candidate_id.clone()))
    {
        return Err(QueueSchedulerError::InvalidRequest(
            "candidate identifiers must be unique".into(),
        ));
    }
    Ok(())
}
impl ReleaseQueueSchedule {
    pub fn validate(&self) -> Result<(), QueueSchedulerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].rank >= pair[1].rank)
        {
            return Err(QueueSchedulerError::InvalidOutput(
                "schedule identity or rank ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| QueueSchedulerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(QueueSchedulerError::InvalidOutput(
                "schedule digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Build a deterministic, capacity-bounded, gate-preserving release schedule.
pub fn schedule_glioma_release_queue(
    request: &ReleaseQueueScheduleRequest,
) -> Result<ReleaseQueueSchedule, QueueSchedulerError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .gate_ready
            .cmp(&left.gate_ready)
            .then_with(|| right.reviewer_ready.cmp(&left.reviewer_ready))
            .then_with(|| right.fairness_credit.cmp(&left.fairness_credit))
            .then_with(|| left.deadline_epoch.cmp(&right.deadline_epoch))
            .then_with(|| right.risk_milli.cmp(&left.risk_milli))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let mut compute_used = 0_u32;
    let mut reviewer_used = 0_u16;
    let mut scheduled = Vec::new();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut entries = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let priority = (candidate.fairness_credit as u64 + 1)
            * (1000_u64.saturating_sub(candidate.risk_milli as u64));
        let mut reasons = BTreeSet::new();
        let action = if !candidate.gate_ready {
            reasons.insert("scientific-or-provenance-gate-not-ready".into());
            blocked.push(candidate.candidate_id.clone());
            QueueScheduleAction::Blocked
        } else if !candidate.reviewer_ready {
            reasons.insert("reviewer-assignment-not-ready".into());
            deferred.push(candidate.candidate_id.clone());
            QueueScheduleAction::Deferred
        } else if scheduled.len() >= request.capacity.max_scheduled
            || compute_used.saturating_add(candidate.compute_units) > request.capacity.compute_units
            || reviewer_used.saturating_add(candidate.reviewer_units)
                > request.capacity.reviewer_units
            || candidate.deadline_epoch
                > request
                    .capacity
                    .now_epoch
                    .saturating_add(request.capacity.planning_horizon_epochs)
        {
            reasons.insert("capacity-or-horizon-deferred".into());
            deferred.push(candidate.candidate_id.clone());
            QueueScheduleAction::Deferred
        } else {
            compute_used += candidate.compute_units;
            reviewer_used += candidate.reviewer_units;
            scheduled.push(candidate.candidate_id.clone());
            QueueScheduleAction::Scheduled
        };
        let start_epoch = matches!(action, QueueScheduleAction::Scheduled)
            .then_some(request.capacity.now_epoch + scheduled.len() as u64 - 1);
        entries.push(QueueScheduleEntry {
            candidate_id: candidate.candidate_id.clone(),
            rank: index + 1,
            action,
            start_epoch,
            reason_order: reasons.into_iter().collect(),
            risk_adjusted_priority_milli: priority,
        });
    }
    let disposition = if !blocked.is_empty() {
        QueueScheduleDisposition::Blocked
    } else if !deferred.is_empty() {
        QueueScheduleDisposition::Partial
    } else {
        QueueScheduleDisposition::Scheduled
    };
    let mut schedule = ReleaseQueueSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        entries,
        scheduled_order: scheduled,
        deferred_order: deferred,
        blocked_order: blocked,
        compute_used,
        reviewer_used,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-queue-schedule"),
    };
    schedule.digest = ContentHash::of_value(&digest_input(&schedule))
        .map_err(|error| QueueSchedulerError::Digest(error.to_string()))?;
    schedule.validate()?;
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(id: &str, ready: bool) -> ReleaseQueueItem {
        ReleaseQueueItem {
            candidate_id: id.into(),
            object_id: format!("object-{id}"),
            gate_ready: ready,
            reviewer_ready: ready,
            compute_units: 2,
            reviewer_units: 1,
            risk_milli: 200,
            deadline_epoch: 105,
            fairness_credit: 3,
        }
    }
    fn request() -> ReleaseQueueScheduleRequest {
        ReleaseQueueScheduleRequest {
            candidates: vec![candidate("a", true), candidate("b", false)],
            capacity: QueueCapacity {
                now_epoch: 100,
                planning_horizon_epochs: 10,
                compute_units: 4,
                reviewer_units: 2,
                max_scheduled: 2,
            },
        }
    }
    #[test]
    fn schedules_ready_and_blocks_unready() {
        let schedule = schedule_glioma_release_queue(&request()).unwrap();
        assert_eq!(schedule.disposition, QueueScheduleDisposition::Blocked);
        assert_eq!(schedule.scheduled_order, vec!["a"]);
        assert_eq!(schedule.blocked_order, vec!["b"]);
        schedule.validate().unwrap();
    }
    #[test]
    fn capacity_defers_ready_candidates() {
        let mut request = request();
        request.candidates[1] = candidate("b", true);
        request.capacity.max_scheduled = 1;
        let schedule = schedule_glioma_release_queue(&request).unwrap();
        assert_eq!(schedule.disposition, QueueScheduleDisposition::Partial);
        assert!(schedule.deferred_order.contains(&"b".into()));
    }
}
