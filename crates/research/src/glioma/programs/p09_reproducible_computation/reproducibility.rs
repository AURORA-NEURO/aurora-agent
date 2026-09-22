//! Repeated-run reproducibility analysis for preclinical glioma computation.
//!
//! A computation can finish successfully while silently changing outputs across workers,
//! containers, model versions, or random seeds. This feature compares institution-local replay
//! summaries, gates deterministic tasks on byte-identical outputs, and gates numerical tasks on
//! explicit effect and runtime drift. Raw matrices and model payloads stay with the caller; only
//! typed task summaries cross this analysis boundary.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationReproducibility1@1";
pub const MAX_RUNS: usize = 512;
pub const MAX_TASKS: usize = 4_096;
pub const MAX_TASKS_PER_RUN: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationReproducibilityRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_runs: usize,
    pub min_task_coverage_milli: u16,
    pub max_failed_runs: usize,
    pub max_digest_mismatches: usize,
    pub max_effect_drift_milli: u64,
    pub max_runtime_drift_milli: u64,
    pub require_byte_identical_deterministic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationRunOutcome {
    Completed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationReproducibilityTaskObservation {
    pub task_id: String,
    pub output_schema: String,
    pub output_digest: ContentHash,
    pub effect_milli: Option<i64>,
    pub uncertainty_milli: u64,
    pub duration_ticks: u64,
    pub deterministic: bool,
    pub outcome: ComputationRunOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationReproducibilityRun {
    pub run_id: String,
    pub replay_identity: ContentHash,
    pub outcome: ComputationRunOutcome,
    pub tasks: Vec<ComputationReproducibilityTaskObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationTaskReproducibilityDisposition {
    Stable,
    Drifted,
    Undercovered,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationTaskReproducibilitySummary {
    pub task_id: String,
    pub output_schema: String,
    pub run_count: usize,
    pub completed_count: usize,
    pub coverage_milli: u16,
    pub deterministic: bool,
    pub digest_mismatch_count: usize,
    pub effect_low_milli: Option<i64>,
    pub effect_high_milli: Option<i64>,
    pub effect_drift_milli: u64,
    pub runtime_drift_milli: u64,
    pub disposition: ComputationTaskReproducibilityDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationReproducibilityDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationReproducibility {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub run_order: Vec<String>,
    pub task_order: Vec<String>,
    pub summaries: Vec<ComputationTaskReproducibilitySummary>,
    pub qualified_task_order: Vec<String>,
    pub unstable_task_order: Vec<String>,
    pub failed_run_order: Vec<String>,
    pub incomplete_run_order: Vec<String>,
    pub overall_coverage_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationReproducibilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationReproducibilityError {
    #[error("computation reproducibility request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation reproducibility input is invalid: {0}")]
    InvalidInput(String),
    #[error("computation reproducibility output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation reproducibility digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ComputationReproducibility) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "replay_identity": output.replay_identity,
        "run_order": output.run_order,
        "task_order": output.task_order,
        "summaries": output.summaries,
        "qualified_task_order": output.qualified_task_order,
        "unstable_task_order": output.unstable_task_order,
        "failed_run_order": output.failed_run_order,
        "incomplete_run_order": output.incomplete_run_order,
        "overall_coverage_milli": output.overall_coverage_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn relative_drift(low: u64, high: u64) -> u64 {
    if high == 0 {
        0
    } else {
        high.saturating_sub(low).saturating_mul(1_000) / high
    }
}

impl ComputationTaskReproducibilitySummary {
    pub fn validate(&self) -> Result<(), ComputationReproducibilityError> {
        if self.task_id.trim().is_empty()
            || self.output_schema.trim().is_empty()
            || self.run_count == 0
            || self.completed_count > self.run_count
            || self.coverage_milli > 1_000
            || self.digest_mismatch_count > self.run_count
            || self.effect_drift_milli
                != match (self.effect_low_milli, self.effect_high_milli) {
                    (Some(low), Some(high)) => low.abs_diff(high),
                    (None, None) => 0,
                    _ => {
                        return Err(ComputationReproducibilityError::InvalidOutput(
                            "effect bounds must be both present or both absent".into(),
                        ))
                    }
                }
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(ComputationReproducibilityError::InvalidOutput(
                "task summary identity, coverage, drift, or ordering is invalid".into(),
            ));
        }
        Ok(())
    }
}

impl ComputationReproducibility {
    pub fn validate(&self) -> Result<(), ComputationReproducibilityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.run_order.len() < 2
            || !canonical(&self.run_order)
            || !canonical(&self.task_order)
            || !canonical(&self.qualified_task_order)
            || !canonical(&self.unstable_task_order)
            || !canonical(&self.failed_run_order)
            || !canonical(&self.incomplete_run_order)
            || self.overall_coverage_milli > 1_000
            || self
                .summaries
                .iter()
                .any(|summary| summary.validate().is_err())
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(ComputationReproducibilityError::InvalidOutput(
                "identity, run/task ordering, coverage, summaries, or limitations are invalid"
                    .into(),
            ));
        }
        let task_ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let summary_ids = self
            .summaries
            .iter()
            .map(|summary| summary.task_id.clone())
            .collect::<BTreeSet<_>>();
        let qualified_ids = self
            .qualified_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unstable_ids = self
            .unstable_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let run_ids = self.run_order.iter().cloned().collect::<BTreeSet<_>>();
        if task_ids.len() != self.task_order.len()
            || task_ids != summary_ids
            || qualified_ids.intersection(&unstable_ids).next().is_some()
            || qualified_ids
                .union(&unstable_ids)
                .any(|id| !task_ids.contains(id))
            || self.failed_run_order.iter().any(|id| !run_ids.contains(id))
            || self
                .incomplete_run_order
                .iter()
                .any(|id| !run_ids.contains(id))
        {
            return Err(ComputationReproducibilityError::InvalidOutput(
                "task and run partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationReproducibilityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationReproducibilityError::InvalidOutput(
                "digest is not bound to reproducibility output".into(),
            ));
        }
        Ok(())
    }
}

fn summarize_task(
    task_id: &str,
    observations: &[&ComputationReproducibilityTaskObservation],
    request: &ComputationReproducibilityRequest,
) -> ComputationTaskReproducibilitySummary {
    let completed = observations
        .iter()
        .filter(|observation| observation.outcome == ComputationRunOutcome::Completed)
        .copied()
        .collect::<Vec<_>>();
    let coverage = (completed.len().saturating_mul(1_000) / observations.len().max(1)) as u16;
    let mut digests = completed
        .iter()
        .map(|observation| observation.output_digest.clone())
        .collect::<Vec<_>>();
    digests.sort();
    digests.dedup();
    let digest_mismatches = digests.len().saturating_sub(1);
    let effects = completed
        .iter()
        .filter_map(|observation| observation.effect_milli)
        .collect::<Vec<_>>();
    let (effect_low, effect_high, effect_drift) = if effects.is_empty() {
        (None, None, 0)
    } else {
        let low = *effects.iter().min().unwrap_or(&0);
        let high = *effects.iter().max().unwrap_or(&0);
        (Some(low), Some(high), low.abs_diff(high))
    };
    let mut runtimes = completed
        .iter()
        .map(|observation| observation.duration_ticks)
        .collect::<Vec<_>>();
    runtimes.sort_unstable();
    let runtime_drift = match (runtimes.first(), runtimes.last()) {
        (Some(low), Some(high)) => relative_drift(*low, *high),
        _ => 0,
    };
    let deterministic = observations
        .iter()
        .all(|observation| observation.deterministic);
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let disposition = if completed.is_empty() {
        negative_evidence.push("task has no completed replay run".into());
        ComputationTaskReproducibilityDisposition::Failed
    } else if coverage < request.min_task_coverage_milli {
        negative_evidence.push("task replay coverage is below the promotion floor".into());
        ComputationTaskReproducibilityDisposition::Undercovered
    } else if deterministic
        && request.require_byte_identical_deterministic
        && digest_mismatches > request.max_digest_mismatches
    {
        negative_evidence
            .push("deterministic task output digest changed across replay runs".into());
        ComputationTaskReproducibilityDisposition::Drifted
    } else if effect_drift > request.max_effect_drift_milli
        || runtime_drift > request.max_runtime_drift_milli
    {
        negative_evidence.push("replay effect or runtime drift exceeds the declared gate".into());
        ComputationTaskReproducibilityDisposition::Drifted
    } else if effects.len() < completed.len() {
        uncertainty.push("one or more completed runs omitted a numeric effect summary".into());
        ComputationTaskReproducibilityDisposition::Unresolved
    } else {
        ComputationTaskReproducibilityDisposition::Stable
    };
    if observations
        .iter()
        .any(|observation| observation.outcome != ComputationRunOutcome::Completed)
    {
        uncertainty.push("one or more replay runs reported partial or failed task status".into());
    }
    if completed
        .iter()
        .any(|observation| observation.uncertainty_milli > 500_000)
    {
        uncertainty.push("at least one completed run reported high numeric uncertainty".into());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    ComputationTaskReproducibilitySummary {
        task_id: task_id.into(),
        output_schema: observations
            .first()
            .map(|observation| observation.output_schema.clone())
            .unwrap_or_default(),
        run_count: observations.len(),
        completed_count: completed.len(),
        coverage_milli: coverage,
        deterministic,
        digest_mismatch_count: digest_mismatches,
        effect_low_milli: effect_low,
        effect_high_milli: effect_high,
        effect_drift_milli: effect_drift,
        runtime_drift_milli: runtime_drift,
        disposition,
        negative_evidence,
        uncertainty,
    }
}

pub fn analyze_glioma_computation_reproducibility(
    request: &ComputationReproducibilityRequest,
    runs: &[ComputationReproducibilityRun],
) -> Result<ComputationReproducibility, ComputationReproducibilityError> {
    if request.objective.trim().is_empty()
        || request.min_runs < 2
        || request.min_runs > MAX_RUNS
        || request.min_task_coverage_milli > 1_000
        || request.max_failed_runs > MAX_RUNS
        || request.max_effect_drift_milli > 1_000_000_000
        || request.max_runtime_drift_milli > 1_000
    {
        return Err(ComputationReproducibilityError::InvalidRequest(
            "objective, run floor, coverage, failure, effect-drift, or runtime-drift bounds are invalid"
                .into(),
        ));
    }
    if runs.len() < request.min_runs || runs.len() > MAX_RUNS {
        return Err(ComputationReproducibilityError::InvalidInput(
            "replay run count is outside the supported bound".into(),
        ));
    }
    let mut run_ids = BTreeSet::new();
    let replay_identity = runs[0].replay_identity.clone();
    let mut task_map = BTreeMap::<String, Vec<&ComputationReproducibilityTaskObservation>>::new();
    let mut failed_run_order = Vec::new();
    let mut incomplete_run_order = Vec::new();
    for run in runs {
        if !run_ids.insert(run.run_id.clone())
            || run.run_id.trim().is_empty()
            || run.replay_identity != replay_identity
            || run.tasks.is_empty()
            || run.tasks.len() > MAX_TASKS_PER_RUN
            || run
                .tasks
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
        {
            return Err(ComputationReproducibilityError::InvalidInput(
                "replay identity, run ordering, task ordering, or task count is invalid".into(),
            ));
        }
        match run.outcome {
            ComputationRunOutcome::Failed => failed_run_order.push(run.run_id.clone()),
            ComputationRunOutcome::Partial => incomplete_run_order.push(run.run_id.clone()),
            ComputationRunOutcome::Completed => {}
        }
        for observation in &run.tasks {
            if observation.task_id.trim().is_empty()
                || observation.output_schema.trim().is_empty()
                || observation.output_digest.as_str().len() != 64
                || observation.uncertainty_milli == 0
            {
                return Err(ComputationReproducibilityError::InvalidInput(
                    "task observation identity, schema, digest, or uncertainty is invalid".into(),
                ));
            }
            task_map
                .entry(observation.task_id.clone())
                .or_default()
                .push(observation);
        }
    }
    if failed_run_order.len() > request.max_failed_runs {
        return Err(ComputationReproducibilityError::InvalidInput(
            "failed replay count exceeds the declared bound".into(),
        ));
    }
    if task_map.is_empty() || task_map.len() > MAX_TASKS {
        return Err(ComputationReproducibilityError::InvalidInput(
            "task union is outside the supported bound".into(),
        ));
    }
    let mut summaries = Vec::with_capacity(task_map.len());
    for (task_id, observations) in &task_map {
        if observations.iter().any(|observation| {
            observation.output_schema != observations[0].output_schema
                || observation.deterministic != observations[0].deterministic
        }) {
            return Err(ComputationReproducibilityError::InvalidInput(
                "task schema and deterministic declaration changed across runs".into(),
            ));
        }
        summaries.push(summarize_task(task_id, observations, request));
    }
    let task_order = task_map.keys().cloned().collect::<Vec<_>>();
    let qualified_task_order = summaries
        .iter()
        .filter(|summary| summary.disposition == ComputationTaskReproducibilityDisposition::Stable)
        .map(|summary| summary.task_id.clone())
        .collect::<Vec<_>>();
    let unstable_task_order = summaries
        .iter()
        .filter(|summary| summary.disposition != ComputationTaskReproducibilityDisposition::Stable)
        .map(|summary| summary.task_id.clone())
        .collect::<Vec<_>>();
    let overall_coverage_milli = summaries
        .iter()
        .map(|summary| summary.coverage_milli)
        .min()
        .unwrap_or(0);
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !failed_run_order.is_empty() {
        negative_evidence.push("one or more replay runs failed".into());
    }
    if !incomplete_run_order.is_empty() {
        uncertainty.push("one or more replay runs were partial".into());
    }
    for summary in &summaries {
        negative_evidence.extend(summary.negative_evidence.iter().cloned());
        uncertainty.extend(summary.uncertainty.iter().cloned());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    failed_run_order.sort();
    incomplete_run_order.sort();
    let disposition = if qualified_task_order.is_empty() {
        ComputationReproducibilityDisposition::Unresolved
    } else if unstable_task_order.is_empty()
        && failed_run_order.is_empty()
        && incomplete_run_order.is_empty()
    {
        ComputationReproducibilityDisposition::Qualified
    } else {
        ComputationReproducibilityDisposition::Partial
    };
    let mut output = ComputationReproducibility {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        replay_identity,
        run_order: run_ids.iter().cloned().collect(),
        task_order,
        summaries,
        qualified_task_order,
        unstable_task_order,
        failed_run_order,
        incomplete_run_order,
        overall_coverage_milli,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| ComputationReproducibilityError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationReproducibilityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).expect("digest")
    }

    fn request() -> ComputationReproducibilityRequest {
        ComputationReproducibilityRequest {
            objective: "replay glioma imaging computation".into(),
            model_system: GliomaModelSystem::Organoid,
            min_runs: 2,
            min_task_coverage_milli: 1_000,
            max_failed_runs: 0,
            max_digest_mismatches: 0,
            max_effect_drift_milli: 50,
            max_runtime_drift_milli: 200,
            require_byte_identical_deterministic: true,
        }
    }

    fn run(id: &str, output: &str, effect: i64) -> ComputationReproducibilityRun {
        ComputationReproducibilityRun {
            run_id: id.into(),
            replay_identity: digest("replay"),
            outcome: ComputationRunOutcome::Completed,
            tasks: vec![ComputationReproducibilityTaskObservation {
                task_id: "quantify".into(),
                output_schema: "GliomaQuantification1@1".into(),
                output_digest: digest(output),
                effect_milli: Some(effect),
                uncertainty_milli: 10,
                duration_ticks: 100,
                deterministic: true,
                outcome: ComputationRunOutcome::Completed,
            }],
        }
    }

    #[test]
    fn byte_identical_replays_qualify() {
        let output = analyze_glioma_computation_reproducibility(
            &request(),
            &[run("run-01", "same", 100), run("run-02", "same", 101)],
        )
        .expect("reproducible computation");
        assert_eq!(
            output.disposition,
            ComputationReproducibilityDisposition::Qualified
        );
        assert_eq!(output.qualified_task_order, vec!["quantify"]);
        output.validate().expect("digest and partitions validate");
    }

    #[test]
    fn deterministic_digest_drift_is_negative_evidence() {
        let output = analyze_glioma_computation_reproducibility(
            &request(),
            &[run("run-01", "one", 100), run("run-02", "two", 101)],
        )
        .expect("reproducibility analysis");
        assert_eq!(
            output.disposition,
            ComputationReproducibilityDisposition::Unresolved
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("digest")));
    }
}
