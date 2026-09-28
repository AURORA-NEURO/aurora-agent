//! Prospective, budgeted release-workflow queue for high-throughput preclinical studies.
//!
//! GAF-GLIOMA-P11-F15 preflights every queued P11-F13 request before calling the shared local
//! replay executor, then processes admitted studies in deterministic priority order. The queue is
//! serialized because the executor is caller-owned and mutable. It enforces a single batch replay
//! budget, skips jobs that cannot meet their minimum admitted budget, and stops if an executor
//! result leaves resource accounting uncertain.

use super::dependency_closure::DependencyClosureDisposition;
use super::local_release_workflow::{
    GliomaLocalReleaseWorkflowDisposition, GliomaLocalReleaseWorkflowError,
    GliomaLocalReleaseWorkflowRequest, PreparedLocalReleaseWorkflow,
    execute_prepared_local_release_workflow, prepare_local_release_workflow,
};
use super::replay::ReplayCampaignExecutor;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F15";
pub const INPUT_SCHEMA: &str = "GliomaResearchObjectReleaseBatchRequest1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectReleaseBatch1@1";
pub const MAX_BATCH_ITEMS: usize = 128;
pub const MAX_BATCH_REQUEST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchItem {
    pub job_id: String,
    pub priority: u16,
    /// Unique caller sequence used to make equal-priority ordering stable.
    pub submission_order: u64,
    pub workflow: GliomaLocalReleaseWorkflowRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchRequest {
    pub batch_id: String,
    pub total_budget_units: u64,
    pub max_workflows: u16,
    pub items: Vec<ReleaseBatchItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchItemDisposition {
    WorkflowProduced,
    DeferredBudget,
    DeferredWorkflowLimit,
    WorkflowFailed,
    NotAttemptedAfterFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchFailureStage {
    WorkflowFinalization,
    ReplayExecutionOrFinalization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchItemResult {
    pub job_id: String,
    pub priority: u16,
    pub submission_order: u64,
    pub disposition: ReleaseBatchItemDisposition,
    pub package_digest: ContentHash,
    pub dependency_closure_digest: ContentHash,
    pub budget_before_units: Option<u64>,
    pub budget_allocation_units: u64,
    pub budget_spent_units: Option<u64>,
    pub workflow_digest: Option<ContentHash>,
    pub workflow_disposition: Option<GliomaLocalReleaseWorkflowDisposition>,
    pub failure_stage: Option<ReleaseBatchFailureStage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchStopReason {
    QueueDrained,
    BudgetLimit,
    WorkflowLimit,
    WorkflowFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseBatchDisposition {
    Complete,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchCounts {
    pub submitted: u16,
    pub workflow_produced: u16,
    pub deferred_budget: u16,
    pub deferred_workflow_limit: u16,
    pub workflow_failed: u16,
    pub not_attempted: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBatchReport {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub total_budget_units: u64,
    pub accounted_spend_units: u64,
    pub remaining_budget_units: Option<u64>,
    pub budget_accounting_complete: bool,
    /// Results are listed in deterministic priority, submission, and job-id order.
    pub results: Vec<ReleaseBatchItemResult>,
    pub counts: ReleaseBatchCounts,
    pub stop_reason: ReleaseBatchStopReason,
    pub disposition: ReleaseBatchDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseBatchError {
    #[error("release batch request is invalid: {0}")]
    InvalidRequest(String),
    #[error("a preflighted local release workflow failed: {0}")]
    Workflow(#[from] GliomaLocalReleaseWorkflowError),
    #[error("release batch report is invalid: {0}")]
    InvalidOutput(String),
    #[error("release batch digest failed: {0}")]
    Digest(String),
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@".contains(&byte))
}

fn schedule_order(left: &ReleaseBatchItem, right: &ReleaseBatchItem) -> std::cmp::Ordering {
    right
        .priority
        .cmp(&left.priority)
        .then_with(|| left.submission_order.cmp(&right.submission_order))
        .then_with(|| left.job_id.cmp(&right.job_id))
}

fn digest_report(report: &ReleaseBatchReport) -> Result<ContentHash, ReleaseBatchError> {
    ContentHash::of_serializable(&(
        &report.feature_id,
        &report.input_schema,
        &report.output_schema,
        &report.batch_id,
        report.total_budget_units,
        report.accounted_spend_units,
        report.remaining_budget_units,
        report.budget_accounting_complete,
        &report.results,
        &report.counts,
        report.stop_reason,
        report.disposition,
    ))
    .map_err(|error| ReleaseBatchError::Digest(error.to_string()))
}

fn counts(results: &[ReleaseBatchItemResult]) -> ReleaseBatchCounts {
    let count = |disposition| {
        results
            .iter()
            .filter(|result| result.disposition == disposition)
            .count() as u16
    };
    ReleaseBatchCounts {
        submitted: results.len() as u16,
        workflow_produced: count(ReleaseBatchItemDisposition::WorkflowProduced),
        deferred_budget: count(ReleaseBatchItemDisposition::DeferredBudget),
        deferred_workflow_limit: count(ReleaseBatchItemDisposition::DeferredWorkflowLimit),
        workflow_failed: count(ReleaseBatchItemDisposition::WorkflowFailed),
        not_attempted: count(ReleaseBatchItemDisposition::NotAttemptedAfterFailure),
    }
}

fn stop_reason(counts: &ReleaseBatchCounts) -> ReleaseBatchStopReason {
    if counts.workflow_failed > 0 || counts.not_attempted > 0 {
        ReleaseBatchStopReason::WorkflowFailure
    } else if counts.deferred_workflow_limit > 0 {
        ReleaseBatchStopReason::WorkflowLimit
    } else if counts.deferred_budget > 0 {
        ReleaseBatchStopReason::BudgetLimit
    } else {
        ReleaseBatchStopReason::QueueDrained
    }
}

fn disposition(counts: &ReleaseBatchCounts) -> ReleaseBatchDisposition {
    if counts.workflow_failed > 0 || counts.not_attempted > 0 {
        ReleaseBatchDisposition::Unresolved
    } else if counts.deferred_budget > 0 || counts.deferred_workflow_limit > 0 {
        ReleaseBatchDisposition::Partial
    } else {
        ReleaseBatchDisposition::Complete
    }
}

fn minimum_admission_budget(prepared: &PreparedLocalReleaseWorkflow) -> u64 {
    let required_cost = prepared
        .replay_request
        .tasks
        .iter()
        .filter(|task| task.required)
        .map(|task| u64::from(task.cost_units))
        .fold(0_u64, u64::saturating_add);
    let minimum_cost = if required_cost > 0 {
        required_cost
    } else {
        prepared
            .replay_request
            .tasks
            .iter()
            .map(|task| u64::from(task.cost_units))
            .min()
            .unwrap_or(1)
    };
    minimum_cost
        .min(prepared.replay_request.budget_units)
        .max(1)
}

fn validate_request(request: &ReleaseBatchRequest) -> Result<(), ReleaseBatchError> {
    if !valid_id(&request.batch_id)
        || request.total_budget_units == 0
        || request.items.is_empty()
        || request.items.len() > MAX_BATCH_ITEMS
        || request.max_workflows == 0
        || usize::from(request.max_workflows) > request.items.len()
    {
        return Err(ReleaseBatchError::InvalidRequest(
            "batch identity, positive shared budget, bounded queue, and workflow limit are required".into(),
        ));
    }
    let mut job_ids = BTreeSet::new();
    let mut submission_orders = BTreeSet::new();
    let mut total_bytes = 0_usize;
    for item in &request.items {
        if !valid_id(&item.job_id)
            || !job_ids.insert(item.job_id.clone())
            || !submission_orders.insert(item.submission_order)
        {
            return Err(ReleaseBatchError::InvalidRequest(
                "batch job IDs and submission order values must be valid and unique".into(),
            ));
        }
        let encoded = serde_json::to_vec(&item.workflow)
            .map_err(|error| ReleaseBatchError::InvalidRequest(error.to_string()))?;
        total_bytes = total_bytes.saturating_add(encoded.len());
        if total_bytes > MAX_BATCH_REQUEST_BYTES {
            return Err(ReleaseBatchError::InvalidRequest(
                "serialized workflow requests exceed the batch input size bound".into(),
            ));
        }
    }
    Ok(())
}

/// Execute a bounded priority queue of single-study release workflows under one spend ceiling.
pub fn execute_glioma_release_batch<E: ReplayCampaignExecutor>(
    request: &ReleaseBatchRequest,
    executor: &mut E,
) -> Result<ReleaseBatchReport, ReleaseBatchError> {
    validate_request(request)?;

    // Prepare every queued package, closure, replay task graph, and gate before any executor call.
    let mut ordered_items = request.items.iter().collect::<Vec<_>>();
    ordered_items.sort_by(|left, right| schedule_order(left, right));
    let mut prepared_items = Vec::with_capacity(ordered_items.len());
    for item in ordered_items {
        let prepared = prepare_local_release_workflow(&item.workflow)?;
        prepared_items.push((item, prepared));
    }

    let mut remaining = request.total_budget_units;
    let mut admitted = 0_u16;
    let mut failure_stopped = false;
    let mut accounting_complete = true;
    let mut accounted_spend = 0_u64;
    let mut results = Vec::with_capacity(prepared_items.len());

    for (item, mut prepared) in prepared_items {
        let package_digest = prepared.bundle.digest.clone();
        let dependency_closure_digest = prepared.dependency_closure.digest.clone();
        if failure_stopped {
            results.push(ReleaseBatchItemResult {
                job_id: item.job_id.clone(),
                priority: item.priority,
                submission_order: item.submission_order,
                disposition: ReleaseBatchItemDisposition::NotAttemptedAfterFailure,
                package_digest,
                dependency_closure_digest,
                budget_before_units: None,
                budget_allocation_units: 0,
                budget_spent_units: Some(0),
                workflow_digest: None,
                workflow_disposition: None,
                failure_stage: None,
            });
            continue;
        }
        if admitted >= request.max_workflows {
            results.push(ReleaseBatchItemResult {
                job_id: item.job_id.clone(),
                priority: item.priority,
                submission_order: item.submission_order,
                disposition: ReleaseBatchItemDisposition::DeferredWorkflowLimit,
                package_digest,
                dependency_closure_digest,
                budget_before_units: Some(remaining),
                budget_allocation_units: 0,
                budget_spent_units: Some(0),
                workflow_digest: None,
                workflow_disposition: None,
                failure_stage: None,
            });
            continue;
        }

        if prepared.dependency_closure.disposition == DependencyClosureDisposition::Closed {
            let required = minimum_admission_budget(&prepared);
            if remaining < required {
                results.push(ReleaseBatchItemResult {
                    job_id: item.job_id.clone(),
                    priority: item.priority,
                    submission_order: item.submission_order,
                    disposition: ReleaseBatchItemDisposition::DeferredBudget,
                    package_digest,
                    dependency_closure_digest,
                    budget_before_units: Some(remaining),
                    budget_allocation_units: 0,
                    budget_spent_units: Some(0),
                    workflow_digest: None,
                    workflow_disposition: None,
                    failure_stage: None,
                });
                continue;
            }
            let allocation = prepared.replay_request.budget_units.min(remaining);
            prepared.replay_request.budget_units = allocation;
            let before = remaining;
            match execute_prepared_local_release_workflow(prepared, executor) {
                Ok(workflow) => {
                    let spent = workflow
                        .campaign
                        .as_ref()
                        .map(|campaign| campaign.budget_spent_units)
                        .unwrap_or(0);
                    if spent > allocation {
                        failure_stopped = true;
                        accounting_complete = false;
                        results.push(ReleaseBatchItemResult {
                            job_id: item.job_id.clone(),
                            priority: item.priority,
                            submission_order: item.submission_order,
                            disposition: ReleaseBatchItemDisposition::WorkflowFailed,
                            package_digest,
                            dependency_closure_digest,
                            budget_before_units: Some(before),
                            budget_allocation_units: allocation,
                            budget_spent_units: None,
                            workflow_digest: None,
                            workflow_disposition: None,
                            failure_stage: Some(
                                ReleaseBatchFailureStage::ReplayExecutionOrFinalization,
                            ),
                        });
                        continue;
                    }
                    remaining = remaining.saturating_sub(spent);
                    accounted_spend = accounted_spend.saturating_add(spent);
                    admitted = admitted.saturating_add(1);
                    results.push(ReleaseBatchItemResult {
                        job_id: item.job_id.clone(),
                        priority: item.priority,
                        submission_order: item.submission_order,
                        disposition: ReleaseBatchItemDisposition::WorkflowProduced,
                        package_digest,
                        dependency_closure_digest,
                        budget_before_units: Some(before),
                        budget_allocation_units: allocation,
                        budget_spent_units: Some(spent),
                        workflow_digest: Some(workflow.digest),
                        workflow_disposition: Some(workflow.disposition),
                        failure_stage: None,
                    });
                }
                Err(_) => {
                    failure_stopped = true;
                    accounting_complete = false;
                    results.push(ReleaseBatchItemResult {
                        job_id: item.job_id.clone(),
                        priority: item.priority,
                        submission_order: item.submission_order,
                        disposition: ReleaseBatchItemDisposition::WorkflowFailed,
                        package_digest,
                        dependency_closure_digest,
                        budget_before_units: Some(before),
                        budget_allocation_units: allocation,
                        budget_spent_units: None,
                        workflow_digest: None,
                        workflow_disposition: None,
                        failure_stage: Some(
                            ReleaseBatchFailureStage::ReplayExecutionOrFinalization,
                        ),
                    });
                }
            }
        } else {
            match execute_prepared_local_release_workflow(prepared, executor) {
                Ok(workflow) => {
                    admitted = admitted.saturating_add(1);
                    results.push(ReleaseBatchItemResult {
                        job_id: item.job_id.clone(),
                        priority: item.priority,
                        submission_order: item.submission_order,
                        disposition: ReleaseBatchItemDisposition::WorkflowProduced,
                        package_digest,
                        dependency_closure_digest,
                        budget_before_units: Some(remaining),
                        budget_allocation_units: 0,
                        budget_spent_units: Some(0),
                        workflow_digest: Some(workflow.digest),
                        workflow_disposition: Some(workflow.disposition),
                        failure_stage: None,
                    });
                }
                Err(_) => {
                    failure_stopped = true;
                    results.push(ReleaseBatchItemResult {
                        job_id: item.job_id.clone(),
                        priority: item.priority,
                        submission_order: item.submission_order,
                        disposition: ReleaseBatchItemDisposition::WorkflowFailed,
                        package_digest,
                        dependency_closure_digest,
                        budget_before_units: Some(remaining),
                        budget_allocation_units: 0,
                        budget_spent_units: Some(0),
                        workflow_digest: None,
                        workflow_disposition: None,
                        failure_stage: Some(ReleaseBatchFailureStage::WorkflowFinalization),
                    });
                }
            }
        }
    }

    let counts = counts(&results);
    let stop_reason = stop_reason(&counts);
    let batch_disposition = disposition(&counts);
    let mut report = ReleaseBatchReport {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        batch_id: request.batch_id.clone(),
        total_budget_units: request.total_budget_units,
        accounted_spend_units: accounted_spend,
        remaining_budget_units: accounting_complete.then_some(remaining),
        budget_accounting_complete: accounting_complete,
        results,
        counts,
        stop_reason,
        disposition: batch_disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-batch"),
    };
    report.digest = digest_report(&report)?;
    report.validate()?;
    Ok(report)
}

impl ReleaseBatchReport {
    /// Validate stable ordering, spend accounting, result partitions, and the report digest.
    pub fn validate(&self) -> Result<(), ReleaseBatchError> {
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_id(&self.batch_id)
            || self.total_budget_units == 0
            || self.results.is_empty()
            || self.results.len() > MAX_BATCH_ITEMS
            || self.counts != counts(&self.results)
            || self.stop_reason != stop_reason(&self.counts)
            || self.disposition != disposition(&self.counts)
        {
            return Err(ReleaseBatchError::InvalidOutput(
                "batch identity, bounds, counts, stop reason, or disposition is invalid".into(),
            ));
        }
        let mut job_ids = BTreeSet::new();
        let mut submission_orders = BTreeSet::new();
        let mut known_spend = 0_u64;
        let mut unknown_spend_count = 0_u8;
        let mut previous_order: Option<(std::cmp::Reverse<u16>, u64, String)> = None;
        for result in &self.results {
            let order = (
                std::cmp::Reverse(result.priority),
                result.submission_order,
                result.job_id.clone(),
            );
            if !valid_id(&result.job_id)
                || !job_ids.insert(result.job_id.clone())
                || !submission_orders.insert(result.submission_order)
                || previous_order
                    .as_ref()
                    .is_some_and(|previous| previous >= &order)
                || result.budget_allocation_units > self.total_budget_units
            {
                return Err(ReleaseBatchError::InvalidOutput(
                    "result job identity, order, or allocation is invalid".into(),
                ));
            }
            previous_order = Some(order);
            match result.disposition {
                ReleaseBatchItemDisposition::WorkflowProduced => {
                    if result.budget_before_units.is_none()
                        || result.budget_allocation_units
                            > result.budget_before_units.unwrap_or_default()
                        || result.workflow_digest.is_none()
                        || result.workflow_disposition.is_none()
                        || result.budget_spent_units.is_none()
                        || result.failure_stage.is_some()
                        || result
                            .budget_spent_units
                            .is_some_and(|spent| spent > result.budget_allocation_units)
                    {
                        return Err(ReleaseBatchError::InvalidOutput(
                            "produced workflow evidence or spend does not reconcile".into(),
                        ));
                    }
                }
                ReleaseBatchItemDisposition::DeferredBudget
                | ReleaseBatchItemDisposition::DeferredWorkflowLimit => {
                    if result.workflow_digest.is_some()
                        || result.workflow_disposition.is_some()
                        || result.budget_before_units.is_none()
                        || result.budget_allocation_units != 0
                        || result.budget_spent_units != Some(0)
                        || result.failure_stage.is_some()
                    {
                        return Err(ReleaseBatchError::InvalidOutput(
                            "deferred job carries workflow or budget execution evidence".into(),
                        ));
                    }
                }
                ReleaseBatchItemDisposition::NotAttemptedAfterFailure => {
                    if result.workflow_digest.is_some()
                        || result.workflow_disposition.is_some()
                        || result.budget_before_units.is_some()
                        || result.budget_allocation_units != 0
                        || result.budget_spent_units != Some(0)
                        || result.failure_stage.is_some()
                    {
                        return Err(ReleaseBatchError::InvalidOutput(
                            "post-failure item carries execution or budget evidence".into(),
                        ));
                    }
                }
                ReleaseBatchItemDisposition::WorkflowFailed => {
                    if result.workflow_digest.is_some()
                        || result.workflow_disposition.is_some()
                        || result.budget_before_units.is_none()
                        || result.failure_stage.is_none()
                        || result.failure_stage.is_some_and(|stage| match stage {
                            ReleaseBatchFailureStage::WorkflowFinalization => {
                                result.budget_allocation_units != 0
                                    || result.budget_spent_units != Some(0)
                            }
                            ReleaseBatchFailureStage::ReplayExecutionOrFinalization => {
                                result.budget_allocation_units == 0
                                    || result.budget_spent_units.is_some()
                            }
                        })
                    {
                        return Err(ReleaseBatchError::InvalidOutput(
                            "failed workflow carries an invalid result partition".into(),
                        ));
                    }
                }
            }
            if let Some(spent) = result.budget_spent_units {
                known_spend = known_spend.saturating_add(spent);
            } else {
                unknown_spend_count = unknown_spend_count.saturating_add(1);
            }
        }
        if unknown_spend_count > 1
            || self.accounted_spend_units != known_spend
            || self.accounted_spend_units > self.total_budget_units
            || self.budget_accounting_complete != (unknown_spend_count == 0)
            || self.remaining_budget_units
                != self
                    .budget_accounting_complete
                    .then_some(self.total_budget_units - self.accounted_spend_units)
            || self.digest != digest_report(self)?
        {
            return Err(ReleaseBatchError::InvalidOutput(
                "budget reconciliation or batch content digest is invalid".into(),
            ));
        }
        Ok(())
    }
}
