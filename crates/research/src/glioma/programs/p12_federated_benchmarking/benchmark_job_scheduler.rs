//! Durable, aggregate-only federated benchmark job state machine for glioma research.
//!
//! The scheduler consumes a typed event stream rather than contacting institutions. It provides
//! the state semantics an institution-owned adapter can implement: idempotent event identities,
//! resumable checkpoints, bounded retries, quorum revalidation, cancellation, and explicit budget
//! or partition stops. It never transports raw traces, credentials, or clinical decisions.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkJob1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_EVENTS: usize = 4_096;
pub const MAX_REASON_LENGTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkJobStage {
    Created,
    AwaitingApproval,
    Running,
    Paused,
    Completed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkJobSiteStage {
    Pending,
    Approved,
    Running,
    Completed,
    Withdrawn,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkJobDisposition {
    Queued,
    Running,
    Partial,
    Completed,
    Cancelled,
    Partitioned,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkJobEventKind {
    Created,
    ApprovalGranted,
    ApprovalDenied {
        reason: String,
    },
    QueryStarted,
    QueryCompleted {
        result_digest: ContentHash,
        budget_cost_units: u64,
        privacy_cost_milli: u64,
    },
    Checkpoint {
        checkpoint_digest: ContentHash,
    },
    RetryRequested,
    Withdrawn {
        reason: String,
    },
    CancelRequested,
    ResumeRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkJobEvent {
    pub event_id: String,
    pub sequence: u64,
    pub site_id: Option<String>,
    pub kind: BenchmarkJobEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkJobSiteInput {
    pub site_id: String,
    pub initially_approved: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkJobRequest {
    pub job_id: String,
    pub objective: String,
    pub idempotency_key: String,
    pub site_order: Vec<BenchmarkJobSiteInput>,
    pub quorum_sites: usize,
    pub max_query_sites: usize,
    pub max_retry_attempts: u16,
    pub max_events: usize,
    pub budget_units: u64,
    pub privacy_budget_milli: u64,
    pub events: Vec<BenchmarkJobEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkJobSiteState {
    pub site_id: String,
    pub stage: BenchmarkJobSiteStage,
    pub approved: bool,
    pub revoked: bool,
    pub active_query: bool,
    pub query_count: u32,
    pub retry_count: u16,
    pub checkpoint_digest: Option<ContentHash>,
    pub result_digest_order: Vec<ContentHash>,
    pub last_sequence: Option<u64>,
    pub blocker_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkJob {
    pub feature_id: String,
    pub output_schema: String,
    pub job_id: String,
    pub objective: String,
    pub idempotency_key: String,
    pub stage: BenchmarkJobStage,
    pub site_order: Vec<String>,
    pub sites: Vec<BenchmarkJobSiteState>,
    pub applied_event_order: Vec<String>,
    pub duplicate_event_order: Vec<String>,
    pub conflicting_event_order: Vec<String>,
    pub ignored_event_order: Vec<String>,
    pub missing_sequence_order: Vec<u64>,
    pub blockers: Vec<String>,
    pub budget_units: u64,
    pub budget_spent_units: u64,
    pub privacy_budget_milli: u64,
    pub privacy_spent_milli: u64,
    pub active_query_site_order: Vec<String>,
    pub completed_site_order: Vec<String>,
    pub quorum_satisfied: bool,
    pub dispatch_permitted: bool,
    pub disposition: BenchmarkJobDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BenchmarkJobError {
    #[error("benchmark job request is invalid: {0}")]
    InvalidRequest(String),
    #[error("benchmark job output is invalid: {0}")]
    InvalidOutput(String),
    #[error("benchmark job digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_REASON_LENGTH
        && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(job: &FederatedBenchmarkJob) -> serde_json::Value {
    serde_json::json!({
        "feature_id": job.feature_id,
        "output_schema": job.output_schema,
        "job_id": job.job_id,
        "objective": job.objective,
        "idempotency_key": job.idempotency_key,
        "stage": job.stage,
        "site_order": job.site_order,
        "sites": job.sites,
        "applied_event_order": job.applied_event_order,
        "duplicate_event_order": job.duplicate_event_order,
        "conflicting_event_order": job.conflicting_event_order,
        "ignored_event_order": job.ignored_event_order,
        "missing_sequence_order": job.missing_sequence_order,
        "blockers": job.blockers,
        "budget_units": job.budget_units,
        "budget_spent_units": job.budget_spent_units,
        "privacy_budget_milli": job.privacy_budget_milli,
        "privacy_spent_milli": job.privacy_spent_milli,
        "active_query_site_order": job.active_query_site_order,
        "completed_site_order": job.completed_site_order,
        "quorum_satisfied": job.quorum_satisfied,
        "dispatch_permitted": job.dispatch_permitted,
        "disposition": job.disposition,
        "next_action": job.next_action,
    })
}

fn validate_request(request: &BenchmarkJobRequest) -> Result<(), BenchmarkJobError> {
    if !safe_text(&request.job_id)
        || !safe_text(&request.objective)
        || !safe_text(&request.idempotency_key)
        || request.site_order.is_empty()
        || request.site_order.len() > MAX_SITES
        || request.quorum_sites == 0
        || request.quorum_sites > request.site_order.len()
        || request.max_query_sites == 0
        || request.max_events == 0
        || request.max_events > MAX_EVENTS
        || request.events.len() > request.max_events
    {
        return Err(BenchmarkJobError::InvalidRequest(
            "job identity, bounded sites/events, positive quorum, and query limits are required"
                .into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    for site in &request.site_order {
        if !safe_text(&site.site_id) || !site_ids.insert(site.site_id.clone()) {
            return Err(BenchmarkJobError::InvalidRequest(
                "site identifiers must be unique and bounded".into(),
            ));
        }
    }
    let site_order = request
        .site_order
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    if !canonical(&site_order) {
        return Err(BenchmarkJobError::InvalidRequest(
            "site_order must be canonical".into(),
        ));
    }
    for event in &request.events {
        if !safe_text(&event.event_id) {
            return Err(BenchmarkJobError::InvalidRequest(format!(
                "event {} has an invalid identity",
                event.event_id
            )));
        }
        // Duplicate identities are valid input for replay: the executor records identical
        // duplicates and conflicting payloads separately. They still pass the same shape
        // validation as first-seen events so a malformed retry cannot bypass safety checks.
        if event
            .site_id
            .as_ref()
            .is_some_and(|site_id| !site_ids.contains(site_id))
        {
            return Err(BenchmarkJobError::InvalidRequest(format!(
                "event {} references an unknown site",
                event.event_id
            )));
        }
        let requires_site = matches!(
            event.kind,
            BenchmarkJobEventKind::ApprovalGranted
                | BenchmarkJobEventKind::ApprovalDenied { .. }
                | BenchmarkJobEventKind::QueryStarted
                | BenchmarkJobEventKind::QueryCompleted { .. }
                | BenchmarkJobEventKind::Checkpoint { .. }
                | BenchmarkJobEventKind::RetryRequested
                | BenchmarkJobEventKind::Withdrawn { .. }
        );
        if requires_site != event.site_id.is_some() {
            return Err(BenchmarkJobError::InvalidRequest(format!(
                "event {} has an invalid site binding",
                event.event_id
            )));
        }
        match &event.kind {
            BenchmarkJobEventKind::Created
            | BenchmarkJobEventKind::ApprovalGranted
            | BenchmarkJobEventKind::QueryStarted
            | BenchmarkJobEventKind::Checkpoint { .. }
            | BenchmarkJobEventKind::RetryRequested
            | BenchmarkJobEventKind::CancelRequested
            | BenchmarkJobEventKind::ResumeRequested
            | BenchmarkJobEventKind::QueryCompleted { .. }
            | BenchmarkJobEventKind::ApprovalDenied { .. }
            | BenchmarkJobEventKind::Withdrawn { .. } => {}
        }
        if matches!(
            event.kind,
            BenchmarkJobEventKind::ApprovalDenied { .. } | BenchmarkJobEventKind::Withdrawn { .. }
        ) {
            let reason = match &event.kind {
                BenchmarkJobEventKind::ApprovalDenied { reason }
                | BenchmarkJobEventKind::Withdrawn { reason } => reason,
                _ => unreachable!(),
            };
            if !safe_text(reason) {
                return Err(BenchmarkJobError::InvalidRequest(format!(
                    "event {} has an invalid reason",
                    event.event_id
                )));
            }
        }
        if matches!(
            event.kind,
            BenchmarkJobEventKind::QueryCompleted { .. } | BenchmarkJobEventKind::Checkpoint { .. }
        ) {
            if let BenchmarkJobEventKind::QueryCompleted {
                budget_cost_units,
                privacy_cost_milli,
                ..
            } = event.kind
            {
                if budget_cost_units == 0 && privacy_cost_milli == 0 {
                    return Err(BenchmarkJobError::InvalidRequest(format!(
                        "event {} must declare a positive cost",
                        event.event_id
                    )));
                }
            }
        }
    }
    Ok(())
}

impl FederatedBenchmarkJob {
    pub fn validate(&self) -> Result<(), BenchmarkJobError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.job_id)
            || !safe_text(&self.objective)
            || !safe_text(&self.idempotency_key)
            || !canonical(&self.site_order)
            || self.sites.len() != self.site_order.len()
            || !canonical(&self.applied_event_order)
            || !canonical(&self.duplicate_event_order)
            || !canonical(&self.conflicting_event_order)
            || !canonical(&self.ignored_event_order)
            || !canonical(&self.missing_sequence_order)
            || !canonical(&self.blockers)
            || !canonical(&self.active_query_site_order)
            || !canonical(&self.completed_site_order)
            || self.budget_spent_units > self.budget_units
            || self.privacy_spent_milli > self.privacy_budget_milli
            || self.dispatch_permitted
            || self.digest.as_str().len() != 64
        {
            return Err(BenchmarkJobError::InvalidOutput(
                "job identity, ordering, budget, or dispatch invariants are invalid".into(),
            ));
        }
        if self.sites.iter().any(|site| {
            !safe_text(&site.site_id)
                || !canonical(&site.blocker_order)
                || site.site_order_invariant_violation()
        }) {
            return Err(BenchmarkJobError::InvalidOutput(
                "site state ordering or lifecycle invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| BenchmarkJobError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(BenchmarkJobError::InvalidOutput(
                "job digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

impl BenchmarkJobSiteState {
    fn site_order_invariant_violation(&self) -> bool {
        self.revoked && self.active_query
    }
}

/// Reconcile an asynchronous benchmark event stream into an explicit, resumable job state.
pub fn execute_glioma_benchmark_job(
    request: &BenchmarkJobRequest,
) -> Result<FederatedBenchmarkJob, BenchmarkJobError> {
    validate_request(request)?;
    let site_ids = request
        .site_order
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let mut sites = request
        .site_order
        .iter()
        .map(|site| BenchmarkJobSiteState {
            site_id: site.site_id.clone(),
            stage: if site.revoked {
                BenchmarkJobSiteStage::Withdrawn
            } else if site.initially_approved {
                BenchmarkJobSiteStage::Approved
            } else {
                BenchmarkJobSiteStage::Pending
            },
            approved: site.initially_approved && !site.revoked,
            revoked: site.revoked,
            active_query: false,
            query_count: 0,
            retry_count: 0,
            checkpoint_digest: None,
            result_digest_order: Vec::new(),
            last_sequence: None,
            blocker_order: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut by_id = BTreeMap::<String, BenchmarkJobEvent>::new();
    let mut duplicate_event_order = BTreeSet::new();
    let mut conflicting_event_order = BTreeSet::new();
    for event in &request.events {
        if let Some(previous) = by_id.get(&event.event_id) {
            if previous == event {
                duplicate_event_order.insert(event.event_id.clone());
            } else {
                conflicting_event_order.insert(event.event_id.clone());
            }
            continue;
        }
        by_id.insert(event.event_id.clone(), event.clone());
    }
    let mut events = by_id.into_values().collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let mut missing_sequence_order = BTreeSet::new();
    if let (Some(first), Some(last)) = (events.first(), events.last()) {
        let present = events
            .iter()
            .map(|event| event.sequence)
            .collect::<BTreeSet<_>>();
        for sequence in first.sequence..=last.sequence {
            if !present.contains(&sequence) {
                missing_sequence_order.insert(sequence);
            }
        }
    }
    let mut applied_event_order = BTreeSet::new();
    let mut ignored_event_order = BTreeSet::new();
    let mut blockers = BTreeSet::new();
    let mut stage = if events.is_empty() {
        BenchmarkJobStage::Created
    } else {
        BenchmarkJobStage::AwaitingApproval
    };
    let mut budget_spent = 0u64;
    let mut privacy_spent = 0u64;
    let mut created = false;
    let mut cancel_requested = false;
    for event in &events {
        let site_index = event
            .site_id
            .as_ref()
            .and_then(|site_id| site_ids.binary_search(site_id).ok());
        if event.site_id.is_some() && site_index.is_none() {
            blockers.insert(format!("unknown-site-event:{}", event.event_id));
            ignored_event_order.insert(event.event_id.clone());
            continue;
        }
        let mut event_applied = true;
        match &event.kind {
            BenchmarkJobEventKind::Created => {
                if event.site_id.is_some() || created {
                    blockers.insert(format!("duplicate-create:{}", event.event_id));
                    event_applied = false;
                } else {
                    created = true;
                    stage = BenchmarkJobStage::AwaitingApproval;
                }
            }
            BenchmarkJobEventKind::ApprovalGranted => {
                let index = site_index.expect("validated site for approval");
                let site = &mut sites[index];
                if site.revoked {
                    blockers.insert(format!("approval-after-withdrawal:{}", site.site_id));
                    event_applied = false;
                } else {
                    site.approved = true;
                    site.stage = BenchmarkJobSiteStage::Approved;
                    stage = BenchmarkJobStage::AwaitingApproval;
                }
            }
            BenchmarkJobEventKind::ApprovalDenied { reason } => {
                let index = site_index.expect("validated site for denial");
                let site = &mut sites[index];
                site.approved = false;
                site.stage = BenchmarkJobSiteStage::Blocked;
                site.blocker_order.push(format!("approval-denied:{reason}"));
                blockers.insert(format!("approval-denied:{}", site.site_id));
                stage = BenchmarkJobStage::Blocked;
            }
            BenchmarkJobEventKind::QueryStarted => {
                let index = site_index.expect("validated site for query start");
                let active_count = sites.iter().filter(|site| site.active_query).count();
                let site = &mut sites[index];
                if !site.approved || site.revoked || site.active_query {
                    site.blocker_order
                        .push("query-start-without-approval".into());
                    blockers.insert(format!("query-start-denied:{}", site.site_id));
                    event_applied = false;
                } else if active_count >= request.max_query_sites {
                    site.blocker_order.push("query-concurrency-cap".into());
                    blockers.insert("query-concurrency-cap".into());
                    event_applied = false;
                } else {
                    site.active_query = true;
                    site.stage = BenchmarkJobSiteStage::Running;
                    stage = BenchmarkJobStage::Running;
                }
            }
            BenchmarkJobEventKind::QueryCompleted {
                result_digest,
                budget_cost_units,
                privacy_cost_milli,
            } => {
                let index = site_index.expect("validated site for query completion");
                let site = &mut sites[index];
                if !site.active_query || site.revoked {
                    site.blocker_order
                        .push("query-complete-without-active-query".into());
                    blockers.insert(format!("query-complete-denied:{}", site.site_id));
                    event_applied = false;
                } else if budget_spent.saturating_add(*budget_cost_units) > request.budget_units {
                    site.blocker_order.push("budget-exhausted".into());
                    blockers.insert("budget-exhausted".into());
                    stage = BenchmarkJobStage::Blocked;
                    event_applied = false;
                } else if privacy_spent.saturating_add(*privacy_cost_milli)
                    > request.privacy_budget_milli
                {
                    site.blocker_order.push("privacy-budget-exhausted".into());
                    blockers.insert("privacy-budget-exhausted".into());
                    stage = BenchmarkJobStage::Blocked;
                    event_applied = false;
                } else {
                    site.active_query = false;
                    // `query_count` is a completed aggregate-query count, not a dispatch count.
                    // Counting at both start and completion made one successful query appear as
                    // two queries and caused autonomous quota/reconciliation consumers to
                    // overestimate completed work.
                    site.query_count = site.query_count.saturating_add(1);
                    site.stage = BenchmarkJobSiteStage::Completed;
                    site.result_digest_order.push(result_digest.clone());
                    budget_spent = budget_spent.saturating_add(*budget_cost_units);
                    privacy_spent = privacy_spent.saturating_add(*privacy_cost_milli);
                }
            }
            BenchmarkJobEventKind::Checkpoint { checkpoint_digest } => {
                let index = site_index.expect("validated site for checkpoint");
                let site = &mut sites[index];
                if site.revoked {
                    blockers.insert(format!("checkpoint-after-withdrawal:{}", site.site_id));
                    event_applied = false;
                } else {
                    site.checkpoint_digest = Some(checkpoint_digest.clone());
                }
            }
            BenchmarkJobEventKind::RetryRequested => {
                let index = site_index.expect("validated site for retry");
                let site = &mut sites[index];
                if site.retry_count >= request.max_retry_attempts {
                    site.blocker_order.push("retry-cap-exhausted".into());
                    blockers.insert(format!("retry-cap-exhausted:{}", site.site_id));
                    stage = BenchmarkJobStage::Blocked;
                    event_applied = false;
                } else {
                    site.retry_count = site.retry_count.saturating_add(1);
                    site.active_query = false;
                    site.stage = BenchmarkJobSiteStage::Approved;
                }
            }
            BenchmarkJobEventKind::Withdrawn { reason } => {
                let index = site_index.expect("validated site for withdrawal");
                let site = &mut sites[index];
                site.revoked = true;
                site.approved = false;
                site.active_query = false;
                site.stage = BenchmarkJobSiteStage::Withdrawn;
                site.blocker_order.push(format!("withdrawn:{reason}"));
            }
            BenchmarkJobEventKind::CancelRequested => {
                cancel_requested = true;
                stage = BenchmarkJobStage::Cancelled;
            }
            BenchmarkJobEventKind::ResumeRequested => {
                if stage == BenchmarkJobStage::Paused {
                    stage = BenchmarkJobStage::Running;
                } else {
                    ignored_event_order.insert(event.event_id.clone());
                    event_applied = false;
                }
            }
        }
        if event_applied {
            applied_event_order.insert(event.event_id.clone());
            for site in &mut sites {
                if event.site_id.as_deref() == Some(site.site_id.as_str()) {
                    site.last_sequence = Some(event.sequence);
                }
            }
        } else {
            ignored_event_order.insert(event.event_id.clone());
        }
    }
    if !missing_sequence_order.is_empty() && !cancel_requested {
        stage = BenchmarkJobStage::Paused;
        blockers.insert("event-sequence-partition".into());
    }
    if !conflicting_event_order.is_empty() {
        stage = BenchmarkJobStage::Blocked;
        blockers.insert("conflicting-idempotency-event".into());
    }
    let active_query_site_order = sites
        .iter()
        .filter(|site| site.active_query)
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let completed_site_order = sites
        .iter()
        .filter(|site| site.stage == BenchmarkJobSiteStage::Completed && !site.revoked)
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let eligible_sites = sites
        .iter()
        .filter(|site| site.approved && !site.revoked)
        .count();
    for site in &mut sites {
        site.blocker_order.sort();
        site.blocker_order.dedup();
    }
    let quorum_satisfied = eligible_sites >= request.quorum_sites;
    if !quorum_satisfied && !cancel_requested && stage != BenchmarkJobStage::Blocked {
        blockers.insert("quorum-revalidation-failed".into());
    }
    if stage != BenchmarkJobStage::Blocked
        && stage != BenchmarkJobStage::Cancelled
        && missing_sequence_order.is_empty()
        && quorum_satisfied
        && active_query_site_order.is_empty()
        && completed_site_order.len() >= request.quorum_sites
    {
        stage = BenchmarkJobStage::Completed;
    }
    let disposition = if stage == BenchmarkJobStage::Cancelled {
        BenchmarkJobDisposition::Cancelled
    } else if stage == BenchmarkJobStage::Blocked {
        BenchmarkJobDisposition::Blocked
    } else if !missing_sequence_order.is_empty() {
        BenchmarkJobDisposition::Partitioned
    } else if stage == BenchmarkJobStage::Completed {
        BenchmarkJobDisposition::Completed
    } else if !completed_site_order.is_empty() {
        BenchmarkJobDisposition::Partial
    } else if !events.is_empty() {
        BenchmarkJobDisposition::Running
    } else {
        BenchmarkJobDisposition::Queued
    };
    let next_action = match disposition {
        BenchmarkJobDisposition::Queued => {
            "append a signed creation or approval event; no query has started".into()
        }
        BenchmarkJobDisposition::Running => {
            "continue through institution-local adapters and persist the next checkpoint".into()
        }
        BenchmarkJobDisposition::Partial => {
            "resume remaining approved sites while preserving completed aggregate results".into()
        }
        BenchmarkJobDisposition::Completed => {
            "revalidate release, reproducibility, and scientific interpretation gates".into()
        }
        BenchmarkJobDisposition::Cancelled => {
            "retain the cancelled job and require an explicit new idempotency identity to restart"
                .into()
        }
        BenchmarkJobDisposition::Partitioned => {
            "obtain missing event sequences or site acknowledgements; partition is not success"
                .into()
        }
        BenchmarkJobDisposition::Blocked => {
            "hold dispatch and resolve budget, privacy, approval, or idempotency blockers locally"
                .into()
        }
    };
    let mut job = FederatedBenchmarkJob {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        job_id: request.job_id.clone(),
        objective: request.objective.clone(),
        idempotency_key: request.idempotency_key.clone(),
        stage,
        site_order: site_ids,
        sites,
        applied_event_order: applied_event_order.into_iter().collect(),
        duplicate_event_order: duplicate_event_order.into_iter().collect(),
        conflicting_event_order: conflicting_event_order.into_iter().collect(),
        ignored_event_order: ignored_event_order.into_iter().collect(),
        missing_sequence_order: missing_sequence_order.into_iter().collect(),
        blockers: blockers.into_iter().collect(),
        budget_units: request.budget_units,
        budget_spent_units: budget_spent,
        privacy_budget_milli: request.privacy_budget_milli,
        privacy_spent_milli: privacy_spent,
        active_query_site_order,
        completed_site_order,
        quorum_satisfied,
        dispatch_permitted: false,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-benchmark-job"),
    };
    job.digest = ContentHash::of_value(&digest_body(&job))
        .map_err(|error| BenchmarkJobError::Digest(error.to_string()))?;
    job.validate()?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn event(
        id: &str,
        sequence: u64,
        site_id: Option<&str>,
        kind: BenchmarkJobEventKind,
    ) -> BenchmarkJobEvent {
        BenchmarkJobEvent {
            event_id: id.into(),
            sequence,
            site_id: site_id.map(str::to_string),
            kind,
        }
    }

    fn request(events: Vec<BenchmarkJobEvent>) -> BenchmarkJobRequest {
        BenchmarkJobRequest {
            job_id: "job-a".into(),
            objective: "compare invasion phenotypes".into(),
            idempotency_key: "idem-a".into(),
            site_order: vec![
                BenchmarkJobSiteInput {
                    site_id: "site-a".into(),
                    initially_approved: false,
                    revoked: false,
                },
                BenchmarkJobSiteInput {
                    site_id: "site-b".into(),
                    initially_approved: false,
                    revoked: false,
                },
            ],
            quorum_sites: 2,
            max_query_sites: 2,
            max_retry_attempts: 2,
            max_events: 32,
            budget_units: 100,
            privacy_budget_milli: 100,
            events,
        }
    }

    #[test]
    fn approved_queries_complete_only_after_quorum() {
        let events = vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            event(
                "approve-a",
                2,
                Some("site-a"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "approve-b",
                3,
                Some("site-b"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "start-a",
                4,
                Some("site-a"),
                BenchmarkJobEventKind::QueryStarted,
            ),
            event(
                "start-b",
                5,
                Some("site-b"),
                BenchmarkJobEventKind::QueryStarted,
            ),
            event(
                "done-a",
                6,
                Some("site-a"),
                BenchmarkJobEventKind::QueryCompleted {
                    result_digest: digest("a"),
                    budget_cost_units: 10,
                    privacy_cost_milli: 10,
                },
            ),
            event(
                "done-b",
                7,
                Some("site-b"),
                BenchmarkJobEventKind::QueryCompleted {
                    result_digest: digest("b"),
                    budget_cost_units: 10,
                    privacy_cost_milli: 10,
                },
            ),
        ];
        let job = execute_glioma_benchmark_job(&request(events)).unwrap();
        assert_eq!(job.disposition, BenchmarkJobDisposition::Completed);
        assert!(job.quorum_satisfied);
        assert_eq!(job.completed_site_order, vec!["site-a", "site-b"]);
        assert_eq!(job.sites[0].query_count, 1);
        assert_eq!(job.sites[1].query_count, 1);
        assert!(!job.dispatch_permitted);
    }

    #[test]
    fn malformed_duplicate_identity_cannot_bypass_event_validation() {
        let mut events = vec![event(
            "approve-a",
            1,
            Some("site-a"),
            BenchmarkJobEventKind::ApprovalGranted,
        )];
        events.push(BenchmarkJobEvent {
            event_id: "approve-a".into(),
            sequence: 2,
            site_id: None,
            kind: BenchmarkJobEventKind::ApprovalGranted,
        });
        let error = execute_glioma_benchmark_job(&request(events)).unwrap_err();
        assert!(
            matches!(error, BenchmarkJobError::InvalidRequest(message) if message.contains("approve-a"))
        );
    }

    #[test]
    fn invalid_event_identity_is_rejected_instead_of_being_ignored() {
        let events = vec![BenchmarkJobEvent {
            event_id: "\n".into(),
            sequence: 1,
            site_id: None,
            kind: BenchmarkJobEventKind::Created,
        }];
        let error = execute_glioma_benchmark_job(&request(events)).unwrap_err();
        assert!(
            matches!(error, BenchmarkJobError::InvalidRequest(message) if message.contains("invalid identity"))
        );
    }

    #[test]
    fn identical_retries_are_idempotent_and_conflicts_block() {
        let original = event(
            "approve-a",
            2,
            Some("site-a"),
            BenchmarkJobEventKind::ApprovalGranted,
        );
        let mut events = vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            original.clone(),
            original.clone(),
        ];
        let job = execute_glioma_benchmark_job(&request(events.clone())).unwrap();
        assert_eq!(job.duplicate_event_order, vec!["approve-a"]);
        events.push(event(
            "approve-a",
            9,
            Some("site-a"),
            BenchmarkJobEventKind::ApprovalDenied {
                reason: "conflict".into(),
            },
        ));
        let blocked = execute_glioma_benchmark_job(&request(events)).unwrap();
        assert_eq!(blocked.disposition, BenchmarkJobDisposition::Blocked);
        assert_eq!(blocked.conflicting_event_order, vec!["approve-a"]);
    }

    #[test]
    fn missing_sequences_pause_without_claiming_success() {
        let events = vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            event(
                "approve-a",
                3,
                Some("site-a"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
        ];
        let job = execute_glioma_benchmark_job(&request(events)).unwrap();
        assert_eq!(job.disposition, BenchmarkJobDisposition::Partitioned);
        assert_eq!(job.missing_sequence_order, vec![2]);
        assert!(!job.quorum_satisfied);
    }

    #[test]
    fn revoked_site_cannot_query_and_budget_stop_is_explicit() {
        let events = vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            event(
                "approve-a",
                2,
                Some("site-a"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "withdraw-a",
                3,
                Some("site-a"),
                BenchmarkJobEventKind::Withdrawn {
                    reason: "site policy changed".into(),
                },
            ),
            event(
                "start-a",
                4,
                Some("site-a"),
                BenchmarkJobEventKind::QueryStarted,
            ),
        ];
        let job = execute_glioma_benchmark_job(&request(events)).unwrap();
        assert!(job
            .blockers
            .iter()
            .any(|blocker| blocker.contains("query-start-denied")));
        assert!(job.sites[0].revoked);
        let mut budget_request = request(vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            event(
                "approve-a",
                2,
                Some("site-a"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "approve-b",
                3,
                Some("site-b"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "start-a",
                4,
                Some("site-a"),
                BenchmarkJobEventKind::QueryStarted,
            ),
            event(
                "done-a",
                5,
                Some("site-a"),
                BenchmarkJobEventKind::QueryCompleted {
                    result_digest: digest("a"),
                    budget_cost_units: 101,
                    privacy_cost_milli: 1,
                },
            ),
        ]);
        budget_request.budget_units = 100;
        let stopped = execute_glioma_benchmark_job(&budget_request).unwrap();
        assert_eq!(stopped.disposition, BenchmarkJobDisposition::Blocked);
        assert!(stopped.blockers.contains(&"budget-exhausted".into()));
    }

    #[test]
    fn retry_cap_and_checkpoint_are_replayable() {
        let events = vec![
            event("create", 1, None, BenchmarkJobEventKind::Created),
            event(
                "approve-a",
                2,
                Some("site-a"),
                BenchmarkJobEventKind::ApprovalGranted,
            ),
            event(
                "checkpoint-a",
                3,
                Some("site-a"),
                BenchmarkJobEventKind::Checkpoint {
                    checkpoint_digest: digest("checkpoint"),
                },
            ),
            event(
                "retry-a",
                4,
                Some("site-a"),
                BenchmarkJobEventKind::RetryRequested,
            ),
            event(
                "retry-a-2",
                5,
                Some("site-a"),
                BenchmarkJobEventKind::RetryRequested,
            ),
            event(
                "retry-a-3",
                6,
                Some("site-a"),
                BenchmarkJobEventKind::RetryRequested,
            ),
        ];
        let job = execute_glioma_benchmark_job(&request(events)).unwrap();
        assert_eq!(job.disposition, BenchmarkJobDisposition::Blocked);
        assert_eq!(job.sites[0].retry_count, 2);
        assert!(job.sites[0].checkpoint_digest.is_some());
    }
}
