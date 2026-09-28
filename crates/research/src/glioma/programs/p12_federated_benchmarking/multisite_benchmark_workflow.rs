//! Resumable multi-site benchmark workflow orchestration for `GAF-GLIOMA-P12-F14`.
//!
//! This feature coordinates the research lifecycle around an aggregate-only benchmark without
//! pretending that a query, review, or release happened merely because a plan exists.  A typed
//! event stream advances each site through local validation, approval, aggregate query,
//! reconciliation, review, and release.  Duplicate event identities are idempotent, sequence
//! gaps remain visible as partitions, late withdrawal removes a site's aggregate from active
//! consensus, and failed approval cannot unlock a query.  Raw traces, credentials, human data,
//! and clinical decisions are permanently outside this contract.

use super::consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkRequest,
    FederatedBenchmarkSite,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaMultisiteBenchmarkWorkflow1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_EVENTS: usize = 4_096;
pub const MAX_REASON_LENGTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedWorkflowStage {
    LocalValidation,
    Approval,
    AggregateQuery,
    Reconciliation,
    Review,
    Release,
}

const STAGES: [FederatedWorkflowStage; 6] = [
    FederatedWorkflowStage::LocalValidation,
    FederatedWorkflowStage::Approval,
    FederatedWorkflowStage::AggregateQuery,
    FederatedWorkflowStage::Reconciliation,
    FederatedWorkflowStage::Review,
    FederatedWorkflowStage::Release,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedWorkflowStageState {
    Pending,
    Succeeded,
    Blocked,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkSitePolicyResponse {
    pub site_id: String,
    pub policy_version: String,
    pub approval_granted: bool,
    pub query_allowed: bool,
    pub release_allowed: bool,
    pub withdrawn: bool,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowSiteInput {
    pub site_id: String,
    pub study_id: String,
    pub policy: FederatedBenchmarkSitePolicyResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowBudget {
    pub max_events: usize,
    pub max_query_sites: usize,
    pub max_retry_attempts_per_stage: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkWorkflowEventKind {
    StageSucceeded {
        stage: FederatedWorkflowStage,
        aggregate: Option<FederatedBenchmarkSite>,
    },
    StageFailed {
        stage: FederatedWorkflowStage,
        retryable: bool,
        reason: String,
    },
    Withdrawn {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowEvent {
    pub event_id: String,
    pub sequence: u64,
    pub site_id: String,
    pub kind: FederatedBenchmarkWorkflowEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowStageRecord {
    pub stage: FederatedWorkflowStage,
    pub state: FederatedWorkflowStageState,
    pub attempts: u16,
    pub last_sequence: Option<u64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowSiteState {
    pub site_id: String,
    pub study_id: String,
    pub stage_order: Vec<FederatedBenchmarkWorkflowStageRecord>,
    pub withdrawn: bool,
    pub last_sequence: Option<u64>,
    pub blocker_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkWorkflowDisposition {
    Released,
    Complete,
    Partial,
    Partitioned,
    Blocked,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub site_order: Vec<String>,
    pub sites: Vec<FederatedBenchmarkWorkflowSiteState>,
    pub active_aggregate_site_order: Vec<String>,
    pub released_site_order: Vec<String>,
    pub applied_event_order: Vec<String>,
    pub ignored_event_order: Vec<String>,
    pub duplicate_event_order: Vec<String>,
    pub missing_event_sequence_order: Vec<u64>,
    pub retry_count: u32,
    pub query_count: u32,
    pub blockers: Vec<String>,
    pub pending_site_order: Vec<String>,
    pub consensus: Option<FederatedBenchmarkConsensus>,
    pub completion_conditions: BTreeMap<String, bool>,
    pub disposition: FederatedBenchmarkWorkflowDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkWorkflowError {
    #[error("multi-site benchmark workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-site benchmark workflow event is invalid: {0}")]
    InvalidEvent(String),
    #[error("multi-site benchmark workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-site benchmark workflow digest failed: {0}")]
    Digest(String),
}

fn bounded_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= MAX_REASON_LENGTH
        && !trimmed.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn stage_index(stage: FederatedWorkflowStage) -> usize {
    STAGES
        .iter()
        .position(|candidate| *candidate == stage)
        .expect("all workflow stages are listed")
}

fn digest_input(output: &FederatedBenchmarkWorkflowRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "site_order": output.site_order,
        "sites": output.sites,
        "active_aggregate_site_order": output.active_aggregate_site_order,
        "released_site_order": output.released_site_order,
        "applied_event_order": output.applied_event_order,
        "ignored_event_order": output.ignored_event_order,
        "duplicate_event_order": output.duplicate_event_order,
        "missing_event_sequence_order": output.missing_event_sequence_order,
        "retry_count": output.retry_count,
        "query_count": output.query_count,
        "blockers": output.blockers,
        "pending_site_order": output.pending_site_order,
        "consensus": output.consensus,
        "completion_conditions": output.completion_conditions,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

impl FederatedBenchmarkWorkflowRun {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !bounded_text(&self.objective)
            || !canonical(&self.site_order)
            || !canonical(&self.active_aggregate_site_order)
            || !canonical(&self.released_site_order)
            || !canonical(&self.applied_event_order)
            || !canonical(&self.ignored_event_order)
            || !canonical(&self.duplicate_event_order)
            || !canonical(&self.missing_event_sequence_order)
            || !canonical(&self.blockers)
            || !canonical(&self.pending_site_order)
            || self.sites.len() != self.site_order.len()
            || self
                .sites
                .iter()
                .map(|site| site.site_id.clone())
                .collect::<Vec<_>>()
                != self.site_order
            || self.digest.as_str().len() != 64
        {
            return Err(FederatedBenchmarkWorkflowError::InvalidOutput(
                "identity, canonical ordering, site binding, or digest bounds are invalid".into(),
            ));
        }
        let mut site_ids = BTreeSet::new();
        let mut released = BTreeSet::new();
        let mut pending = BTreeSet::new();
        for site in &self.sites {
            if !bounded_text(&site.site_id)
                || !bounded_text(&site.study_id)
                || !site_ids.insert(site.site_id.clone())
                || site.stage_order.len() != STAGES.len()
                || site
                    .stage_order
                    .iter()
                    .map(|record| record.stage)
                    .collect::<Vec<_>>()
                    != STAGES
                || !canonical(&site.blocker_order)
            {
                return Err(FederatedBenchmarkWorkflowError::InvalidOutput(
                    "site identity, stage topology, or blocker ordering is invalid".into(),
                ));
            }
            let last = site.last_sequence;
            for record in &site.stage_order {
                if record.attempts == 0
                    && !matches!(
                        record.state,
                        FederatedWorkflowStageState::Pending
                            | FederatedWorkflowStageState::Withdrawn
                    )
                    || record.last_sequence > last
                    || record
                        .reason
                        .as_ref()
                        .is_some_and(|reason| !bounded_text(reason))
                {
                    return Err(FederatedBenchmarkWorkflowError::InvalidOutput(
                        "stage attempts, sequence binding, or reason is invalid".into(),
                    ));
                }
            }
            if !site.withdrawn
                && site.stage_order.iter().any(|record| {
                    record.state == FederatedWorkflowStageState::Succeeded
                        && record.stage == FederatedWorkflowStage::Release
                })
            {
                released.insert(site.site_id.clone());
            }
            if !site.withdrawn
                && site
                    .stage_order
                    .iter()
                    .any(|record| record.state == FederatedWorkflowStageState::Pending)
            {
                pending.insert(site.site_id.clone());
            }
        }
        if released.iter().cloned().collect::<Vec<_>>() != self.released_site_order
            || pending.iter().cloned().collect::<Vec<_>>() != self.pending_site_order
            || !self
                .active_aggregate_site_order
                .iter()
                .all(|site_id| site_ids.contains(site_id))
        {
            return Err(FederatedBenchmarkWorkflowError::InvalidOutput(
                "released, pending, or active aggregate partitions do not reconcile".into(),
            ));
        }
        if let Some(consensus) = &self.consensus {
            consensus.validate().map_err(|error| {
                FederatedBenchmarkWorkflowError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkWorkflowError::InvalidOutput(
                "workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_benchmark_request(
    request: &FederatedBenchmarkRequest,
) -> Result<(), FederatedBenchmarkWorkflowError> {
    if !bounded_text(&request.objective)
        || !bounded_text(&request.capability_id)
        || !bounded_text(&request.benchmark_world)
        || !bounded_text(&request.metric_name)
        || request.minimum_sites == 0
        || request.minimum_replicates_per_site == 0
        || request.effect_threshold_milli == 0
        || request.min_signal_to_noise_milli == 0
        || request.max_i2_milli > 1_000
        || request.max_site_spread_milli == 0
        || request.max_leave_one_out_shift_milli == 0
    {
        return Err(FederatedBenchmarkWorkflowError::InvalidRequest(
            "bounded benchmark identity and positive quorum/quality thresholds are required".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &FederatedBenchmarkWorkflowRequest,
) -> Result<(), FederatedBenchmarkWorkflowError> {
    validate_benchmark_request(&request.benchmark)?;
    if request.sites.is_empty()
        || request.sites.len() > MAX_SITES
        || request.events.len() > request.budget.max_events.min(MAX_EVENTS)
        || request.budget.max_events == 0
        || request.budget.max_events > MAX_EVENTS
        || request.budget.max_query_sites == 0
        || request.budget.max_query_sites > MAX_SITES
        || request.budget.max_retry_attempts_per_stage == 0
    {
        return Err(FederatedBenchmarkWorkflowError::InvalidRequest(
            "bounded sites, event budget, query budget, and retry budget are required".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut policies = BTreeSet::new();
    for site in &request.sites {
        if !bounded_text(&site.site_id)
            || !bounded_text(&site.study_id)
            || !sites.insert(site.site_id.clone())
            || !studies.insert(site.study_id.clone())
            || site.policy.site_id != site.site_id
            || !bounded_text(&site.policy.policy_version)
            || site
                .policy
                .rationale
                .as_ref()
                .is_some_and(|reason| !bounded_text(reason))
            || !policies.insert(site.policy.site_id.clone())
        {
            return Err(FederatedBenchmarkWorkflowError::InvalidRequest(
                "site, study, and policy identities must be unique and bounded".into(),
            ));
        }
    }
    let mut event_ids = BTreeMap::new();
    for event in &request.events {
        if !bounded_text(&event.event_id)
            || !bounded_text(&event.site_id)
            || event.sequence == 0
            || !sites.contains(&event.site_id)
        {
            return Err(FederatedBenchmarkWorkflowError::InvalidEvent(
                "event identity, sequence, and site binding are invalid".into(),
            ));
        }
        if let Some(previous) = event_ids.insert(event.event_id.clone(), event.clone()) {
            if previous != *event {
                return Err(FederatedBenchmarkWorkflowError::InvalidEvent(
                    "duplicate event identity has conflicting content".into(),
                ));
            }
        }
        match &event.kind {
            FederatedBenchmarkWorkflowEventKind::StageSucceeded { aggregate, .. } => {
                if let Some(aggregate) = aggregate {
                    aggregate.artifact.validate().map_err(|error| {
                        FederatedBenchmarkWorkflowError::InvalidEvent(error.to_string())
                    })?;
                }
            }
            FederatedBenchmarkWorkflowEventKind::StageFailed { reason, .. }
            | FederatedBenchmarkWorkflowEventKind::Withdrawn { reason } => {
                if !bounded_text(reason) {
                    return Err(FederatedBenchmarkWorkflowError::InvalidEvent(
                        "event reason must be bounded and non-empty".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn stage_record_mut(
    site: &mut FederatedBenchmarkWorkflowSiteState,
    stage: FederatedWorkflowStage,
) -> &mut FederatedBenchmarkWorkflowStageRecord {
    &mut site.stage_order[stage_index(stage)]
}

fn stage_state(
    site: &FederatedBenchmarkWorkflowSiteState,
    stage: FederatedWorkflowStage,
) -> FederatedWorkflowStageState {
    site.stage_order[stage_index(stage)].state
}

fn prerequisite_succeeded(
    site: &FederatedBenchmarkWorkflowSiteState,
    stage: FederatedWorkflowStage,
) -> bool {
    let index = stage_index(stage);
    index == 0
        || site.stage_order[..index]
            .iter()
            .all(|record| record.state == FederatedWorkflowStageState::Succeeded)
}

fn stage_label(stage: FederatedWorkflowStage) -> &'static str {
    match stage {
        FederatedWorkflowStage::LocalValidation => "local_validation",
        FederatedWorkflowStage::Approval => "approval",
        FederatedWorkflowStage::AggregateQuery => "aggregate_query",
        FederatedWorkflowStage::Reconciliation => "reconciliation",
        FederatedWorkflowStage::Review => "review",
        FederatedWorkflowStage::Release => "release",
    }
}

/// Apply a bounded, replayable multi-site workflow event stream.
pub fn execute_glioma_multisite_benchmark_workflow(
    request: &FederatedBenchmarkWorkflowRequest,
) -> Result<FederatedBenchmarkWorkflowRun, FederatedBenchmarkWorkflowError> {
    validate_request(request)?;
    let mut inputs = request.sites.clone();
    inputs.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = inputs
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let policies = inputs
        .iter()
        .map(|site| (site.site_id.clone(), site.policy.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut states = inputs
        .iter()
        .map(|site| FederatedBenchmarkWorkflowSiteState {
            site_id: site.site_id.clone(),
            study_id: site.study_id.clone(),
            stage_order: STAGES
                .iter()
                .copied()
                .map(|stage| FederatedBenchmarkWorkflowStageRecord {
                    stage,
                    state: FederatedWorkflowStageState::Pending,
                    attempts: 0,
                    last_sequence: None,
                    reason: None,
                })
                .collect(),
            withdrawn: site.policy.withdrawn,
            last_sequence: None,
            blocker_order: Vec::new(),
        })
        .collect::<Vec<_>>();
    for state in &mut states {
        if state.withdrawn {
            for record in &mut state.stage_order {
                record.state = FederatedWorkflowStageState::Withdrawn;
                record.reason = Some("site-withdrawn-before-workflow".into());
            }
        }
    }
    let mut aggregate_by_site = BTreeMap::<String, FederatedBenchmarkSite>::new();
    let mut unique_events = BTreeMap::<String, FederatedBenchmarkWorkflowEvent>::new();
    let mut duplicate_event_order = BTreeSet::new();
    for event in &request.events {
        if unique_events.contains_key(&event.event_id) {
            duplicate_event_order.insert(event.event_id.clone());
        } else {
            unique_events.insert(event.event_id.clone(), event.clone());
        }
    }
    let mut events = unique_events.into_values().collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let mut missing_event_sequence_order = BTreeSet::new();
    if let (Some(first), Some(last)) = (events.first(), events.last()) {
        let observed = events
            .iter()
            .map(|event| event.sequence)
            .collect::<BTreeSet<_>>();
        for sequence in first.sequence..=last.sequence {
            if !observed.contains(&sequence) {
                missing_event_sequence_order.insert(sequence);
            }
        }
    }
    let mut applied_event_order = BTreeSet::new();
    let mut ignored_event_order = BTreeSet::new();
    let mut blockers = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut query_count = 0_u32;
    for event in events {
        let site_index = states
            .iter()
            .position(|site| site.site_id == event.site_id)
            .expect("validated event site exists");
        let site = &mut states[site_index];
        site.last_sequence = Some(site.last_sequence.unwrap_or(0).max(event.sequence));
        let policy = policies
            .get(&site.site_id)
            .expect("validated policy exists");
        match event.kind {
            FederatedBenchmarkWorkflowEventKind::Withdrawn { reason } => {
                site.withdrawn = true;
                for record in &mut site.stage_order {
                    if record.state == FederatedWorkflowStageState::Pending {
                        record.state = FederatedWorkflowStageState::Withdrawn;
                        record.last_sequence = Some(event.sequence);
                        record.reason = Some(reason.clone());
                    }
                }
                site.blocker_order.push(format!("withdrawn:{reason}"));
                applied_event_order.insert(event.event_id);
            }
            FederatedBenchmarkWorkflowEventKind::StageSucceeded { stage, aggregate } => {
                let withdrawn = site.withdrawn;
                let prerequisites_ready = prerequisite_succeeded(site, stage);
                let current_state = stage_state(site, stage);
                let query_budget_exhausted = stage == FederatedWorkflowStage::AggregateQuery
                    && query_count >= request.budget.max_query_sites as u32;
                if withdrawn
                    || !prerequisites_ready
                    || current_state != FederatedWorkflowStageState::Pending
                    || (stage == FederatedWorkflowStage::Approval && !policy.approval_granted)
                    || (stage == FederatedWorkflowStage::AggregateQuery && !policy.query_allowed)
                    || (stage == FederatedWorkflowStage::Release && !policy.release_allowed)
                    || (stage == FederatedWorkflowStage::AggregateQuery
                        && (aggregate.is_none() || query_budget_exhausted))
                {
                    let reason = if withdrawn {
                        "site-withdrawn"
                    } else if !prerequisites_ready {
                        "prerequisite-not-succeeded"
                    } else if stage == FederatedWorkflowStage::Approval && !policy.approval_granted
                    {
                        "approval-denied-by-site-policy"
                    } else if stage == FederatedWorkflowStage::AggregateQuery
                        && !policy.query_allowed
                    {
                        "aggregate-query-denied-by-site-policy"
                    } else if stage == FederatedWorkflowStage::Release && !policy.release_allowed {
                        "release-denied-by-site-policy"
                    } else if query_budget_exhausted {
                        "query-budget-exhausted"
                    } else {
                        "duplicate-or-stale-stage-success"
                    };
                    site.blocker_order
                        .push(format!("{}:{reason}", stage_label(stage)));
                    blockers.insert(format!("{}:{}", site.site_id, reason));
                    ignored_event_order.insert(event.event_id);
                    continue;
                }
                {
                    let record = stage_record_mut(site, stage);
                    record.state = FederatedWorkflowStageState::Succeeded;
                    record.attempts = record.attempts.saturating_add(1);
                    record.last_sequence = Some(event.sequence);
                    record.reason = None;
                }
                if stage == FederatedWorkflowStage::AggregateQuery {
                    let aggregate = aggregate.expect("query success requires aggregate");
                    if aggregate.site_id != site.site_id || aggregate.study_id != site.study_id {
                        let record = stage_record_mut(site, stage);
                        record.state = FederatedWorkflowStageState::Blocked;
                        record.reason = Some("aggregate-site-identity-mismatch".into());
                        site.blocker_order
                            .push("aggregate-site-identity-mismatch".into());
                        blockers
                            .insert(format!("{}:aggregate-site-identity-mismatch", site.site_id));
                        ignored_event_order.insert(event.event_id);
                        continue;
                    }
                    aggregate_by_site.insert(site.site_id.clone(), aggregate);
                    query_count = query_count.saturating_add(1);
                } else if aggregate.is_some() {
                    let record = stage_record_mut(site, stage);
                    record.state = FederatedWorkflowStageState::Blocked;
                    record.reason = Some("aggregate-attached-to-non-query-stage".into());
                    site.blocker_order
                        .push("aggregate-attached-to-non-query-stage".into());
                    blockers.insert(format!(
                        "{}:aggregate-attached-to-non-query-stage",
                        site.site_id
                    ));
                    ignored_event_order.insert(event.event_id);
                    continue;
                }
                applied_event_order.insert(event.event_id);
            }
            FederatedBenchmarkWorkflowEventKind::StageFailed {
                stage,
                retryable,
                reason,
            } => {
                let current_state = stage_state(site, stage);
                if site.withdrawn || current_state == FederatedWorkflowStageState::Withdrawn {
                    // Withdrawal is a terminal local-authority decision. A delayed worker
                    // failure must remain visible for audit, but it cannot rewrite the site's
                    // withdrawn stage into a blocked or retryable state.
                    let blocker = format!("{}:late-failure-after-withdrawal", stage_label(stage));
                    site.blocker_order.push(blocker.clone());
                    blockers.insert(format!("{}:{blocker}", site.site_id));
                    ignored_event_order.insert(event.event_id);
                    continue;
                }
                if matches!(
                    current_state,
                    FederatedWorkflowStageState::Succeeded | FederatedWorkflowStageState::Blocked
                ) {
                    // A stage that already reached a terminal result cannot be reopened by a
                    // late failure. Treat the contradiction as an explicit blocker instead of
                    // mutating a successful stage or consuming retry capacity.
                    let blocker =
                        format!("{}:late-failure-after-terminal-state", stage_label(stage));
                    site.blocker_order.push(blocker.clone());
                    blockers.insert(format!("{}:{blocker}", site.site_id));
                    ignored_event_order.insert(event.event_id);
                    continue;
                }
                let attempts = {
                    let record = stage_record_mut(site, stage);
                    record.attempts = record.attempts.saturating_add(1);
                    record.last_sequence = Some(event.sequence);
                    record.reason = Some(reason.clone());
                    record.attempts
                };
                if retryable && attempts <= request.budget.max_retry_attempts_per_stage {
                    retry_count = retry_count.saturating_add(1);
                    applied_event_order.insert(event.event_id);
                } else {
                    stage_record_mut(site, stage).state = FederatedWorkflowStageState::Blocked;
                    site.blocker_order
                        .push(format!("{}:{reason}", stage_label(stage)));
                    blockers.insert(format!("{}:{}", site.site_id, reason));
                    applied_event_order.insert(event.event_id);
                }
            }
        }
    }
    for site in &mut states {
        site.blocker_order.sort();
        site.blocker_order.dedup();
    }
    let active_aggregate_site_order = aggregate_by_site
        .keys()
        .filter(|site_id| {
            states
                .iter()
                .find(|site| &site.site_id == *site_id)
                .is_some_and(|site| !site.withdrawn)
        })
        .cloned()
        .collect::<Vec<_>>();
    let active_aggregates = active_aggregate_site_order
        .iter()
        .filter_map(|site_id| aggregate_by_site.get(site_id).cloned())
        .collect::<Vec<_>>();
    let consensus = if active_aggregates.is_empty() {
        None
    } else {
        Some(
            analyze_federated_benchmark(&request.benchmark, &active_aggregates).map_err(
                |error| FederatedBenchmarkWorkflowError::InvalidOutput(error.to_string()),
            )?,
        )
    };
    let released_site_order = states
        .iter()
        .filter(|site| {
            !site.withdrawn
                && stage_state(site, FederatedWorkflowStage::Release)
                    == FederatedWorkflowStageState::Succeeded
        })
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let pending_site_order = states
        .iter()
        .filter(|site| {
            !site.withdrawn
                && site
                    .stage_order
                    .iter()
                    .any(|record| record.state == FederatedWorkflowStageState::Pending)
        })
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let missing = missing_event_sequence_order
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let blockers = blockers.into_iter().collect::<Vec<_>>();
    let non_withdrawn_site_count = states.iter().filter(|site| !site.withdrawn).count();
    let completion_conditions = BTreeMap::from([
        (
            "all_non_withdrawn_sites_released".into(),
            non_withdrawn_site_count > 0
                && released_site_order.len() == non_withdrawn_site_count
                && pending_site_order.is_empty()
                && blockers.is_empty(),
        ),
        (
            "minimum_active_aggregate_sites".into(),
            active_aggregate_site_order.len() >= request.benchmark.minimum_sites,
        ),
        ("no_event_partition_gap".into(), missing.is_empty()),
        ("no_site_blocker".into(), blockers.is_empty()),
    ]);
    let disposition = if non_withdrawn_site_count == 0 {
        FederatedBenchmarkWorkflowDisposition::Withdrawn
    } else if !missing.is_empty() {
        FederatedBenchmarkWorkflowDisposition::Partitioned
    } else if !blockers.is_empty() {
        FederatedBenchmarkWorkflowDisposition::Blocked
    } else if !pending_site_order.is_empty() {
        FederatedBenchmarkWorkflowDisposition::Partial
    } else if !released_site_order.is_empty() {
        FederatedBenchmarkWorkflowDisposition::Released
    } else {
        FederatedBenchmarkWorkflowDisposition::Complete
    };
    let next_action = match disposition {
        FederatedBenchmarkWorkflowDisposition::Released => {
            "publish the reviewed aggregate-only result through the release gate".into()
        }
        FederatedBenchmarkWorkflowDisposition::Complete => {
            "review the completed workflow and decide whether the benchmark is publishable".into()
        }
        FederatedBenchmarkWorkflowDisposition::Partial => {
            "resume pending site stages; no incomplete stage is evidence of completion".into()
        }
        FederatedBenchmarkWorkflowDisposition::Partitioned => {
            "reconcile the missing event sequence before evaluating completion".into()
        }
        FederatedBenchmarkWorkflowDisposition::Blocked => {
            "resolve policy, identity, budget, withdrawal, or stage blockers before retrying".into()
        }
        FederatedBenchmarkWorkflowDisposition::Withdrawn => {
            "no participating site remains; obtain a new authorized contribution before restarting"
                .into()
        }
    };
    let mut output = FederatedBenchmarkWorkflowRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.benchmark.objective.clone(),
        site_order,
        sites: states,
        active_aggregate_site_order,
        released_site_order,
        applied_event_order: applied_event_order.into_iter().collect(),
        ignored_event_order: ignored_event_order.into_iter().collect(),
        duplicate_event_order: duplicate_event_order.into_iter().collect(),
        missing_event_sequence_order: missing,
        retry_count,
        query_count,
        blockers,
        pending_site_order,
        consensus,
        completion_conditions,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multisite-benchmark-workflow"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkWorkflowRequest {
    pub benchmark: FederatedBenchmarkRequest,
    pub sites: Vec<FederatedBenchmarkWorkflowSiteInput>,
    pub events: Vec<FederatedBenchmarkWorkflowEvent>,
    pub budget: FederatedBenchmarkWorkflowBudget,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p12_federated_benchmarking::consensus::FederatedBenchmarkSite;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).expect("hash")
    }

    fn benchmark() -> FederatedBenchmarkRequest {
        FederatedBenchmarkRequest {
            objective: "compare glioma invasion workflow across sites".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            minimum_replicates_per_site: 3,
            effect_threshold_milli: 20,
            max_i2_milli: 900,
            min_signal_to_noise_milli: 1,
            max_site_spread_milli: 1_000,
            max_leave_one_out_shift_milli: 1_000,
        }
    }

    fn aggregate(site_id: &str, study_id: &str, score: u64) -> FederatedBenchmarkSite {
        FederatedBenchmarkSite {
            site_id: site_id.into(),
            study_id: study_id.into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{site_id}"),
                content_hash: hash(site_id),
                content_type: "application/vnd.aurora.glioma.aggregate+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: 30,
            replicate_count: 4,
        }
    }

    fn request(events: Vec<FederatedBenchmarkWorkflowEvent>) -> FederatedBenchmarkWorkflowRequest {
        FederatedBenchmarkWorkflowRequest {
            benchmark: benchmark(),
            sites: vec![
                FederatedBenchmarkWorkflowSiteInput {
                    site_id: "site-a".into(),
                    study_id: "study-a".into(),
                    policy: FederatedBenchmarkSitePolicyResponse {
                        site_id: "site-a".into(),
                        policy_version: "policy-1".into(),
                        approval_granted: true,
                        query_allowed: true,
                        release_allowed: true,
                        withdrawn: false,
                        rationale: None,
                    },
                },
                FederatedBenchmarkWorkflowSiteInput {
                    site_id: "site-b".into(),
                    study_id: "study-b".into(),
                    policy: FederatedBenchmarkSitePolicyResponse {
                        site_id: "site-b".into(),
                        policy_version: "policy-1".into(),
                        approval_granted: true,
                        query_allowed: true,
                        release_allowed: true,
                        withdrawn: false,
                        rationale: None,
                    },
                },
            ],
            events,
            budget: FederatedBenchmarkWorkflowBudget {
                max_events: 128,
                max_query_sites: 8,
                max_retry_attempts_per_stage: 2,
            },
        }
    }

    fn event(
        id: &str,
        sequence: u64,
        site_id: &str,
        kind: FederatedBenchmarkWorkflowEventKind,
    ) -> FederatedBenchmarkWorkflowEvent {
        FederatedBenchmarkWorkflowEvent {
            event_id: id.into(),
            sequence,
            site_id: site_id.into(),
            kind,
        }
    }

    fn happy_events(
        site_id: &str,
        study_id: &str,
        start: u64,
    ) -> Vec<FederatedBenchmarkWorkflowEvent> {
        vec![
            event(
                &format!("{site_id}-validate"),
                start,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::LocalValidation,
                    aggregate: None,
                },
            ),
            event(
                &format!("{site_id}-approve"),
                start + 1,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Approval,
                    aggregate: None,
                },
            ),
            event(
                &format!("{site_id}-query"),
                start + 2,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::AggregateQuery,
                    aggregate: Some(aggregate(site_id, study_id, 620)),
                },
            ),
            event(
                &format!("{site_id}-reconcile"),
                start + 3,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Reconciliation,
                    aggregate: None,
                },
            ),
            event(
                &format!("{site_id}-review"),
                start + 4,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Review,
                    aggregate: None,
                },
            ),
            event(
                &format!("{site_id}-release"),
                start + 5,
                site_id,
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Release,
                    aggregate: None,
                },
            ),
        ]
    }

    #[test]
    fn happy_path_releases_two_independent_sites() {
        let mut events = happy_events("site-a", "study-a", 1);
        events.extend(happy_events("site-b", "study-b", 7));
        let output =
            execute_glioma_multisite_benchmark_workflow(&request(events)).expect("workflow");
        assert_eq!(
            output.disposition,
            FederatedBenchmarkWorkflowDisposition::Released
        );
        assert_eq!(output.released_site_order, vec!["site-a", "site-b"]);
        assert_eq!(output.active_aggregate_site_order, vec!["site-a", "site-b"]);
        assert_eq!(output.query_count, 2);
        assert!(output.consensus.is_some());
        output.validate().expect("valid workflow");
    }

    #[test]
    fn denied_approval_cannot_unlock_query() {
        let mut request = request(vec![
            event(
                "validate",
                1,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::LocalValidation,
                    aggregate: None,
                },
            ),
            event(
                "approve",
                2,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Approval,
                    aggregate: None,
                },
            ),
            event(
                "query",
                3,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::AggregateQuery,
                    aggregate: Some(aggregate("site-a", "study-a", 650)),
                },
            ),
        ]);
        request.sites[0].policy.approval_granted = false;
        let output = execute_glioma_multisite_benchmark_workflow(&request).expect("workflow");
        assert_eq!(output.query_count, 0);
        assert!(output.ignored_event_order.contains(&"approve".into()));
        assert!(output
            .blockers
            .iter()
            .any(|value| value.contains("approval-denied")));
        assert_eq!(
            output.sites[0].stage_order[2].state,
            FederatedWorkflowStageState::Pending
        );
    }

    #[test]
    fn late_withdrawal_removes_an_already_queried_site() {
        let mut events = happy_events("site-a", "study-a", 1);
        events.truncate(4);
        events.push(event(
            "withdraw-late",
            7,
            "site-a",
            FederatedBenchmarkWorkflowEventKind::Withdrawn {
                reason: "site-local review withdrew contribution".into(),
            },
        ));
        let mut workflow_request = request(events);
        workflow_request.sites.truncate(1);
        let output =
            execute_glioma_multisite_benchmark_workflow(&workflow_request).expect("workflow");
        assert!(output.active_aggregate_site_order.is_empty());
        assert!(output.released_site_order.is_empty());
        assert!(output.sites[0].withdrawn);
        assert_eq!(output.consensus, None);
        assert_eq!(
            output.disposition,
            FederatedBenchmarkWorkflowDisposition::Withdrawn
        );
    }

    #[test]
    fn duplicate_events_are_idempotent_and_partition_is_visible() {
        let mut events = happy_events("site-a", "study-a", 1);
        events.truncate(4);
        events.push(events[2].clone());
        events[3].sequence = 9;
        let left = execute_glioma_multisite_benchmark_workflow(&request(events.clone()))
            .expect("workflow");
        events.reverse();
        let right =
            execute_glioma_multisite_benchmark_workflow(&request(events)).expect("workflow");
        assert_eq!(left.digest, right.digest);
        assert_eq!(left.duplicate_event_order, vec!["site-a-query"]);
        assert_eq!(left.missing_event_sequence_order, vec![4, 5, 6, 7, 8]);
        assert_eq!(
            left.disposition,
            FederatedBenchmarkWorkflowDisposition::Partitioned
        );
    }

    #[test]
    fn retryable_failure_then_success_is_counted_once() {
        let events = vec![
            event(
                "validate-fail",
                1,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageFailed {
                    stage: FederatedWorkflowStage::LocalValidation,
                    retryable: true,
                    reason: "temporary local worker partition".into(),
                },
            ),
            event(
                "validate-ok",
                2,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::LocalValidation,
                    aggregate: None,
                },
            ),
        ];
        let output =
            execute_glioma_multisite_benchmark_workflow(&request(events)).expect("workflow");
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.sites[0].stage_order[0].attempts, 2);
        assert_eq!(
            output.sites[0].stage_order[0].state,
            FederatedWorkflowStageState::Succeeded
        );
    }

    #[test]
    fn aggregate_identity_mismatch_is_blocked() {
        let events = vec![
            event(
                "validate",
                1,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::LocalValidation,
                    aggregate: None,
                },
            ),
            event(
                "approve",
                2,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::Approval,
                    aggregate: None,
                },
            ),
            event(
                "query",
                3,
                "site-a",
                FederatedBenchmarkWorkflowEventKind::StageSucceeded {
                    stage: FederatedWorkflowStage::AggregateQuery,
                    aggregate: Some(aggregate("site-b", "study-b", 650)),
                },
            ),
        ];
        let output =
            execute_glioma_multisite_benchmark_workflow(&request(events)).expect("workflow");
        assert!(output
            .blockers
            .iter()
            .any(|value| value.contains("identity-mismatch")));
        assert_eq!(output.query_count, 0);
        assert_eq!(
            output.sites[0].stage_order[2].state,
            FederatedWorkflowStageState::Blocked
        );
    }

    #[test]
    fn late_failure_cannot_reopen_a_succeeded_stage() {
        let mut events = happy_events("site-a", "study-a", 1);
        events.push(event(
            "late-review-failure",
            7,
            "site-a",
            FederatedBenchmarkWorkflowEventKind::StageFailed {
                stage: FederatedWorkflowStage::Review,
                retryable: true,
                reason: "stale worker report".into(),
            },
        ));
        let mut workflow_request = request(events);
        workflow_request.sites.truncate(1);
        let output =
            execute_glioma_multisite_benchmark_workflow(&workflow_request).expect("workflow");
        assert_eq!(
            output.sites[0].stage_order[4].state,
            FederatedWorkflowStageState::Succeeded
        );
        assert_eq!(output.retry_count, 0);
        assert!(output
            .ignored_event_order
            .contains(&"late-review-failure".into()));
        assert!(output
            .blockers
            .iter()
            .any(|value| value.contains("late-failure-after-terminal-state")));
    }

    #[test]
    fn late_failure_cannot_rewrite_withdrawn_stage_state() {
        let mut events = happy_events("site-a", "study-a", 1);
        events.truncate(2);
        events.push(event(
            "withdraw",
            3,
            "site-a",
            FederatedBenchmarkWorkflowEventKind::Withdrawn {
                reason: "site-local withdrawal".into(),
            },
        ));
        events.push(event(
            "late-query-failure",
            4,
            "site-a",
            FederatedBenchmarkWorkflowEventKind::StageFailed {
                stage: FederatedWorkflowStage::AggregateQuery,
                retryable: false,
                reason: "worker disconnected".into(),
            },
        ));
        let mut workflow_request = request(events);
        workflow_request.sites.truncate(1);
        let output =
            execute_glioma_multisite_benchmark_workflow(&workflow_request).expect("workflow");
        assert!(output.sites[0].withdrawn);
        assert!(output.sites[0]
            .stage_order
            .iter()
            .skip(2)
            .all(|record| record.state == FederatedWorkflowStageState::Withdrawn));
        assert!(output.sites[0]
            .stage_order
            .iter()
            .take(2)
            .all(|record| record.state == FederatedWorkflowStageState::Succeeded));
        assert_eq!(output.retry_count, 0);
        assert!(output
            .ignored_event_order
            .contains(&"late-query-failure".into()));
    }
}
