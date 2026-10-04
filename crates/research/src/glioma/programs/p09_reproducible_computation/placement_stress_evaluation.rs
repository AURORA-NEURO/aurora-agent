//! Stress evaluation for the reproducible glioma computation placement planner.
//!
//! A placement schedule is only useful if it remains robust when local workers disappear,
//! transfer costs rise, or the compute envelope contracts. This composition replays the existing
//! deterministic placement planner under investigator-declared operational scenarios and compares
//! it with a constrained fastest-single-worker baseline. It evaluates scheduling utility only;
//! it never runs code, moves raw data, or treats scheduling performance as biological evidence.

use super::placement::{
    schedule_glioma_computation_placement, ComputationPlacementDisposition,
    ComputationPlacementError, ComputationPlacementRequest, ComputationPlacementSchedule,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-computation-placement-stress-evaluation";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationPlacementStressEvaluation1@1";
pub const MAX_SCENARIOS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementStressScenario {
    pub scenario_id: String,
    pub disabled_worker_order: Vec<String>,
    pub transfer_cost_multiplier_milli: u16,
    pub transfer_ticks_multiplier_milli: u16,
    pub budget_multiplier_milli: u16,
    pub end_tick_multiplier_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementStressEvaluationRequest {
    pub base: ComputationPlacementRequest,
    pub scenarios: Vec<ComputationPlacementStressScenario>,
    pub require_non_degradation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementStressMetric {
    pub scenario_id: String,
    pub proposed_disposition: ComputationPlacementDisposition,
    pub baseline_disposition: ComputationPlacementDisposition,
    pub proposed_assigned_count: usize,
    pub baseline_assigned_count: usize,
    pub proposed_makespan_ticks: u64,
    pub baseline_makespan_ticks: u64,
    pub proposed_transfer_cost_units: u64,
    pub baseline_transfer_cost_units: u64,
    pub advantage_milli: i64,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationPlacementStressDisposition {
    Qualified,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementStressEvaluation {
    pub composition_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub scenario_order: Vec<String>,
    pub metrics: Vec<ComputationPlacementStressMetric>,
    pub qualified_scenario_order: Vec<String>,
    pub blocked_scenario_order: Vec<String>,
    pub mean_advantage_milli: i64,
    pub worst_advantage_milli: i64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationPlacementStressDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationPlacementStressEvaluationError {
    #[error("computation placement stress request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation placement scenario is invalid: {0}")]
    Scenario(String),
    #[error("computation placement planner failed: {0}")]
    Placement(String),
    #[error("computation placement stress output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation placement stress digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn digest_input(output: &ComputationPlacementStressEvaluation) -> serde_json::Value {
    serde_json::json!({
        "composition_id": output.composition_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "scenario_order": output.scenario_order,
        "metrics": output.metrics,
        "qualified_scenario_order": output.qualified_scenario_order,
        "blocked_scenario_order": output.blocked_scenario_order,
        "mean_advantage_milli": output.mean_advantage_milli,
        "worst_advantage_milli": output.worst_advantage_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &ComputationPlacementStressEvaluationRequest,
) -> Result<(), ComputationPlacementStressEvaluationError> {
    if request.base.objective.trim().is_empty()
        || request.base.replay_identity.as_str().len() != 64
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
    {
        return Err(ComputationPlacementStressEvaluationError::InvalidRequest(
            "base objective/replay identity and bounded non-empty scenarios are required".into(),
        ));
    }
    let worker_ids = request
        .base
        .workers
        .iter()
        .map(|worker| worker.worker_id.clone())
        .collect::<BTreeSet<_>>();
    let mut scenario_ids = BTreeSet::new();
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !scenario_ids.insert(scenario.scenario_id.clone())
            || scenario.transfer_cost_multiplier_milli == 0
            || scenario.transfer_ticks_multiplier_milli == 0
            || scenario.budget_multiplier_milli == 0
            || scenario.end_tick_multiplier_milli == 0
            || scenario
                .disabled_worker_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || scenario
                .disabled_worker_order
                .iter()
                .any(|worker_id| !worker_ids.contains(worker_id))
        {
            return Err(ComputationPlacementStressEvaluationError::Scenario(
                "scenario ids, multipliers, disabled-worker order, and worker references must be bounded and canonical".into(),
            ));
        }
    }
    Ok(())
}

impl ComputationPlacementStressEvaluation {
    pub fn validate(&self) -> Result<(), ComputationPlacementStressEvaluationError> {
        if self.composition_id != COMPOSITION_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || !canonical(&self.scenario_order)
            || self.metrics.len() != self.scenario_order.len()
            || self
                .metrics
                .windows(2)
                .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
            || !canonical(&self.qualified_scenario_order)
            || !canonical(&self.blocked_scenario_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .qualified_scenario_order
                .iter()
                .any(|id| self.blocked_scenario_order.binary_search(id).is_ok())
            || self.metrics.iter().any(|metric| {
                metric.scenario_id.trim().is_empty() || !canonical(&metric.uncertainty)
            })
        {
            return Err(ComputationPlacementStressEvaluationError::InvalidOutput(
                "identity, scenario ordering, metrics, or partition invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self)).map_err(|error| {
            ComputationPlacementStressEvaluationError::Digest(error.to_string())
        })?;
        if expected != self.digest {
            return Err(ComputationPlacementStressEvaluationError::InvalidOutput(
                "stress evaluation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn scenario_request(
    base: &ComputationPlacementRequest,
    scenario: &ComputationPlacementStressScenario,
) -> Result<ComputationPlacementRequest, ComputationPlacementStressEvaluationError> {
    let mut request = base.clone();
    request.workers.retain(|worker| {
        !scenario
            .disabled_worker_order
            .binary_search(&worker.worker_id)
            .is_ok()
    });
    for worker in &mut request.workers {
        worker.transfer_cost_units_per_artifact = worker
            .transfer_cost_units_per_artifact
            .saturating_mul(u64::from(scenario.transfer_cost_multiplier_milli))
            .saturating_div(1_000);
        worker.transfer_ticks_per_artifact = worker
            .transfer_ticks_per_artifact
            .saturating_mul(u64::from(scenario.transfer_ticks_multiplier_milli))
            .saturating_div(1_000);
    }
    request.max_budget_units = request
        .max_budget_units
        .saturating_mul(u64::from(scenario.budget_multiplier_milli))
        .saturating_div(1_000);
    request.max_end_tick = request.current_tick.saturating_add(
        request
            .max_end_tick
            .saturating_sub(request.current_tick)
            .saturating_mul(u64::from(scenario.end_tick_multiplier_milli))
            .saturating_div(1_000),
    );
    if request.max_budget_units == 0 || request.max_end_tick <= request.current_tick {
        return Err(ComputationPlacementStressEvaluationError::Scenario(
            format!(
                "scenario {} contracts the execution envelope to zero",
                scenario.scenario_id
            ),
        ));
    }
    Ok(request)
}

fn fastest_single_worker(request: &ComputationPlacementRequest) -> ComputationPlacementRequest {
    let mut baseline = request.clone();
    baseline.workers.sort_by(|left, right| {
        right
            .speed_milli
            .cmp(&left.speed_milli)
            .then_with(|| left.worker_id.cmp(&right.worker_id))
    });
    baseline.workers.truncate(1);
    baseline
}

fn placement_or_blocked(
    request: &ComputationPlacementRequest,
) -> Result<ComputationPlacementSchedule, ComputationPlacementStressEvaluationError> {
    schedule_glioma_computation_placement(request).map_err(|error: ComputationPlacementError| {
        ComputationPlacementStressEvaluationError::Placement(error.to_string())
    })
}

fn advantage_milli(
    proposed: &ComputationPlacementSchedule,
    baseline: &ComputationPlacementSchedule,
) -> i64 {
    let coverage = (proposed.assigned_order.len() as i64 - baseline.assigned_order.len() as i64)
        .saturating_mul(10_000);
    let time = (baseline.makespan_ticks as i64 - proposed.makespan_ticks as i64)
        .clamp(-1_000_000, 1_000_000);
    let transfer = (baseline.total_transfer_cost_units as i64
        - proposed.total_transfer_cost_units as i64)
        .clamp(-1_000_000, 1_000_000)
        .saturating_mul(100);
    coverage.saturating_add(time).saturating_add(transfer)
}

fn disposition_is_blocked(disposition: ComputationPlacementDisposition) -> bool {
    matches!(disposition, ComputationPlacementDisposition::Blocked)
}

/// Replay computation placement under operational stress and compare it with a fastest-worker
/// baseline. This is an evaluation artifact only; no computation worker is invoked.
pub fn evaluate_glioma_computation_placement_stress(
    request: &ComputationPlacementStressEvaluationRequest,
) -> Result<ComputationPlacementStressEvaluation, ComputationPlacementStressEvaluationError> {
    validate_request(request)?;
    let mut scenarios = request.scenarios.clone();
    scenarios.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let mut metrics = Vec::with_capacity(scenarios.len());
    let mut qualified = Vec::new();
    let mut blocked = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for scenario in scenarios {
        let scenario_request = scenario_request(&request.base, &scenario)?;
        let proposed = placement_or_blocked(&scenario_request)?;
        let baseline = placement_or_blocked(&fastest_single_worker(&scenario_request))?;
        let advantage = advantage_milli(&proposed, &baseline);
        let mut metric_uncertainty = Vec::new();
        if proposed.assigned_order.len() < proposed.task_order.len() {
            metric_uncertainty.push("proposed-placement-partial-coverage".into());
        }
        if disposition_is_blocked(proposed.disposition) {
            metric_uncertainty.push("proposed-placement-blocked".into());
            blocked.push(scenario.scenario_id.clone());
        } else if request.require_non_degradation
            && proposed.assigned_order.len() < baseline.assigned_order.len()
        {
            metric_uncertainty.push("coverage-degraded-against-single-worker-baseline".into());
            negative_evidence.push(format!(
                "{} proposed coverage {} below baseline {}",
                scenario.scenario_id,
                proposed.assigned_order.len(),
                baseline.assigned_order.len()
            ));
        } else {
            qualified.push(scenario.scenario_id.clone());
        }
        if !metric_uncertainty.is_empty() {
            uncertainty.extend(
                metric_uncertainty
                    .iter()
                    .map(|reason| format!("{}:{reason}", scenario.scenario_id)),
            );
        }
        metrics.push(ComputationPlacementStressMetric {
            scenario_id: scenario.scenario_id,
            proposed_disposition: proposed.disposition,
            baseline_disposition: baseline.disposition,
            proposed_assigned_count: proposed.assigned_order.len(),
            baseline_assigned_count: baseline.assigned_order.len(),
            proposed_makespan_ticks: proposed.makespan_ticks,
            baseline_makespan_ticks: baseline.makespan_ticks,
            proposed_transfer_cost_units: proposed.total_transfer_cost_units,
            baseline_transfer_cost_units: baseline.total_transfer_cost_units,
            advantage_milli: advantage,
            uncertainty: sorted_unique(metric_uncertainty),
        });
    }
    let advantages = metrics
        .iter()
        .map(|metric| metric.advantage_milli)
        .collect::<Vec<_>>();
    let mean_advantage_milli = advantages.iter().sum::<i64>() / advantages.len().max(1) as i64;
    let worst_advantage_milli = advantages.iter().copied().min().unwrap_or_default();
    let disposition = if qualified.len() == metrics.len() {
        ComputationPlacementStressDisposition::Qualified
    } else if qualified.is_empty() {
        ComputationPlacementStressDisposition::Blocked
    } else {
        ComputationPlacementStressDisposition::Partial
    };
    let mut output = ComputationPlacementStressEvaluation {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.base.objective.clone(),
        replay_identity: request.base.replay_identity.clone(),
        scenario_order: metrics
            .iter()
            .map(|metric| metric.scenario_id.clone())
            .collect(),
        metrics,
        qualified_scenario_order: sorted_unique(qualified),
        blocked_scenario_order: sorted_unique(blocked),
        mean_advantage_milli,
        worst_advantage_milli,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-placement-stress"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationPlacementStressEvaluationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::execution::{ComputationCacheEntry, ComputationOperation, ComputationTask};
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn request() -> ComputationPlacementStressEvaluationRequest {
        let task = |task_id: &str, depends_on: Vec<&str>| ComputationTask {
            task_id: task_id.into(),
            operation: ComputationOperation::Normalize,
            model_system: GliomaModelSystem::Organoid,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            input_artifact_ids: vec!["matrix".into()],
            output_schema: format!("{}@1", task_id),
            estimated_cost_units: 10,
            estimated_duration_ticks: 5,
            deterministic: true,
        };
        ComputationPlacementStressEvaluationRequest {
            base: ComputationPlacementRequest {
                objective: "stress organoid multimodal placement".into(),
                model_system: GliomaModelSystem::Organoid,
                replay_identity: ContentHash::of_bytes(b"placement-stress"),
                current_tick: 0,
                max_end_tick: 100,
                max_budget_units: 100,
                max_transfer_cost_units: 100,
                tasks: vec![
                    task("integrate", vec!["normalize"]),
                    task("normalize", vec![]),
                ],
                workers: vec![
                    super::super::placement::ComputationWorkerProfile {
                        worker_id: "gpu-a".into(),
                        model_system_order: vec![GliomaModelSystem::Organoid],
                        operation_order: vec![ComputationOperation::Normalize],
                        local_artifact_order: vec!["matrix".into()],
                        available_from_tick: 0,
                        available_until_tick: 100,
                        max_task_cost_units: 100,
                        transfer_ticks_per_artifact: 2,
                        transfer_cost_units_per_artifact: 3,
                        speed_milli: 1000,
                        enabled: true,
                    },
                    super::super::placement::ComputationWorkerProfile {
                        worker_id: "cpu-b".into(),
                        model_system_order: vec![GliomaModelSystem::Organoid],
                        operation_order: vec![ComputationOperation::Normalize],
                        local_artifact_order: vec![],
                        available_from_tick: 0,
                        available_until_tick: 100,
                        max_task_cost_units: 100,
                        transfer_ticks_per_artifact: 2,
                        transfer_cost_units_per_artifact: 3,
                        speed_milli: 700,
                        enabled: true,
                    },
                ],
                completed_task_order: Vec::new(),
                cache: Vec::<ComputationCacheEntry>::new(),
            },
            scenarios: vec![
                ComputationPlacementStressScenario {
                    scenario_id: "nominal".into(),
                    disabled_worker_order: Vec::new(),
                    transfer_cost_multiplier_milli: 1_000,
                    transfer_ticks_multiplier_milli: 1_000,
                    budget_multiplier_milli: 1_000,
                    end_tick_multiplier_milli: 1_000,
                },
                ComputationPlacementStressScenario {
                    scenario_id: "gpu-loss".into(),
                    disabled_worker_order: vec!["gpu-a".into()],
                    transfer_cost_multiplier_milli: 1_500,
                    transfer_ticks_multiplier_milli: 1_500,
                    budget_multiplier_milli: 1_000,
                    end_tick_multiplier_milli: 1_000,
                },
            ],
            require_non_degradation: false,
        }
    }

    #[test]
    fn placement_stress_evaluation_reports_worker_loss_without_dispatch() {
        let output = evaluate_glioma_computation_placement_stress(&request()).unwrap();
        assert_eq!(output.scenario_order, vec!["gpu-loss", "nominal"]);
        assert!(output
            .metrics
            .iter()
            .any(|metric| metric.scenario_id == "gpu-loss"));
        output.validate().unwrap();
    }

    #[test]
    fn placement_stress_evaluation_rejects_unknown_disabled_worker() {
        let mut request = request();
        request.scenarios[0].disabled_worker_order = vec!["unknown".into()];
        assert!(matches!(
            evaluate_glioma_computation_placement_stress(&request),
            Err(ComputationPlacementStressEvaluationError::Scenario(_))
        ));
    }
}
