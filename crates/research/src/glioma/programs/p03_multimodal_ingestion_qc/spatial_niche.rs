//! Spatial niche graph analysis for preclinical glioma research.
//!
//! This feature turns de-identified spatial observations into a deterministic cell-neighbour
//! graph.  Same-lineage connected components become candidate niches; cross-lineage edges are
//! retained as interaction evidence, with an expected-edge null model for enrichment.  The
//! output is deliberately descriptive rather than clinical: sparse neighbourhoods, undersized
//! components, and missing spatial support remain explicit unresolved or partial states.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaSpatialNiche1@1";
pub const MAX_CELLS: usize = 65_536;
pub const MAX_NICHES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialNicheRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub radius_milli: u64,
    pub min_neighbors: usize,
    pub min_cells_per_niche: usize,
    pub min_interaction_enrichment_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialCell {
    pub cell_id: String,
    pub sample_id: String,
    pub lineage: String,
    pub x_milli: i64,
    pub y_milli: i64,
    pub state_milli: i64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialNicheDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialNiche {
    pub niche_id: String,
    pub sample_id: String,
    pub lineage: String,
    pub cell_order: Vec<String>,
    pub mean_state_milli: i64,
    pub variance_milli_squared: u64,
    pub mean_neighbor_distance_squared_milli: u64,
    pub mean_local_enrichment_milli: i64,
    pub internal_edge_count: u32,
    pub boundary_fraction_milli: u16,
    pub disposition: SpatialNicheDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialNicheInteraction {
    pub source_niche_id: String,
    pub target_niche_id: String,
    pub observed_edge_count: u32,
    /// Expected cross-niche edges under random mixing, multiplied by 1,000.
    pub expected_edge_milli: u64,
    /// Observed/expected enrichment, multiplied by 1,000.
    pub enrichment_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialNicheAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub cell_order: Vec<String>,
    pub niche_order: Vec<String>,
    pub niches: Vec<SpatialNiche>,
    pub interactions: Vec<SpatialNicheInteraction>,
    pub isolated_cell_order: Vec<String>,
    pub boundary_cell_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SpatialNicheDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpatialNicheError {
    #[error("spatial niche request is invalid: {0}")]
    InvalidRequest(String),
    #[error("spatial cell is invalid: {0}")]
    InvalidCell(String),
    #[error("spatial niche output is invalid: {0}")]
    InvalidOutput(String),
    #[error("spatial niche digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &SpatialNicheAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "cell_order": output.cell_order,
        "niche_order": output.niche_order,
        "niches": output.niches,
        "interactions": output.interactions,
        "isolated_cell_order": output.isolated_cell_order,
        "boundary_cell_order": output.boundary_cell_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn mean_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    (values.iter().map(|value| *value as i128).sum::<i128>() / values.len() as i128)
        .clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn squared_distance(left: &SpatialCell, right: &SpatialCell) -> Option<u64> {
    let dx = i128::from(left.x_milli) - i128::from(right.x_milli);
    let dy = i128::from(left.y_milli) - i128::from(right.y_milli);
    let distance = dx.checked_mul(dx)?.checked_add(dy.checked_mul(dy)?)?;
    u64::try_from(distance).ok()
}

fn variance(values: &[i64], mean: i64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let total = values
        .iter()
        .map(|value| {
            let delta = i128::from(*value) - i128::from(mean);
            delta
                .checked_mul(delta)
                .and_then(|squared| u128::try_from(squared).ok())
                .unwrap_or(u128::from(u64::MAX))
        })
        .sum::<u128>();
    (total / values.len() as u128).min(u128::from(u64::MAX)) as u64
}

fn find(parent: &mut [usize], value: usize) -> usize {
    if parent[value] != value {
        let root = find(parent, parent[value]);
        parent[value] = root;
    }
    parent[value]
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        // Choosing the smaller root is deterministic and avoids a second rank structure.
        if left_root < right_root {
            parent[right_root] = left_root;
        } else {
            parent[left_root] = right_root;
        }
    }
}

impl SpatialNicheAnalysis {
    pub fn validate(&self) -> Result<(), SpatialNicheError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.cell_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.niche_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .niches
                .windows(2)
                .any(|pair| pair[0].niche_id >= pair[1].niche_id)
            || self.interactions.windows(2).any(|pair| {
                (&pair[0].source_niche_id, &pair[0].target_niche_id)
                    >= (&pair[1].source_niche_id, &pair[1].target_niche_id)
            })
            || self
                .isolated_cell_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .boundary_cell_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self.niches.iter().any(|niche| {
                niche.cell_order.windows(2).any(|pair| pair[0] >= pair[1])
                    || niche.boundary_fraction_milli > 1_000
                    || niche.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            })
            || self.interactions.iter().any(|interaction| {
                interaction.source_niche_id >= interaction.target_niche_id
                    || interaction.observed_edge_count == 0
                    || interaction.expected_edge_milli == 0
            })
        {
            return Err(SpatialNicheError::InvalidOutput(
                "identity, graph ordering, bounds, or interaction contract is invalid".into(),
            ));
        }
        let niche_ids = self
            .niches
            .iter()
            .map(|niche| niche.niche_id.clone())
            .collect::<BTreeSet<_>>();
        let cell_ids = self.cell_order.iter().cloned().collect::<BTreeSet<_>>();
        let partition_ids = self
            .niches
            .iter()
            .flat_map(|niche| niche.cell_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if niche_ids != self.niche_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.niche_order.len() != niche_ids.len()
            || partition_ids != cell_ids
            || partition_ids.len() != self.cell_order.len()
        {
            return Err(SpatialNicheError::InvalidOutput(
                "niche and cell partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SpatialNicheError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SpatialNicheError::InvalidOutput(
                "spatial niche digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Build a spatial graph, connected same-lineage niches, and cross-niche enrichment statistics.
pub fn analyze_glioma_spatial_niches(
    request: &SpatialNicheRequest,
    cells: &[SpatialCell],
) -> Result<SpatialNicheAnalysis, SpatialNicheError> {
    if request.study_id.trim().is_empty()
        || request.radius_milli == 0
        || request.radius_milli > 2_000_000_000
        || request.min_neighbors == 0
        || request.min_cells_per_niche == 0
        || request.min_interaction_enrichment_milli < 0
        || cells.is_empty()
        || cells.len() > MAX_CELLS
    {
        return Err(SpatialNicheError::InvalidRequest(
            "study, positive radius/neighbour/niche floors, non-negative enrichment floor, and bounded cells are required".into(),
        ));
    }
    let mut ordered = cells.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let mut ids = BTreeSet::new();
    for cell in &ordered {
        cell.artifact
            .validate()
            .map_err(|error| SpatialNicheError::InvalidCell(error.to_string()))?;
        if cell.cell_id.trim().is_empty()
            || cell.sample_id.trim().is_empty()
            || cell.lineage.trim().is_empty()
            || cell.x_milli.abs() > 1_000_000_000
            || cell.y_milli.abs() > 1_000_000_000
            || cell.state_milli.abs() > 1_000_000_000_000
            || !ids.insert(cell.cell_id.clone())
        {
            return Err(SpatialNicheError::InvalidCell(
                "cell identity, sample/lineage, coordinate, state, or uniqueness is invalid".into(),
            ));
        }
    }
    let radius_squared = request
        .radius_milli
        .checked_mul(request.radius_milli)
        .ok_or_else(|| SpatialNicheError::InvalidRequest("radius squared overflows".into()))?;
    let mut parent = (0..ordered.len()).collect::<Vec<_>>();
    let mut neighbours = vec![Vec::<(usize, u64)>::new(); ordered.len()];
    let mut sample_edge_counts = BTreeMap::<String, u64>::new();
    for left in 0..ordered.len() {
        for right in (left + 1)..ordered.len() {
            if ordered[left].sample_id != ordered[right].sample_id {
                continue;
            }
            let distance = squared_distance(ordered[left], ordered[right]).ok_or_else(|| {
                SpatialNicheError::InvalidCell("coordinate distance overflows".into())
            })?;
            if distance > radius_squared {
                continue;
            }
            neighbours[left].push((right, distance));
            neighbours[right].push((left, distance));
            *sample_edge_counts
                .entry(ordered[left].sample_id.clone())
                .or_default() += 1;
            if ordered[left].lineage == ordered[right].lineage {
                union(&mut parent, left, right);
            }
        }
    }
    let mut components = BTreeMap::<usize, Vec<usize>>::new();
    for index in 0..ordered.len() {
        let root = find(&mut parent, index);
        components.entry(root).or_default().push(index);
    }
    let mut component_order = components.into_values().collect::<Vec<_>>();
    component_order.sort_by_key(|component| ordered[component[0]].cell_id.clone());
    if component_order.len() > MAX_NICHES {
        return Err(SpatialNicheError::InvalidCell(
            "candidate niche count exceeds bounded capacity".into(),
        ));
    }
    let sample_sizes = ordered
        .iter()
        .fold(BTreeMap::<String, usize>::new(), |mut sizes, cell| {
            *sizes.entry(cell.sample_id.clone()).or_default() += 1;
            sizes
        });
    let sample_means =
        ordered
            .iter()
            .fold(BTreeMap::<String, Vec<i64>>::new(), |mut values, cell| {
                values
                    .entry(cell.sample_id.clone())
                    .or_default()
                    .push(cell.state_milli);
                values
            });
    let sample_means = sample_means
        .into_iter()
        .map(|(sample, values)| (sample, mean_i64(&values)))
        .collect::<BTreeMap<_, _>>();
    let mut cell_niche = vec![String::new(); ordered.len()];
    let mut niches = Vec::with_capacity(component_order.len());
    let mut boundary_cells = BTreeSet::new();
    let mut isolated_cells = BTreeSet::new();
    for component in component_order {
        let first = ordered[component[0]];
        let niche_id = format!(
            "niche:{}:{}:{}",
            first.sample_id, first.lineage, first.cell_id
        );
        let cell_order = component
            .iter()
            .map(|index| ordered[*index].cell_id.clone())
            .collect::<Vec<_>>();
        let values = component
            .iter()
            .map(|index| ordered[*index].state_milli)
            .collect::<Vec<_>>();
        let mut local_enrichments = Vec::new();
        let mut distances = Vec::new();
        let mut internal_edges = 0_u32;
        let mut boundary_count = 0_u32;
        let component_set = component.iter().copied().collect::<BTreeSet<_>>();
        for index in &component {
            if neighbours[*index].len() < request.min_neighbors {
                isolated_cells.insert(ordered[*index].cell_id.clone());
            }
            let mut neighbour_values = Vec::new();
            let mut has_other_lineage = false;
            for (other, distance) in &neighbours[*index] {
                distances.push(*distance);
                neighbour_values.push(ordered[*other].state_milli);
                if component_set.contains(other) {
                    internal_edges = internal_edges.saturating_add(1);
                }
                if ordered[*other].lineage != first.lineage {
                    has_other_lineage = true;
                }
            }
            if has_other_lineage {
                boundary_count += 1;
                boundary_cells.insert(ordered[*index].cell_id.clone());
            }
            if !neighbour_values.is_empty() {
                local_enrichments
                    .push(mean_i64(&neighbour_values) - sample_means[&first.sample_id]);
            }
            cell_niche[*index] = niche_id.clone();
        }
        internal_edges /= 2;
        let mut uncertainty = BTreeSet::new();
        if component.len() < request.min_cells_per_niche {
            uncertainty.insert("niche-cell-floor-not-met".into());
        }
        if component
            .iter()
            .any(|index| neighbours[*index].len() < request.min_neighbors)
        {
            uncertainty.insert("niche-neighbour-floor-not-met".into());
        }
        if distances.is_empty() {
            uncertainty.insert("niche-has-no-spatial-edges".into());
        }
        let disposition = if component.len() >= request.min_cells_per_niche
            && !distances.is_empty()
            && component
                .iter()
                .all(|index| neighbours[*index].len() >= request.min_neighbors)
        {
            SpatialNicheDisposition::Qualified
        } else {
            SpatialNicheDisposition::Unresolved
        };
        niches.push(SpatialNiche {
            niche_id,
            sample_id: first.sample_id.clone(),
            lineage: first.lineage.clone(),
            cell_order,
            mean_state_milli: mean_i64(&values),
            variance_milli_squared: variance(&values, mean_i64(&values)),
            mean_neighbor_distance_squared_milli: if distances.is_empty() {
                0
            } else {
                distances.iter().sum::<u64>() / distances.len() as u64
            },
            mean_local_enrichment_milli: mean_i64(&local_enrichments),
            internal_edge_count: internal_edges,
            boundary_fraction_milli: ((u64::from(boundary_count) * 1_000) / component.len() as u64)
                as u16,
            disposition,
            uncertainty: uncertainty.into_iter().collect(),
        });
    }
    niches.sort_by(|left, right| left.niche_id.cmp(&right.niche_id));
    let niche_order = niches
        .iter()
        .map(|niche| niche.niche_id.clone())
        .collect::<Vec<_>>();
    let mut interaction_edges = BTreeMap::<(String, String), u32>::new();
    for left in 0..ordered.len() {
        for (right, _) in &neighbours[left] {
            if left >= *right || cell_niche[left] == cell_niche[*right] {
                continue;
            }
            let pair = if cell_niche[left] < cell_niche[*right] {
                (cell_niche[left].clone(), cell_niche[*right].clone())
            } else {
                (cell_niche[*right].clone(), cell_niche[left].clone())
            };
            *interaction_edges.entry(pair).or_default() += 1;
        }
    }
    let niche_sizes = niches
        .iter()
        .map(|niche| (niche.niche_id.clone(), niche.cell_order.len() as u64))
        .collect::<BTreeMap<_, _>>();
    let mut interactions = Vec::new();
    for ((source, target), observed) in interaction_edges {
        let source_niche = niches
            .iter()
            .find(|niche| niche.niche_id == source)
            .unwrap();
        let target_niche = niches
            .iter()
            .find(|niche| niche.niche_id == target)
            .unwrap();
        if source_niche.sample_id != target_niche.sample_id {
            continue;
        }
        let total = sample_sizes[&source_niche.sample_id] as u128;
        let denominator = total.saturating_mul(total.saturating_sub(1));
        let expected_edge_milli = if denominator == 0 {
            0
        } else {
            (u128::from(sample_edge_counts[&source_niche.sample_id])
                .saturating_mul(2)
                .saturating_mul(u128::from(niche_sizes[&source]))
                .saturating_mul(u128::from(niche_sizes[&target]))
                .saturating_mul(1_000)
                .checked_div(denominator)
                .unwrap_or(0))
            .min(u128::from(u64::MAX)) as u64
        };
        if expected_edge_milli == 0 {
            continue;
        }
        let enrichment_milli = (u128::from(observed)
            .saturating_mul(1_000_000)
            .checked_div(u128::from(expected_edge_milli))
            .unwrap_or(0)
            .min(i128::from(i64::MAX) as u128)) as i64;
        if enrichment_milli >= request.min_interaction_enrichment_milli {
            interactions.push(SpatialNicheInteraction {
                source_niche_id: source,
                target_niche_id: target,
                observed_edge_count: observed,
                expected_edge_milli,
                enrichment_milli,
            });
        }
    }
    interactions.sort_by(|left, right| {
        (&left.source_niche_id, &left.target_niche_id)
            .cmp(&(&right.source_niche_id, &right.target_niche_id))
    });
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if isolated_cells.len() == ordered.len() {
        negative.insert("no-cell-meets-neighbour-floor".into());
    } else if !isolated_cells.is_empty() {
        uncertainty.insert("isolated-cells-retained-as-unresolved".into());
    }
    if interactions.is_empty() {
        negative.insert("no-cross-niche-interaction-meets-enrichment-floor".into());
    }
    if niches.len() < 2 {
        uncertainty.insert("fewer-than-two-spatial-niches".into());
    }
    let unresolved_niches = niches
        .iter()
        .filter(|niche| niche.disposition == SpatialNicheDisposition::Unresolved)
        .count();
    let disposition = if niches.is_empty() || isolated_cells.len() == ordered.len() {
        SpatialNicheDisposition::Unresolved
    } else if unresolved_niches > 0 || !isolated_cells.is_empty() {
        SpatialNicheDisposition::Partial
    } else {
        SpatialNicheDisposition::Qualified
    };
    let mut output = SpatialNicheAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        cell_order: ordered.iter().map(|cell| cell.cell_id.clone()).collect(),
        niche_order,
        niches,
        interactions,
        isolated_cell_order: isolated_cells.into_iter().collect(),
        boundary_cell_order: boundary_cells.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-spatial-niche"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SpatialNicheError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "spatial/cell".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> SpatialNicheRequest {
        SpatialNicheRequest {
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            radius_milli: 1_500,
            min_neighbors: 1,
            min_cells_per_niche: 2,
            min_interaction_enrichment_milli: 0,
        }
    }

    fn cell(id: &str, lineage: &str, x: i64, y: i64, state: i64) -> SpatialCell {
        SpatialCell {
            cell_id: id.into(),
            sample_id: "sample-a".into(),
            lineage: lineage.into(),
            x_milli: x,
            y_milli: y,
            state_milli: state,
            artifact: artifact(id),
        }
    }

    #[test]
    fn graph_builds_niches_and_cross_lineage_interaction() {
        let cells = vec![
            cell("a1", "tumour", 0, 0, 900),
            cell("a2", "tumour", 1_000, 0, 800),
            cell("a3", "tumour", 0, 1_000, 850),
            cell("b1", "myeloid", 2_000, 0, 200),
            cell("b2", "myeloid", 3_000, 0, 250),
            cell("b3", "myeloid", 2_000, 1_000, 150),
        ];
        let output = analyze_glioma_spatial_niches(&request(), &cells).unwrap();
        assert_eq!(output.disposition, SpatialNicheDisposition::Qualified);
        assert_eq!(output.niches.len(), 2);
        assert!(!output.interactions.is_empty());
        assert!(!output.boundary_cell_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn input_order_permutation_is_replay_stable() {
        let cells = vec![
            cell("a1", "tumour", 0, 0, 900),
            cell("a2", "tumour", 1_000, 0, 800),
            cell("b1", "myeloid", 2_000, 0, 200),
            cell("b2", "myeloid", 3_000, 0, 250),
        ];
        let first = analyze_glioma_spatial_niches(&request(), &cells).unwrap();
        let mut reversed = cells;
        reversed.reverse();
        let second = analyze_glioma_spatial_niches(&request(), &reversed).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn sparse_spatial_support_is_explicitly_unresolved() {
        let cells = vec![
            cell("a1", "tumour", 0, 0, 900),
            cell("a2", "tumour", 10_000, 0, 800),
        ];
        let output = analyze_glioma_spatial_niches(&request(), &cells).unwrap();
        assert_eq!(output.disposition, SpatialNicheDisposition::Unresolved);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item == "no-cell-meets-neighbour-floor"));
        assert_eq!(output.isolated_cell_order, vec!["a1", "a2"]);
    }

    #[test]
    fn human_data_artifact_is_refused() {
        let mut cells = vec![
            cell("a1", "tumour", 0, 0, 900),
            cell("a2", "tumour", 1_000, 0, 800),
        ];
        cells[0].artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_spatial_niches(&request(), &cells),
            Err(SpatialNicheError::InvalidCell(_))
        ));
    }
}
