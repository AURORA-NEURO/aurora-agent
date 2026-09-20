//! Quality- and uncertainty-aware evidence compilation from a local protocol run.
//!
//! This feature is the scientific handoff between protocol execution and downstream mechanism or
//! interpretation programs. It aggregates typed endpoint measurements with robust summaries,
//! preserves null/negative and contradictory signals, and refuses to promote missing, noisy, or
//! failed task outputs. It does not infer a clinical conclusion and does not impute measurements.

use super::execution::{ProtocolExecution, ProtocolTaskDisposition};
use super::simulator::{simulate_glioma_protocol, ProtocolSimulationRequest};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolEvidenceSurface1@1";
pub const MAX_MEASUREMENTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolMeasurement {
    pub measurement_id: String,
    pub task_id: String,
    pub output_schema: String,
    pub endpoint_id: String,
    pub modality: String,
    pub value_milli: i32,
    pub uncertainty_milli: u16,
    pub quality_milli: u16,
    pub replicate_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceSurfaceRequest {
    pub objective: String,
    pub protocol: ProtocolSimulationRequest,
    pub execution: ProtocolExecution,
    pub measurements: Vec<ProtocolMeasurement>,
    pub min_replicates: u16,
    pub min_quality_milli: u16,
    pub max_uncertainty_milli: u16,
    pub contradiction_threshold_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEvidenceDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
    Contradictory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceCell {
    pub endpoint_id: String,
    pub measurement_order: Vec<String>,
    pub task_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub measurement_count: u16,
    pub replicate_count: u16,
    pub robust_value_milli: Option<i32>,
    pub spread_milli: u32,
    pub quality_milli: u16,
    pub max_uncertainty_milli: u16,
    pub disposition: ProtocolEvidenceDisposition,
    pub information_milli: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEvidenceSurfaceDisposition {
    Qualified,
    Partial,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceSurface {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub protocol_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub cells: Vec<ProtocolEvidenceCell>,
    pub qualified_endpoint_order: Vec<String>,
    pub negative_endpoint_order: Vec<String>,
    pub partial_endpoint_order: Vec<String>,
    pub unresolved_endpoint_order: Vec<String>,
    pub contradictory_endpoint_order: Vec<String>,
    pub overall_information_milli: u32,
    pub next_actions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProtocolEvidenceSurfaceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolEvidenceSurfaceError {
    #[error("protocol evidence surface request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol evidence measurement is invalid: {0}")]
    InvalidMeasurement(String),
    #[error("protocol evidence input is invalid: {0}")]
    InvalidInput(String),
    #[error("protocol evidence output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol evidence digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(surface: &ProtocolEvidenceSurface) -> serde_json::Value {
    serde_json::json!({
        "feature_id": surface.feature_id,
        "output_schema": surface.output_schema,
        "objective": surface.objective,
        "protocol_digest": surface.protocol_digest,
        "execution_digest": surface.execution_digest,
        "cells": surface.cells,
        "qualified_endpoint_order": surface.qualified_endpoint_order,
        "negative_endpoint_order": surface.negative_endpoint_order,
        "partial_endpoint_order": surface.partial_endpoint_order,
        "unresolved_endpoint_order": surface.unresolved_endpoint_order,
        "contradictory_endpoint_order": surface.contradictory_endpoint_order,
        "overall_information_milli": surface.overall_information_milli,
        "next_actions": surface.next_actions,
        "negative_evidence": surface.negative_evidence,
        "uncertainty": surface.uncertainty,
        "disposition": surface.disposition,
    })
}

fn median_i32(values: &mut [i32]) -> i32 {
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        ((i64::from(values[middle - 1]) + i64::from(values[middle])) / 2) as i32
    }
}

fn median_u16(values: &mut [u16]) -> u16 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn validate_request(
    request: &ProtocolEvidenceSurfaceRequest,
) -> Result<(), ProtocolEvidenceSurfaceError> {
    if request.objective.trim().is_empty()
        || request.protocol.objective != request.objective
        || request.measurements.is_empty()
        || request.measurements.len() > MAX_MEASUREMENTS
        || request.min_replicates == 0
        || request.min_quality_milli > 1_000
        || request.max_uncertainty_milli > 1_000
        || request.contradiction_threshold_milli == 0
    {
        return Err(ProtocolEvidenceSurfaceError::InvalidRequest(
            "objective must bind the protocol, measurements must be bounded, and quality/replicate/contradiction gates must be positive and finite".into(),
        ));
    }
    Ok(())
}

fn validate_measurement(
    measurement: &ProtocolMeasurement,
    task_map: &BTreeMap<String, &super::simulator::ProtocolTask>,
    seen: &mut BTreeSet<String>,
) -> Result<(), ProtocolEvidenceSurfaceError> {
    if measurement.measurement_id.trim().is_empty()
        || !seen.insert(measurement.measurement_id.clone())
        || measurement.task_id.trim().is_empty()
        || measurement.output_schema.trim().is_empty()
        || measurement.endpoint_id.trim().is_empty()
        || measurement.modality.trim().is_empty()
        || measurement.replicate_index == 0
        || measurement.uncertainty_milli > 1_000
        || measurement.quality_milli > 1_000
    {
        return Err(ProtocolEvidenceSurfaceError::InvalidMeasurement(format!(
            "measurement {} has invalid identity, schema, modality, replicate, quality, or uncertainty bounds",
            measurement.measurement_id
        )));
    }
    let task = task_map.get(&measurement.task_id).ok_or_else(|| {
        ProtocolEvidenceSurfaceError::InvalidMeasurement(format!(
            "measurement {} references unknown task {}",
            measurement.measurement_id, measurement.task_id
        ))
    })?;
    if task.output_schema != measurement.output_schema {
        return Err(ProtocolEvidenceSurfaceError::InvalidMeasurement(format!(
            "measurement {} output schema does not match task {}",
            measurement.measurement_id, measurement.task_id
        )));
    }
    Ok(())
}

fn validate_surface(surface: &ProtocolEvidenceSurface) -> Result<(), ProtocolEvidenceSurfaceError> {
    if surface.feature_id != FEATURE_ID
        || surface.output_schema != OUTPUT_SCHEMA
        || surface.objective.trim().is_empty()
        || surface.protocol_digest.as_str().len() != 64
        || surface.execution_digest.as_str().len() != 64
        || surface.cells.is_empty()
        || !canonical(&surface.qualified_endpoint_order)
        || !canonical(&surface.negative_endpoint_order)
        || !canonical(&surface.partial_endpoint_order)
        || !canonical(&surface.unresolved_endpoint_order)
        || !canonical(&surface.contradictory_endpoint_order)
        || !canonical(&surface.next_actions)
        || !canonical(&surface.negative_evidence)
        || !canonical(&surface.uncertainty)
        || surface
            .cells
            .windows(2)
            .any(|pair| pair[0].endpoint_id >= pair[1].endpoint_id)
        || surface.cells.iter().any(|cell| {
            cell.endpoint_id.trim().is_empty()
                || !canonical(&cell.measurement_order)
                || !canonical(&cell.task_order)
                || !canonical(&cell.modality_order)
                || cell.measurement_count == 0
                || cell.replicate_count == 0
                || cell.quality_milli > 1_000
                || cell.max_uncertainty_milli > 1_000
                || cell.information_milli > 1_000_000
                || cell.next_action.trim().is_empty()
                || !canonical(&cell.negative_evidence)
                || !canonical(&cell.uncertainty)
        })
    {
        return Err(ProtocolEvidenceSurfaceError::InvalidOutput(
            "identity, ordering, cell, quality, information, or limitation invariants are invalid"
                .into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(surface))
        .map_err(|error| ProtocolEvidenceSurfaceError::Digest(error.to_string()))?;
    if expected != surface.digest {
        return Err(ProtocolEvidenceSurfaceError::InvalidOutput(
            "digest is not bound to the evidence surface".into(),
        ));
    }
    Ok(())
}

impl ProtocolEvidenceSurface {
    pub fn validate(&self) -> Result<(), ProtocolEvidenceSurfaceError> {
        validate_surface(self)
    }
}

/// Compile robust endpoint evidence from a validated local protocol execution.
pub fn compile_glioma_protocol_evidence_surface(
    request: &ProtocolEvidenceSurfaceRequest,
) -> Result<ProtocolEvidenceSurface, ProtocolEvidenceSurfaceError> {
    validate_request(request)?;
    request
        .execution
        .validate()
        .map_err(|error| ProtocolEvidenceSurfaceError::InvalidInput(error.to_string()))?;
    let simulation = simulate_glioma_protocol(&request.protocol)
        .map_err(|error| ProtocolEvidenceSurfaceError::InvalidInput(error.to_string()))?;
    if simulation.digest != request.execution.protocol_digest {
        return Err(ProtocolEvidenceSurfaceError::InvalidInput(
            "execution is not bound to the declared protocol simulation".into(),
        ));
    }
    let task_map = request
        .protocol
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let execution_map = request
        .execution
        .task_results
        .iter()
        .map(|result| (result.task_id.clone(), result))
        .collect::<BTreeMap<_, _>>();
    let mut seen_measurements = BTreeSet::new();
    let mut grouped = BTreeMap::<String, Vec<&ProtocolMeasurement>>::new();
    for measurement in &request.measurements {
        validate_measurement(measurement, &task_map, &mut seen_measurements)?;
        grouped
            .entry(measurement.endpoint_id.clone())
            .or_default()
            .push(measurement);
    }
    let mut cells = Vec::new();
    let mut qualified = Vec::new();
    let mut negative = Vec::new();
    let mut partial = Vec::new();
    let mut unresolved = Vec::new();
    let mut contradictory = Vec::new();
    let mut next_actions = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for (endpoint_id, mut measurements) in grouped {
        measurements.sort_by(|left, right| left.measurement_id.cmp(&right.measurement_id));
        let measurement_order = measurements
            .iter()
            .map(|measurement| measurement.measurement_id.clone())
            .collect::<Vec<_>>();
        let mut task_order = measurements
            .iter()
            .map(|measurement| measurement.task_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let modality_order = measurements
            .iter()
            .map(|measurement| measurement.modality.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        task_order.sort();
        let values = measurements
            .iter()
            .map(|measurement| measurement.value_milli)
            .collect::<Vec<_>>();
        let mut median_values = values.clone();
        let robust_value_milli = Some(median_i32(&mut median_values));
        let min_value = values.iter().copied().min().unwrap_or_default();
        let max_value = values.iter().copied().max().unwrap_or_default();
        let spread_milli = i64::from(max_value)
            .saturating_sub(i64::from(min_value))
            .clamp(0, i64::from(u32::MAX)) as u32;
        let mut qualities = measurements
            .iter()
            .map(|measurement| measurement.quality_milli)
            .collect::<Vec<_>>();
        let quality_milli = median_u16(&mut qualities);
        let max_uncertainty_milli = measurements
            .iter()
            .map(|measurement| measurement.uncertainty_milli)
            .max()
            .unwrap_or_default();
        let replicate_count = measurements
            .iter()
            .map(|measurement| measurement.replicate_index)
            .collect::<BTreeSet<_>>()
            .len() as u16;
        let has_positive = values
            .iter()
            .any(|value| i64::from(*value) >= i64::from(request.contradiction_threshold_milli));
        let has_negative = values
            .iter()
            .any(|value| i64::from(*value) <= -i64::from(request.contradiction_threshold_milli));
        let execution_dispositions = task_order
            .iter()
            .filter_map(|task_id| execution_map.get(task_id))
            .map(|result| result.disposition)
            .collect::<Vec<_>>();
        let failed_or_skipped = execution_dispositions.iter().any(|disposition| {
            matches!(
                disposition,
                ProtocolTaskDisposition::Failed | ProtocolTaskDisposition::Skipped
            )
        });
        let has_partial_execution = execution_dispositions
            .iter()
            .any(|disposition| *disposition == ProtocolTaskDisposition::Partial);
        let has_negative_execution = execution_dispositions
            .iter()
            .any(|disposition| *disposition == ProtocolTaskDisposition::Negative);
        let (disposition, next_action) = if has_positive && has_negative {
            contradictory.push(endpoint_id.clone());
            (
                ProtocolEvidenceDisposition::Contradictory,
                format!("replicate endpoint {endpoint_id} across an independent assay or modality"),
            )
        } else if failed_or_skipped || execution_dispositions.is_empty() {
            unresolved.push(endpoint_id.clone());
            (
                ProtocolEvidenceDisposition::Unresolved,
                format!("restore a completed local task result before interpreting endpoint {endpoint_id}"),
            )
        } else if has_partial_execution
            || replicate_count < request.min_replicates
            || quality_milli < request.min_quality_milli
            || max_uncertainty_milli > request.max_uncertainty_milli
        {
            partial.push(endpoint_id.clone());
            (
                ProtocolEvidenceDisposition::Partial,
                format!("acquire more or higher-quality replicates for endpoint {endpoint_id}"),
            )
        } else if robust_value_milli == Some(0) || has_negative_execution {
            negative.push(endpoint_id.clone());
            (
                ProtocolEvidenceDisposition::Negative,
                format!("publish the null or negative endpoint {endpoint_id} and test a competing explanation"),
            )
        } else {
            qualified.push(endpoint_id.clone());
            (
                ProtocolEvidenceDisposition::Qualified,
                format!("handoff endpoint {endpoint_id} to downstream mechanism or interpretation analysis"),
            )
        };
        let replicate_factor = (u32::from(replicate_count).saturating_mul(1_000)
            / u32::from(request.min_replicates.max(1)))
        .min(1_000);
        let quality_factor = u32::from(quality_milli);
        let uncertainty_factor = 1_000u32.saturating_sub(u32::from(max_uncertainty_milli));
        let information_milli = replicate_factor
            .saturating_mul(quality_factor)
            .saturating_mul(uncertainty_factor)
            / 1_000_000;
        let mut cell_negative = Vec::new();
        if matches!(
            disposition,
            ProtocolEvidenceDisposition::Negative | ProtocolEvidenceDisposition::Contradictory
        ) {
            cell_negative.push(format!("{endpoint_id}:negative-or-conflicted-signal"));
        }
        if failed_or_skipped {
            cell_negative.push(format!("{endpoint_id}:execution-failed-or-skipped"));
        }
        cell_negative.sort();
        let mut cell_uncertainty = Vec::new();
        if replicate_count < request.min_replicates {
            cell_uncertainty.push(format!("{endpoint_id}:replicate-count-below-gate"));
        }
        if quality_milli < request.min_quality_milli {
            cell_uncertainty.push(format!("{endpoint_id}:quality-below-gate"));
        }
        if max_uncertainty_milli > request.max_uncertainty_milli {
            cell_uncertainty.push(format!("{endpoint_id}:uncertainty-above-gate"));
        }
        cell_uncertainty.sort();
        negative_evidence.extend(cell_negative.clone());
        uncertainty.extend(cell_uncertainty.clone());
        next_actions.push(next_action.clone());
        cells.push(ProtocolEvidenceCell {
            endpoint_id,
            measurement_order,
            task_order,
            modality_order,
            measurement_count: measurements.len() as u16,
            replicate_count,
            robust_value_milli,
            spread_milli,
            quality_milli,
            max_uncertainty_milli,
            disposition,
            information_milli,
            negative_evidence: cell_negative,
            uncertainty: cell_uncertainty,
            next_action,
        });
    }
    cells.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    qualified.sort();
    negative.sort();
    partial.sort();
    unresolved.sort();
    contradictory.sort();
    next_actions.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let overall_information_milli = cells
        .iter()
        .map(|cell| cell.information_milli)
        .sum::<u32>()
        .min(1_000_000);
    let disposition = if !contradictory.is_empty() {
        ProtocolEvidenceSurfaceDisposition::Contradictory
    } else if !qualified.is_empty() && partial.is_empty() && unresolved.is_empty() {
        ProtocolEvidenceSurfaceDisposition::Qualified
    } else if !qualified.is_empty() || !negative.is_empty() || !partial.is_empty() {
        ProtocolEvidenceSurfaceDisposition::Partial
    } else {
        ProtocolEvidenceSurfaceDisposition::Unresolved
    };
    let mut surface = ProtocolEvidenceSurface {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        protocol_digest: simulation.digest,
        execution_digest: request.execution.digest.clone(),
        cells,
        qualified_endpoint_order: qualified,
        negative_endpoint_order: negative,
        partial_endpoint_order: partial,
        unresolved_endpoint_order: unresolved,
        contradictory_endpoint_order: contradictory,
        overall_information_milli,
        next_actions,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-evidence-surface"),
    };
    surface.digest = ContentHash::of_value(&digest_input(&surface))
        .map_err(|error| ProtocolEvidenceSurfaceError::Digest(error.to_string()))?;
    validate_surface(&surface)?;
    Ok(surface)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::execution::{
        execute_glioma_protocol, DryRunGliomaProtocolExecutor, ProtocolExecutionRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolResourceKind, ProtocolTask,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn protocol() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "compile organoid invasion evidence".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![ProtocolTask {
                task_id: "assay".into(),
                label: "run invasion assay".into(),
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                depends_on: Vec::new(),
                model_system: GliomaModelSystem::Organoid,
                output_schema: "Assay1@1".into(),
                risk_milli: 100,
                requires_instrument: false,
            }],
            resources: vec![ProtocolResource {
                resource_id: "imaging".into(),
                kind: ProtocolResourceKind::Imaging,
                capacity_units: 1,
            }],
            max_ticks: 10,
            max_risk_milli: 500,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: ContentHash::of_bytes(b"evidence-surface"),
        }
    }

    fn execution() -> ProtocolExecution {
        let mut executor = DryRunGliomaProtocolExecutor;
        execute_glioma_protocol(
            &ProtocolExecutionRequest {
                protocol: protocol(),
                max_retries: 0,
                require_artifacts: true,
            },
            &mut executor,
        )
        .unwrap()
    }

    fn measurement(id: &str, value: i32, replicate_index: u16) -> ProtocolMeasurement {
        ProtocolMeasurement {
            measurement_id: id.into(),
            task_id: "assay".into(),
            output_schema: "Assay1@1".into(),
            endpoint_id: "invasion".into(),
            modality: "imaging".into(),
            value_milli: value,
            uncertainty_milli: 50,
            quality_milli: 900,
            replicate_index,
        }
    }

    fn request(measurements: Vec<ProtocolMeasurement>) -> ProtocolEvidenceSurfaceRequest {
        ProtocolEvidenceSurfaceRequest {
            objective: "compile organoid invasion evidence".into(),
            protocol: protocol(),
            execution: execution(),
            measurements,
            min_replicates: 2,
            min_quality_milli: 700,
            max_uncertainty_milli: 200,
            contradiction_threshold_milli: 100,
        }
    }

    #[test]
    fn evidence_surface_qualifies_robust_replicates() {
        let surface = compile_glioma_protocol_evidence_surface(&request(vec![
            measurement("m1", 400, 1),
            measurement("m2", 420, 2),
            measurement("m3", 410, 3),
        ]))
        .unwrap();
        assert_eq!(
            surface.disposition,
            ProtocolEvidenceSurfaceDisposition::Qualified
        );
        assert_eq!(surface.qualified_endpoint_order, vec!["invasion"]);
        assert_eq!(surface.cells[0].robust_value_milli, Some(410));
        surface.validate().unwrap();
    }

    #[test]
    fn evidence_surface_preserves_contradiction_and_negative_evidence() {
        let surface = compile_glioma_protocol_evidence_surface(&request(vec![
            measurement("m1", 400, 1),
            measurement("m2", -450, 2),
        ]))
        .unwrap();
        assert_eq!(
            surface.disposition,
            ProtocolEvidenceSurfaceDisposition::Contradictory
        );
        assert_eq!(surface.contradictory_endpoint_order, vec!["invasion"]);
        assert!(!surface.negative_evidence.is_empty());
    }

    #[test]
    fn evidence_surface_keeps_low_replicate_signal_partial() {
        let surface =
            compile_glioma_protocol_evidence_surface(&request(vec![measurement("m1", 400, 1)]))
                .unwrap();
        assert_eq!(
            surface.disposition,
            ProtocolEvidenceSurfaceDisposition::Partial
        );
        assert_eq!(surface.partial_endpoint_order, vec!["invasion"]);
        assert!(surface
            .uncertainty
            .iter()
            .any(|item| item.contains("replicate")));
    }
}
