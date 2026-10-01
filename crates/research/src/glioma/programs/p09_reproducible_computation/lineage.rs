//! Artifact-lineage joins and adaptive recomputation frontiers for glioma computation runs.
//!
//! A computation result is useful only when its task, inputs, dependencies, replay identity, and
//! local artifact all reconcile.  This feature joins those records into a deterministic lineage
//! graph and expands every invalid or negative node to the smallest dependency-safe recomputation
//! frontier.  It does not execute workers, move raw data, or turn a failed computation into a
//! biological conclusion.

use super::execution::{
    ComputationTask, ComputationTaskDisposition, ComputationTaskResult, MAX_TASKS,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationLineage1@1";
pub const MAX_FRONTIER: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationLineageNodeStatus {
    Complete,
    Cached,
    Negative,
    Partial,
    Failed,
    Missing,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationLineageDisposition {
    Ready,
    RecomputeRequired,
    Blocked,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationLineageNode {
    pub task_id: String,
    pub depends_on: Vec<String>,
    pub input_artifact_ids: Vec<String>,
    pub output_artifact_id: Option<String>,
    pub output_content_hash: Option<ContentHash>,
    pub output_schema: String,
    pub task_disposition: Option<ComputationTaskDisposition>,
    pub status: ComputationLineageNodeStatus,
    pub issue_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationLineageRequest {
    pub objective: String,
    pub replay_identity: ContentHash,
    pub tasks: Vec<ComputationTask>,
    pub results: Vec<ComputationTaskResult>,
    pub max_frontier: usize,
    pub require_deterministic: bool,
    pub require_local_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationLineage {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub task_order: Vec<String>,
    pub root_order: Vec<String>,
    pub terminal_order: Vec<String>,
    pub nodes: Vec<ComputationLineageNode>,
    pub recomputation_frontier: Vec<String>,
    pub reusable_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationLineageDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationLineageError {
    #[error("computation lineage request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation lineage output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation lineage digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(lineage: &ComputationLineage) -> serde_json::Value {
    serde_json::json!({
        "feature_id": lineage.feature_id,
        "output_schema": lineage.output_schema,
        "objective": lineage.objective,
        "replay_identity": lineage.replay_identity,
        "task_order": lineage.task_order,
        "root_order": lineage.root_order,
        "terminal_order": lineage.terminal_order,
        "nodes": lineage.nodes,
        "recomputation_frontier": lineage.recomputation_frontier,
        "reusable_order": lineage.reusable_order,
        "negative_evidence": lineage.negative_evidence,
        "uncertainty": lineage.uncertainty,
        "disposition": lineage.disposition,
    })
}

impl ComputationLineage {
    pub fn validate(&self) -> Result<(), ComputationLineageError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || !canonical(&self.task_order)
            || !unique_nonempty(&self.task_order)
            || !canonical(&self.root_order)
            || !canonical(&self.terminal_order)
            || !canonical(&self.recomputation_frontier)
            || !canonical(&self.reusable_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.nodes.len() != self.task_order.len()
            || self.nodes.iter().any(|node| {
                node.task_id.trim().is_empty()
                    || !canonical(&node.depends_on)
                    || !canonical(&node.input_artifact_ids)
                    || !canonical(&node.issue_order)
            })
        {
            return Err(ComputationLineageError::InvalidOutput(
                "identity, graph ordering, node ordering, or digest shape is invalid".into(),
            ));
        }
        let task_ids = self
            .nodes
            .iter()
            .map(|node| node.task_id.clone())
            .collect::<BTreeSet<_>>();
        if task_ids != self.task_order.iter().cloned().collect::<BTreeSet<_>>()
            || self
                .root_order
                .iter()
                .chain(self.terminal_order.iter())
                .chain(self.recomputation_frontier.iter())
                .chain(self.reusable_order.iter())
                .any(|id| !task_ids.contains(id))
        {
            return Err(ComputationLineageError::InvalidOutput(
                "lineage partitions contain unknown tasks".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationLineageError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationLineageError::Digest(
                "computation lineage digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ComputationLineageRequest) -> Result<(), ComputationLineageError> {
    if request.objective.trim().is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
        || request.results.len() > request.tasks.len()
        || request.max_frontier == 0
        || request.max_frontier > MAX_FRONTIER
    {
        return Err(ComputationLineageError::InvalidRequest(
            "objective, replay identity, bounded tasks/results, and frontier limit are required"
                .into(),
        ));
    }
    let task_ids = request
        .tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<BTreeSet<_>>();
    if task_ids.len() != request.tasks.len()
        || request.tasks.iter().any(|task| {
            task.task_id.trim().is_empty()
                || task.output_schema.trim().is_empty()
                || task
                    .depends_on
                    .iter()
                    .any(|dependency| dependency == &task.task_id || !task_ids.contains(dependency))
                || (request.require_deterministic && !task.deterministic)
        })
    {
        return Err(ComputationLineageError::InvalidRequest(
            "task identities, dependency closure, schemas, and determinism requirements must reconcile".into(),
        ));
    }
    let result_ids = request
        .results
        .iter()
        .map(|result| result.task_id.clone())
        .collect::<BTreeSet<_>>();
    if result_ids.len() != request.results.len()
        || request.results.iter().any(|result| {
            result.task_id.trim().is_empty()
                || !task_ids.contains(&result.task_id)
                || result.attempt_count == 0
                || result.note.trim().is_empty()
                || result
                    .artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.validate().is_err())
        })
    {
        return Err(ComputationLineageError::InvalidRequest(
            "result identities, attempts, notes, and local artifact shapes must reconcile".into(),
        ));
    }
    Ok(())
}

fn topological_order(
    tasks: &BTreeMap<String, &ComputationTask>,
) -> Result<Vec<String>, ComputationLineageError> {
    let mut indegree = tasks
        .iter()
        .map(|(id, task)| (id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for (id, task) in tasks {
        for dependency in &task.depends_on {
            children
                .entry(dependency.clone())
                .or_default()
                .push(id.clone());
        }
    }
    for child_order in children.values_mut() {
        child_order.sort();
    }
    let mut queue = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<VecDeque<_>>();
    let mut order = Vec::new();
    while let Some(id) = queue.pop_front() {
        order.push(id.clone());
        if let Some(children) = children.get(&id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("child exists");
                *degree -= 1;
                if *degree == 0 {
                    let position = queue
                        .iter()
                        .position(|queued| queued > child)
                        .unwrap_or(queue.len());
                    queue.insert(position, child.clone());
                }
            }
        }
    }
    if order.len() != tasks.len() {
        return Err(ComputationLineageError::InvalidRequest(
            "computation task dependencies contain a cycle".into(),
        ));
    }
    Ok(order)
}

/// Join computation tasks and local results, then expand invalid/negative nodes to a bounded
/// dependency-safe recomputation frontier.
pub fn join_glioma_computation_lineage(
    request: &ComputationLineageRequest,
) -> Result<ComputationLineage, ComputationLineageError> {
    validate_request(request)?;
    let tasks = request
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let task_order = topological_order(&tasks)?;
    let results = request
        .results
        .iter()
        .map(|result| (result.task_id.clone(), result))
        .collect::<BTreeMap<_, _>>();
    let mut nodes = Vec::new();
    let mut seed_frontier = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for task_id in &task_order {
        let task = tasks.get(task_id).expect("task exists");
        let result = results.get(task_id).copied();
        let mut issues = BTreeSet::new();
        let mut status = match result.map(|result| result.disposition) {
            Some(ComputationTaskDisposition::Completed) => ComputationLineageNodeStatus::Complete,
            Some(ComputationTaskDisposition::Cached) => ComputationLineageNodeStatus::Cached,
            Some(ComputationTaskDisposition::Negative) => ComputationLineageNodeStatus::Negative,
            Some(ComputationTaskDisposition::Partial) => ComputationLineageNodeStatus::Partial,
            Some(ComputationTaskDisposition::Failed) => ComputationLineageNodeStatus::Failed,
            Some(ComputationTaskDisposition::Skipped) => ComputationLineageNodeStatus::Blocked,
            None => ComputationLineageNodeStatus::Missing,
        };
        if result.is_none() {
            issues.insert("missing-result".into());
        }
        if let Some(result) = result {
            if result.output_schema != task.output_schema {
                issues.insert("output-schema-mismatch".into());
            }
            let requires_artifact = matches!(
                result.disposition,
                ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached
            );
            if request.require_local_artifacts && requires_artifact && result.artifact.is_none() {
                issues.insert("missing-local-artifact".into());
            }
            if result
                .artifact
                .as_ref()
                .is_some_and(|artifact| !artifact.local_only)
            {
                issues.insert("non-local-artifact".into());
            }
            if matches!(result.disposition, ComputationTaskDisposition::Negative) {
                negative_evidence.insert(format!("{}:negative-result", task_id));
            }
            if matches!(
                result.disposition,
                ComputationTaskDisposition::Failed
                    | ComputationTaskDisposition::Partial
                    | ComputationTaskDisposition::Skipped
            ) {
                issues.insert(format!("result:{:?}", result.disposition));
            }
        }
        for dependency in &task.depends_on {
            if let Some(dependency_node) = nodes
                .iter()
                .find(|node: &&ComputationLineageNode| &node.task_id == dependency)
            {
                if !matches!(
                    dependency_node.status,
                    ComputationLineageNodeStatus::Complete | ComputationLineageNodeStatus::Cached
                ) {
                    issues.insert(format!("dependency-not-reusable:{dependency}"));
                }
            }
        }
        if request.require_deterministic && !task.deterministic {
            issues.insert("non-deterministic-task".into());
        }
        if !issues.is_empty() {
            seed_frontier.insert(task_id.clone());
            if matches!(
                status,
                ComputationLineageNodeStatus::Complete | ComputationLineageNodeStatus::Cached
            ) {
                status = ComputationLineageNodeStatus::Blocked;
            }
            uncertainty.extend(issues.iter().map(|issue| format!("{task_id}:{issue}")));
        }
        let (output_artifact_id, output_content_hash) = result
            .and_then(|result| result.artifact.as_ref())
            .map(|artifact| {
                (
                    Some(artifact.artifact_id.clone()),
                    Some(artifact.content_hash.clone()),
                )
            })
            .unwrap_or((None, None));
        nodes.push(ComputationLineageNode {
            task_id: task_id.clone(),
            depends_on: {
                let mut values = task.depends_on.clone();
                values.sort();
                values
            },
            input_artifact_ids: {
                let mut values = task.input_artifact_ids.clone();
                values.sort();
                values
            },
            output_artifact_id,
            output_content_hash,
            output_schema: task.output_schema.clone(),
            task_disposition: result.map(|result| result.disposition),
            status,
            issue_order: issues.into_iter().collect(),
        });
    }
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.values() {
        for dependency in &task.depends_on {
            children
                .entry(dependency.clone())
                .or_default()
                .push(task.task_id.clone());
        }
    }
    for descendants in children.values_mut() {
        descendants.sort();
    }
    let mut frontier = seed_frontier.clone();
    let mut queue = seed_frontier.into_iter().collect::<VecDeque<_>>();
    while let Some(task_id) = queue.pop_front() {
        for child in children.get(&task_id).into_iter().flatten() {
            if frontier.insert(child.clone()) {
                queue.push_back(child.clone());
            }
        }
    }
    if frontier.len() > request.max_frontier {
        return Err(ComputationLineageError::InvalidRequest(
            "recomputation frontier exceeds the caller limit".into(),
        ));
    }
    let recomputation_frontier = task_order
        .iter()
        .filter(|task_id| frontier.contains(*task_id))
        .cloned()
        .collect::<Vec<_>>();
    let reusable_order = task_order
        .iter()
        .filter(|task_id| !frontier.contains(*task_id))
        .cloned()
        .collect::<Vec<_>>();
    let root_order = task_order
        .iter()
        .filter(|task_id| {
            tasks
                .get(*task_id)
                .is_some_and(|task| task.depends_on.is_empty())
        })
        .cloned()
        .collect::<Vec<_>>();
    let terminal_order = task_order
        .iter()
        .filter(|task_id| !children.contains_key(*task_id))
        .cloned()
        .collect::<Vec<_>>();
    let disposition = if nodes
        .iter()
        .any(|node| node.status == ComputationLineageNodeStatus::Negative)
        && recomputation_frontier.is_empty()
    {
        ComputationLineageDisposition::Negative
    } else if nodes
        .iter()
        .any(|node| node.status == ComputationLineageNodeStatus::Blocked)
    {
        ComputationLineageDisposition::Blocked
    } else if recomputation_frontier.is_empty() {
        ComputationLineageDisposition::Ready
    } else {
        ComputationLineageDisposition::RecomputeRequired
    };
    let mut lineage = ComputationLineage {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_identity: request.replay_identity.clone(),
        task_order,
        root_order,
        terminal_order,
        nodes,
        recomputation_frontier,
        reusable_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    lineage.digest = ContentHash::of_value(&digest_input(&lineage))
        .map_err(|error| ComputationLineageError::Digest(error.to_string()))?;
    lineage.validate()?;
    Ok(lineage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn task(task_id: &str, depends_on: Vec<&str>, deterministic: bool) -> ComputationTask {
        ComputationTask {
            task_id: task_id.into(),
            operation: super::super::execution::ComputationOperation::ModelFit,
            model_system: crate::glioma_engine::GliomaModelSystem::Organoid,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            input_artifact_ids: vec![format!("input:{task_id}")],
            output_schema: format!("application/vnd.aurora.{task_id}"),
            estimated_cost_units: 1,
            estimated_duration_ticks: 1,
            deterministic,
        }
    }

    fn result(
        task: &ComputationTask,
        disposition: ComputationTaskDisposition,
    ) -> ComputationTaskResult {
        ComputationTaskResult {
            task_id: task.task_id.clone(),
            output_schema: task.output_schema.clone(),
            disposition,
            attempt_count: 1,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("artifact:{}", task.task_id),
                content_hash: hash(&task.task_id),
                content_type: task.output_schema.clone(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            cache_hit: disposition == ComputationTaskDisposition::Cached,
            note: "local result".into(),
        }
    }

    #[test]
    fn lineage_expands_failed_node_to_dependency_safe_frontier() {
        let first = task("a", vec![], true);
        let second = task("b", vec!["a"], true);
        let third = task("c", vec!["b"], true);
        let lineage = join_glioma_computation_lineage(&ComputationLineageRequest {
            objective: "lineage test".into(),
            replay_identity: hash("replay"),
            tasks: vec![first.clone(), second.clone(), third.clone()],
            results: vec![
                result(&first, ComputationTaskDisposition::Completed),
                result(&second, ComputationTaskDisposition::Failed),
                result(&third, ComputationTaskDisposition::Completed),
            ],
            max_frontier: 3,
            require_deterministic: true,
            require_local_artifacts: true,
        })
        .unwrap();
        assert_eq!(lineage.recomputation_frontier, vec!["b", "c"]);
        assert_eq!(lineage.reusable_order, vec!["a"]);
        assert_eq!(lineage.disposition, ComputationLineageDisposition::Blocked);
        lineage.validate().unwrap();
    }

    #[test]
    fn lineage_rejects_nondeterministic_tasks_when_required() {
        let error = join_glioma_computation_lineage(&ComputationLineageRequest {
            objective: "lineage test".into(),
            replay_identity: hash("replay"),
            tasks: vec![task("a", vec![], false)],
            results: Vec::new(),
            max_frontier: 2,
            require_deterministic: true,
            require_local_artifacts: true,
        })
        .unwrap_err();
        assert!(error.to_string().contains("determinism"));
    }
}
