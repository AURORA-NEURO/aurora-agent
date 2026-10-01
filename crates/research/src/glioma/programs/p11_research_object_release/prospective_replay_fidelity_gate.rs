//! Prospective replay-fidelity gate for preclinical glioma research releases.
//!
//! A reproducibility bundle is only useful when a clean-room replay can account for the
//! scientific output, its lineage, uncertainty, and negative findings.  This module executes a
//! bounded task graph against a pinned environment, distinguishes exact matches from explicitly
//! tolerated numeric deltas, and blocks promotion on unexplained divergence, missing protected
//! closure, or resource exhaustion.  It compares typed metadata and content hashes only; raw
//! experimental payloads remain with the institution-owned executor.

use super::release_bundle_compiler::ReproducibilityBundle;
use crate::glioma::release::ResearchObjectManifest;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaReplayFidelityReport1@1";
pub const MAX_TASKS: usize = 512;
pub const MAX_RETRIES: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityMetric {
    pub name: String,
    pub value_milli: i64,
    pub tolerance_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityTask {
    pub task_id: String,
    pub artifact_id: String,
    pub expected_content_hash: ContentHash,
    pub expected_lineage_digest: ContentHash,
    pub expected_metrics: Vec<ReplayFidelityMetric>,
    pub expected_uncertainty_order: Vec<String>,
    pub expected_negative_evidence_order: Vec<String>,
    pub cost_units: u32,
    pub required: bool,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityObservation {
    pub task_id: String,
    pub observed_content_hash: Option<ContentHash>,
    pub observed_lineage_digest: Option<ContentHash>,
    pub observed_metrics: Vec<ReplayFidelityMetric>,
    pub observed_uncertainty_order: Vec<String>,
    pub observed_negative_evidence_order: Vec<String>,
    pub runtime_ticks: u64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityRequest {
    pub candidate: ResearchObjectManifest,
    pub bundle: ReproducibilityBundle,
    pub tasks: Vec<ReplayFidelityTask>,
    pub reference_environment_digest: ContentHash,
    pub replay_environment_digest: ContentHash,
    pub resource_cap_ticks: u64,
    pub max_retries: u8,
    pub min_required_coverage_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayFidelityExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

pub trait ReplayFidelityExecutor {
    fn replay_fidelity_task(
        &mut self,
        task: &ReplayFidelityTask,
        request: &ReplayFidelityRequest,
        attempt: u8,
    ) -> Result<ReplayFidelityObservation, ReplayFidelityExecutionFailure>;
}

#[derive(Debug, Default)]
pub struct DryRunReplayFidelityExecutor;

impl ReplayFidelityExecutor for DryRunReplayFidelityExecutor {
    fn replay_fidelity_task(
        &mut self,
        task: &ReplayFidelityTask,
        _request: &ReplayFidelityRequest,
        _attempt: u8,
    ) -> Result<ReplayFidelityObservation, ReplayFidelityExecutionFailure> {
        Ok(ReplayFidelityObservation {
            task_id: task.task_id.clone(),
            observed_content_hash: Some(task.expected_content_hash.clone()),
            observed_lineage_digest: Some(task.expected_lineage_digest.clone()),
            observed_metrics: task.expected_metrics.clone(),
            observed_uncertainty_order: task.expected_uncertainty_order.clone(),
            observed_negative_evidence_order: task.expected_negative_evidence_order.clone(),
            runtime_ticks: u64::from(task.cost_units).saturating_mul(10),
            note: "synthetic clean-room replay only; no biological evidence".into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayFidelityStatus {
    Exact,
    Tolerated,
    Diverged,
    Blocked,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayFidelityDisposition {
    Pass,
    PassWithTolerance,
    Diverged,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayFidelityStopReason {
    Passed,
    PassedWithTolerance,
    Divergence,
    ResourceExhausted,
    EnvironmentMismatch,
    NoRunnableTasks,
    ExecutorFailed,
    MaxTasks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityTaskReport {
    pub task_id: String,
    pub status: ReplayFidelityStatus,
    pub metric_delta_order: Vec<String>,
    pub uncertainty_delta_order: Vec<String>,
    pub negative_evidence_delta_order: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayFidelityReport {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_manifest_digest: ContentHash,
    pub bundle_digest: ContentHash,
    pub reference_environment_digest: ContentHash,
    pub replay_environment_digest: ContentHash,
    pub task_reports: Vec<ReplayFidelityTaskReport>,
    pub exact_order: Vec<String>,
    pub tolerated_order: Vec<String>,
    pub diverged_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub exhausted_order: Vec<String>,
    pub coverage_milli: u16,
    pub required_coverage_milli: u16,
    pub resource_spent_ticks: u64,
    pub resource_cap_ticks: u64,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub disposition: ReplayFidelityDisposition,
    pub stop_reason: ReplayFidelityStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayFidelityError {
    #[error("replay-fidelity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replay-fidelity execution failed: {0}")]
    Execution(String),
    #[error("replay-fidelity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replay-fidelity digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(report: &ReplayFidelityReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "candidate_manifest_digest": report.candidate_manifest_digest,
        "bundle_digest": report.bundle_digest,
        "reference_environment_digest": report.reference_environment_digest,
        "replay_environment_digest": report.replay_environment_digest,
        "task_reports": report.task_reports,
        "exact_order": report.exact_order,
        "tolerated_order": report.tolerated_order,
        "diverged_order": report.diverged_order,
        "blocked_order": report.blocked_order,
        "exhausted_order": report.exhausted_order,
        "coverage_milli": report.coverage_milli,
        "required_coverage_milli": report.required_coverage_milli,
        "resource_spent_ticks": report.resource_spent_ticks,
        "resource_cap_ticks": report.resource_cap_ticks,
        "uncertainty_order": report.uncertainty_order,
        "negative_evidence_order": report.negative_evidence_order,
        "disposition": report.disposition,
        "stop_reason": report.stop_reason,
    })
}

fn validate_metric(metric: &ReplayFidelityMetric) -> bool {
    !metric.name.trim().is_empty() && metric.name.len() <= 160
}

fn validate_tasks(request: &ReplayFidelityRequest) -> Result<(), ReplayFidelityError> {
    request
        .candidate
        .validate()
        .map_err(|error| ReplayFidelityError::InvalidRequest(error.to_string()))?;
    request
        .bundle
        .validate()
        .map_err(|error| ReplayFidelityError::InvalidRequest(error.to_string()))?;
    if request.bundle.manifest_digest != request.candidate.manifest_digest {
        return Err(ReplayFidelityError::InvalidRequest(
            "reproducibility bundle is bound to a different release manifest".into(),
        ));
    }
    if request.reference_environment_digest.as_str().len() != 64
        || request.replay_environment_digest.as_str().len() != 64
        || request.resource_cap_ticks == 0
        || request.max_retries > MAX_RETRIES
        || request.min_required_coverage_milli > 1_000
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
    {
        return Err(ReplayFidelityError::InvalidRequest(
            "pinned environments, positive resource cap, bounded retries/coverage, and tasks are required".into(),
        ));
    }
    if request.bundle.environment_digest != request.reference_environment_digest {
        return Err(ReplayFidelityError::InvalidRequest(
            "reference environment does not match the bundle environment digest".into(),
        ));
    }
    let members = request
        .bundle
        .member_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for task in &request.tasks {
        let mut metric_names = BTreeSet::new();
        if task.task_id.trim().is_empty()
            || !ids.insert(task.task_id.clone())
            || task.artifact_id.trim().is_empty()
            || !members.contains(&task.artifact_id)
            || task.expected_content_hash.as_str().len() != 64
            || task.expected_lineage_digest.as_str().len() != 64
            || task.cost_units == 0
            || !canonical(&task.depends_on)
            || task
                .depends_on
                .iter()
                .any(|dependency| dependency == &task.task_id)
            || !task
                .expected_uncertainty_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || !task
                .expected_negative_evidence_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || task
                .expected_metrics
                .iter()
                .any(|metric| !validate_metric(metric) || !metric_names.insert(metric.name.clone()))
        {
            return Err(ReplayFidelityError::InvalidRequest(
                "task identity, bundle binding, hashes, cost, ordering, and unique metrics are required".into(),
            ));
        }
    }
    let map = request
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    for task in &request.tasks {
        if task
            .depends_on
            .iter()
            .any(|dependency| !map.contains_key(dependency))
        {
            return Err(ReplayFidelityError::InvalidRequest(
                "task dependencies must reference declared tasks".into(),
            ));
        }
    }
    let mut degree = request
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = degree
        .iter()
        .filter(|(_, value)| **value == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        for task in &request.tasks {
            if task.depends_on.iter().any(|dependency| dependency == &id) {
                let value = degree.get_mut(&task.task_id).expect("task degree exists");
                *value = value.saturating_sub(1);
                if *value == 0 {
                    ready.insert(task.task_id.clone());
                }
            }
        }
    }
    if visited != request.tasks.len() {
        return Err(ReplayFidelityError::InvalidRequest(
            "replay-fidelity task graph must be acyclic".into(),
        ));
    }
    Ok(())
}

fn metric_map(metrics: &[ReplayFidelityMetric]) -> BTreeMap<&str, &ReplayFidelityMetric> {
    metrics
        .iter()
        .map(|metric| (metric.name.as_str(), metric))
        .collect()
}

fn diff_order(left: &[String], right: &[String]) -> Vec<String> {
    let left = left.iter().collect::<BTreeSet<_>>();
    let right = right.iter().collect::<BTreeSet<_>>();
    left.symmetric_difference(&right)
        .map(|item| (*item).clone())
        .collect()
}

fn compare_task(
    task: &ReplayFidelityTask,
    observation: &ReplayFidelityObservation,
) -> ReplayFidelityTaskReport {
    let mut metric_delta_order = Vec::new();
    let expected = metric_map(&task.expected_metrics);
    let observed = metric_map(&observation.observed_metrics);
    let mut metric_failed = false;
    for name in expected.keys().chain(observed.keys()) {
        if metric_delta_order.iter().any(|item| item == *name) {
            continue;
        }
        match (expected.get(name), observed.get(name)) {
            (Some(expected), Some(observed)) => {
                let delta = (observed.value_milli - expected.value_milli).unsigned_abs();
                if delta > expected.tolerance_milli {
                    metric_failed = true;
                }
                if delta > 0 {
                    metric_delta_order.push((*name).to_string());
                }
            }
            _ => {
                metric_failed = true;
                metric_delta_order.push((*name).to_string());
            }
        }
    }
    metric_delta_order.sort();
    let uncertainty_delta_order = diff_order(
        &task.expected_uncertainty_order,
        &observation.observed_uncertainty_order,
    );
    let negative_evidence_delta_order = diff_order(
        &task.expected_negative_evidence_order,
        &observation.observed_negative_evidence_order,
    );
    let content_match =
        observation.observed_content_hash.as_ref() == Some(&task.expected_content_hash);
    let lineage_match =
        observation.observed_lineage_digest.as_ref() == Some(&task.expected_lineage_digest);
    let status = if !content_match
        || !lineage_match
        || metric_failed
        || !uncertainty_delta_order.is_empty()
        || !negative_evidence_delta_order.is_empty()
    {
        ReplayFidelityStatus::Diverged
    } else if !metric_delta_order.is_empty() {
        ReplayFidelityStatus::Tolerated
    } else {
        ReplayFidelityStatus::Exact
    };
    let note = match status {
        ReplayFidelityStatus::Exact => "content, lineage, metrics, uncertainty, and negative findings match".into(),
        ReplayFidelityStatus::Tolerated => "numeric delta remains within the task-declared tolerance".into(),
        ReplayFidelityStatus::Diverged => "replay differs in protected scientific output, lineage, uncertainty, or negative findings".into(),
        ReplayFidelityStatus::Blocked | ReplayFidelityStatus::Exhausted => observation.note.clone(),
    };
    ReplayFidelityTaskReport {
        task_id: task.task_id.clone(),
        status,
        metric_delta_order,
        uncertainty_delta_order,
        negative_evidence_delta_order,
        note,
    }
}

impl ReplayFidelityReport {
    pub fn validate(&self) -> Result<(), ReplayFidelityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.candidate_manifest_digest.as_str().len() != 64
            || self.bundle_digest.as_str().len() != 64
            || self.reference_environment_digest.as_str().len() != 64
            || self.replay_environment_digest.as_str().len() != 64
            || self.resource_cap_ticks == 0
            || self.resource_spent_ticks > self.resource_cap_ticks
            || self.coverage_milli > 1_000
            || self.required_coverage_milli > 1_000
            || !canonical(&self.exact_order)
            || !canonical(&self.tolerated_order)
            || !canonical(&self.diverged_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.exhausted_order)
            || !canonical(&self.uncertainty_order)
            || !canonical(&self.negative_evidence_order)
        {
            return Err(ReplayFidelityError::InvalidOutput(
                "fidelity identity, bounds, ordering, or resource invariants are invalid".into(),
            ));
        }
        let partitions = self
            .exact_order
            .iter()
            .chain(self.tolerated_order.iter())
            .chain(self.diverged_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.exhausted_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partitions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ReplayFidelityError::InvalidOutput(
                "a task cannot appear in multiple fidelity partitions".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReplayFidelityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReplayFidelityError::InvalidOutput(
                "fidelity report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a bounded clean-room replay and produce a release-gating fidelity report.
pub fn execute_glioma_prospective_replay_fidelity<E: ReplayFidelityExecutor>(
    request: &ReplayFidelityRequest,
    executor: &mut E,
) -> Result<ReplayFidelityReport, ReplayFidelityError> {
    validate_tasks(request)?;
    let mut reports = Vec::new();
    let mut exact = BTreeSet::new();
    let mut tolerated = BTreeSet::new();
    let mut diverged = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut exhausted = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut resource_spent = 0_u64;
    let mut stop_reason =
        if request.reference_environment_digest != request.replay_environment_digest {
            ReplayFidelityStopReason::EnvironmentMismatch
        } else {
            ReplayFidelityStopReason::MaxTasks
        };

    if request.reference_environment_digest != request.replay_environment_digest {
        for task in &request.tasks {
            blocked.insert(task.task_id.clone());
            reports.push(ReplayFidelityTaskReport {
                task_id: task.task_id.clone(),
                status: ReplayFidelityStatus::Blocked,
                metric_delta_order: Vec::new(),
                uncertainty_delta_order: Vec::new(),
                negative_evidence_delta_order: Vec::new(),
                note: "reference and replay environments differ; clean-room execution refused"
                    .into(),
            });
        }
    } else {
        while completed.len() + blocked.len() + diverged.len() + exhausted.len()
            < request.tasks.len()
        {
            let mut eligible = request
                .tasks
                .iter()
                .filter(|task| {
                    !completed.contains(&task.task_id)
                        && !blocked.contains(&task.task_id)
                        && !diverged.contains(&task.task_id)
                        && !exhausted.contains(&task.task_id)
                        && task.depends_on.iter().all(|dependency| {
                            exact.contains(dependency) || tolerated.contains(dependency)
                        })
                })
                .collect::<Vec<_>>();
            eligible.sort_by(|left, right| {
                right
                    .required
                    .cmp(&left.required)
                    .then_with(|| left.cost_units.cmp(&right.cost_units))
                    .then_with(|| left.task_id.cmp(&right.task_id))
            });
            let Some(task) = eligible.first().copied() else {
                stop_reason = if request.tasks.iter().any(|task| {
                    !completed.contains(&task.task_id) && !blocked.contains(&task.task_id)
                }) {
                    ReplayFidelityStopReason::NoRunnableTasks
                } else {
                    ReplayFidelityStopReason::MaxTasks
                };
                break;
            };
            if resource_spent >= request.resource_cap_ticks
                || task.cost_units as u64
                    > request.resource_cap_ticks.saturating_sub(resource_spent)
            {
                exhausted.insert(task.task_id.clone());
                reports.push(ReplayFidelityTaskReport {
                    task_id: task.task_id.clone(),
                    status: ReplayFidelityStatus::Exhausted,
                    metric_delta_order: Vec::new(),
                    uncertainty_delta_order: Vec::new(),
                    negative_evidence_delta_order: Vec::new(),
                    note: "resource cap reached before clean-room replay".into(),
                });
                continue;
            }
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.replay_fidelity_task(task, request, attempt) {
                    Ok(observation) => {
                        if observation.task_id != task.task_id || observation.note.trim().is_empty()
                        {
                            return Err(ReplayFidelityError::Execution(
                                "executor returned an invalid fidelity observation".into(),
                            ));
                        }
                        accepted = Some(observation);
                        break;
                    }
                    Err(error) if error.retryable && attempt <= request.max_retries => continue,
                    Err(error) => {
                        blocked.insert(task.task_id.clone());
                        reports.push(ReplayFidelityTaskReport {
                            task_id: task.task_id.clone(),
                            status: ReplayFidelityStatus::Blocked,
                            metric_delta_order: Vec::new(),
                            uncertainty_delta_order: Vec::new(),
                            negative_evidence_delta_order: Vec::new(),
                            note: if error.reason.trim().is_empty() {
                                "executor failed without a reason".into()
                            } else {
                                error.reason
                            },
                        });
                        break;
                    }
                }
            }
            let Some(observation) = accepted else {
                continue;
            };
            resource_spent = resource_spent.saturating_add(observation.runtime_ticks);
            if resource_spent > request.resource_cap_ticks {
                resource_spent = request.resource_cap_ticks;
                exhausted.insert(task.task_id.clone());
                uncertainty.insert(format!("resource-cap-overrun:{}", task.task_id));
                reports.push(ReplayFidelityTaskReport {
                    task_id: task.task_id.clone(),
                    status: ReplayFidelityStatus::Exhausted,
                    metric_delta_order: Vec::new(),
                    uncertainty_delta_order: Vec::new(),
                    negative_evidence_delta_order: Vec::new(),
                    note: "clean-room replay exceeded the declared resource cap".into(),
                });
                break;
            }
            let report = compare_task(task, &observation);
            for item in &report.uncertainty_delta_order {
                uncertainty.insert(format!("{}:{item}", task.task_id));
            }
            for item in &report.negative_evidence_delta_order {
                negative.insert(format!("{}:{item}", task.task_id));
            }
            match report.status {
                ReplayFidelityStatus::Exact => {
                    exact.insert(task.task_id.clone());
                    completed.insert(task.task_id.clone());
                }
                ReplayFidelityStatus::Tolerated => {
                    tolerated.insert(task.task_id.clone());
                    completed.insert(task.task_id.clone());
                    uncertainty.insert(format!("numeric-tolerance:{}", task.task_id));
                }
                ReplayFidelityStatus::Diverged => {
                    diverged.insert(task.task_id.clone());
                }
                ReplayFidelityStatus::Blocked | ReplayFidelityStatus::Exhausted => {
                    blocked.insert(task.task_id.clone());
                }
            };
            reports.push(report);
            if !diverged.is_empty() {
                stop_reason = ReplayFidelityStopReason::Divergence;
                break;
            }
        }
    }

    let total = request.tasks.len() as u64;
    let completed_count = (exact.len() + tolerated.len()) as u64;
    let required_total = request.tasks.iter().filter(|task| task.required).count() as u64;
    let required_completed = request
        .tasks
        .iter()
        .filter(|task| {
            task.required && (exact.contains(&task.task_id) || tolerated.contains(&task.task_id))
        })
        .count() as u64;
    let coverage_milli = completed_count
        .saturating_mul(1_000)
        .checked_div(total)
        .unwrap_or(0) as u16;
    let required_coverage_milli = if required_total == 0 {
        1_000
    } else {
        required_completed
            .saturating_mul(1_000)
            .checked_div(required_total)
            .unwrap_or(0) as u16
    };
    if diverged.is_empty()
        && blocked.is_empty()
        && exhausted.is_empty()
        && required_coverage_milli >= request.min_required_coverage_milli
        && exact.len() + tolerated.len() == request.tasks.len()
    {
        stop_reason = if tolerated.is_empty() {
            ReplayFidelityStopReason::Passed
        } else {
            ReplayFidelityStopReason::PassedWithTolerance
        };
    }
    let disposition = if !diverged.is_empty() {
        ReplayFidelityDisposition::Diverged
    } else if !blocked.is_empty() || !exhausted.is_empty() {
        ReplayFidelityDisposition::Blocked
    } else if exact.len() + tolerated.len() == request.tasks.len()
        && required_coverage_milli >= request.min_required_coverage_milli
    {
        if tolerated.is_empty() {
            ReplayFidelityDisposition::Pass
        } else {
            ReplayFidelityDisposition::PassWithTolerance
        }
    } else {
        ReplayFidelityDisposition::Unresolved
    };
    reports.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let mut output = ReplayFidelityReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_manifest_digest: request.candidate.manifest_digest.clone(),
        bundle_digest: request.bundle.digest.clone(),
        reference_environment_digest: request.reference_environment_digest.clone(),
        replay_environment_digest: request.replay_environment_digest.clone(),
        task_reports: reports,
        exact_order: exact.into_iter().collect(),
        tolerated_order: tolerated.into_iter().collect(),
        diverged_order: diverged.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        exhausted_order: exhausted.into_iter().collect(),
        coverage_milli,
        required_coverage_milli,
        resource_spent_ticks: resource_spent,
        resource_cap_ticks: request.resource_cap_ticks,
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replay-fidelity"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReplayFidelityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::release_bundle_compiler::{
        BundleMember, ReproducibilityBundleDisposition,
    };
    use crate::glioma::release::{ReleaseStatus, ResearchObjectRequest};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> ReplayFidelityRequest {
        let manifest =
            crate::glioma::release::build_research_object_manifest(&ResearchObjectRequest {
                research_id: "fidelity-research".into(),
                study_id: "fidelity-study".into(),
                objective: "replay a preclinical glioma mechanism workflow".into(),
                plan_digest: hash("plan"),
                execution_digest: hash("execution"),
                replay_identity: hash("replay"),
                program_order: vec!["p05-mechanism".into()],
                artifacts: vec![crate::glioma_engine::LocalArtifactRef {
                    artifact_id: "result".into(),
                    content_hash: hash("result"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                negative_evidence: vec!["null-result".into()],
                limitations: vec!["preclinical-only".into()],
                raw_data_local: true,
                aggregate_only: true,
            })
            .unwrap();
        assert_eq!(manifest.release_status, ReleaseStatus::ReadyForSigning);
        let shareability = crate::glioma::programs::p11_research_object_release::evaluate_glioma_release_shareability(
            &crate::glioma::programs::p11_research_object_release::ReleaseShareabilityRequest {
                candidate_manifest_digest: manifest.manifest_digest.clone(),
                root_order: vec!["result".into()],
                dependencies: vec![
                    crate::glioma::programs::p11_research_object_release::LicenseDependency {
                        artifact_id: "result".into(),
                        dependency_order: Vec::new(),
                        license_id: Some("CC-BY-4.0".into()),
                        fields: vec![
                            crate::glioma::programs::p11_research_object_release::ShareableField {
                                field_id: "result".into(),
                                classification: crate::glioma::programs::p11_research_object_release::FieldClassification::AggregateResult,
                                requested_export: true,
                                source_digest: hash("result"),
                            },
                        ],
                        local_only: false,
                        embargo_until_epoch: None,
                        rights_confirmed: true,
                        contains_human_data: false,
                        intended_audience: "consortium".into(),
                    },
                ],
                policy: crate::glioma::programs::p11_research_object_release::LicenseScopePolicy {
                    allowed_license_order: vec!["CC-BY-4.0".into()],
                    forbidden_license_order: Vec::new(),
                    audience: "consortium".into(),
                    now_epoch: 1,
                    permit_aggregate_export: true,
                    permit_local_only_export: true,
                    permit_human_data: false,
                },
            },
        )
        .unwrap();
        let bundle = crate::glioma::programs::p11_research_object_release::compile_glioma_reproducibility_bundle(&crate::glioma::programs::p11_research_object_release::ReproducibilityBundleRequest {
            manifest: manifest.clone(),
            shareability,
            members: vec![BundleMember {
                artifact_id: "result".into(),
                relative_path: "result.json".into(),
                content_hash: hash("result"),
                content_type: "application/json".into(),
                dependency_order: Vec::new(),
                export_permitted: true,
                local_only: false,
            }],
            workflow_digest: hash("workflow"),
            environment_digest: hash("environment"),
            replay_instruction_order: vec!["run-local".into()],
            target_profile: "clean-room".into(),
            max_members: 8,
        })
        .unwrap();
        assert_eq!(
            bundle.disposition,
            ReproducibilityBundleDisposition::Complete
        );
        let task = ReplayFidelityTask {
            task_id: "mechanism".into(),
            artifact_id: "result".into(),
            expected_content_hash: hash("result"),
            expected_lineage_digest: hash("lineage"),
            expected_metrics: vec![ReplayFidelityMetric {
                name: "effect_milli".into(),
                value_milli: 100,
                tolerance_milli: 5,
            }],
            expected_uncertainty_order: vec!["uncertain-tail".into()],
            expected_negative_evidence_order: vec!["null-result".into()],
            cost_units: 10,
            required: true,
            depends_on: Vec::new(),
        };
        ReplayFidelityRequest {
            candidate: manifest,
            bundle,
            tasks: vec![task],
            reference_environment_digest: hash("environment"),
            replay_environment_digest: hash("environment"),
            resource_cap_ticks: 1_000,
            max_retries: 1,
            min_required_coverage_milli: 1_000,
        }
    }

    #[test]
    fn clean_room_replay_passes_exactly_and_is_replayable() {
        let request = request();
        let mut executor = DryRunReplayFidelityExecutor;
        let report = execute_glioma_prospective_replay_fidelity(&request, &mut executor).unwrap();
        assert_eq!(report.disposition, ReplayFidelityDisposition::Pass);
        assert_eq!(report.exact_order, vec!["mechanism"]);
        report.validate().unwrap();
    }

    #[test]
    fn numeric_delta_is_tolerated_but_uncertainty_delta_diverges() {
        let request = request();
        struct DeltaExecutor;
        impl ReplayFidelityExecutor for DeltaExecutor {
            fn replay_fidelity_task(
                &mut self,
                task: &ReplayFidelityTask,
                _request: &ReplayFidelityRequest,
                _attempt: u8,
            ) -> Result<ReplayFidelityObservation, ReplayFidelityExecutionFailure> {
                Ok(ReplayFidelityObservation {
                    task_id: task.task_id.clone(),
                    observed_content_hash: Some(task.expected_content_hash.clone()),
                    observed_lineage_digest: Some(task.expected_lineage_digest.clone()),
                    observed_metrics: vec![ReplayFidelityMetric {
                        name: "effect_milli".into(),
                        value_milli: 103,
                        tolerance_milli: 5,
                    }],
                    observed_uncertainty_order: vec!["new-uncertainty".into()],
                    observed_negative_evidence_order: task.expected_negative_evidence_order.clone(),
                    runtime_ticks: 1,
                    note: "delta".into(),
                })
            }
        }
        let mut executor = DeltaExecutor;
        let report = execute_glioma_prospective_replay_fidelity(&request, &mut executor).unwrap();
        assert_eq!(report.disposition, ReplayFidelityDisposition::Diverged);
        assert!(report
            .uncertainty_order
            .iter()
            .any(|item| item.contains("new-uncertainty")));
        assert!(report.diverged_order.contains(&"mechanism".into()));
    }

    #[test]
    fn environment_mismatch_blocks_before_executor() {
        let mut request = request();
        request.replay_environment_digest = hash("different-environment");
        let mut executor = DryRunReplayFidelityExecutor;
        let report = execute_glioma_prospective_replay_fidelity(&request, &mut executor).unwrap();
        assert_eq!(report.disposition, ReplayFidelityDisposition::Blocked);
        assert_eq!(
            report.stop_reason,
            ReplayFidelityStopReason::EnvironmentMismatch
        );
        assert_eq!(report.blocked_order, vec!["mechanism"]);
    }

    #[test]
    fn resource_exhaustion_remains_unresolved() {
        let mut request = request();
        request.resource_cap_ticks = 1;
        let mut executor = DryRunReplayFidelityExecutor;
        let report = execute_glioma_prospective_replay_fidelity(&request, &mut executor).unwrap();
        assert_eq!(report.disposition, ReplayFidelityDisposition::Blocked);
        assert!(report.exhausted_order.contains(&"mechanism".into()));
    }
}
