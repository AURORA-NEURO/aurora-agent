//! Researcher-facing inspection of long-running preclinical glioma computations.
//!
//! A successful scheduler response is not enough for a scientist to understand a run. This
//! feature turns immutable task results, local telemetry events, artifact lineage, and explicit
//! partial-result semantics into a deterministic timeline plus a reproducibility issue bundle.
//! Counts are reconciled to task identities, stale telemetry is visible, and every recovery
//! suggestion points back to a task or artifact gap. The inspector is metadata-only and does not
//! re-run code, open raw payloads, or interpret a result biologically.

use super::artifact_lineage_index::{ArtifactLineageIndex, ArtifactLineageNodeStatus};
use super::execution::{ComputationExecution, ComputationTaskDisposition};
use super::partial_result_semantics::{PartialResultBundle, PartialResultFieldState};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationRunInspection1@1";
pub const MAX_EVENTS: usize = 8_192;
pub const MAX_NOTE_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationInspectionEventKind {
    Planned,
    Queued,
    Started,
    Retry,
    ResourceSample,
    QualityCheck,
    Recovery,
    Completed,
    Cached,
    Negative,
    Partial,
    Failed,
    Skipped,
}

impl ComputationInspectionEventKind {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Cached
                | Self::Negative
                | Self::Partial
                | Self::Failed
                | Self::Skipped
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInspectionEvent {
    pub sequence: u64,
    pub task_id: String,
    pub tick: u64,
    pub kind: ComputationInspectionEventKind,
    pub cost_units: u64,
    pub memory_mb: u32,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationRunInspectionRequest {
    pub objective: String,
    pub replay_identity: ContentHash,
    pub execution: ComputationExecution,
    pub lineage: Option<ArtifactLineageIndex>,
    pub partial_results: Option<PartialResultBundle>,
    pub events: Vec<ComputationInspectionEvent>,
    pub observed_at_tick: u64,
    pub stale_after_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationInspectionTaskStatus {
    Completed,
    Cached,
    Negative,
    Partial,
    Failed,
    Skipped,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInspectionTask {
    pub task_id: String,
    pub status: ComputationInspectionTaskStatus,
    pub terminal_sequence_order: Vec<u64>,
    pub artifact_id: Option<String>,
    pub artifact_lineage_status: Option<ArtifactLineageNodeStatus>,
    pub partial_state_order: Vec<PartialResultFieldState>,
    pub attempt_count: u8,
    pub retry_count: u8,
    pub cost_units: u64,
    pub peak_memory_mb: u32,
    pub telemetry_stale: bool,
    pub issue_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationInspectionDisposition {
    Ready,
    Partial,
    Failed,
    Stale,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationRunInspection {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub timeline: Vec<ComputationInspectionEvent>,
    pub task_order: Vec<String>,
    pub tasks: Vec<ComputationInspectionTask>,
    pub total_cost_units: u64,
    pub peak_memory_mb: u32,
    pub stale_task_order: Vec<String>,
    pub issue_order: Vec<String>,
    pub recovery_task_order: Vec<String>,
    pub next_action: String,
    pub disposition: ComputationInspectionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationRunInspectorError {
    #[error("computation run-inspector request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation run-inspector output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation run-inspector digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_NOTE_BYTES
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ComputationRunInspection) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "timeline": output.timeline,
        "task_order": output.task_order,
        "tasks": output.tasks,
        "total_cost_units": output.total_cost_units,
        "peak_memory_mb": output.peak_memory_mb,
        "stale_task_order": output.stale_task_order,
        "issue_order": output.issue_order,
        "recovery_task_order": output.recovery_task_order,
        "next_action": output.next_action,
        "disposition": output.disposition,
    })
}

fn status_for_disposition(
    disposition: ComputationTaskDisposition,
) -> ComputationInspectionTaskStatus {
    match disposition {
        ComputationTaskDisposition::Completed => ComputationInspectionTaskStatus::Completed,
        ComputationTaskDisposition::Cached => ComputationInspectionTaskStatus::Cached,
        ComputationTaskDisposition::Negative => ComputationInspectionTaskStatus::Negative,
        ComputationTaskDisposition::Partial => ComputationInspectionTaskStatus::Partial,
        ComputationTaskDisposition::Failed => ComputationInspectionTaskStatus::Failed,
        ComputationTaskDisposition::Skipped => ComputationInspectionTaskStatus::Skipped,
    }
}

impl ComputationRunInspection {
    pub fn validate(&self) -> Result<(), ComputationRunInspectorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.replay_identity.as_str().len() != 64
            || self
                .timeline
                .windows(2)
                .any(|pair| pair[0].sequence >= pair[1].sequence)
            || self.task_order.iter().collect::<BTreeSet<_>>().len() != self.task_order.len()
            || self.tasks.len() != self.task_order.len()
            || self
                .tasks
                .iter()
                .zip(&self.task_order)
                .any(|(task, task_id)| {
                    task.task_id != *task_id
                        || !safe_text(&task.task_id)
                        || !canonical(&task.terminal_sequence_order)
                        || !canonical(&task.partial_state_order)
                        || !canonical(&task.issue_order)
                        || task.artifact_id.as_ref().is_some_and(|id| !safe_text(id))
                })
            || !canonical(&self.stale_task_order)
            || !canonical(&self.issue_order)
            || !canonical(&self.recovery_task_order)
            || !safe_text(&self.next_action)
            || self.timeline.iter().any(|event| {
                event.sequence == 0 || !safe_text(&event.task_id) || !safe_text(&event.note)
            })
        {
            return Err(ComputationRunInspectorError::InvalidOutput(
                "identity, task/timeline ordering, bounds, or inspection invariants are invalid"
                    .into(),
            ));
        }
        let task_ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        if self
            .stale_task_order
            .iter()
            .any(|id| !task_ids.contains(id))
            || self
                .recovery_task_order
                .iter()
                .any(|id| !task_ids.contains(id))
            || self.tasks.iter().any(|task| {
                task.terminal_sequence_order.iter().any(|sequence| {
                    !self
                        .timeline
                        .iter()
                        .any(|event| event.sequence == *sequence)
                })
            })
        {
            return Err(ComputationRunInspectorError::InvalidOutput(
                "task references do not reconcile with the inspected timeline".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationRunInspectorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationRunInspectorError::InvalidOutput(
                "run-inspection digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ComputationRunInspectionRequest,
) -> Result<(), ComputationRunInspectorError> {
    if !safe_text(&request.objective)
        || request.replay_identity.as_str().len() != 64
        || request.execution.objective != request.objective
        || request.execution.replay_identity != request.replay_identity
        || request.events.len() > MAX_EVENTS
        || request.stale_after_ticks == 0
    {
        return Err(ComputationRunInspectorError::InvalidRequest(
            "objective/replay binding, bounded event history, and a positive staleness window are required".into(),
        ));
    }
    request
        .execution
        .validate()
        .map_err(|error| ComputationRunInspectorError::InvalidRequest(error.to_string()))?;
    if let Some(lineage) = &request.lineage {
        lineage
            .validate()
            .map_err(|error| ComputationRunInspectorError::InvalidRequest(error.to_string()))?;
        if lineage.objective != request.objective
            || lineage.replay_identity != request.replay_identity
        {
            return Err(ComputationRunInspectorError::InvalidRequest(
                "lineage is not bound to the inspected objective/replay identity".into(),
            ));
        }
    }
    if let Some(partial_results) = &request.partial_results {
        partial_results
            .validate()
            .map_err(|error| ComputationRunInspectorError::InvalidRequest(error.to_string()))?;
        if partial_results.objective != request.objective
            || partial_results.replay_identity != request.replay_identity
        {
            return Err(ComputationRunInspectorError::InvalidRequest(
                "partial-result bundle is not bound to the inspected objective/replay identity"
                    .into(),
            ));
        }
    }
    let task_ids = request
        .execution
        .task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut sequences = BTreeSet::new();
    let mut ordered_events = request.events.iter().collect::<Vec<_>>();
    ordered_events.sort_by_key(|event| event.sequence);
    let mut previous_tick = 0;
    for event in ordered_events {
        if event.sequence == 0
            || !sequences.insert(event.sequence)
            || !safe_text(&event.task_id)
            || !safe_text(&event.note)
            || event.tick < previous_tick
        {
            return Err(ComputationRunInspectorError::InvalidRequest(
                "events require unique positive sequences, bounded text, and non-decreasing ticks"
                    .into(),
            ));
        }
        previous_tick = event.tick;
        if !task_ids.contains(&event.task_id) {
            return Err(ComputationRunInspectorError::InvalidRequest(format!(
                "event {} references unknown task {}",
                event.sequence, event.task_id
            )));
        }
    }
    Ok(())
}

/// Produce a deterministic, lineage-linked inspection timeline and issue bundle for a local run.
pub fn inspect_glioma_computation_run(
    request: &ComputationRunInspectionRequest,
) -> Result<ComputationRunInspection, ComputationRunInspectorError> {
    validate_request(request)?;
    let task_ids = request
        .execution
        .task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let result_by_task = request
        .execution
        .task_results
        .iter()
        .map(|result| (result.task_id.clone(), result))
        .collect::<BTreeMap<_, _>>();
    let mut timeline = request.events.clone();
    timeline.sort_by_key(|event| event.sequence);
    let mut terminals = BTreeMap::<String, Vec<&ComputationInspectionEvent>>::new();
    let mut events_by_task = BTreeMap::<String, Vec<&ComputationInspectionEvent>>::new();
    for event in &timeline {
        events_by_task
            .entry(event.task_id.clone())
            .or_default()
            .push(event);
        if event.kind.is_terminal() {
            terminals
                .entry(event.task_id.clone())
                .or_default()
                .push(event);
        }
    }
    let mut tasks = Vec::new();
    let mut global_issues = Vec::new();
    let mut stale_task_order = Vec::new();
    let mut recovery_task_order = Vec::new();
    let mut total_cost_units = 0_u64;
    let mut peak_memory_mb = 0_u32;
    for task_id in &request.execution.task_order {
        let result = result_by_task
            .get(task_id)
            .expect("validated execution task identity");
        let task_events = events_by_task.get(task_id).cloned().unwrap_or_default();
        let terminal_events = terminals.get(task_id).cloned().unwrap_or_default();
        let terminal_sequence_order = terminal_events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>();
        let task_cost = task_events
            .iter()
            .fold(0_u64, |total, event| total.saturating_add(event.cost_units));
        let task_peak_memory = task_events
            .iter()
            .map(|event| event.memory_mb)
            .max()
            .unwrap_or(0);
        total_cost_units = total_cost_units.saturating_add(task_cost);
        peak_memory_mb = peak_memory_mb.max(task_peak_memory);
        let latest_tick = task_events.iter().map(|event| event.tick).max();
        let telemetry_stale = latest_tick.is_none_or(|tick| {
            request.observed_at_tick.saturating_sub(tick) > request.stale_after_ticks
        });
        if telemetry_stale {
            stale_task_order.push(task_id.clone());
        }
        let mut issue_order = Vec::new();
        if terminal_events.is_empty() {
            issue_order.push("missing-terminal-event".into());
        } else if terminal_events.last().is_some_and(|event| {
            let expected = match result.disposition {
                ComputationTaskDisposition::Completed => ComputationInspectionEventKind::Completed,
                ComputationTaskDisposition::Cached => ComputationInspectionEventKind::Cached,
                ComputationTaskDisposition::Negative => ComputationInspectionEventKind::Negative,
                ComputationTaskDisposition::Partial => ComputationInspectionEventKind::Partial,
                ComputationTaskDisposition::Failed => ComputationInspectionEventKind::Failed,
                ComputationTaskDisposition::Skipped => ComputationInspectionEventKind::Skipped,
            };
            event.kind != expected
        }) {
            issue_order.push("terminal-event-disposition-mismatch".into());
        }
        let artifact_id = result
            .artifact
            .as_ref()
            .map(|artifact| artifact.artifact_id.clone());
        let artifact_lineage_status =
            if let (Some(lineage), Some(artifact_id)) = (&request.lineage, artifact_id.as_ref()) {
                let status = lineage
                    .nodes
                    .iter()
                    .find(|node| node.artifact_id == *artifact_id)
                    .map(|node| node.status);
                if status.is_none() {
                    issue_order.push("artifact-lineage-unresolved".into());
                }
                status
            } else {
                None
            };
        let partial_state_order = request
            .partial_results
            .as_ref()
            .map(|partial| {
                let mut states = partial
                    .fields
                    .iter()
                    .filter(|field| field.task_id == *task_id)
                    .map(|field| field.state)
                    .collect::<Vec<_>>();
                states.sort();
                states.dedup();
                states
            })
            .unwrap_or_default();
        if partial_state_order.iter().any(|state| {
            matches!(
                state,
                PartialResultFieldState::Censored
                    | PartialResultFieldState::Interrupted
                    | PartialResultFieldState::Failed
                    | PartialResultFieldState::Unavailable
                    | PartialResultFieldState::Redacted
                    | PartialResultFieldState::Invalid
            )
        }) {
            issue_order.push("partial-result-limitations-present".into());
        }
        if matches!(
            result.disposition,
            ComputationTaskDisposition::Partial
                | ComputationTaskDisposition::Failed
                | ComputationTaskDisposition::Skipped
        ) {
            issue_order.push(format!("task-disposition:{:?}", result.disposition));
        }
        if telemetry_stale {
            issue_order.push("telemetry-stale".into());
        }
        issue_order.sort();
        issue_order.dedup();
        let status = if terminal_events.is_empty()
            || issue_order
                .iter()
                .any(|issue| issue == "terminal-event-disposition-mismatch")
        {
            ComputationInspectionTaskStatus::Unresolved
        } else {
            status_for_disposition(result.disposition)
        };
        if status != ComputationInspectionTaskStatus::Completed
            && status != ComputationInspectionTaskStatus::Cached
        {
            recovery_task_order.push(task_id.clone());
        }
        global_issues.extend(
            issue_order
                .iter()
                .map(|issue| format!("{task_id}: {issue}")),
        );
        tasks.push(ComputationInspectionTask {
            task_id: task_id.clone(),
            status,
            terminal_sequence_order,
            artifact_id,
            artifact_lineage_status,
            partial_state_order,
            attempt_count: result.attempt_count,
            retry_count: result.attempt_count.saturating_sub(1),
            cost_units: task_cost,
            peak_memory_mb: task_peak_memory,
            telemetry_stale,
            issue_order,
        });
    }
    let unknown_task_events = task_ids.len() < events_by_task.len();
    if unknown_task_events {
        global_issues.push("unknown-task-event".into());
    }
    stale_task_order.sort();
    recovery_task_order.sort();
    global_issues.sort();
    global_issues.dedup();
    let has_unresolved = tasks
        .iter()
        .any(|task| task.status == ComputationInspectionTaskStatus::Unresolved)
        || unknown_task_events;
    let has_failed = tasks
        .iter()
        .any(|task| task.status == ComputationInspectionTaskStatus::Failed);
    let has_partial = !recovery_task_order.is_empty();
    let disposition = if has_unresolved {
        ComputationInspectionDisposition::Unresolved
    } else if has_failed {
        ComputationInspectionDisposition::Failed
    } else if !stale_task_order.is_empty() {
        ComputationInspectionDisposition::Stale
    } else if has_partial {
        ComputationInspectionDisposition::Partial
    } else {
        ComputationInspectionDisposition::Ready
    };
    let next_action = match disposition {
        ComputationInspectionDisposition::Ready => {
            "inspect the replay-linked artifacts and route qualified outputs downstream".into()
        }
        ComputationInspectionDisposition::Partial => {
            "review partial tasks and their lineage before interpretation or recovery".into()
        }
        ComputationInspectionDisposition::Failed => {
            "open the reproducibility issue bundle and route failed tasks to recovery".into()
        }
        ComputationInspectionDisposition::Stale => {
            "refresh stale telemetry before making an operational claim about this run".into()
        }
        ComputationInspectionDisposition::Unresolved => {
            "repair missing or mismatched terminal events before treating the run as observed"
                .into()
        }
    };
    let mut output = ComputationRunInspection {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_identity: request.replay_identity.clone(),
        timeline,
        task_order: request.execution.task_order.clone(),
        tasks,
        total_cost_units,
        peak_memory_mb,
        stale_task_order,
        issue_order: global_issues,
        recovery_task_order,
        next_action,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-run-inspection"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationRunInspectorError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::execution::{
        ComputationExecutionDisposition, ComputationExecutionStopReason, ComputationOperation,
        ComputationTask, ComputationTaskResult,
    };
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn artifact(id: &str, schema: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: schema.into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn execution() -> ComputationExecution {
        let _task = ComputationTask {
            task_id: "integrate".into(),
            operation: ComputationOperation::Integrate,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            input_artifact_ids: vec!["raw".into()],
            output_schema: "GliomaIntegrated1@1".into(),
            estimated_cost_units: 1,
            estimated_duration_ticks: 1,
            deterministic: true,
        };
        let result = ComputationTaskResult {
            task_id: "integrate".into(),
            output_schema: "GliomaIntegrated1@1".into(),
            disposition: ComputationTaskDisposition::Completed,
            attempt_count: 1,
            artifact: Some(artifact("result", "GliomaIntegrated1@1")),
            cache_hit: false,
            note: "local result".into(),
        };
        let mut output = ComputationExecution {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "inspect integration".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: hash("inspection-replay"),
            task_order: vec!["integrate".into()],
            task_results: vec![result],
            completed_order: vec!["integrate".into()],
            cached_order: Vec::new(),
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            skipped_order: Vec::new(),
            budget_used_units: 3,
            duration_used_ticks: 3,
            cache_hit_count: 0,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: ComputationExecutionDisposition::Completed,
            stop_reason: ComputationExecutionStopReason::Completed,
            digest: hash("unsealed"),
        };
        output.digest = ContentHash::of_value(&super::super::execution::digest_input(&output))
            .expect("execution digest");
        output.validate().expect("execution");
        output
    }

    fn event(
        sequence: u64,
        kind: ComputationInspectionEventKind,
        tick: u64,
    ) -> ComputationInspectionEvent {
        ComputationInspectionEvent {
            sequence,
            task_id: "integrate".into(),
            tick,
            kind,
            cost_units: 1,
            memory_mb: 10,
            note: "local telemetry".into(),
        }
    }

    fn request(
        events: Vec<ComputationInspectionEvent>,
        observed_at_tick: u64,
    ) -> ComputationRunInspectionRequest {
        let execution = execution();
        ComputationRunInspectionRequest {
            objective: execution.objective.clone(),
            replay_identity: execution.replay_identity.clone(),
            execution,
            lineage: None,
            partial_results: None,
            events,
            observed_at_tick,
            stale_after_ticks: 10,
        }
    }

    #[test]
    fn complete_run_reconciles_terminal_event_and_counts() {
        let output = inspect_glioma_computation_run(&request(
            vec![
                event(1, ComputationInspectionEventKind::Started, 1),
                event(2, ComputationInspectionEventKind::ResourceSample, 2),
                event(3, ComputationInspectionEventKind::Completed, 3),
            ],
            4,
        ))
        .expect("inspection");
        assert_eq!(output.disposition, ComputationInspectionDisposition::Ready);
        assert_eq!(output.tasks[0].terminal_sequence_order, vec![3]);
        assert_eq!(output.total_cost_units, 3);
        assert!(output.issue_order.is_empty());
    }

    #[test]
    fn missing_terminal_is_unresolved_not_success() {
        let output = inspect_glioma_computation_run(&request(
            vec![event(1, ComputationInspectionEventKind::Started, 1)],
            2,
        ))
        .expect("inspection");
        assert_eq!(
            output.disposition,
            ComputationInspectionDisposition::Unresolved
        );
        assert!(output
            .issue_order
            .iter()
            .any(|issue| issue.contains("missing-terminal-event")));
        assert_eq!(
            output.tasks[0].status,
            ComputationInspectionTaskStatus::Unresolved
        );
    }

    #[test]
    fn stale_telemetry_is_visible_and_actionable() {
        let output = inspect_glioma_computation_run(&request(
            vec![event(1, ComputationInspectionEventKind::Completed, 1)],
            100,
        ))
        .expect("inspection");
        assert_eq!(output.disposition, ComputationInspectionDisposition::Stale);
        assert_eq!(output.stale_task_order, vec!["integrate"]);
        assert!(output
            .issue_order
            .iter()
            .any(|issue| issue.contains("telemetry-stale")));
    }

    #[test]
    fn insertion_order_does_not_change_inspection_digest() {
        let mut events = vec![
            event(2, ComputationInspectionEventKind::Completed, 2),
            event(1, ComputationInspectionEventKind::Started, 1),
        ];
        let left = inspect_glioma_computation_run(&request(events.clone(), 3)).expect("left");
        events.reverse();
        let right = inspect_glioma_computation_run(&request(events, 3)).expect("right");
        assert_eq!(left.digest, right.digest);
    }
}
