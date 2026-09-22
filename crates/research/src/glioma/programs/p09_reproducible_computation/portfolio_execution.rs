//! Autonomous bridge from computation-portfolio selection to local execution.
//!
//! P09-F11 chooses a bounded multimodal portfolio. This module turns that selected closure into
//! the existing typed computation request and executes it through a caller-owned worker. The
//! bridge never invents a task, bypasses a prerequisite, downloads code, moves raw data, or
//! converts a partial computation into a scientific conclusion.

use super::execution::{
    execute_glioma_computation, ComputationCacheEntry, ComputationExecution,
    ComputationExecutionDisposition, ComputationExecutionError, ComputationExecutionRequest,
    GliomaComputationExecutor,
};
use super::planning::{
    plan_glioma_computation_portfolio, ComputationCandidate, ComputationPortfolioError,
    ComputationPortfolioPlan, ComputationPortfolioRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationPortfolioExecution1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPortfolioExecutionRequest {
    pub portfolio: ComputationPortfolioRequest,
    pub candidates: Vec<ComputationCandidate>,
    pub replay_identity: ContentHash,
    pub max_retries: u8,
    pub allow_cache: bool,
    pub require_local_artifacts: bool,
    pub cache: Vec<ComputationCacheEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationPortfolioExecutionDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPortfolioExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan: ComputationPortfolioPlan,
    pub execution: Option<ComputationExecution>,
    pub planned_task_order: Vec<String>,
    pub execution_digest: Option<ContentHash>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationPortfolioExecutionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationPortfolioExecutionError {
    #[error("computation portfolio planning failed: {0}")]
    Planning(#[from] ComputationPortfolioError),
    #[error("computation portfolio execution failed: {0}")]
    Execution(#[from] ComputationExecutionError),
    #[error("computation portfolio execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation portfolio execution digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &ComputationPortfolioExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan": output.plan,
        "execution": output.execution,
        "planned_task_order": output.planned_task_order,
        "execution_digest": output.execution_digest,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl ComputationPortfolioExecution {
    pub fn validate(&self) -> Result<(), ComputationPortfolioExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.planned_task_order != self.plan.dependency_order
            || self
                .planned_task_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ComputationPortfolioExecutionError::InvalidOutput(
                "identity, planned order, duplicate, or canonical evidence bounds are invalid"
                    .into(),
            ));
        }
        self.plan.validate()?;
        match (&self.execution, &self.execution_digest) {
            (Some(execution), Some(digest)) => {
                execution.validate()?;
                if execution.digest != *digest
                    || execution.objective != self.objective
                    || execution.task_order.iter().any(|task_id| {
                        !self
                            .planned_task_order
                            .iter()
                            .any(|planned| planned == task_id)
                            && !execution
                                .cached_order
                                .iter()
                                .any(|cached| cached == task_id)
                    })
                {
                    return Err(ComputationPortfolioExecutionError::InvalidOutput(
                        "execution does not reconcile with the selected portfolio".into(),
                    ));
                }
            }
            (None, None) => {
                if !self.planned_task_order.is_empty() {
                    return Err(ComputationPortfolioExecutionError::InvalidOutput(
                        "a non-empty portfolio must carry an execution result".into(),
                    ));
                }
            }
            _ => {
                return Err(ComputationPortfolioExecutionError::InvalidOutput(
                    "execution and execution digest must be present together".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationPortfolioExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationPortfolioExecutionError::InvalidOutput(
                "portfolio execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn disposition(
    plan: &ComputationPortfolioPlan,
    execution: Option<&ComputationExecution>,
) -> ComputationPortfolioExecutionDisposition {
    match execution.map(|run| run.disposition) {
        Some(ComputationExecutionDisposition::Failed) => {
            ComputationPortfolioExecutionDisposition::Failed
        }
        Some(ComputationExecutionDisposition::Partial) => {
            ComputationPortfolioExecutionDisposition::Partial
        }
        Some(ComputationExecutionDisposition::Blocked) => {
            ComputationPortfolioExecutionDisposition::Blocked
        }
        Some(ComputationExecutionDisposition::Completed) => {
            if matches!(
                plan.disposition,
                super::planning::ComputationPortfolioDisposition::Qualified
            ) {
                ComputationPortfolioExecutionDisposition::Completed
            } else {
                ComputationPortfolioExecutionDisposition::Partial
            }
        }
        None => match plan.disposition {
            super::planning::ComputationPortfolioDisposition::Blocked => {
                ComputationPortfolioExecutionDisposition::Blocked
            }
            super::planning::ComputationPortfolioDisposition::Unresolved => {
                ComputationPortfolioExecutionDisposition::Unresolved
            }
            _ => ComputationPortfolioExecutionDisposition::Partial,
        },
    }
}

/// Plan and execute the selected computation closure through a caller-owned local worker.
pub fn execute_glioma_computation_portfolio<E: GliomaComputationExecutor>(
    request: &ComputationPortfolioExecutionRequest,
    executor: &mut E,
) -> Result<ComputationPortfolioExecution, ComputationPortfolioExecutionError> {
    let plan = plan_glioma_computation_portfolio(&request.portfolio, &request.candidates)?;
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let completed = request
        .portfolio
        .completed_order
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut required_context = std::collections::BTreeSet::new();
    let mut pending = plan.dependency_order.clone();
    while let Some(task_id) = pending.pop() {
        let candidate = candidate_map.get(&task_id).ok_or_else(|| {
            ComputationPortfolioExecutionError::InvalidOutput(format!(
                "planned task {task_id} is missing from the candidate map"
            ))
        })?;
        for dependency in &candidate.task.depends_on {
            if completed.contains(dependency) && required_context.insert(dependency.clone()) {
                pending.push(dependency.clone());
            }
        }
    }
    let mut task_ids = required_context;
    for context_id in task_ids.iter() {
        if !request
            .cache
            .iter()
            .any(|entry| entry.task_id == *context_id)
        {
            return Err(ComputationPortfolioExecutionError::InvalidOutput(format!(
                "completed dependency {context_id} requires a replay-keyed cache artifact"
            )));
        }
    }
    task_ids.extend(plan.dependency_order.iter().cloned());
    let mut tasks = task_ids
        .iter()
        .map(|task_id| {
            candidate_map
                .get(task_id)
                .map(|candidate| candidate.task.clone())
                .ok_or_else(|| {
                    ComputationPortfolioExecutionError::InvalidOutput(format!(
                        "context task {task_id} is missing from the candidate map"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let execution = if tasks.is_empty() {
        None
    } else {
        let execution_request = ComputationExecutionRequest {
            objective: request.portfolio.objective.clone(),
            model_system: request.portfolio.model_system,
            tasks,
            replay_identity: request.replay_identity.clone(),
            max_budget_units: request.portfolio.budget_units,
            max_retries: request.max_retries,
            allow_cache: request.allow_cache,
            require_local_artifacts: request.require_local_artifacts,
            cache: request.cache.clone(),
        };
        Some(execute_glioma_computation(&execution_request, executor)?)
    };
    let mut negative_evidence = plan.negative_evidence.clone();
    let mut uncertainty = plan.uncertainty.clone();
    if let Some(run) = &execution {
        negative_evidence.extend(run.negative_evidence.iter().cloned());
        uncertainty.extend(run.uncertainty.iter().cloned());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let execution_digest = execution.as_ref().map(|run| run.digest.clone());
    let mut output = ComputationPortfolioExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.portfolio.objective.clone(),
        planned_task_order: plan.dependency_order.clone(),
        plan,
        execution,
        execution_digest,
        negative_evidence,
        uncertainty,
        disposition: ComputationPortfolioExecutionDisposition::Partial,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-portfolio-execution"),
    };
    output.disposition = disposition(&output.plan, output.execution.as_ref());
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationPortfolioExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution::{
        ComputationOperation, DryRunGliomaComputationExecutor,
    };
    use crate::glioma::programs::p09_reproducible_computation::planning::ComputationCandidate;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn candidate(
        id: &str,
        operation: ComputationOperation,
        depends_on: Vec<&str>,
        modality: GliomaModality,
        gain: u32,
    ) -> ComputationCandidate {
        ComputationCandidate {
            candidate_id: id.into(),
            task: super::super::execution::ComputationTask {
                task_id: id.into(),
                operation,
                model_system: GliomaModelSystem::Organoid,
                depends_on: depends_on.into_iter().map(str::to_string).collect(),
                input_artifact_ids: vec![format!("input:{id}")],
                output_schema: format!("{id}@1"),
                estimated_cost_units: 2,
                estimated_duration_ticks: 1,
                deterministic: true,
            },
            modality,
            information_gain_milli: gain,
            uncertainty_reduction_milli: 500,
            coverage_debt_milli: 300,
            redundancy_group: id.into(),
            required: false,
        }
    }

    fn request() -> ComputationPortfolioExecutionRequest {
        ComputationPortfolioExecutionRequest {
            portfolio: ComputationPortfolioRequest {
                objective: "execute an organoid multimodal computation portfolio".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_units: 4,
                duration_ticks: 2,
                max_tasks: 2,
                max_modalities: 2,
                min_modalities: 2,
                information_weight_milli: 5,
                uncertainty_weight_milli: 3,
                coverage_weight_milli: 2,
                cost_penalty_milli: 1,
                duration_penalty_milli: 1,
                require_deterministic: true,
                completed_order: Vec::new(),
            },
            candidates: vec![
                candidate(
                    "integrate",
                    ComputationOperation::Integrate,
                    vec!["normalize"],
                    GliomaModality::Spatial,
                    900,
                ),
                candidate(
                    "normalize",
                    ComputationOperation::Normalize,
                    vec![],
                    GliomaModality::Transcriptomics,
                    400,
                ),
            ],
            replay_identity: ContentHash::of_bytes(b"portfolio-run"),
            max_retries: 1,
            allow_cache: true,
            require_local_artifacts: true,
            cache: Vec::new(),
        }
    }

    #[test]
    fn executes_selected_dependency_closure_and_replays() {
        let mut executor = DryRunGliomaComputationExecutor;
        let first = execute_glioma_computation_portfolio(&request(), &mut executor).unwrap();
        assert_eq!(
            first.planned_task_order,
            vec!["normalize".to_string(), "integrate".to_string()]
        );
        assert_eq!(
            first.disposition,
            ComputationPortfolioExecutionDisposition::Completed
        );
        first.validate().unwrap();
        let mut reversed = request();
        reversed.candidates.reverse();
        let mut second_executor = DryRunGliomaComputationExecutor;
        let second = execute_glioma_computation_portfolio(&reversed, &mut second_executor).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn refuses_to_execute_when_portfolio_has_no_runnable_task() {
        let mut request = request();
        request.portfolio.budget_units = 1;
        let mut executor = DryRunGliomaComputationExecutor;
        let output = execute_glioma_computation_portfolio(&request, &mut executor).unwrap();
        assert!(output.execution.is_none());
        assert_eq!(
            output.disposition,
            ComputationPortfolioExecutionDisposition::Partial
        );
        assert!(!output.plan.deferred_order.is_empty());
    }
}
