//! Robust scenario-ensemble simulation for autonomous preclinical glioma workflows.
//!
//! A single protocol schedule is not enough for an autonomous engine: capacity changes, slower
//! biology, risk-budget pressure, and missing instrument approval can all invalidate a plan. This
//! feature evaluates a bounded, probability-weighted set of typed perturbations through the
//! deterministic P07 simulator and returns the failure topology, coverage proxy, expected burden,
//! and an explicit robustness disposition. It never treats schedule coverage as biological
//! efficacy and never dispatches an instrument or moves raw data.

use super::simulator::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolSimulation, ProtocolSimulationError,
    ProtocolSimulationRequest,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

// This is an extension surface of the existing P07-F01 adaptive workflow feature. Keeping the
// owner id here makes the extension's digest and catalog relationship explicit without minting a
// duplicate implementation id for the workflow module.
const OWNER_FEATURE_ID: &str = crate::glioma::workflow::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolScenarioEnsemble1@1";
pub const MAX_SCENARIOS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenario {
    pub scenario_id: String,
    pub duration_scale_milli: u16,
    pub capacity_scale_milli: u16,
    pub risk_delta_milli: i16,
    pub probability_milli: u16,
    pub allow_instrument_execution: bool,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenarioEnsembleRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub base_request: ProtocolSimulationRequest,
    pub scenarios: Vec<ProtocolScenario>,
    pub minimum_schedule_coverage_milli: u16,
    pub maximum_expected_makespan_ticks: u32,
    pub maximum_expected_risk_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioFailureClass {
    None,
    Capacity,
    Risk,
    Approval,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenarioResult {
    pub scenario_id: String,
    pub probability_milli: u16,
    pub schedule_coverage_milli: u16,
    pub failure_class: ScenarioFailureClass,
    pub simulation: ProtocolSimulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolScenarioEnsembleDisposition {
    Robust,
    Fragile,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenarioEnsemble {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub scenario_order: Vec<String>,
    pub results: Vec<ProtocolScenarioResult>,
    pub weighted_schedule_coverage_milli: u16,
    pub worst_case_schedule_coverage_milli: u16,
    pub weighted_makespan_ticks: u32,
    pub weighted_risk_milli: u32,
    pub failed_scenario_order: Vec<String>,
    pub bottleneck_task_order: Vec<String>,
    pub disposition: ProtocolScenarioEnsembleDisposition,
    pub acceptance_gate: String,
    pub stop_conditions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolScenarioEnsembleError {
    #[error("scenario ensemble request is invalid: {0}")]
    InvalidRequest(String),
    #[error("scenario simulation failed: {0}")]
    Simulation(String),
    #[error("scenario ensemble output is invalid: {0}")]
    InvalidOutput(String),
    #[error("scenario ensemble digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ProtocolScenarioEnsemble) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "scenario_order": output.scenario_order,
        "results": output.results,
        "weighted_schedule_coverage_milli": output.weighted_schedule_coverage_milli,
        "worst_case_schedule_coverage_milli": output.worst_case_schedule_coverage_milli,
        "weighted_makespan_ticks": output.weighted_makespan_ticks,
        "weighted_risk_milli": output.weighted_risk_milli,
        "failed_scenario_order": output.failed_scenario_order,
        "bottleneck_task_order": output.bottleneck_task_order,
        "disposition": output.disposition,
        "acceptance_gate": output.acceptance_gate,
        "stop_conditions": output.stop_conditions,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "boundary": output.boundary,
    })
}

fn validate_request(
    request: &ProtocolScenarioEnsembleRequest,
) -> Result<(), ProtocolScenarioEnsembleError> {
    if request.objective.trim().is_empty()
        || request.model_system != request.base_request.model_system
        || request.base_request.objective.trim().is_empty()
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || request.minimum_schedule_coverage_milli > 1_000
        || request.maximum_expected_makespan_ticks == 0
        || request.maximum_expected_risk_milli == 0
    {
        return Err(ProtocolScenarioEnsembleError::InvalidRequest(
            "objective, model binding, scenarios, coverage, makespan, and risk bounds are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut probability_total = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !ids.insert(scenario.scenario_id.clone())
            || scenario.duration_scale_milli == 0
            || scenario.duration_scale_milli > 5_000
            || scenario.capacity_scale_milli == 0
            || scenario.capacity_scale_milli > 5_000
            || scenario.risk_delta_milli.unsigned_abs() > 1_000
            || scenario.probability_milli == 0
            || scenario
                .approval_reference
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ProtocolScenarioEnsembleError::InvalidRequest(
                "scenario identity, scales, risk delta, probability, or approval is invalid".into(),
            ));
        }
        probability_total = probability_total.saturating_add(u32::from(scenario.probability_milli));
    }
    if probability_total != 1_000 {
        return Err(ProtocolScenarioEnsembleError::InvalidRequest(
            "scenario probabilities must sum to exactly 1000 milli-units".into(),
        ));
    }
    Ok(())
}

fn scale_u32(value: u32, scale_milli: u16) -> u32 {
    (u128::from(value)
        .saturating_mul(u128::from(scale_milli))
        .saturating_add(999)
        .saturating_div(1_000)
        .clamp(1, u128::from(u32::MAX))) as u32
}

fn scale_u16(value: u16, scale_milli: u16) -> u16 {
    (u32::from(value)
        .saturating_mul(u32::from(scale_milli))
        .saturating_add(999)
        .saturating_div(1_000)
        .clamp(1, u32::from(u16::MAX))) as u16
}

fn scenario_request(
    base: &ProtocolSimulationRequest,
    scenario: &ProtocolScenario,
) -> ProtocolSimulationRequest {
    let mut request = base.clone();
    request.tasks.iter_mut().for_each(|task| {
        task.duration_ticks = scale_u32(task.duration_ticks, scenario.duration_scale_milli);
        task.risk_milli = (i32::from(task.risk_milli) + i32::from(scenario.risk_delta_milli))
            .clamp(0, 1_000) as u16;
    });
    request.resources.iter_mut().for_each(|resource| {
        resource.capacity_units = scale_u16(resource.capacity_units, scenario.capacity_scale_milli);
    });
    request.allow_instrument_execution = scenario.allow_instrument_execution;
    request.approval_reference = if scenario.allow_instrument_execution {
        scenario
            .approval_reference
            .clone()
            .or_else(|| base.approval_reference.clone())
    } else {
        None
    };
    request
}

fn failure_class(disposition: ProtocolDisposition) -> ScenarioFailureClass {
    match disposition {
        ProtocolDisposition::Feasible => ScenarioFailureClass::None,
        ProtocolDisposition::CapacityBlocked => ScenarioFailureClass::Capacity,
        ProtocolDisposition::RiskBlocked => ScenarioFailureClass::Risk,
        ProtocolDisposition::ApprovalRequired => ScenarioFailureClass::Approval,
        ProtocolDisposition::Unresolved => ScenarioFailureClass::Unresolved,
    }
}

fn schedule_coverage(simulation: &ProtocolSimulation) -> u16 {
    let total = simulation.topological_order.len().max(1) as u64;
    let scheduled = total.saturating_sub(simulation.unscheduled_order.len() as u64);
    (scheduled
        .saturating_mul(1_000)
        .saturating_div(total)
        .min(1_000)) as u16
}

fn validate_output(output: &ProtocolScenarioEnsemble) -> Result<(), ProtocolScenarioEnsembleError> {
    if output.feature_id != OWNER_FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || output.scenario_order.is_empty()
        || !canonical(&output.scenario_order)
        || !canonical(&output.failed_scenario_order)
        || output
            .bottleneck_task_order
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || output.results.len() != output.scenario_order.len()
        || output.weighted_schedule_coverage_milli > 1_000
        || output.worst_case_schedule_coverage_milli > 1_000
        || output.acceptance_gate.trim().is_empty()
        || output.stop_conditions.is_empty()
        || output
            .stop_conditions
            .iter()
            .any(|item| item.trim().is_empty())
        || output.uncertainty.iter().any(|item| item.trim().is_empty())
        || output
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
    {
        return Err(ProtocolScenarioEnsembleError::InvalidOutput(
            "identity, ordering, metrics, boundary, or limitation fields are invalid".into(),
        ));
    }
    let result_ids = output
        .results
        .iter()
        .map(|result| result.scenario_id.clone())
        .collect::<Vec<_>>();
    if result_ids != output.scenario_order
        || output.results.iter().any(|result| {
            result.probability_milli == 0
                || result.schedule_coverage_milli > 1_000
                || result.simulation.model_system != output.model_system
                || result.simulation.validate().is_err()
                || (result.failure_class == ScenarioFailureClass::None
                    && result.simulation.disposition != ProtocolDisposition::Feasible)
        })
    {
        return Err(ProtocolScenarioEnsembleError::InvalidOutput(
            "scenario results are not ordered, bounded, model-bound, or simulator-valid".into(),
        ));
    }
    if output
        .failed_scenario_order
        .iter()
        .any(|scenario| !output.scenario_order.contains(scenario))
    {
        return Err(ProtocolScenarioEnsembleError::InvalidOutput(
            "failed scenarios must be a subset of the scenario order".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ProtocolScenarioEnsembleError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ProtocolScenarioEnsembleError::InvalidOutput(
            "digest is not bound to the scenario ensemble".into(),
        ));
    }
    Ok(())
}

impl ProtocolScenarioEnsemble {
    pub fn validate(&self) -> Result<(), ProtocolScenarioEnsembleError> {
        validate_output(self)
    }
}

/// Evaluate bounded resource, timing, risk, and approval perturbations for a glioma protocol.
pub fn simulate_glioma_protocol_scenario_ensemble(
    request: &ProtocolScenarioEnsembleRequest,
) -> Result<ProtocolScenarioEnsemble, ProtocolScenarioEnsembleError> {
    validate_request(request)?;
    let mut scenarios = request.scenarios.clone();
    scenarios.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let mut results = Vec::with_capacity(scenarios.len());
    let mut bottleneck_counts = BTreeMap::<String, u32>::new();
    for scenario in scenarios {
        let simulation = simulate_glioma_protocol(&scenario_request(
            &request.base_request,
            &scenario,
        ))
        .map_err(|error: ProtocolSimulationError| {
            ProtocolScenarioEnsembleError::Simulation(format!("{}: {error}", scenario.scenario_id))
        })?;
        for task_id in &simulation.unscheduled_order {
            *bottleneck_counts.entry(task_id.clone()).or_default() += 1;
        }
        results.push(ProtocolScenarioResult {
            scenario_id: scenario.scenario_id,
            probability_milli: scenario.probability_milli,
            schedule_coverage_milli: schedule_coverage(&simulation),
            failure_class: failure_class(simulation.disposition),
            simulation,
        });
    }
    let weighted_schedule = results
        .iter()
        .map(|result| {
            u64::from(result.probability_milli) * u64::from(result.schedule_coverage_milli)
        })
        .sum::<u64>()
        .saturating_div(1_000)
        .min(1_000) as u16;
    let worst_case = results
        .iter()
        .map(|result| result.schedule_coverage_milli)
        .min()
        .unwrap_or(0);
    let weighted_makespan = results
        .iter()
        .map(|result| {
            u64::from(result.probability_milli) * u64::from(result.simulation.makespan_ticks)
        })
        .sum::<u64>()
        .saturating_div(1_000)
        .min(u64::from(u32::MAX)) as u32;
    let weighted_risk = results
        .iter()
        .map(|result| {
            u64::from(result.probability_milli) * u64::from(result.simulation.risk_total_milli)
        })
        .sum::<u64>()
        .saturating_div(1_000)
        .min(u64::from(u32::MAX)) as u32;
    let failed_scenario_order = results
        .iter()
        .filter(|result| result.failure_class != ScenarioFailureClass::None)
        .map(|result| result.scenario_id.clone())
        .collect::<Vec<_>>();
    let mut bottleneck_task_order = bottleneck_counts.into_iter().collect::<Vec<_>>();
    bottleneck_task_order
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let bottleneck_task_order = bottleneck_task_order
        .into_iter()
        .map(|(task_id, _)| task_id)
        .collect::<Vec<_>>();
    let mut uncertainty = vec![
        "schedule coverage is a feasibility proxy, not biological efficacy or assay validity".into(),
        "scenario probabilities, duration scales, capacity scales, and risk deltas are caller-declared".into(),
        "the ensemble is deterministic and local; it does not observe live instruments or specimens".into(),
    ];
    let mut negative_evidence = Vec::new();
    if weighted_schedule < request.minimum_schedule_coverage_milli {
        uncertainty.push(format!(
            "weighted-schedule-coverage:{}<{}",
            weighted_schedule, request.minimum_schedule_coverage_milli
        ));
        negative_evidence
            .push("weighted schedule coverage is below the acceptance threshold".into());
    }
    if worst_case < request.minimum_schedule_coverage_milli {
        negative_evidence.push(format!(
            "worst-case-schedule-coverage:{}<{}",
            worst_case, request.minimum_schedule_coverage_milli
        ));
    }
    if weighted_makespan > request.maximum_expected_makespan_ticks {
        negative_evidence.push(format!(
            "expected-makespan:{}>{}",
            weighted_makespan, request.maximum_expected_makespan_ticks
        ));
    }
    if weighted_risk > request.maximum_expected_risk_milli {
        negative_evidence.push(format!(
            "expected-risk:{}>{}",
            weighted_risk, request.maximum_expected_risk_milli
        ));
    }
    for scenario in &failed_scenario_order {
        negative_evidence.push(format!("scenario:{scenario}:not-feasible"));
    }
    let all_feasible = failed_scenario_order.is_empty();
    let disposition = if all_feasible
        && weighted_schedule >= request.minimum_schedule_coverage_milli
        && worst_case >= request.minimum_schedule_coverage_milli
        && weighted_makespan <= request.maximum_expected_makespan_ticks
        && weighted_risk <= request.maximum_expected_risk_milli
    {
        ProtocolScenarioEnsembleDisposition::Robust
    } else if results
        .iter()
        .any(|result| result.failure_class == ScenarioFailureClass::Unresolved)
        && worst_case == 0
    {
        ProtocolScenarioEnsembleDisposition::Unresolved
    } else {
        ProtocolScenarioEnsembleDisposition::Fragile
    };
    let acceptance_gate = match disposition {
        ProtocolScenarioEnsembleDisposition::Robust => {
            "all declared scenarios schedule completely within expected timing and risk gates"
        }
        ProtocolScenarioEnsembleDisposition::Fragile => {
            "hold autonomous dispatch; resolve failed scenarios or tighten the local resource envelope"
        }
        ProtocolScenarioEnsembleDisposition::Unresolved => {
            "hold the workflow because one or more scenario dependency closures are unresolved"
        }
    }
    .to_string();
    let mut output = ProtocolScenarioEnsemble {
        feature_id: OWNER_FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        scenario_order: results
            .iter()
            .map(|result| result.scenario_id.clone())
            .collect(),
        results,
        weighted_schedule_coverage_milli: weighted_schedule,
        worst_case_schedule_coverage_milli: worst_case,
        weighted_makespan_ticks: weighted_makespan,
        weighted_risk_milli: weighted_risk,
        failed_scenario_order,
        bottleneck_task_order,
        disposition,
        acceptance_gate,
        stop_conditions: vec![
            "never interpret schedule coverage as a biological conclusion".into(),
            "never dispatch an instrument or provider from the ensemble output".into(),
            "stop and replan when a scenario is capacity, risk, approval, or dependency blocked"
                .into(),
        ],
        uncertainty,
        negative_evidence,
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-scenario-ensemble"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProtocolScenarioEnsembleError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolResourceKind, ProtocolTask,
    };

    fn base_request() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "run an organoid invasion workflow".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![
                ProtocolTask {
                    task_id: "setup".into(),
                    label: "prepare organoid".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 2,
                    depends_on: vec![],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Setup1@1".into(),
                    risk_milli: 20,
                    requires_instrument: false,
                },
                ProtocolTask {
                    task_id: "image".into(),
                    label: "image invasion".into(),
                    resource_kind: ProtocolResourceKind::Imaging,
                    resource_units: 1,
                    duration_ticks: 4,
                    depends_on: vec!["setup".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Image1@1".into(),
                    risk_milli: 30,
                    requires_instrument: true,
                },
            ],
            resources: vec![
                ProtocolResource {
                    resource_id: "culture-1".into(),
                    kind: ProtocolResourceKind::Culture,
                    capacity_units: 1,
                },
                ProtocolResource {
                    resource_id: "image-1".into(),
                    kind: ProtocolResourceKind::Imaging,
                    capacity_units: 1,
                },
            ],
            max_ticks: 20,
            max_risk_milli: 200,
            allow_instrument_execution: true,
            approval_reference: Some("approval-1".into()),
            randomization_seed: ContentHash::of_bytes(b"scenario-ensemble-test"),
        }
    }

    fn request() -> ProtocolScenarioEnsembleRequest {
        ProtocolScenarioEnsembleRequest {
            objective: "run an organoid invasion workflow".into(),
            model_system: GliomaModelSystem::Organoid,
            base_request: base_request(),
            scenarios: vec![
                ProtocolScenario {
                    scenario_id: "nominal".into(),
                    duration_scale_milli: 1_000,
                    capacity_scale_milli: 1_000,
                    risk_delta_milli: 0,
                    probability_milli: 700,
                    allow_instrument_execution: true,
                    approval_reference: Some("approval-1".into()),
                },
                ProtocolScenario {
                    scenario_id: "no-approval".into(),
                    duration_scale_milli: 1_000,
                    capacity_scale_milli: 1_000,
                    risk_delta_milli: 0,
                    probability_milli: 300,
                    allow_instrument_execution: false,
                    approval_reference: None,
                },
            ],
            minimum_schedule_coverage_milli: 700,
            maximum_expected_makespan_ticks: 20,
            maximum_expected_risk_milli: 200,
        }
    }

    #[test]
    fn ensemble_is_replay_stable_and_surfaces_approval_failure() {
        let request = request();
        let left = simulate_glioma_protocol_scenario_ensemble(&request).unwrap();
        let right = simulate_glioma_protocol_scenario_ensemble(&request).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.scenario_order, vec!["no-approval", "nominal"]);
        assert_eq!(
            left.disposition,
            ProtocolScenarioEnsembleDisposition::Fragile
        );
        assert!(left
            .results
            .iter()
            .any(|result| result.failure_class == ScenarioFailureClass::Approval));
        assert!(left
            .negative_evidence
            .iter()
            .any(|item| item.contains("no-approval")));
        left.validate().unwrap();
    }

    #[test]
    fn capacity_perturbation_reports_bottleneck_without_passing() {
        let mut request = request();
        request.scenarios = vec![ProtocolScenario {
            scenario_id: "capacity-collapse".into(),
            duration_scale_milli: 5_000,
            capacity_scale_milli: 1,
            risk_delta_milli: 0,
            probability_milli: 1_000,
            allow_instrument_execution: true,
            approval_reference: Some("approval-1".into()),
        }];
        request.minimum_schedule_coverage_milli = 1_000;
        let output = simulate_glioma_protocol_scenario_ensemble(&request).unwrap();
        assert_eq!(
            output.disposition,
            ProtocolScenarioEnsembleDisposition::Fragile
        );
        assert!(!output.bottleneck_task_order.is_empty());
        assert!(output.worst_case_schedule_coverage_milli < 1_000);
    }
}
