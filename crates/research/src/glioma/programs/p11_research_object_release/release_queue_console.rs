//! High-throughput research-object release queue snapshot and safe reorder proposals.
//!
//! This feature is an operations surface over an immutable candidate ledger.  It never changes a
//! candidate's scientific state: it only reconciles CI/reviewer telemetry, marks stale observations,
//! identifies bottlenecks, and proposes an ordering that cannot bypass a release gate.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseQueueSnapshot1@1";
pub const MAX_CANDIDATES: usize = 2048;
pub const MAX_EVENTS: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCandidateState {
    Ready,
    Reviewing,
    PendingEvidence,
    Blocked,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseQueueEventKind {
    CheckPassed,
    CheckFailed,
    ReviewerAssigned,
    StateObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueCandidate {
    pub candidate_id: String,
    pub version: String,
    pub state: ReleaseCandidateState,
    pub required_check_order: Vec<String>,
    pub passed_check_order: Vec<String>,
    pub failed_check_order: Vec<String>,
    pub reviewer_id: Option<String>,
    pub priority: u16,
    pub deadline_epoch: u64,
    pub lineage_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueEvent {
    pub event_id: String,
    pub candidate_id: String,
    pub kind: ReleaseQueueEventKind,
    pub check_id: Option<String>,
    pub reviewer_id: Option<String>,
    pub observed_state: Option<ReleaseCandidateState>,
    pub event_epoch: u64,
    pub telemetry_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReviewerAssignment {
    pub reviewer_id: String,
    pub assigned_count: u16,
    pub capacity: u16,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueRequest {
    pub candidates: Vec<ReleaseQueueCandidate>,
    pub events: Vec<ReleaseQueueEvent>,
    pub reviewers: Vec<ReleaseReviewerAssignment>,
    pub now_epoch: u64,
    pub stale_after_epochs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueReorderProposal {
    pub candidate_id: String,
    pub current_rank: u32,
    pub proposed_rank: u32,
    pub reason: String,
    pub gate_bypass: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQueueSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub now_epoch: u64,
    pub candidate_order: Vec<String>,
    pub proposed_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub reviewing_order: Vec<String>,
    pub pending_evidence_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub published_order: Vec<String>,
    pub stale_candidate_order: Vec<String>,
    pub failed_check_order: Vec<String>,
    pub bottleneck_order: Vec<String>,
    pub reorder_proposals: Vec<ReleaseQueueReorderProposal>,
    pub reviewer_load_order: Vec<String>,
    pub ledger_candidate_count: u32,
    pub reconciled_candidate_count: u32,
    pub telemetry_stale: bool,
    pub queue_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseQueueError {
    #[error("release queue request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release queue output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release queue digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn digest_input(snapshot: &ReleaseQueueSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "now_epoch": snapshot.now_epoch,
        "candidate_order": snapshot.candidate_order,
        "proposed_order": snapshot.proposed_order,
        "ready_order": snapshot.ready_order,
        "reviewing_order": snapshot.reviewing_order,
        "pending_evidence_order": snapshot.pending_evidence_order,
        "blocked_order": snapshot.blocked_order,
        "published_order": snapshot.published_order,
        "stale_candidate_order": snapshot.stale_candidate_order,
        "failed_check_order": snapshot.failed_check_order,
        "bottleneck_order": snapshot.bottleneck_order,
        "reorder_proposals": snapshot.reorder_proposals,
        "reviewer_load_order": snapshot.reviewer_load_order,
        "ledger_candidate_count": snapshot.ledger_candidate_count,
        "reconciled_candidate_count": snapshot.reconciled_candidate_count,
        "telemetry_stale": snapshot.telemetry_stale,
    })
}

impl ReleaseQueueSnapshot {
    pub fn validate(&self) -> Result<(), ReleaseQueueError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.now_epoch == 0
            || !canonical(&self.ready_order)
            || !canonical(&self.reviewing_order)
            || !canonical(&self.pending_evidence_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.published_order)
            || !canonical(&self.stale_candidate_order)
            || !canonical(&self.failed_check_order)
            || !canonical(&self.bottleneck_order)
            || !canonical(&self.reviewer_load_order)
            || self.ledger_candidate_count != self.reconciled_candidate_count
            || !digest(&self.queue_digest)
        {
            return Err(ReleaseQueueError::InvalidOutput(
                "queue identity, canonical partitions, reconciled counts, or digest is invalid"
                    .into(),
            ));
        }
        if self.reorder_proposals.iter().any(|proposal| {
            !identifier(&proposal.candidate_id)
                || proposal.current_rank == 0
                || proposal.proposed_rank == 0
                || proposal.gate_bypass
                || proposal.reason.trim().is_empty()
        }) {
            return Err(ReleaseQueueError::InvalidOutput(
                "reorder proposals cannot bypass gates".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseQueueError::Digest(error.to_string()))?;
        if expected != self.queue_digest {
            return Err(ReleaseQueueError::InvalidOutput(
                "queue digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ReleaseQueueRequest) -> Result<(), ReleaseQueueError> {
    if request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.events.len() > MAX_EVENTS
        || request.now_epoch == 0
        || request.stale_after_epochs == 0
    {
        return Err(ReleaseQueueError::InvalidRequest(
            "bounded candidate/event ledgers, current epoch, and staleness bound are required"
                .into(),
        ));
    }
    let mut candidates = BTreeSet::new();
    for candidate in &request.candidates {
        if !identifier(&candidate.candidate_id)
            || !identifier(&candidate.version)
            || !candidates.insert(candidate.candidate_id.clone())
            || !canonical(&candidate.required_check_order)
            || !canonical(&candidate.passed_check_order)
            || !canonical(&candidate.failed_check_order)
            || candidate
                .required_check_order
                .iter()
                .any(|check| !identifier(check))
            || candidate
                .reviewer_id
                .as_deref()
                .is_some_and(|reviewer| !identifier(reviewer))
            || candidate.deadline_epoch == 0
            || !digest(&candidate.lineage_digest)
        {
            return Err(ReleaseQueueError::InvalidRequest(
                "candidate identity, canonical checks, reviewer, deadline, and lineage are required".into(),
            ));
        }
    }
    let mut events = BTreeSet::new();
    for event in &request.events {
        if !identifier(&event.event_id)
            || !events.insert(event.event_id.clone())
            || !identifier(&event.candidate_id)
            || event.event_epoch == 0
            || event.telemetry_epoch == 0
            || event
                .check_id
                .as_deref()
                .is_some_and(|check| !identifier(check))
            || event
                .reviewer_id
                .as_deref()
                .is_some_and(|reviewer| !identifier(reviewer))
        {
            return Err(ReleaseQueueError::InvalidRequest(
                "events require unique identity, candidate, epoch, telemetry, and bounded optional fields".into(),
            ));
        }
    }
    Ok(())
}

/// Reconcile release telemetry and propose a gate-preserving queue order.
pub fn snapshot_glioma_release_queue(
    request: &ReleaseQueueRequest,
) -> Result<ReleaseQueueSnapshot, ReleaseQueueError> {
    validate_request(request)?;
    let candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let mut latest_telemetry = BTreeMap::<String, u64>::new();
    let mut failed_checks = BTreeSet::new();
    for event in &request.events {
        if !candidate_ids.contains(&event.candidate_id) {
            continue;
        }
        latest_telemetry
            .entry(event.candidate_id.clone())
            .and_modify(|epoch| *epoch = (*epoch).max(event.telemetry_epoch))
            .or_insert(event.telemetry_epoch);
        if event.kind == ReleaseQueueEventKind::CheckFailed {
            if let Some(check) = &event.check_id {
                failed_checks.insert(format!("{}:{check}", event.candidate_id));
            }
        }
    }
    let mut ordered = request.candidates.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left.deadline_epoch.cmp(&right.deadline_epoch))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let candidate_order = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let proposed_order = ordered
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut ready = BTreeSet::new();
    let mut reviewing = BTreeSet::new();
    let mut pending = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut published = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut bottlenecks = BTreeSet::new();
    for candidate in &request.candidates {
        match candidate.state {
            ReleaseCandidateState::Ready => {
                ready.insert(candidate.candidate_id.clone());
            }
            ReleaseCandidateState::Reviewing => {
                reviewing.insert(candidate.candidate_id.clone());
            }
            ReleaseCandidateState::PendingEvidence => {
                pending.insert(candidate.candidate_id.clone());
                bottlenecks.insert(format!("{}:pending-evidence", candidate.candidate_id));
            }
            ReleaseCandidateState::Blocked => {
                blocked.insert(candidate.candidate_id.clone());
                bottlenecks.insert(format!("{}:blocked-gate", candidate.candidate_id));
            }
            ReleaseCandidateState::Published => {
                published.insert(candidate.candidate_id.clone());
            }
        }
        let telemetry = latest_telemetry.get(&candidate.candidate_id).copied();
        if telemetry.is_none_or(|epoch| {
            request.now_epoch.saturating_sub(epoch) > request.stale_after_epochs
        }) {
            stale.insert(candidate.candidate_id.clone());
        }
        if candidate.reviewer_id.is_none() && candidate.state == ReleaseCandidateState::Reviewing {
            bottlenecks.insert(format!("{}:reviewer-unassigned", candidate.candidate_id));
        }
    }
    let mut reviewer_load_order = Vec::new();
    for reviewer in &request.reviewers {
        let state = if !reviewer.active {
            "inactive"
        } else if reviewer.assigned_count >= reviewer.capacity {
            bottlenecks.insert(format!("reviewer:{}:capacity", reviewer.reviewer_id));
            "at-capacity"
        } else {
            "available"
        };
        reviewer_load_order.push(format!("{}:{state}", reviewer.reviewer_id));
    }
    reviewer_load_order.sort();
    let mut reorder_proposals = Vec::new();
    for (index, candidate_id) in proposed_order.iter().enumerate() {
        let current = candidate_order
            .iter()
            .position(|id| id == candidate_id)
            .unwrap_or(index);
        if current != index {
            let candidate = request
                .candidates
                .iter()
                .find(|candidate| candidate.candidate_id == *candidate_id)
                .expect("candidate order derives from ledger");
            reorder_proposals.push(ReleaseQueueReorderProposal {
                candidate_id: candidate_id.clone(),
                current_rank: (current + 1) as u32,
                proposed_rank: (index + 1) as u32,
                reason: format!("state-priority:{:?}", candidate.state),
                gate_bypass: false,
            });
        }
    }
    let telemetry_stale = !stale.is_empty();
    let mut output = ReleaseQueueSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        now_epoch: request.now_epoch,
        candidate_order,
        proposed_order,
        ready_order: ready.into_iter().collect(),
        reviewing_order: reviewing.into_iter().collect(),
        pending_evidence_order: pending.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        published_order: published.into_iter().collect(),
        stale_candidate_order: stale.into_iter().collect(),
        failed_check_order: failed_checks.into_iter().collect(),
        bottleneck_order: bottlenecks.into_iter().collect(),
        reorder_proposals,
        reviewer_load_order,
        ledger_candidate_count: request.candidates.len() as u32,
        reconciled_candidate_count: request.candidates.len() as u32,
        telemetry_stale,
        queue_digest: ContentHash::of_bytes(b"unsealed-glioma-release-queue"),
    };
    output.queue_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReleaseQueueError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn candidate(id: &str, state: ReleaseCandidateState) -> ReleaseQueueCandidate {
        ReleaseQueueCandidate {
            candidate_id: id.into(),
            version: "v1".into(),
            state,
            required_check_order: vec!["replay".into(), "review".into()],
            passed_check_order: vec!["replay".into()],
            failed_check_order: Vec::new(),
            reviewer_id: None,
            priority: 5,
            deadline_epoch: 120,
            lineage_digest: hash(id),
        }
    }

    fn request() -> ReleaseQueueRequest {
        ReleaseQueueRequest {
            candidates: vec![
                candidate("candidate-blocked", ReleaseCandidateState::Blocked),
                candidate("candidate-ready", ReleaseCandidateState::Ready),
                candidate("candidate-review", ReleaseCandidateState::Reviewing),
            ],
            events: vec![ReleaseQueueEvent {
                event_id: "event-ready".into(),
                candidate_id: "candidate-ready".into(),
                kind: ReleaseQueueEventKind::StateObserved,
                check_id: None,
                reviewer_id: None,
                observed_state: Some(ReleaseCandidateState::Ready),
                event_epoch: 99,
                telemetry_epoch: 99,
            }],
            reviewers: vec![ReleaseReviewerAssignment {
                reviewer_id: "reviewer-a".into(),
                assigned_count: 2,
                capacity: 2,
                active: true,
            }],
            now_epoch: 100,
            stale_after_epochs: 10,
        }
    }

    #[test]
    fn snapshot_reconciles_counts_and_proposes_gate_preserving_order() {
        let output = snapshot_glioma_release_queue(&request()).unwrap();
        assert_eq!(output.ledger_candidate_count, 3);
        assert_eq!(output.reconciled_candidate_count, 3);
        assert_eq!(output.proposed_order[0], "candidate-ready");
        assert!(output
            .reorder_proposals
            .iter()
            .all(|proposal| !proposal.gate_bypass));
        output.validate().unwrap();
    }

    #[test]
    fn blocked_candidates_remain_blocked_after_reorder() {
        let output = snapshot_glioma_release_queue(&request()).unwrap();
        assert!(output.blocked_order.contains(&"candidate-blocked".into()));
        assert!(!output.proposed_order.is_empty());
    }

    #[test]
    fn stale_telemetry_and_reviewer_capacity_are_bottlenecks() {
        let mut request = request();
        request.events[0].telemetry_epoch = 1;
        let output = snapshot_glioma_release_queue(&request).unwrap();
        assert!(output.telemetry_stale);
        assert!(output
            .bottleneck_order
            .iter()
            .any(|value| value == "reviewer:reviewer-a:capacity"));
    }

    #[test]
    fn queue_mutation_breaks_digest() {
        let mut output = snapshot_glioma_release_queue(&request()).unwrap();
        output.now_epoch += 1;
        assert!(matches!(
            output.validate(),
            Err(ReleaseQueueError::InvalidOutput(_))
        ));
    }
}
