//! Spatial state propagation for preclinical glioma research.
//!
//! This feature provides a bounded integer diffusion model over de-identified spatial cells.  It
//! is useful for asking which local neighborhoods could transmit a declared state signal (for
//! example, an invasion-associated phenotype) and where an additional spatial measurement would
//! be most informative.  It is a simulation and sampling-prioritisation artifact: it does not
//! infer biology from geometry, move payloads, or make a clinical decision.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaSpatialStatePropagation1@1";
pub const MAX_CELLS: usize = 65_536;
pub const MAX_STEPS: u16 = 512;
pub const MAX_COORDINATE_MILLI: i64 = 1_000_000_000;
pub const MAX_STATE_MILLI: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialPropagationRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub radius_milli: u64,
    pub max_steps: u16,
    pub self_retention_milli: u16,
    pub neighbor_weight_milli: u16,
    /// Cross-lineage neighbors are down-weighted by this factor; 1,000 means no penalty.
    pub cross_lineage_weight_milli: u16,
    pub convergence_tolerance_milli: u64,
    pub hotspot_threshold_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialPropagationEdge {
    pub edge_id: String,
    pub source_cell_id: String,
    pub target_cell_id: String,
    pub distance_squared_milli: u64,
    pub raw_weight_milli: u64,
    pub cross_lineage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialPropagationTrajectory {
    pub cell_id: String,
    pub sample_id: String,
    pub lineage: String,
    pub neighbor_order: Vec<String>,
    /// Includes the supplied state at index zero and each simulated state thereafter.
    pub state_order: Vec<i64>,
    pub total_delta_milli: i64,
    pub max_step_delta_milli: u64,
    pub converged_step: Option<u16>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialPropagationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialPropagationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub cell_order: Vec<String>,
    pub edge_order: Vec<String>,
    pub edges: Vec<SpatialPropagationEdge>,
    pub trajectories: Vec<SpatialPropagationTrajectory>,
    pub hotspot_order: Vec<String>,
    pub simulated_steps: u16,
    pub converged: bool,
    pub max_delta_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SpatialPropagationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpatialPropagationError {
    #[error("spatial propagation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("spatial propagation cell is invalid: {0}")]
    InvalidCell(String),
    #[error("spatial propagation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("spatial propagation digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &SpatialPropagationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "cell_order": output.cell_order,
        "edge_order": output.edge_order,
        "edges": output.edges,
        "trajectories": output.trajectories,
        "hotspot_order": output.hotspot_order,
        "simulated_steps": output.simulated_steps,
        "converged": output.converged,
        "max_delta_milli": output.max_delta_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn squared_distance(left_x: i64, left_y: i64, right_x: i64, right_y: i64) -> Option<u64> {
    let dx = i128::from(left_x) - i128::from(right_x);
    let dy = i128::from(left_y) - i128::from(right_y);
    let distance = dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)?;
    u64::try_from(distance).ok()
}

impl SpatialPropagationAnalysis {
    pub fn validate(&self) -> Result<(), SpatialPropagationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || !canonical(&self.cell_order)
            || !canonical(&self.edge_order)
            || self.edge_order.len() != self.edges.len()
            || self.edges.iter().any(|edge| {
                edge.edge_id.trim().is_empty()
                    || edge.source_cell_id >= edge.target_cell_id
                    || edge.distance_squared_milli == 0
                    || edge.raw_weight_milli == 0
            })
            || self
                .edges
                .windows(2)
                .any(|pair| pair[0].edge_id >= pair[1].edge_id)
            || self
                .trajectories
                .windows(2)
                .any(|pair| pair[0].cell_id >= pair[1].cell_id)
            || self.trajectories.iter().any(|trajectory| {
                trajectory.cell_id.trim().is_empty()
                    || trajectory.sample_id.trim().is_empty()
                    || trajectory.lineage.trim().is_empty()
                    || trajectory.state_order.is_empty()
                    || trajectory.state_order.len() != usize::from(self.simulated_steps) + 1
                    || !canonical(&trajectory.neighbor_order)
                    || !canonical(&trajectory.uncertainty)
            })
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.simulated_steps > MAX_STEPS
        {
            return Err(SpatialPropagationError::InvalidOutput(
                "identity, ordering, edge geometry, trajectory, or bound invariants are invalid"
                    .into(),
            ));
        }
        let cells = self.cell_order.iter().cloned().collect::<BTreeSet<_>>();
        let trajectory_cells = self
            .trajectories
            .iter()
            .map(|trajectory| trajectory.cell_id.clone())
            .collect::<BTreeSet<_>>();
        let edge_ids = self
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone())
            .collect::<BTreeSet<_>>();
        if cells.is_empty()
            || cells != trajectory_cells
            || edge_ids != self.edge_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.hotspot_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.edges.iter().any(|edge| {
                !cells.contains(&edge.source_cell_id) || !cells.contains(&edge.target_cell_id)
            })
            || self
                .hotspot_order
                .iter()
                .any(|cell_id| !cells.contains(cell_id))
        {
            return Err(SpatialPropagationError::InvalidOutput(
                "cell, edge, and hotspot partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SpatialPropagationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SpatialPropagationError::InvalidOutput(
                "spatial propagation digest is not bound to the simulation".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &SpatialPropagationRequest,
    cells: &[crate::glioma::programs::p03_multimodal_ingestion_qc::SpatialCell],
) -> Result<(), SpatialPropagationError> {
    if request.study_id.trim().is_empty()
        || request.radius_milli == 0
        || request.max_steps == 0
        || request.max_steps > MAX_STEPS
        || request.self_retention_milli > 1_000
        || request.neighbor_weight_milli > 1_000
        || u32::from(request.self_retention_milli)
            .saturating_add(u32::from(request.neighbor_weight_milli))
            != 1_000
        || request.cross_lineage_weight_milli > 1_000
        || cells.is_empty()
        || cells.len() > MAX_CELLS
    {
        return Err(SpatialPropagationError::InvalidRequest(
            "study, radius, bounded steps, retention/neighbor weights summing to 1000, and non-empty cells are required".into(),
        ));
    }
    let mut cell_ids = BTreeSet::new();
    for cell in cells {
        cell.artifact
            .validate()
            .map_err(|error| SpatialPropagationError::InvalidCell(error.to_string()))?;
        if cell.cell_id.trim().is_empty()
            || !cell_ids.insert(cell.cell_id.clone())
            || cell.sample_id.trim().is_empty()
            || cell.lineage.trim().is_empty()
            || cell.x_milli.unsigned_abs() > MAX_COORDINATE_MILLI as u64
            || cell.y_milli.unsigned_abs() > MAX_COORDINATE_MILLI as u64
            || cell.state_milli.unsigned_abs() > MAX_STATE_MILLI as u64
            || cell.artifact.contains_human_data
            || cell.artifact.contains_direct_identifiers
        {
            return Err(SpatialPropagationError::InvalidCell(
                "cell identity, sample/lineage, coordinate/state bounds, or privacy boundary is invalid".into(),
            ));
        }
    }
    Ok(())
}

/// Run a bounded integer diffusion over local spatial cells.
pub fn analyze_glioma_spatial_state_propagation(
    request: &SpatialPropagationRequest,
    cells: &[crate::glioma::programs::p03_multimodal_ingestion_qc::SpatialCell],
) -> Result<SpatialPropagationAnalysis, SpatialPropagationError> {
    validate_request(request, cells)?;
    let mut ordered_cells = cells.to_vec();
    ordered_cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let cell_order = ordered_cells
        .iter()
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    let radius_squared = u128::from(request.radius_milli)
        .checked_mul(u128::from(request.radius_milli))
        .ok_or_else(|| SpatialPropagationError::InvalidRequest("radius overflow".into()))?;
    let mut edges = Vec::new();
    let mut adjacency = BTreeMap::<String, Vec<(String, u64, bool)>>::new();
    for (index, left) in ordered_cells.iter().enumerate() {
        for right in ordered_cells.iter().skip(index + 1) {
            if left.sample_id != right.sample_id {
                continue;
            }
            let Some(distance_squared) =
                squared_distance(left.x_milli, left.y_milli, right.x_milli, right.y_milli)
            else {
                continue;
            };
            if distance_squared == 0 || u128::from(distance_squared) > radius_squared {
                continue;
            }
            let raw_weight = (radius_squared
                .saturating_mul(1_000)
                .checked_div(u128::from(distance_squared))
                .unwrap_or(1)
                .min(u128::from(u64::MAX))) as u64;
            let cross_lineage = left.lineage != right.lineage;
            let edge_id = format!("{}>{}", left.cell_id, right.cell_id);
            edges.push(SpatialPropagationEdge {
                edge_id,
                source_cell_id: left.cell_id.clone(),
                target_cell_id: right.cell_id.clone(),
                distance_squared_milli: distance_squared,
                raw_weight_milli: raw_weight.max(1),
                cross_lineage,
            });
            adjacency.entry(left.cell_id.clone()).or_default().push((
                right.cell_id.clone(),
                raw_weight.max(1),
                cross_lineage,
            ));
            adjacency.entry(right.cell_id.clone()).or_default().push((
                left.cell_id.clone(),
                raw_weight.max(1),
                cross_lineage,
            ));
        }
    }
    edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    let edge_order = edges
        .iter()
        .map(|edge| edge.edge_id.clone())
        .collect::<Vec<_>>();
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|left, right| left.0.cmp(&right.0));
    }

    let mut states = ordered_cells
        .iter()
        .map(|cell| (cell.cell_id.clone(), cell.state_milli))
        .collect::<BTreeMap<_, _>>();
    let mut state_orders = ordered_cells
        .iter()
        .map(|cell| (cell.cell_id.clone(), vec![cell.state_milli]))
        .collect::<BTreeMap<_, _>>();
    let mut max_delta_milli = 0_u64;
    let mut converged_step = None;
    let mut simulated_steps = 0_u16;
    let mut globally_converged = false;
    for step in 1..=request.max_steps {
        let mut next = BTreeMap::new();
        let mut step_delta = 0_u64;
        for cell in &ordered_cells {
            let neighbors = adjacency.get(&cell.cell_id).cloned().unwrap_or_default();
            let mut weighted_sum = 0_i128;
            let mut total_weight = 0_u128;
            for (neighbor_id, raw_weight, cross_lineage) in neighbors {
                let factor = if cross_lineage {
                    u128::from(request.cross_lineage_weight_milli)
                } else {
                    1_000
                };
                let effective_weight = u128::from(raw_weight).saturating_mul(factor);
                let neighbor_state = states.get(&neighbor_id).copied().unwrap_or(0);
                weighted_sum = weighted_sum.saturating_add(
                    i128::from(neighbor_state)
                        .saturating_mul(i128::try_from(effective_weight).unwrap_or(i128::MAX)),
                );
                total_weight = total_weight.saturating_add(effective_weight);
            }
            let current = states
                .get(&cell.cell_id)
                .copied()
                .unwrap_or(cell.state_milli);
            let neighbor_average = if total_weight == 0 {
                current
            } else {
                (weighted_sum / i128::try_from(total_weight).unwrap_or(i128::MAX))
                    .clamp(i128::from(-MAX_STATE_MILLI), i128::from(MAX_STATE_MILLI))
                    as i64
            };
            let updated = (i128::from(current)
                .saturating_mul(i128::from(request.self_retention_milli))
                .saturating_add(
                    i128::from(neighbor_average)
                        .saturating_mul(i128::from(request.neighbor_weight_milli)),
                )
                / 1_000)
                .clamp(i128::from(-MAX_STATE_MILLI), i128::from(MAX_STATE_MILLI))
                as i64;
            step_delta = step_delta.max(current.saturating_sub(updated).unsigned_abs());
            next.insert(cell.cell_id.clone(), updated);
        }
        states = next;
        simulated_steps = step;
        max_delta_milli = max_delta_milli.max(step_delta);
        for cell_id in &cell_order {
            state_orders
                .get_mut(cell_id)
                .expect("state order exists")
                .push(states[cell_id]);
        }
        if step_delta <= request.convergence_tolerance_milli {
            converged_step = Some(step);
            globally_converged = true;
            break;
        }
    }

    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if edges.is_empty() {
        negative.insert("no-same-sample-neighborhood-within-radius".into());
    }
    let isolated = ordered_cells
        .iter()
        .filter(|cell| {
            adjacency
                .get(&cell.cell_id)
                .is_none_or(|neighbors| neighbors.is_empty())
        })
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    if !isolated.is_empty() {
        uncertainty.insert("one-or-more-cells-have-no-spatial-neighbors".into());
    }
    if !globally_converged {
        uncertainty.insert("bounded-diffusion-did-not-converge-before-step-limit".into());
    }
    let mut trajectories = ordered_cells
        .iter()
        .map(|cell| {
            let states = state_orders
                .remove(&cell.cell_id)
                .expect("state order exists");
            let total_delta = states
                .last()
                .copied()
                .unwrap_or(cell.state_milli)
                .saturating_sub(cell.state_milli);
            let max_step_delta = states
                .windows(2)
                .map(|pair| pair[0].saturating_sub(pair[1]).unsigned_abs())
                .max()
                .unwrap_or(0);
            let mut local_uncertainty = BTreeSet::new();
            if adjacency
                .get(&cell.cell_id)
                .is_none_or(|neighbors| neighbors.is_empty())
            {
                local_uncertainty.insert("no-neighbor-update".into());
            }
            if !globally_converged {
                local_uncertainty.insert("step-limit-before-convergence".into());
            }
            SpatialPropagationTrajectory {
                cell_id: cell.cell_id.clone(),
                sample_id: cell.sample_id.clone(),
                lineage: cell.lineage.clone(),
                neighbor_order: adjacency
                    .get(&cell.cell_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, _, _)| id)
                    .collect(),
                state_order: states,
                total_delta_milli: total_delta,
                max_step_delta_milli: max_step_delta,
                converged_step,
                uncertainty: local_uncertainty.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    let mut hotspots = trajectories
        .iter()
        .filter(|trajectory| {
            trajectory.total_delta_milli.unsigned_abs() >= request.hotspot_threshold_milli
        })
        .map(|trajectory| trajectory.cell_id.clone())
        .collect::<Vec<_>>();
    hotspots.sort_by(|left, right| {
        let left_delta = trajectories
            .iter()
            .find(|trajectory| &trajectory.cell_id == left)
            .map(|trajectory| trajectory.total_delta_milli.unsigned_abs())
            .unwrap_or(0);
        let right_delta = trajectories
            .iter()
            .find(|trajectory| &trajectory.cell_id == right)
            .map(|trajectory| trajectory.total_delta_milli.unsigned_abs())
            .unwrap_or(0);
        right_delta.cmp(&left_delta).then_with(|| left.cmp(right))
    });
    let disposition = if edges.is_empty() {
        SpatialPropagationDisposition::Unresolved
    } else if !globally_converged || !isolated.is_empty() {
        SpatialPropagationDisposition::Partial
    } else {
        SpatialPropagationDisposition::Qualified
    };
    trajectories.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let mut output = SpatialPropagationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        cell_order,
        edge_order,
        edges,
        trajectories,
        hotspot_order: hotspots,
        simulated_steps,
        converged: globally_converged,
        max_delta_milli,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-spatial-state-propagation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SpatialPropagationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::SpatialCell;
    use crate::glioma_engine::LocalArtifactRef;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-spatial+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn cell(id: &str, x: i64, lineage: &str, state: i64) -> SpatialCell {
        SpatialCell {
            cell_id: id.into(),
            sample_id: "sample-1".into(),
            lineage: lineage.into(),
            x_milli: x,
            y_milli: 0,
            state_milli: state,
            artifact: artifact(id),
        }
    }

    fn request() -> SpatialPropagationRequest {
        SpatialPropagationRequest {
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            radius_milli: 2_000,
            max_steps: 20,
            self_retention_milli: 700,
            neighbor_weight_milli: 300,
            cross_lineage_weight_milli: 500,
            convergence_tolerance_milli: 1,
            hotspot_threshold_milli: 50,
        }
    }

    #[test]
    fn diffusion_is_replay_stable_and_exposes_hotspot() {
        let cells = vec![cell("a", 0, "tumor", 900), cell("b", 1_000, "tumor", 0)];
        let first = analyze_glioma_spatial_state_propagation(&request(), &cells).unwrap();
        let second = analyze_glioma_spatial_state_propagation(&request(), &cells).unwrap();
        assert_eq!(first, second);
        assert!(!first.edge_order.is_empty());
        assert_eq!(first.hotspot_order.first().map(String::as_str), Some("a"));
        assert_eq!(first.hotspot_order.len(), 2);
        first.validate().unwrap();
    }

    #[test]
    fn different_samples_do_not_diffuse_into_each_other() {
        let first = cell("a", 0, "tumor", 900);
        let mut second = cell("b", 1_000, "tumor", 0);
        second.sample_id = "sample-2".into();
        let output =
            analyze_glioma_spatial_state_propagation(&request(), &[first, second]).unwrap();
        assert!(output.edge_order.is_empty());
        assert_eq!(
            output.disposition,
            SpatialPropagationDisposition::Unresolved
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item == "no-same-sample-neighborhood-within-radius"));
    }

    #[test]
    fn privacy_and_weight_contracts_are_enforced() {
        let mut cells = vec![cell("a", 0, "tumor", 900), cell("b", 1_000, "tumor", 0)];
        cells[0].artifact.contains_human_data = true;
        assert!(analyze_glioma_spatial_state_propagation(&request(), &cells).is_err());
        let mut invalid = request();
        invalid.self_retention_milli = 500;
        assert!(analyze_glioma_spatial_state_propagation(&invalid, &cells).is_err());
    }
}
