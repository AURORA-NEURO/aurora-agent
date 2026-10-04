//! Campaign-level throughput and bottleneck intelligence for `GAF-GLIOMA-P09-F19`.
//!
//! A run monitor is not enough for an autonomous glioma research program: screening and
//! multimodal campaigns need to know whether work was actually completed, where time and
//! resources were spent, and whether a capacity forecast was earned on data it did not train
//! on. This module consumes only the typed, metadata-only run inspections produced by P09-F17.
//! It never opens scientific payloads and never treats computation volume as a biological result.

use super::computation_run_inspector::{
    ComputationInspectionEventKind, ComputationInspectionTaskStatus, ComputationRunInspection,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationCampaignTimeline1@1";
pub const MAX_RUNS: usize = 4_096;
pub const MAX_HOLDOUT_RUNS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCampaignTimelineRequest {
    pub objective: String,
    pub replay_identity: ContentHash,
    pub runs: Vec<ComputationCampaignRunInput>,
    /// Explicit held-out runs prevent the forecast from being evaluated on its training data.
    pub holdout_run_ids: Vec<String>,
    pub forecast_horizon_ticks: u64,
    pub saturation_threshold_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCampaignRunInput {
    pub run_id: String,
    pub scientific_scope: String,
    pub workflow_class: String,
    pub admitted_at_tick: u64,
    pub resource_capacity_units: u64,
    pub memory_capacity_mb: u32,
    pub inspection: ComputationRunInspection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignRunDisposition {
    Successful,
    Incomplete,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignBottleneck {
    QueueLatency,
    ComputeDuration,
    RetryBurden,
    ResourceSaturation,
    FailureBurden,
    TelemetryStaleness,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCampaignRunSummary {
    pub run_id: String,
    pub scientific_scope: String,
    pub workflow_class: String,
    pub disposition: CampaignRunDisposition,
    pub task_count: u32,
    pub successful_task_count: u32,
    pub failed_task_count: u32,
    pub incomplete_task_count: u32,
    pub queue_latency_ticks: Option<u64>,
    pub compute_duration_ticks: Option<u64>,
    pub retry_count: u32,
    pub peak_memory_mb: u32,
    pub memory_saturation_milli: Option<u16>,
    pub resource_saturation_milli: Option<u16>,
    pub bottleneck: CampaignBottleneck,
    pub issue_order: Vec<String>,
    pub inspection_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignThroughputForecast {
    pub workflow_class: String,
    pub training_run_ids: Vec<String>,
    pub held_out_run_ids: Vec<String>,
    pub forecast_horizon_ticks: u64,
    pub predicted_successful_tasks: u32,
    pub lower_successful_tasks: u32,
    pub upper_successful_tasks: u32,
    pub actual_held_out_successful_tasks: u32,
    pub absolute_error_tasks: u32,
    pub within_interval: bool,
    pub calibrated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationCampaignTimelineDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCampaignTimeline {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub run_order: Vec<String>,
    pub runs: Vec<ComputationCampaignRunSummary>,
    pub forecasts: Vec<CampaignThroughputForecast>,
    pub successful_task_count: u64,
    pub failed_task_count: u64,
    pub incomplete_task_count: u64,
    pub retry_count: u64,
    pub total_queue_latency_ticks: u64,
    pub total_compute_duration_ticks: u64,
    pub bottleneck_order: Vec<CampaignBottleneck>,
    pub issue_order: Vec<String>,
    pub disposition: ComputationCampaignTimelineDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationCampaignTimelineError {
    #[error("campaign timeline request is invalid: {0}")]
    InvalidRequest(String),
    #[error("campaign timeline output is invalid: {0}")]
    InvalidOutput(String),
    #[error("campaign timeline digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.len() <= 512 && !trimmed.chars().any(|ch| ch.is_control())
}

fn mean(values: &[u32]) -> u32 {
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| u64::from(*value)).sum::<u64>() / values.len() as u64) as u32
    }
}

fn percentile_bounds(values: &[u32]) -> (u32, u32) {
    if values.is_empty() {
        return (0, 0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let low_index = sorted.len().saturating_sub(1) / 4;
    let high_index = (sorted.len().saturating_sub(1) * 3) / 4;
    (sorted[low_index], sorted[high_index])
}

fn saturation_milli(used: u64, capacity: u64) -> Option<u16> {
    if capacity == 0 {
        None
    } else {
        Some(
            (used
                .saturating_mul(1_000)
                .checked_div(capacity)
                .unwrap_or(0)
                .min(1_000)) as u16,
        )
    }
}

fn memory_saturation_milli(used: u32, capacity: u32) -> Option<u16> {
    saturation_milli(u64::from(used), u64::from(capacity))
}

fn terminal_tick(run: &ComputationRunInspection) -> Option<u64> {
    run.timeline
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                ComputationInspectionEventKind::Completed
                    | ComputationInspectionEventKind::Cached
                    | ComputationInspectionEventKind::Negative
                    | ComputationInspectionEventKind::Partial
                    | ComputationInspectionEventKind::Failed
                    | ComputationInspectionEventKind::Skipped
            )
        })
        .map(|event| event.tick)
        .max()
}

fn first_tick(run: &ComputationRunInspection, kind: ComputationInspectionEventKind) -> Option<u64> {
    run.timeline
        .iter()
        .filter(|event| event.kind == kind)
        .map(|event| event.tick)
        .min()
}

fn digest_input(output: &ComputationCampaignTimeline) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "run_order": output.run_order,
        "runs": output.runs,
        "forecasts": output.forecasts,
        "successful_task_count": output.successful_task_count,
        "failed_task_count": output.failed_task_count,
        "incomplete_task_count": output.incomplete_task_count,
        "retry_count": output.retry_count,
        "total_queue_latency_ticks": output.total_queue_latency_ticks,
        "total_compute_duration_ticks": output.total_compute_duration_ticks,
        "bottleneck_order": output.bottleneck_order,
        "issue_order": output.issue_order,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

impl ComputationCampaignTimeline {
    pub fn validate(&self) -> Result<(), ComputationCampaignTimelineError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.replay_identity.as_str().len() != 64
            || self.run_order.len() != self.runs.len()
            || self.run_order
                != self
                    .runs
                    .iter()
                    .map(|run| run.run_id.clone())
                    .collect::<Vec<_>>()
            || self
                .run_order
                .windows(2)
                .any(|window| window[0] >= window[1])
            || self.runs.iter().any(|run| {
                !safe_text(&run.run_id)
                    || !safe_text(&run.scientific_scope)
                    || !safe_text(&run.workflow_class)
                    || run.task_count
                        != run.successful_task_count
                            + run.failed_task_count
                            + run.incomplete_task_count
                    || run
                        .issue_order
                        .windows(2)
                        .any(|window| window[0] >= window[1])
            })
            || self
                .bottleneck_order
                .windows(2)
                .any(|window| window[0] == window[1])
            || self
                .issue_order
                .windows(2)
                .any(|window| window[0] >= window[1])
            || self.successful_task_count
                != self
                    .runs
                    .iter()
                    .map(|run| u64::from(run.successful_task_count))
                    .sum::<u64>()
            || self.failed_task_count
                != self
                    .runs
                    .iter()
                    .map(|run| u64::from(run.failed_task_count))
                    .sum::<u64>()
            || self.incomplete_task_count
                != self
                    .runs
                    .iter()
                    .map(|run| u64::from(run.incomplete_task_count))
                    .sum::<u64>()
            || self.retry_count
                != self
                    .runs
                    .iter()
                    .map(|run| u64::from(run.retry_count))
                    .sum::<u64>()
        {
            return Err(ComputationCampaignTimelineError::InvalidOutput(
                "identity, canonical ordering, task partition, or issue ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationCampaignTimelineError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationCampaignTimelineError::InvalidOutput(
                "campaign timeline digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ComputationCampaignTimelineRequest,
) -> Result<(), ComputationCampaignTimelineError> {
    if !safe_text(&request.objective)
        || request.replay_identity.as_str().len() != 64
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.holdout_run_ids.len() > MAX_HOLDOUT_RUNS
        || request.forecast_horizon_ticks == 0
        || request.saturation_threshold_milli > 1_000
        || request.runs.iter().any(|run| {
            !safe_text(&run.run_id)
                || !safe_text(&run.scientific_scope)
                || !safe_text(&run.workflow_class)
                || run.memory_capacity_mb == 0
                || run.resource_capacity_units == 0
                || run.inspection.validate().is_err()
        })
    {
        return Err(ComputationCampaignTimelineError::InvalidRequest(
            "bounded runs, valid run inspections, scopes, capacities, and a positive forecast horizon are required".into(),
        ));
    }
    let ids = request
        .runs
        .iter()
        .map(|run| run.run_id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != request.runs.len()
        || request
            .holdout_run_ids
            .iter()
            .any(|id| !safe_text(id) || !ids.contains(id.as_str()))
    {
        return Err(ComputationCampaignTimelineError::InvalidRequest(
            "run identifiers and holdout identifiers must be unique and refer to supplied runs"
                .into(),
        ));
    }
    Ok(())
}

fn summarize_run(
    input: &ComputationCampaignRunInput,
    saturation_threshold_milli: u16,
) -> ComputationCampaignRunSummary {
    let inspection = &input.inspection;
    let successful_task_count = inspection
        .tasks
        .iter()
        .filter(|task| {
            matches!(
                task.status,
                ComputationInspectionTaskStatus::Completed
                    | ComputationInspectionTaskStatus::Cached
            )
        })
        .count() as u32;
    let failed_task_count = inspection
        .tasks
        .iter()
        .filter(|task| task.status == ComputationInspectionTaskStatus::Failed)
        .count() as u32;
    let incomplete_task_count = inspection
        .tasks
        .iter()
        .filter(|task| {
            !matches!(
                task.status,
                ComputationInspectionTaskStatus::Completed
                    | ComputationInspectionTaskStatus::Cached
                    | ComputationInspectionTaskStatus::Failed
            )
        })
        .count() as u32;
    let task_count = inspection.tasks.len() as u32;
    let started = first_tick(inspection, ComputationInspectionEventKind::Started);
    let queue_latency_ticks = started.map(|tick| tick.saturating_sub(input.admitted_at_tick));
    let compute_duration_ticks = started
        .zip(terminal_tick(inspection))
        .map(|(start, end)| end.saturating_sub(start));
    let retry_count = inspection
        .tasks
        .iter()
        .map(|task| u32::from(task.retry_count))
        .sum();
    let peak_memory_mb = inspection.peak_memory_mb;
    let memory_saturation = memory_saturation_milli(peak_memory_mb, input.memory_capacity_mb);
    let resource_saturation =
        saturation_milli(inspection.total_cost_units, input.resource_capacity_units);
    let mut issue_order = inspection.issue_order.clone();
    if queue_latency_ticks.is_none() {
        issue_order.push("missing-start-event".into());
    }
    if terminal_tick(inspection).is_none() {
        issue_order.push("missing-terminal-event".into());
    }
    issue_order.sort();
    issue_order.dedup();
    let disposition = if inspection.disposition
        == super::computation_run_inspector::ComputationInspectionDisposition::Unresolved
    {
        CampaignRunDisposition::Unresolved
    } else if failed_task_count > 0 {
        CampaignRunDisposition::Failed
    } else if incomplete_task_count > 0
        || inspection.disposition
            != super::computation_run_inspector::ComputationInspectionDisposition::Ready
    {
        CampaignRunDisposition::Incomplete
    } else {
        CampaignRunDisposition::Successful
    };
    let mut candidates = [
        (
            CampaignBottleneck::QueueLatency,
            queue_latency_ticks.unwrap_or(0),
        ),
        (
            CampaignBottleneck::ComputeDuration,
            compute_duration_ticks.unwrap_or(0),
        ),
        (
            CampaignBottleneck::RetryBurden,
            u64::from(retry_count) * 100,
        ),
        (
            CampaignBottleneck::ResourceSaturation,
            u64::from(resource_saturation.unwrap_or(0)),
        ),
        (
            CampaignBottleneck::TelemetryStaleness,
            if inspection.stale_task_order.is_empty() {
                0
            } else {
                1_001
            },
        ),
        (
            CampaignBottleneck::FailureBurden,
            u64::from(failed_task_count) * 1_000,
        ),
    ];
    candidates.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| (left.0 as u8).cmp(&(right.0 as u8)))
    });
    let bottleneck = if candidates.first().is_some_and(|(_, score)| *score > 0) {
        candidates[0].0
    } else {
        CampaignBottleneck::None
    };
    if memory_saturation.is_some_and(|value| value >= saturation_threshold_milli) {
        issue_order.push("memory-saturation-threshold-reached".into());
        issue_order.sort();
        issue_order.dedup();
    }
    ComputationCampaignRunSummary {
        run_id: input.run_id.clone(),
        scientific_scope: input.scientific_scope.clone(),
        workflow_class: input.workflow_class.clone(),
        disposition,
        task_count,
        successful_task_count,
        failed_task_count,
        incomplete_task_count,
        queue_latency_ticks,
        compute_duration_ticks,
        retry_count,
        peak_memory_mb,
        memory_saturation_milli: memory_saturation,
        resource_saturation_milli: resource_saturation,
        bottleneck,
        issue_order,
        inspection_digest: inspection.digest.clone(),
    }
}

fn build_forecasts(
    runs: &[ComputationCampaignRunInput],
    summaries: &[ComputationCampaignRunSummary],
    holdouts: &BTreeSet<String>,
    horizon: u64,
) -> Vec<CampaignThroughputForecast> {
    let mut classes = BTreeSet::new();
    for run in runs {
        classes.insert(run.workflow_class.clone());
    }
    let summary_by_id = summaries
        .iter()
        .map(|summary| (summary.run_id.as_str(), summary))
        .collect::<BTreeMap<_, _>>();
    let mut output = Vec::new();
    for class in classes {
        let training = runs
            .iter()
            .filter(|run| run.workflow_class == class && !holdouts.contains(&run.run_id))
            .collect::<Vec<_>>();
        let held_out = runs
            .iter()
            .filter(|run| run.workflow_class == class && holdouts.contains(&run.run_id))
            .collect::<Vec<_>>();
        if training.is_empty() || held_out.is_empty() {
            continue;
        }
        let values = training
            .iter()
            .filter_map(|run| {
                summary_by_id
                    .get(run.run_id.as_str())
                    .map(|summary| summary.successful_task_count)
            })
            .collect::<Vec<_>>();
        let held_values = held_out
            .iter()
            .filter_map(|run| {
                summary_by_id
                    .get(run.run_id.as_str())
                    .map(|summary| summary.successful_task_count)
            })
            .collect::<Vec<_>>();
        let predicted = mean(&values);
        let (lower, upper) = percentile_bounds(&values);
        let actual = held_values
            .iter()
            .map(|value| u64::from(*value))
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32;
        let predicted_total = predicted.saturating_mul(held_values.len() as u32);
        let lower_total = lower.saturating_mul(held_values.len() as u32);
        let upper_total = upper.saturating_mul(held_values.len() as u32);
        let (lower_total, upper_total) =
            (lower_total.min(upper_total), upper_total.max(lower_total));
        output.push(CampaignThroughputForecast {
            workflow_class: class,
            training_run_ids: training.iter().map(|run| run.run_id.clone()).collect(),
            held_out_run_ids: held_out.iter().map(|run| run.run_id.clone()).collect(),
            forecast_horizon_ticks: horizon,
            predicted_successful_tasks: predicted_total,
            lower_successful_tasks: lower_total,
            upper_successful_tasks: upper_total,
            actual_held_out_successful_tasks: actual,
            absolute_error_tasks: predicted_total.abs_diff(actual),
            within_interval: actual >= lower_total && actual <= upper_total,
            calibrated: actual >= lower_total && actual <= upper_total,
        });
    }
    output.sort_by(|left, right| left.workflow_class.cmp(&right.workflow_class));
    output
}

/// Summarize a bounded high-throughput computation campaign for autonomous operations.
pub fn compile_glioma_high_throughput_compute_timeline(
    request: &ComputationCampaignTimelineRequest,
) -> Result<ComputationCampaignTimeline, ComputationCampaignTimelineError> {
    validate_request(request)?;
    let mut inputs = request.runs.clone();
    inputs.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    let holdouts = request
        .holdout_run_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut runs = inputs
        .iter()
        .map(|input| summarize_run(input, request.saturation_threshold_milli))
        .collect::<Vec<_>>();
    runs.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    let forecasts = build_forecasts(&inputs, &runs, &holdouts, request.forecast_horizon_ticks);
    let mut bottleneck_scores = BTreeMap::<CampaignBottleneck, u64>::new();
    for summary in &runs {
        *bottleneck_scores.entry(summary.bottleneck).or_default() += 1;
    }
    let mut bottleneck_order = bottleneck_scores.into_iter().collect::<Vec<_>>();
    bottleneck_order.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| (left.0 as u8).cmp(&(right.0 as u8)))
    });
    let bottleneck_order = bottleneck_order
        .into_iter()
        .map(|(bottleneck, _)| bottleneck)
        .collect::<Vec<_>>();
    let successful_task_count = runs
        .iter()
        .map(|run| u64::from(run.successful_task_count))
        .sum();
    let failed_task_count = runs
        .iter()
        .map(|run| u64::from(run.failed_task_count))
        .sum();
    let incomplete_task_count = runs
        .iter()
        .map(|run| u64::from(run.incomplete_task_count))
        .sum();
    let retry_count = runs.iter().map(|run| u64::from(run.retry_count)).sum();
    let total_queue_latency_ticks = runs.iter().filter_map(|run| run.queue_latency_ticks).sum();
    let total_compute_duration_ticks = runs
        .iter()
        .filter_map(|run| run.compute_duration_ticks)
        .sum();
    let mut issue_order = runs
        .iter()
        .flat_map(|run| {
            run.issue_order
                .iter()
                .map(|issue| format!("{}: {}", run.run_id, issue))
        })
        .collect::<Vec<_>>();
    if forecasts.is_empty() {
        issue_order.push("forecast-not-evaluable-with-current-training-holdout-partition".into());
    }
    issue_order.sort();
    issue_order.dedup();
    let disposition = if runs
        .iter()
        .any(|run| run.disposition == CampaignRunDisposition::Unresolved)
    {
        ComputationCampaignTimelineDisposition::Blocked
    } else if runs
        .iter()
        .any(|run| run.disposition != CampaignRunDisposition::Successful)
        || forecasts.iter().any(|forecast| !forecast.calibrated)
    {
        ComputationCampaignTimelineDisposition::Partial
    } else {
        ComputationCampaignTimelineDisposition::Ready
    };
    let next_action = match disposition {
        ComputationCampaignTimelineDisposition::Ready => "use the held-out forecast and bottleneck order to admit the next bounded campaign window".into(),
        ComputationCampaignTimelineDisposition::Partial => "separate failed or incomplete throughput from successes, then refresh the forecast with an explicit held-out window".into(),
        ComputationCampaignTimelineDisposition::Blocked => "repair unresolved run telemetry before using campaign throughput for scheduling".into(),
    };
    let mut output = ComputationCampaignTimeline {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_identity: request.replay_identity.clone(),
        run_order: runs.iter().map(|run| run.run_id.clone()).collect(),
        runs,
        forecasts,
        successful_task_count,
        failed_task_count,
        incomplete_task_count,
        retry_count,
        total_queue_latency_ticks,
        total_compute_duration_ticks,
        bottleneck_order,
        issue_order,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-campaign-timeline"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationCampaignTimelineError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::computation_run_inspector::{
        inspect_glioma_computation_run, ComputationInspectionEvent, ComputationInspectionEventKind,
        ComputationRunInspectionRequest,
    };
    use super::super::execution::{
        ComputationExecution, ComputationExecutionDisposition, ComputationExecutionStopReason,
        ComputationTaskDisposition, ComputationTaskResult,
    };
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn inspection(
        run_seed: &str,
        task_count: usize,
        failed: bool,
        start: u64,
        end: u64,
    ) -> ComputationRunInspection {
        let mut task_order = Vec::new();
        let mut task_results = Vec::new();
        for index in 0..task_count {
            let task_id = format!("{run_seed}-task-{index}");
            task_order.push(task_id.clone());
            task_results.push(ComputationTaskResult {
                task_id,
                output_schema: "GliomaResult1@1".into(),
                disposition: if failed && index == task_count.saturating_sub(1) {
                    ComputationTaskDisposition::Failed
                } else {
                    ComputationTaskDisposition::Completed
                },
                attempt_count: if failed && index == 0 { 2 } else { 1 },
                artifact: None,
                cache_hit: false,
                note: "local".into(),
            });
        }
        let completed_order = task_results
            .iter()
            .filter(|result| result.disposition == ComputationTaskDisposition::Completed)
            .map(|result| result.task_id.clone())
            .collect::<Vec<_>>();
        let failed_order = task_results
            .iter()
            .filter(|result| result.disposition == ComputationTaskDisposition::Failed)
            .map(|result| result.task_id.clone())
            .collect::<Vec<_>>();
        let mut execution = ComputationExecution {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "campaign".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: hash(run_seed),
            task_order,
            task_results,
            completed_order,
            cached_order: Vec::new(),
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order,
            skipped_order: Vec::new(),
            budget_used_units: 4,
            duration_used_ticks: end.saturating_sub(start),
            cache_hit_count: 0,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: if failed {
                ComputationExecutionDisposition::Failed
            } else {
                ComputationExecutionDisposition::Completed
            },
            stop_reason: if failed {
                ComputationExecutionStopReason::TaskFailed
            } else {
                ComputationExecutionStopReason::Completed
            },
            digest: hash("unsealed"),
        };
        execution.digest =
            ContentHash::of_value(&super::super::execution::digest_input(&execution))
                .expect("execution digest");
        execution.validate().expect("execution");
        let mut events = Vec::new();
        let mut sequence = 1_u64;
        for (index, task_id) in execution.task_order.iter().enumerate() {
            let task_start = start + index as u64 * 10;
            for (kind, tick) in [
                (ComputationInspectionEventKind::Queued, task_start),
                (ComputationInspectionEventKind::Started, task_start + 1),
                (ComputationInspectionEventKind::Retry, task_start + 2),
                (
                    if failed && index == task_count.saturating_sub(1) {
                        ComputationInspectionEventKind::Failed
                    } else {
                        ComputationInspectionEventKind::Completed
                    },
                    task_start + 3,
                ),
            ] {
                events.push(ComputationInspectionEvent {
                    sequence,
                    task_id: task_id.clone(),
                    tick,
                    kind,
                    cost_units: 2,
                    memory_mb: 200,
                    note: "telemetry".into(),
                });
                sequence += 1;
            }
        }
        inspect_glioma_computation_run(&ComputationRunInspectionRequest {
            objective: execution.objective.clone(),
            replay_identity: execution.replay_identity.clone(),
            execution,
            lineage: None,
            partial_results: None,
            events,
            observed_at_tick: end + 1,
            stale_after_ticks: 100,
        })
        .expect("inspection")
    }

    fn input(
        run_id: &str,
        workflow_class: &str,
        inspection: ComputationRunInspection,
    ) -> ComputationCampaignRunInput {
        ComputationCampaignRunInput {
            run_id: run_id.into(),
            scientific_scope: "organoid-invasion-screen".into(),
            workflow_class: workflow_class.into(),
            admitted_at_tick: 0,
            resource_capacity_units: 1_000,
            memory_capacity_mb: 1_000,
            inspection,
        }
    }

    #[test]
    fn successful_throughput_excludes_failed_tasks_and_surfaces_bottleneck() {
        let request = ComputationCampaignTimelineRequest {
            objective: "screen invasion drivers".into(),
            replay_identity: hash("campaign"),
            runs: vec![
                input("run-a", "imaging", inspection("a", 2, false, 100, 120)),
                input("run-b", "imaging", inspection("b", 2, true, 2, 30)),
            ],
            holdout_run_ids: vec!["run-b".into()],
            forecast_horizon_ticks: 100,
            saturation_threshold_milli: 800,
        };
        let output = compile_glioma_high_throughput_compute_timeline(&request).expect("timeline");
        assert_eq!(output.successful_task_count, 3);
        assert_eq!(output.failed_task_count, 1);
        assert!(output
            .bottleneck_order
            .contains(&CampaignBottleneck::QueueLatency));
        assert_eq!(output.forecasts.len(), 1);
        assert_eq!(output.forecasts[0].actual_held_out_successful_tasks, 1);
    }

    #[test]
    fn failed_or_unresolved_work_never_counts_as_ready_throughput() {
        let request = ComputationCampaignTimelineRequest {
            objective: "screen".into(),
            replay_identity: hash("campaign"),
            runs: vec![input("run-a", "imaging", inspection("a", 1, true, 1, 4))],
            holdout_run_ids: Vec::new(),
            forecast_horizon_ticks: 10,
            saturation_threshold_milli: 800,
        };
        let output = compile_glioma_high_throughput_compute_timeline(&request).expect("timeline");
        assert_eq!(
            output.disposition,
            ComputationCampaignTimelineDisposition::Partial
        );
        assert_eq!(output.successful_task_count, 0);
        assert_eq!(output.failed_task_count, 1);
    }

    #[test]
    fn holdout_forecast_is_calibrated_only_when_actual_is_inside_interval() {
        let runs = vec![
            input("run-a", "same", inspection("a", 2, false, 1, 3)),
            input("run-b", "same", inspection("b", 2, false, 1, 3)),
            input("run-c", "same", inspection("c", 2, false, 1, 3)),
        ];
        let request = ComputationCampaignTimelineRequest {
            objective: "forecast".into(),
            replay_identity: hash("campaign"),
            runs,
            holdout_run_ids: vec!["run-c".into()],
            forecast_horizon_ticks: 10,
            saturation_threshold_milli: 900,
        };
        let output = compile_glioma_high_throughput_compute_timeline(&request).expect("timeline");
        assert!(output.forecasts[0].calibrated);
        assert_eq!(output.forecasts[0].absolute_error_tasks, 0);
    }

    #[test]
    fn permutation_of_run_inputs_does_not_change_digest() {
        let one = input("run-a", "same", inspection("a", 1, false, 1, 3));
        let two = input("run-b", "same", inspection("b", 1, false, 1, 3));
        let left = ComputationCampaignTimelineRequest {
            objective: "stable".into(),
            replay_identity: hash("campaign"),
            runs: vec![one.clone(), two.clone()],
            holdout_run_ids: vec!["run-b".into()],
            forecast_horizon_ticks: 10,
            saturation_threshold_milli: 900,
        };
        let right = ComputationCampaignTimelineRequest {
            runs: vec![two, one],
            ..left.clone()
        };
        assert_eq!(
            compile_glioma_high_throughput_compute_timeline(&left)
                .unwrap()
                .digest,
            compile_glioma_high_throughput_compute_timeline(&right)
                .unwrap()
                .digest
        );
    }

    #[test]
    fn unknown_holdout_is_rejected_before_computation() {
        let request = ComputationCampaignTimelineRequest {
            objective: "reject".into(),
            replay_identity: hash("campaign"),
            runs: vec![input("run-a", "same", inspection("a", 1, false, 1, 3))],
            holdout_run_ids: vec!["missing".into()],
            forecast_horizon_ticks: 10,
            saturation_threshold_milli: 900,
        };
        assert!(matches!(
            compile_glioma_high_throughput_compute_timeline(&request),
            Err(ComputationCampaignTimelineError::InvalidRequest(_))
        ));
    }
}
