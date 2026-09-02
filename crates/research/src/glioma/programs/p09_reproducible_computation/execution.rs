//! Replayable execution of multimodal glioma computation graphs.
//!
//! The robustness battery in [`super::robustness`] evaluates a completed analysis. This module
//! supplies the missing production computation seam: it validates a typed DAG, chooses a stable
//! topological order, admits work under an explicit budget, reuses only replay-keyed local cache
//! artifacts, and retries bounded transient worker failures. The caller owns the actual compute
//! process; this crate never downloads data, executes code, or moves raw experimental payloads.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationExecution1@1";
pub const MAX_TASKS: usize = 2_048;
pub const MAX_INPUTS_PER_TASK: usize = 128;
pub const MAX_RETRIES: u8 = 8;
pub const MAX_COST_UNITS: u64 = 1_000_000;
pub const MAX_DURATION_TICKS: u64 = 10_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationOperation {
    Ingest,
    Normalize,
    Register,
    Segment,
    Quantify,
    Integrate,
    ModelFit,
    Validate,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationTask {
    pub task_id: String,
    pub operation: ComputationOperation,
    pub model_system: GliomaModelSystem,
    pub depends_on: Vec<String>,
    pub input_artifact_ids: Vec<String>,
    pub output_schema: String,
    pub estimated_cost_units: u64,
    pub estimated_duration_ticks: u64,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCacheEntry {
    pub task_id: String,
    pub replay_identity: ContentHash,
    pub output_schema: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationExecutionRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub tasks: Vec<ComputationTask>,
    pub replay_identity: ContentHash,
    pub max_budget_units: u64,
    pub max_retries: u8,
    pub allow_cache: bool,
    pub require_local_artifacts: bool,
    pub cache: Vec<ComputationCacheEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationTaskDisposition {
    Completed,
    Cached,
    Negative,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationTaskResult {
    pub task_id: String,
    pub output_schema: String,
    pub disposition: ComputationTaskDisposition,
    pub attempt_count: u8,
    pub artifact: Option<LocalArtifactRef>,
    pub cache_hit: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputationExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local worker seam. Implementations may invoke a container, scheduler, GPU worker,
/// or workflow engine, but the research crate only receives a typed local result.
pub trait GliomaComputationExecutor {
    fn execute_task(
        &mut self,
        task: &ComputationTask,
        upstream: &[ComputationTaskResult],
        attempt: u8,
    ) -> Result<ComputationTaskResult, ComputationExecutionFailure>;
}

/// Deterministic sandbox worker that emits synthetic local artifacts and performs no computation
/// on biological data. It is useful for SDK/MCP contract tests and workflow dry runs.
#[derive(Debug, Default)]
pub struct DryRunGliomaComputationExecutor;

impl GliomaComputationExecutor for DryRunGliomaComputationExecutor {
    fn execute_task(
        &mut self,
        task: &ComputationTask,
        upstream: &[ComputationTaskResult],
        _attempt: u8,
    ) -> Result<ComputationTaskResult, ComputationExecutionFailure> {
        let upstream_ids = upstream
            .iter()
            .map(|result| result.task_id.clone())
            .collect::<Vec<_>>();
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "task_id": task.task_id,
            "operation": task.operation,
            "output_schema": task.output_schema,
            "upstream": upstream_ids,
            "simulation_only": true,
        }))
        .map_err(|error| ComputationExecutionFailure {
            reason: format!("dry-run artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(ComputationTaskResult {
            task_id: task.task_id.clone(),
            output_schema: task.output_schema.clone(),
            disposition: ComputationTaskDisposition::Completed,
            attempt_count: 1,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run-computation:{}", task.task_id),
                content_hash,
                content_type: task.output_schema.clone(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            cache_hit: false,
            note: "dry-run computation completed; no biological data or external effect occurred"
                .into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationExecutionDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationExecutionStopReason {
    Completed,
    BudgetExhausted,
    TaskFailed,
    DependencyBlocked,
    GraphInvalid,
    ExecutorRefused,
    ReplayMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub task_order: Vec<String>,
    pub task_results: Vec<ComputationTaskResult>,
    pub completed_order: Vec<String>,
    pub cached_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub budget_used_units: u64,
    pub duration_used_ticks: u64,
    pub cache_hit_count: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ComputationExecutionDisposition,
    pub stop_reason: ComputationExecutionStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationExecutionError {
    #[error("computation execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation execution graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("computation execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ComputationExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "replay_identity": output.replay_identity,
        "task_order": output.task_order,
        "task_results": output.task_results,
        "completed_order": output.completed_order,
        "cached_order": output.cached_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "skipped_order": output.skipped_order,
        "budget_used_units": output.budget_used_units,
        "duration_used_ticks": output.duration_used_ticks,
        "cache_hit_count": output.cache_hit_count,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl ComputationExecution {
    pub fn validate(&self) -> Result<(), ComputationExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.task_order.is_empty()
            || self.task_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.task_results.len() != self.task_order.len()
            || self
                .task_results
                .iter()
                .zip(&self.task_order)
                .any(|(result, task_id)| {
                    result.task_id != *task_id
                        || result.task_id.trim().is_empty()
                        || result.output_schema.trim().is_empty()
                        || result.note.trim().is_empty()
                        || (result.disposition == ComputationTaskDisposition::Skipped
                            && result.attempt_count != 0)
                        || (result.disposition != ComputationTaskDisposition::Skipped
                            && result.attempt_count == 0)
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
            || !canonical(&self.cached_order)
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
            return Err(ComputationExecutionError::InvalidOutput(
                "identity, task results, artifacts, ordering, or limitations are invalid".into(),
            ));
        }
        let ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let result_ids = self
            .task_results
            .iter()
            .map(|result| result.task_id.clone())
            .collect::<BTreeSet<_>>();
        if ids != result_ids {
            return Err(ComputationExecutionError::InvalidOutput(
                "task order and result identities do not reconcile".into(),
            ));
        }
        for (order, disposition) in [
            (&self.completed_order, ComputationTaskDisposition::Completed),
            (&self.cached_order, ComputationTaskDisposition::Cached),
            (&self.negative_order, ComputationTaskDisposition::Negative),
            (&self.partial_order, ComputationTaskDisposition::Partial),
            (&self.failed_order, ComputationTaskDisposition::Failed),
            (&self.skipped_order, ComputationTaskDisposition::Skipped),
        ] {
            let expected = self
                .task_results
                .iter()
                .filter(|result| result.disposition == disposition)
                .map(|result| result.task_id.clone())
                .collect::<BTreeSet<_>>();
            if order.iter().cloned().collect::<BTreeSet<_>>() != expected {
                return Err(ComputationExecutionError::InvalidOutput(
                    "task disposition partitions do not reconcile".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationExecutionError::InvalidOutput(
                "execution digest is not bound to task outcomes".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ComputationExecutionRequest,
) -> Result<(), ComputationExecutionError> {
    if request.objective.trim().is_empty()
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
        || request.replay_identity.as_str().len() != 64
        || request.max_budget_units == 0
        || request.max_budget_units > MAX_COST_UNITS
        || request.max_retries > MAX_RETRIES
    {
        return Err(ComputationExecutionError::InvalidRequest(
            "objective, bounded tasks, replay identity, budget, and retry policy are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || !ids.insert(task.task_id.clone())
            || task.output_schema.trim().is_empty()
            || task
                .depends_on
                .iter()
                .any(|dependency| dependency == &task.task_id)
            || task.input_artifact_ids.len() > MAX_INPUTS_PER_TASK
            || task.estimated_cost_units == 0
            || task.estimated_cost_units > MAX_COST_UNITS
            || task.estimated_duration_ticks == 0
            || task.estimated_duration_ticks > MAX_DURATION_TICKS
        {
            return Err(ComputationExecutionError::InvalidRequest(format!(
                "task {} has invalid identity, schema, dependency, input, cost, or duration bounds",
                task.task_id
            )));
        }
        let mut dependencies = BTreeSet::new();
        if task
            .depends_on
            .iter()
            .any(|dependency| !dependencies.insert(dependency))
        {
            return Err(ComputationExecutionError::InvalidRequest(format!(
                "task {} repeats a dependency",
                task.task_id
            )));
        }
    }
    Ok(())
}

fn stable_topological_order(
    tasks: &[ComputationTask],
) -> Result<Vec<String>, ComputationExecutionError> {
    let task_ids = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<BTreeSet<_>>();
    let mut indegree = tasks
        .iter()
        .map(|task| (task.task_id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for task in tasks {
        for dependency in &task.depends_on {
            if !task_ids.contains(dependency) {
                return Err(ComputationExecutionError::InvalidGraph(format!(
                    "task {} depends on unknown task {}",
                    task.task_id, dependency
                )));
            }
            children
                .entry(dependency.clone())
                .or_default()
                .push(task.task_id.clone());
        }
    }
    for values in children.values_mut() {
        values.sort();
    }
    let mut ready = tasks
        .iter()
        .filter(|task| task.depends_on.is_empty())
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    ready.sort();
    let mut queue = VecDeque::from(ready);
    let mut order = Vec::with_capacity(tasks.len());
    while let Some(task_id) = queue.pop_front() {
        order.push(task_id.clone());
        if let Some(task_children) = children.get(&task_id) {
            for child in task_children {
                let degree = indegree.get_mut(child).ok_or_else(|| {
                    ComputationExecutionError::InvalidGraph("indegree missing".into())
                })?;
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
            let mut values = queue.drain(..).collect::<Vec<_>>();
            values.sort();
            queue.extend(values);
        }
    }
    if order.len() != tasks.len() {
        return Err(ComputationExecutionError::InvalidGraph(
            "computation dependency graph contains a cycle".into(),
        ));
    }
    Ok(order)
}

fn validate_provider_result(
    task: &ComputationTask,
    mut result: ComputationTaskResult,
    attempt: u8,
    require_local_artifacts: bool,
) -> Result<ComputationTaskResult, ComputationExecutionError> {
    if result.task_id != task.task_id
        || result.output_schema != task.output_schema
        || result.note.trim().is_empty()
        || result.cache_hit
        || result.disposition == ComputationTaskDisposition::Cached
        || result.disposition == ComputationTaskDisposition::Skipped
    {
        return Err(ComputationExecutionError::InvalidOutput(format!(
            "executor returned an invalid result for task {}",
            task.task_id
        )));
    }
    if require_local_artifacts && result.artifact.is_none() {
        return Err(ComputationExecutionError::InvalidOutput(format!(
            "executor returned no required local artifact for task {}",
            task.task_id
        )));
    }
    if let Some(artifact) = &result.artifact {
        artifact
            .validate()
            .map_err(|error| ComputationExecutionError::InvalidOutput(error.to_string()))?;
        if artifact.content_type != task.output_schema {
            return Err(ComputationExecutionError::InvalidOutput(format!(
                "task {} artifact content type does not match {}",
                task.task_id, task.output_schema
            )));
        }
    }
    result.attempt_count = attempt;
    Ok(result)
}

fn cache_result(
    task: &ComputationTask,
    cache: &BTreeMap<String, &ComputationCacheEntry>,
    replay_identity: &ContentHash,
    require_local_artifacts: bool,
) -> Result<Option<ComputationTaskResult>, ComputationExecutionError> {
    let Some(entry) = cache.get(&task.task_id) else {
        return Ok(None);
    };
    if &entry.replay_identity != replay_identity || entry.output_schema != task.output_schema {
        return Err(ComputationExecutionError::InvalidOutput(format!(
            "cache entry for task {} does not match replay identity or schema",
            task.task_id
        )));
    }
    entry
        .artifact
        .validate()
        .map_err(|error| ComputationExecutionError::InvalidOutput(error.to_string()))?;
    if require_local_artifacts && !entry.artifact.local_only {
        return Err(ComputationExecutionError::InvalidOutput(format!(
            "cache entry for task {} is not local-only",
            task.task_id
        )));
    }
    Ok(Some(ComputationTaskResult {
        task_id: task.task_id.clone(),
        output_schema: task.output_schema.clone(),
        disposition: ComputationTaskDisposition::Cached,
        attempt_count: 1,
        artifact: Some(entry.artifact.clone()),
        cache_hit: true,
        note: "reused replay-keyed local computation artifact".into(),
    }))
}

/// Execute a validated computation DAG through a caller-owned local executor.
pub fn execute_glioma_computation<E: GliomaComputationExecutor>(
    request: &ComputationExecutionRequest,
    executor: &mut E,
) -> Result<ComputationExecution, ComputationExecutionError> {
    validate_request(request)?;
    let order = stable_topological_order(&request.tasks)?;
    let task_map = request
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let cache = request
        .cache
        .iter()
        .map(|entry| (entry.task_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    if cache.len() != request.cache.len() {
        return Err(ComputationExecutionError::InvalidRequest(
            "cache task identities must be unique".into(),
        ));
    }
    let mut by_task = BTreeMap::<String, ComputationTaskResult>::new();
    let mut task_results = Vec::with_capacity(order.len());
    let mut budget_used_units = 0_u64;
    let mut duration_used_ticks = 0_u64;
    let mut cache_hit_count = 0_u32;
    let mut retry_count = 0_u32;
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut stop_reason = ComputationExecutionStopReason::Completed;
    let mut halted = false;
    for task_id in &order {
        let task = task_map.get(task_id).ok_or_else(|| {
            ComputationExecutionError::InvalidGraph(format!("task {} disappeared", task_id))
        })?;
        if halted {
            let result = ComputationTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ComputationTaskDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                cache_hit: false,
                note:
                    "skipped after an earlier computation failure, partial result, or budget stop"
                        .into(),
            };
            by_task.insert(task.task_id.clone(), result.clone());
            task_results.push(result);
            continue;
        }
        let blocked_dependency = task.depends_on.iter().any(|dependency| {
            !matches!(
                by_task.get(dependency).map(|result| result.disposition),
                Some(
                    ComputationTaskDisposition::Completed
                        | ComputationTaskDisposition::Cached
                        | ComputationTaskDisposition::Negative
                )
            )
        });
        if blocked_dependency {
            let result = ComputationTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ComputationTaskDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                cache_hit: false,
                note: "skipped because a dependency did not produce reusable output".into(),
            };
            by_task.insert(task.task_id.clone(), result.clone());
            task_results.push(result);
            halted = true;
            stop_reason = ComputationExecutionStopReason::DependencyBlocked;
            continue;
        }
        if request.allow_cache {
            if let Some(result) = cache_result(
                task,
                &cache,
                &request.replay_identity,
                request.require_local_artifacts,
            )? {
                cache_hit_count = cache_hit_count.saturating_add(1);
                by_task.insert(task.task_id.clone(), result.clone());
                task_results.push(result);
                budget_used_units = budget_used_units.saturating_add(task.estimated_cost_units);
                duration_used_ticks =
                    duration_used_ticks.saturating_add(task.estimated_duration_ticks);
                continue;
            }
        }
        if budget_used_units.saturating_add(task.estimated_cost_units) > request.max_budget_units {
            uncertainty.insert("computation-budget-exhausted-before-task".into());
            let result = ComputationTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ComputationTaskDisposition::Skipped,
                attempt_count: 0,
                artifact: None,
                cache_hit: false,
                note: "skipped because the declared computation budget was exhausted".into(),
            };
            by_task.insert(task.task_id.clone(), result.clone());
            task_results.push(result);
            halted = true;
            stop_reason = ComputationExecutionStopReason::BudgetExhausted;
            continue;
        }
        let upstream = task
            .depends_on
            .iter()
            .filter_map(|dependency| by_task.get(dependency).cloned())
            .collect::<Vec<_>>();
        let mut final_result = None;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_task(task, &upstream, attempt) {
                Ok(result) => {
                    let result = validate_provider_result(
                        task,
                        result,
                        attempt,
                        request.require_local_artifacts,
                    )?;
                    if result.disposition == ComputationTaskDisposition::Partial {
                        halted = true;
                        stop_reason = ComputationExecutionStopReason::DependencyBlocked;
                    }
                    final_result = Some(result);
                    break;
                }
                Err(failure) => {
                    if failure.reason.trim().is_empty() {
                        return Err(ComputationExecutionError::InvalidOutput(format!(
                            "executor returned an empty failure reason for task {}",
                            task.task_id
                        )));
                    }
                    if failure.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        continue;
                    }
                    final_result = Some(ComputationTaskResult {
                        task_id: task.task_id.clone(),
                        output_schema: task.output_schema.clone(),
                        disposition: ComputationTaskDisposition::Failed,
                        attempt_count: attempt,
                        artifact: None,
                        cache_hit: false,
                        note: failure.reason,
                    });
                    halted = true;
                    stop_reason = ComputationExecutionStopReason::TaskFailed;
                    break;
                }
            }
        }
        let result = final_result.ok_or_else(|| {
            ComputationExecutionError::InvalidOutput(format!(
                "executor produced no result for task {}",
                task.task_id
            ))
        })?;
        budget_used_units = budget_used_units.saturating_add(task.estimated_cost_units);
        duration_used_ticks = duration_used_ticks.saturating_add(task.estimated_duration_ticks);
        if result.disposition == ComputationTaskDisposition::Negative {
            negative_evidence.insert(format!("{}:{}", task.task_id, result.note));
        }
        if result.disposition == ComputationTaskDisposition::Partial {
            negative_evidence.insert(format!("{}:partial-result", task.task_id));
        }
        by_task.insert(task.task_id.clone(), result.clone());
        task_results.push(result);
    }
    if retry_count > 0 {
        uncertainty.insert(format!("{retry_count}-retry-attempts-required"));
    }
    if task_results
        .iter()
        .any(|result| result.disposition == ComputationTaskDisposition::Skipped)
    {
        uncertainty.insert("skipped-work-is-not-computation-evidence".into());
    }
    let mut completed_order = Vec::new();
    let mut cached_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut partial_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut skipped_order = Vec::new();
    for result in &task_results {
        match result.disposition {
            ComputationTaskDisposition::Completed => completed_order.push(result.task_id.clone()),
            ComputationTaskDisposition::Cached => cached_order.push(result.task_id.clone()),
            ComputationTaskDisposition::Negative => negative_order.push(result.task_id.clone()),
            ComputationTaskDisposition::Partial => partial_order.push(result.task_id.clone()),
            ComputationTaskDisposition::Failed => failed_order.push(result.task_id.clone()),
            ComputationTaskDisposition::Skipped => skipped_order.push(result.task_id.clone()),
        }
    }
    for values in [
        &mut completed_order,
        &mut cached_order,
        &mut negative_order,
        &mut partial_order,
        &mut failed_order,
        &mut skipped_order,
    ] {
        values.sort();
    }
    let disposition = if !failed_order.is_empty() {
        ComputationExecutionDisposition::Failed
    } else if !partial_order.is_empty() || !skipped_order.is_empty() {
        ComputationExecutionDisposition::Partial
    } else {
        ComputationExecutionDisposition::Completed
    };
    let mut output = ComputationExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        task_order: order,
        task_results,
        completed_order,
        cached_order,
        negative_order,
        partial_order,
        failed_order,
        skipped_order,
        budget_used_units,
        duration_used_ticks,
        cache_hit_count,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn task(id: &str, depends_on: Vec<&str>) -> ComputationTask {
        ComputationTask {
            task_id: id.into(),
            operation: if id == "fit" {
                ComputationOperation::ModelFit
            } else {
                ComputationOperation::Normalize
            },
            model_system: GliomaModelSystem::Organoid,
            depends_on: depends_on.into_iter().map(str::to_owned).collect(),
            input_artifact_ids: vec![format!("input:{id}")],
            output_schema: format!("{id}1@1"),
            estimated_cost_units: 2,
            estimated_duration_ticks: 1,
            deterministic: true,
        }
    }

    fn request() -> ComputationExecutionRequest {
        ComputationExecutionRequest {
            objective: "replay a glioma organoid multimodal computation".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![task("fit", vec!["normalize"]), task("normalize", vec![])],
            replay_identity: hash("replay-1"),
            max_budget_units: 10,
            max_retries: 1,
            allow_cache: true,
            require_local_artifacts: true,
            cache: Vec::new(),
        }
    }

    struct RecordingExecutor {
        calls: Vec<String>,
        fail_once: bool,
    }

    impl GliomaComputationExecutor for RecordingExecutor {
        fn execute_task(
            &mut self,
            task: &ComputationTask,
            _upstream: &[ComputationTaskResult],
            attempt: u8,
        ) -> Result<ComputationTaskResult, ComputationExecutionFailure> {
            self.calls.push(format!("{}:{attempt}", task.task_id));
            if self.fail_once && task.task_id == "normalize" && attempt == 1 {
                self.fail_once = false;
                return Err(ComputationExecutionFailure {
                    reason: "transient worker restart".into(),
                    retryable: true,
                });
            }
            let artifact = LocalArtifactRef {
                artifact_id: format!("artifact:{}", task.task_id),
                content_hash: hash(&task.task_id),
                content_type: task.output_schema.clone(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            };
            Ok(ComputationTaskResult {
                task_id: task.task_id.clone(),
                output_schema: task.output_schema.clone(),
                disposition: ComputationTaskDisposition::Completed,
                attempt_count: attempt,
                artifact: Some(artifact),
                cache_hit: false,
                note: "worker completed local computation".into(),
            })
        }
    }

    #[test]
    fn topological_execution_retries_and_replays_stably() {
        let mut executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: true,
        };
        let output = execute_glioma_computation(&request(), &mut executor).unwrap();
        assert_eq!(executor.calls, vec!["normalize:1", "normalize:2", "fit:1"]);
        assert_eq!(
            output.disposition,
            ComputationExecutionDisposition::Completed
        );
        assert_eq!(output.task_order, vec!["normalize", "fit"]);
        assert_eq!(output.uncertainty, vec!["1-retry-attempts-required"]);
        output.validate().unwrap();
    }

    #[test]
    fn replay_cache_avoids_worker_and_preserves_artifact() {
        let mut first_executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: false,
        };
        let first = execute_glioma_computation(&request(), &mut first_executor).unwrap();
        let mut cached_request = request();
        cached_request.cache = first
            .task_results
            .iter()
            .map(|result| ComputationCacheEntry {
                task_id: result.task_id.clone(),
                replay_identity: cached_request.replay_identity.clone(),
                output_schema: result.output_schema.clone(),
                artifact: result.artifact.clone().unwrap(),
            })
            .collect();
        let mut second_executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: false,
        };
        let second = execute_glioma_computation(&cached_request, &mut second_executor).unwrap();
        assert!(second_executor.calls.is_empty());
        assert_eq!(second.cached_order, vec!["fit", "normalize"]);
        assert_eq!(second.cache_hit_count, 2);
        assert_eq!(
            first.task_results[0].artifact,
            second.task_results[0].artifact
        );
    }

    #[test]
    fn budget_exhaustion_is_partial_and_skips_dependents() {
        let mut request = request();
        request.max_budget_units = 2;
        let mut executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: false,
        };
        let output = execute_glioma_computation(&request, &mut executor).unwrap();
        assert_eq!(output.disposition, ComputationExecutionDisposition::Partial);
        assert_eq!(
            output.stop_reason,
            ComputationExecutionStopReason::BudgetExhausted
        );
        assert_eq!(output.skipped_order, vec!["fit"]);
    }

    #[test]
    fn cycles_are_refused_before_worker_call() {
        let mut request = request();
        request.tasks = vec![task("a", vec!["b"]), task("b", vec!["a"])]
            .into_iter()
            .collect();
        let mut executor = RecordingExecutor {
            calls: Vec::new(),
            fail_once: false,
        };
        assert!(matches!(
            execute_glioma_computation(&request, &mut executor),
            Err(ComputationExecutionError::InvalidGraph(_))
        ));
        assert!(executor.calls.is_empty());
    }
}
