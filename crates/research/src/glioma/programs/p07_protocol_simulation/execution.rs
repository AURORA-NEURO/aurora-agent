//! Guarded execution of a feasible preclinical glioma protocol.
//!
//! [`super::simulator`] plans a resource-constrained protocol; this module adds the missing
//! product seam for actually running that plan through an institution-local executor. The
//! executor owns software, robotics, and instrument effects. This crate enforces the simulation
//! digest, dependency order, typed output contracts, retries, and fail-closed handling of partial
//! or failed work. It never opens a socket, touches a specimen, or turns a protocol result into a
//! clinical decision.

use super::simulator::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolSimulationRequest, ProtocolTask,
    ScheduleEntry,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolExecution1@1";
pub const MAX_RETRIES: u8 = 8;
pub const MAX_TASKS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolExecutionRequest {
    pub protocol: ProtocolSimulationRequest,
    pub max_retries: u8,
    /// When true, every completed or negative task must return a local typed artifact.
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolTaskDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTaskResult {
    pub task_id: String,
    pub output_schema: String,
    pub disposition: ProtocolTaskDisposition,
    pub attempt_count: u8,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// The only effectful seam for protocol execution. Implementations belong to a local worker,
/// simulator, robotics gateway, or instrument service selected by the institution.
pub trait GliomaProtocolExecutor {
    fn execute_task(
        &mut self,
        task: &ProtocolTask,
        schedule: &ScheduleEntry,
        attempt: u8,
    ) -> Result<ProtocolTaskResult, ProtocolExecutionFailure>;
}

/// A deterministic executor for demonstrations and integration tests. It creates only local
/// synthetic artifacts and is explicitly not biological evidence or instrument execution.
#[derive(Debug, Default)]
pub struct DryRunGliomaProtocolExecutor;

impl GliomaProtocolExecutor for DryRunGliomaProtocolExecutor {
    fn execute_task(
        &mut self,
        task: &ProtocolTask,
        _schedule: &ScheduleEntry,
        attempt: u8,
    ) -> Result<ProtocolTaskResult, ProtocolExecutionFailure> {
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "task_id": task.task_id,
            "output_schema": task.output_schema,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| ProtocolExecutionFailure {
            reason: format!("dry-run artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(ProtocolTaskResult {
            task_id: task.task_id.clone(),
            output_schema: task.output_schema.clone(),
            disposition: ProtocolTaskDisposition::Completed,
            attempt_count: attempt,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run:{}", task.task_id),
                content_hash,
                content_type: task.output_schema.clone(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            note: "dry-run task completed; no biological or instrument effect occurred".into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolExecutionDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolExecutionStopReason {
    Completed,
    ProtocolNotFeasible,
    TaskFailed,
    DependencyBlocked,
    ExecutorRefused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub protocol_digest: ContentHash,
    pub task_order: Vec<String>,
    pub task_results: Vec<ProtocolTaskResult>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub retry_count: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ProtocolExecutionDisposition,
    pub stop_reason: ProtocolExecutionStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolExecutionError {
    #[error("protocol execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol execution is not admitted: {0}")]
    NotAdmitted(String),
    #[error("protocol execution input is invalid: {0}")]
    InvalidInput(String),
    #[error("protocol execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol execution digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &ProtocolExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "protocol_digest": output.protocol_digest,
        "task_order": output.task_order,
        "task_results": output.task_results,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "skipped_order": output.skipped_order,
        "retry_count": output.retry_count,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] <= pair[1])
}

impl ProtocolExecution {
    pub fn validate(&self) -> Result<(), ProtocolExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.protocol_digest.as_str().len() != 64
            || self.digest.as_str().len() != 64
            || self.task_order.is_empty()
            || self.task_order.len() != self.task_results.len()
            || self.task_order.windows(2).any(|pair| pair[0] == pair[1])
            || self
                .task_results
                .iter()
                .zip(self.task_order.iter())
                .any(|(result, task_id)| {
                    result.task_id != *task_id
                        || result.task_id.trim().is_empty()
                        || result.output_schema.trim().is_empty()
                        || result.note.trim().is_empty()
                        || match result.disposition {
                            ProtocolTaskDisposition::Skipped => result.attempt_count != 0,
                            _ => result.attempt_count == 0,
                        }
                        || result
                            .artifact
                            .as_ref()
                            .is_some_and(|artifact| artifact.content_type != result.output_schema)
                })
            || self
                .task_results
                .iter()
                .filter_map(|result| result.artifact.as_ref())
                .any(|artifact| artifact.validate().is_err())
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.skipped_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(ProtocolExecutionError::InvalidOutput(
                "execution identity, task results, artifacts, ordering, or limitations are invalid"
                    .into(),
            ));
        }
        let ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let result_ids = self
            .task_results
            .iter()
            .map(|result| result.task_id.clone())
            .collect::<BTreeSet<_>>();
        if ids != result_ids {
            return Err(ProtocolExecutionError::InvalidOutput(
                "task order and result identities do not reconcile".into(),
            ));
        }
        let status_order = |disposition: ProtocolTaskDisposition| {
            self.task_results
                .iter()
                .filter(|result| result.disposition == disposition)
                .map(|result| result.task_id.clone())
                .collect::<BTreeSet<_>>()
        };
        for (label, order, disposition) in [
            (
                "completed",
                &self.completed_order,
                ProtocolTaskDisposition::Completed,
            ),
            (
                "negative",
                &self.negative_order,
                ProtocolTaskDisposition::Negative,
            ),
            (
                "partial",
                &self.partial_order,
                ProtocolTaskDisposition::Partial,
            ),
            (
                "failed",
                &self.failed_order,
                ProtocolTaskDisposition::Failed,
            ),
            (
                "skipped",
                &self.skipped_order,
                ProtocolTaskDisposition::Skipped,
            ),
        ] {
            if order.iter().cloned().collect::<BTreeSet<_>>() != status_order(disposition) {
                return Err(ProtocolExecutionError::InvalidOutput(format!(
                    "{label} task partition does not reconcile"
                )));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ProtocolExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProtocolExecutionError::InvalidOutput(
                "execution digest is not bound to task outcomes".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ProtocolExecutionRequest) -> Result<(), ProtocolExecutionError> {
    if request.max_retries > MAX_RETRIES {
        return Err(ProtocolExecutionError::InvalidRequest(
            "max_retries exceeds the bounded retry policy".into(),
        ));
    }
    if request.protocol.tasks.is_empty() || request.protocol.tasks.len() > MAX_TASKS {
        return Err(ProtocolExecutionError::InvalidRequest(
            "protocol task count is empty or exceeds the execution bound".into(),
        ));
    }
    Ok(())
}

fn provider_result(
    task: &ProtocolTask,
    mut result: ProtocolTaskResult,
    attempt: u8,
    require_artifacts: bool,
) -> Result<ProtocolTaskResult, ProtocolExecutionError> {
    if result.task_id != task.task_id
        || result.output_schema != task.output_schema
        || result.note.trim().is_empty()
        || matches!(
            result.disposition,
            ProtocolTaskDisposition::Failed | ProtocolTaskDisposition::Skipped
        )
    {
        return Err(ProtocolExecutionError::InvalidOutput(format!(
            "executor returned an invalid result for task {}",
            task.task_id
        )));
    }
    if require_artifacts && result.artifact.is_none() {
        return Err(ProtocolExecutionError::InvalidOutput(format!(
            "executor returned no required local artifact for task {}",
            task.task_id
        )));
    }
    if let Some(artifact) = &result.artifact {
        artifact
            .validate()
            .map_err(|error| ProtocolExecutionError::InvalidOutput(error.to_string()))?;
        if artifact.content_type != task.output_schema {
            return Err(ProtocolExecutionError::InvalidOutput(format!(
                "task {} artifact content type does not match {}",
                task.task_id, task.output_schema
            )));
        }
    }
    result.attempt_count = attempt;
    Ok(result)
}

/// Execute a feasible simulated protocol through a caller-owned local executor.
pub fn execute_glioma_protocol<E: GliomaProtocolExecutor>(
    request: &ProtocolExecutionRequest,
    executor: &mut E,
) -> Result<ProtocolExecution, ProtocolExecutionError> {
    validate_request(request)?;
    let simulation = simulate_glioma_protocol(&request.protocol)
        .map_err(|error| ProtocolExecutionError::InvalidRequest(error.to_string()))?;
    if simulation.disposition != ProtocolDisposition::Feasible {
        return Err(ProtocolExecutionError::NotAdmitted(format!(
            "protocol simulation disposition is {:?}; repair capacity, risk, horizon, dependency, or approval gates before execution",
            simulation.disposition
        )));
    }
    let task_map = request
        .protocol
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let mut task_results = Vec::with_capacity(simulation.schedule.len());
    let mut by_task = BTreeMap::<String, ProtocolTaskDisposition>::new();
    let mut retry_count = 0_u32;
    let mut stop_reason = ProtocolExecutionStopReason::Completed;
    let mut halted = false;
    for (index, schedule) in simulation.schedule.iter().enumerate() {
        let task = task_map.get(&schedule.task_id).ok_or_else(|| {
            ProtocolExecutionError::InvalidInput(format!(
                "simulation scheduled unknown task {}",
                schedule.task_id
            ))
        })?;
        if halted {
            let result = ProtocolTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ProtocolTaskDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                note: "skipped after an earlier protocol execution failure or partial result"
                    .into(),
            };
            by_task.insert(task.task_id.clone(), result.disposition);
            task_results.push(result);
            continue;
        }
        let dependency_block = task.depends_on.iter().any(|dependency| {
            !matches!(
                by_task.get(dependency),
                Some(ProtocolTaskDisposition::Completed | ProtocolTaskDisposition::Negative)
            )
        });
        if dependency_block {
            let result = ProtocolTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ProtocolTaskDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                note: "skipped because a dependency did not produce a completed or negative result"
                    .into(),
            };
            by_task.insert(task.task_id.clone(), result.disposition);
            task_results.push(result);
            halted = true;
            stop_reason = ProtocolExecutionStopReason::DependencyBlocked;
            continue;
        }
        let mut completed = None;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_task(task, schedule, attempt) {
                Ok(result) => {
                    let result = provider_result(task, result, attempt, request.require_artifacts)?;
                    let disposition = result.disposition;
                    completed = Some(result);
                    if disposition == ProtocolTaskDisposition::Partial {
                        halted = true;
                        stop_reason = ProtocolExecutionStopReason::DependencyBlocked;
                    }
                    break;
                }
                Err(failure) => {
                    if failure.reason.trim().is_empty() {
                        return Err(ProtocolExecutionError::InvalidOutput(format!(
                            "executor returned an empty failure reason for task {}",
                            task.task_id
                        )));
                    }
                    if failure.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        continue;
                    }
                    let result = ProtocolTaskResult {
                        task_id: task.task_id.clone(),
                        output_schema: task.output_schema.clone(),
                        disposition: ProtocolTaskDisposition::Failed,
                        attempt_count: attempt,
                        artifact: None,
                        note: failure.reason,
                    };
                    completed = Some(result);
                    halted = true;
                    stop_reason = ProtocolExecutionStopReason::TaskFailed;
                    break;
                }
            }
        }
        let result = completed.ok_or_else(|| {
            ProtocolExecutionError::InvalidOutput(format!(
                "executor produced no result for scheduled task {} at index {index}",
                task.task_id
            ))
        })?;
        by_task.insert(task.task_id.clone(), result.disposition);
        task_results.push(result);
    }
    let task_order = simulation
        .schedule
        .iter()
        .map(|entry| entry.task_id.clone())
        .collect::<Vec<_>>();
    let mut completed_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut partial_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut skipped_order = Vec::new();
    let mut negative_evidence = Vec::new();
    for result in &task_results {
        match result.disposition {
            ProtocolTaskDisposition::Completed => completed_order.push(result.task_id.clone()),
            ProtocolTaskDisposition::Negative => {
                negative_order.push(result.task_id.clone());
                negative_evidence.push(format!("{}:{}", result.task_id, result.note));
            }
            ProtocolTaskDisposition::Partial => {
                partial_order.push(result.task_id.clone());
                negative_evidence.push(format!("{}:partial-result", result.task_id));
            }
            ProtocolTaskDisposition::Failed => failed_order.push(result.task_id.clone()),
            ProtocolTaskDisposition::Skipped => skipped_order.push(result.task_id.clone()),
        }
    }
    for order in [
        &mut completed_order,
        &mut negative_order,
        &mut partial_order,
        &mut failed_order,
        &mut skipped_order,
        &mut negative_evidence,
    ] {
        order.sort();
    }
    let mut uncertainty = Vec::new();
    if retry_count > 0 {
        uncertainty.push(format!("{retry_count} retry attempts were required"));
    }
    if !partial_order.is_empty() {
        uncertainty.push("a partial task result stopped dependent execution".into());
    }
    if !skipped_order.is_empty() {
        uncertainty.push("skipped tasks were not treated as completed evidence".into());
    }
    uncertainty.sort();
    let disposition = if !failed_order.is_empty() {
        ProtocolExecutionDisposition::Failed
    } else if !partial_order.is_empty() || !skipped_order.is_empty() {
        ProtocolExecutionDisposition::Partial
    } else {
        ProtocolExecutionDisposition::Completed
    };
    let mut output = ProtocolExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        protocol_digest: simulation.digest,
        task_order,
        task_results,
        completed_order,
        negative_order,
        partial_order,
        failed_order,
        skipped_order,
        retry_count,
        uncertainty,
        negative_evidence,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProtocolExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolResourceKind,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn artifact(label: &str, content_type: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: content_type.into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> ProtocolExecutionRequest {
        ProtocolExecutionRequest {
            protocol: ProtocolSimulationRequest {
                objective: "execute a preclinical glioma organoid assay".into(),
                model_system: GliomaModelSystem::Organoid,
                tasks: vec![
                    ProtocolTask {
                        task_id: "setup".into(),
                        label: "prepare organoid controls".into(),
                        resource_kind: ProtocolResourceKind::Culture,
                        resource_units: 1,
                        duration_ticks: 1,
                        depends_on: Vec::new(),
                        model_system: GliomaModelSystem::Organoid,
                        output_schema: "Setup1@1".into(),
                        risk_milli: 10,
                        requires_instrument: false,
                    },
                    ProtocolTask {
                        task_id: "assay".into(),
                        label: "run invasion assay".into(),
                        resource_kind: ProtocolResourceKind::Culture,
                        resource_units: 1,
                        duration_ticks: 1,
                        depends_on: vec!["setup".into()],
                        model_system: GliomaModelSystem::Organoid,
                        output_schema: "Assay1@1".into(),
                        risk_milli: 10,
                        requires_instrument: false,
                    },
                ],
                resources: vec![ProtocolResource {
                    resource_id: "culture".into(),
                    kind: ProtocolResourceKind::Culture,
                    capacity_units: 1,
                }],
                max_ticks: 10,
                max_risk_milli: 100,
                allow_instrument_execution: false,
                approval_reference: None,
                randomization_seed: ContentHash::of_bytes(b"protocol-execution-seed"),
            },
            max_retries: 1,
            require_artifacts: true,
        }
    }

    struct RecordingExecutor {
        calls: Vec<String>,
        fail_once: bool,
    }

    impl GliomaProtocolExecutor for RecordingExecutor {
        fn execute_task(
            &mut self,
            task: &ProtocolTask,
            _schedule: &ScheduleEntry,
            attempt: u8,
        ) -> Result<ProtocolTaskResult, ProtocolExecutionFailure> {
            self.calls.push(format!("{}:{attempt}", task.task_id));
            if self.fail_once && task.task_id == "setup" && attempt == 1 {
                self.fail_once = false;
                return Err(ProtocolExecutionFailure {
                    reason: "transient local worker loss".into(),
                    retryable: true,
                });
            }
            Ok(ProtocolTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ProtocolTaskDisposition::Completed,
                attempt_count: attempt,
                artifact: Some(artifact(&task.task_id, &task.output_schema)),
                note: "local task completed".into(),
            })
        }
    }

    #[test]
    fn executes_in_dependency_order_and_retries_transient_failure() {
        let mut executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: true,
        };
        let output = execute_glioma_protocol(&request(), &mut executor).unwrap();
        assert_eq!(executor.calls, vec!["setup:1", "setup:2", "assay:1"]);
        assert_eq!(output.disposition, ProtocolExecutionDisposition::Completed);
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.completed_order, vec!["assay", "setup"]);
        output.validate().unwrap();
    }

    #[test]
    fn malformed_or_missing_artifact_is_refused() {
        struct MissingArtifact;
        impl GliomaProtocolExecutor for MissingArtifact {
            fn execute_task(
                &mut self,
                task: &ProtocolTask,
                _schedule: &ScheduleEntry,
                attempt: u8,
            ) -> Result<ProtocolTaskResult, ProtocolExecutionFailure> {
                Ok(ProtocolTaskResult {
                    task_id: task.task_id.clone(),
                    output_schema: task.output_schema.clone(),
                    disposition: ProtocolTaskDisposition::Completed,
                    attempt_count: attempt,
                    artifact: None,
                    note: "missing artifact".into(),
                })
            }
        }
        assert!(matches!(
            execute_glioma_protocol(&request(), &mut MissingArtifact),
            Err(ProtocolExecutionError::InvalidOutput(_))
        ));
    }

    #[test]
    fn infeasible_protocol_is_not_admitted_and_executor_is_not_called() {
        let mut request = request();
        request.protocol.max_risk_milli = 1;
        let mut executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: false,
        };
        assert!(matches!(
            execute_glioma_protocol(&request, &mut executor),
            Err(ProtocolExecutionError::NotAdmitted(_))
        ));
        assert!(executor.calls.is_empty());
    }
}
