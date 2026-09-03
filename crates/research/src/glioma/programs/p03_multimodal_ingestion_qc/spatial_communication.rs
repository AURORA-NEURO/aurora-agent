//! Spatial ligand–receptor communication inference for preclinical glioma tissue models.
//!
//! The analyzer links a declared ligand/receptor vocabulary to a local spatial neighbourhood
//! graph. It compares observed sender-to-receiver signal against a random-mixing null built from
//! lineage-specific marginal means. This is a bounded association screen for organoids, slices,
//! and animal-model specimens—not a claim of causal signalling, cell identity, or clinical
//! relevance. Missing features, sparse neighbourhoods, and zero expected signal stay explicit.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaSpatialCommunication1@1";
pub const MAX_CELLS: usize = 16_384;
pub const MAX_PAIRS: usize = 4_096;
pub const MAX_FEATURES_PER_CELL: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialCommunicationRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub radius_milli: u64,
    pub min_neighbors: usize,
    pub min_lineage_cells: usize,
    pub min_signal_milli: u64,
    pub min_enrichment_milli: u64,
    pub max_pairs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialCommunicationCell {
    pub cell_id: String,
    pub sample_id: String,
    pub lineage: String,
    pub x_milli: i64,
    pub y_milli: i64,
    pub ligand_scores_milli: BTreeMap<String, u16>,
    pub receptor_scores_milli: BTreeMap<String, u16>,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LigandReceptorPair {
    pub pair_id: String,
    pub ligand_feature: String,
    pub receptor_feature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialCommunicationPairDisposition {
    Enriched,
    NotEnriched,
    MissingFeature,
    Sparse,
    NoExpectedSignal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialCommunicationPair {
    pub pair_id: String,
    pub source_lineage: String,
    pub target_lineage: String,
    pub ligand_feature: String,
    pub receptor_feature: String,
    pub observed_signal_milli: u64,
    pub expected_signal_milli: u64,
    pub enrichment_milli: u64,
    pub neighbor_edges: u32,
    pub sender_cells: u32,
    pub receiver_cells: u32,
    pub disposition: SpatialCommunicationPairDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialCommunicationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialCommunicationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_order: Vec<String>,
    pub lineage_order: Vec<String>,
    pub pair_order: Vec<String>,
    pub pairs: Vec<SpatialCommunicationPair>,
    pub enriched_order: Vec<String>,
    pub not_enriched_order: Vec<String>,
    pub missing_feature_order: Vec<String>,
    pub sparse_order: Vec<String>,
    pub no_expected_signal_order: Vec<String>,
    pub total_neighbor_edges: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SpatialCommunicationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpatialCommunicationError {
    #[error("spatial communication request is invalid: {0}")]
    InvalidRequest(String),
    #[error("spatial communication input is invalid: {0}")]
    InvalidInput(String),
    #[error("spatial communication output is invalid: {0}")]
    InvalidOutput(String),
    #[error("spatial communication digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &SpatialCommunicationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_order": output.sample_order,
        "lineage_order": output.lineage_order,
        "pair_order": output.pair_order,
        "pairs": output.pairs,
        "enriched_order": output.enriched_order,
        "not_enriched_order": output.not_enriched_order,
        "missing_feature_order": output.missing_feature_order,
        "sparse_order": output.sparse_order,
        "no_expected_signal_order": output.no_expected_signal_order,
        "total_neighbor_edges": output.total_neighbor_edges,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl SpatialCommunicationAnalysis {
    pub fn validate(&self) -> Result<(), SpatialCommunicationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.sample_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.lineage_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.pair_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.pairs.len() != self.pair_order.len()
            || self.pairs.windows(2).any(|pair| {
                pair[0].observed_signal_milli < pair[1].observed_signal_milli
                    || (pair[0].observed_signal_milli == pair[1].observed_signal_milli
                        && pair[0].pair_id > pair[1].pair_id)
            })
            || self.pairs.iter().any(|pair| {
                pair.pair_id.trim().is_empty()
                    || pair.source_lineage.trim().is_empty()
                    || pair.target_lineage.trim().is_empty()
                    || pair.ligand_feature.trim().is_empty()
                    || pair.receptor_feature.trim().is_empty()
                    || pair.enrichment_milli > 10_000_000
                    || pair.neighbor_edges == 0
                    || pair.sender_cells == 0
                    || pair.receiver_cells == 0
                    || pair.rationale.trim().is_empty()
            })
            || self
                .enriched_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .not_enriched_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .missing_feature_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.sparse_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .no_expected_signal_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .iter()
                .chain(self.uncertainty.iter())
                .any(|item| item.trim().is_empty())
            || self.digest.as_str().len() != 64
        {
            return Err(SpatialCommunicationError::InvalidOutput(
                "identity, ordering, pair metrics, or digest fields are invalid".into(),
            ));
        }
        let pair_ids = self
            .pairs
            .iter()
            .map(|pair| pair.pair_id.as_str())
            .collect::<BTreeSet<_>>();
        if pair_ids.len() != self.pairs.len()
            || pair_ids
                != self
                    .pair_order
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
        {
            return Err(SpatialCommunicationError::InvalidOutput(
                "pair identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SpatialCommunicationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SpatialCommunicationError::InvalidOutput(
                "spatial communication digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_inputs(
    request: &SpatialCommunicationRequest,
    cells: &[SpatialCommunicationCell],
    pairs: &[LigandReceptorPair],
) -> Result<(), SpatialCommunicationError> {
    if request.study_id.trim().is_empty()
        || request.radius_milli == 0
        || request.min_neighbors == 0
        || request.min_lineage_cells == 0
        || request.min_signal_milli > 1_000_000
        || request.min_enrichment_milli > 10_000_000
        || request.max_pairs == 0
        || request.max_pairs > MAX_PAIRS
    {
        return Err(SpatialCommunicationError::InvalidRequest(
            "study, radius, neighbourhood, lineage, signal, enrichment, or pair bounds are invalid"
                .into(),
        ));
    }
    if cells.is_empty() || cells.len() > MAX_CELLS {
        return Err(SpatialCommunicationError::InvalidInput(
            "cell count is empty or exceeds the bounded spatial capacity".into(),
        ));
    }
    if pairs.is_empty() || pairs.len() > MAX_PAIRS {
        return Err(SpatialCommunicationError::InvalidInput(
            "at least one and at most 4096 ligand-receptor pairs are required".into(),
        ));
    }
    let mut cell_ids = BTreeSet::new();
    let mut pair_ids = BTreeSet::new();
    for cell in cells {
        if cell.cell_id.trim().is_empty()
            || cell.sample_id.trim().is_empty()
            || cell.lineage.trim().is_empty()
            || cell.ligand_scores_milli.len() > MAX_FEATURES_PER_CELL
            || cell.receptor_scores_milli.len() > MAX_FEATURES_PER_CELL
            || !cell_ids.insert(cell.cell_id.clone())
            || cell
                .ligand_scores_milli
                .values()
                .chain(cell.receptor_scores_milli.values())
                .any(|value| *value > 1_000)
        {
            return Err(SpatialCommunicationError::InvalidInput(
                "cell identity, lineage, features, or score bounds are invalid".into(),
            ));
        }
        cell.artifact
            .validate()
            .map_err(|error| SpatialCommunicationError::InvalidInput(error.to_string()))?;
    }
    for pair in pairs {
        if pair.pair_id.trim().is_empty()
            || pair.ligand_feature.trim().is_empty()
            || pair.receptor_feature.trim().is_empty()
            || !pair_ids.insert(pair.pair_id.clone())
        {
            return Err(SpatialCommunicationError::InvalidInput(
                "ligand-receptor pair identity and feature bindings must be unique".into(),
            ));
        }
    }
    Ok(())
}

fn distance_squared(left: &SpatialCommunicationCell, right: &SpatialCommunicationCell) -> u128 {
    let dx = i128::from(left.x_milli) - i128::from(right.x_milli);
    let dy = i128::from(left.y_milli) - i128::from(right.y_milli);
    dx.unsigned_abs()
        .saturating_mul(dx.unsigned_abs())
        .saturating_add(dy.unsigned_abs().saturating_mul(dy.unsigned_abs()))
}

fn mean_for_lineage_feature(
    cells: &[&SpatialCommunicationCell],
    feature: &str,
    ligand: bool,
) -> Option<u64> {
    let values = cells
        .iter()
        .filter_map(|cell| {
            if ligand {
                cell.ligand_scores_milli.get(feature).copied()
            } else {
                cell.receptor_scores_milli.get(feature).copied()
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().map(|value| u64::from(*value)).sum::<u64>() / values.len() as u64)
    }
}

/// Infer local ligand–receptor communication enrichments against a lineage-marginal null model.
pub fn analyze_glioma_spatial_communication(
    request: &SpatialCommunicationRequest,
    cells: &[SpatialCommunicationCell],
    pairs: &[LigandReceptorPair],
) -> Result<SpatialCommunicationAnalysis, SpatialCommunicationError> {
    validate_inputs(request, cells, pairs)?;
    let mut sorted_cells = cells.to_vec();
    sorted_cells.sort_by(|left, right| {
        left.sample_id
            .cmp(&right.sample_id)
            .then_with(|| left.cell_id.cmp(&right.cell_id))
    });
    let mut sorted_pairs = pairs.to_vec();
    sorted_pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    let samples = sorted_cells
        .iter()
        .map(|cell| cell.sample_id.clone())
        .collect::<BTreeSet<_>>();
    let lineages = sorted_cells
        .iter()
        .map(|cell| cell.lineage.clone())
        .collect::<BTreeSet<_>>();
    let mut by_lineage = BTreeMap::<String, Vec<&SpatialCommunicationCell>>::new();
    for cell in &sorted_cells {
        by_lineage
            .entry(cell.lineage.clone())
            .or_default()
            .push(cell);
    }
    let radius_squared =
        u128::from(request.radius_milli).saturating_mul(u128::from(request.radius_milli));
    let mut neighbour_edges = BTreeMap::<
        (String, String),
        Vec<(&SpatialCommunicationCell, &SpatialCommunicationCell)>,
    >::new();
    for sender in &sorted_cells {
        for receiver in &sorted_cells {
            if sender.sample_id != receiver.sample_id
                || sender.lineage == receiver.lineage
                || sender.cell_id == receiver.cell_id
                || distance_squared(sender, receiver) > radius_squared
            {
                continue;
            }
            neighbour_edges
                .entry((sender.lineage.clone(), receiver.lineage.clone()))
                .or_default()
                .push((sender, receiver));
        }
    }
    let total_neighbor_edges = neighbour_edges
        .values()
        .map(|edges| edges.len() as u32)
        .sum::<u32>();
    let mut results = Vec::new();
    for pair in &sorted_pairs {
        for ((source_lineage, target_lineage), edges) in &neighbour_edges {
            let sender_cells = by_lineage[source_lineage].len();
            let receiver_cells = by_lineage[target_lineage].len();
            let key = format!("{}:{}:{}", pair.pair_id, source_lineage, target_lineage);
            let has_sender = edges.iter().any(|(sender, _)| {
                sender
                    .ligand_scores_milli
                    .contains_key(&pair.ligand_feature)
            });
            let has_receiver = edges.iter().any(|(_, receiver)| {
                receiver
                    .receptor_scores_milli
                    .contains_key(&pair.receptor_feature)
            });
            let sender_mean =
                mean_for_lineage_feature(&by_lineage[source_lineage], &pair.ligand_feature, true);
            let receiver_mean = mean_for_lineage_feature(
                &by_lineage[target_lineage],
                &pair.receptor_feature,
                false,
            );
            let (observed, expected, disposition, rationale) = if edges.len()
                < request.min_neighbors
                || sender_cells < request.min_lineage_cells
                || receiver_cells < request.min_lineage_cells
            {
                (
                    0,
                    0,
                    SpatialCommunicationPairDisposition::Sparse,
                    "neighbourhood edge count is below the declared spatial support floor".into(),
                )
            } else if !has_sender
                || !has_receiver
                || sender_mean.is_none()
                || receiver_mean.is_none()
            {
                (
                    0,
                    0,
                    SpatialCommunicationPairDisposition::MissingFeature,
                    "ligand or receptor feature coverage is incomplete; no imputation is performed"
                        .into(),
                )
            } else {
                let observed_sum = edges
                    .iter()
                    .map(|(sender, receiver)| {
                        u64::from(sender.ligand_scores_milli[&pair.ligand_feature]).saturating_mul(
                            u64::from(receiver.receptor_scores_milli[&pair.receptor_feature]),
                        ) / 1_000
                    })
                    .sum::<u64>();
                let observed = observed_sum / edges.len() as u64;
                let expected = sender_mean
                    .unwrap_or(0)
                    .saturating_mul(receiver_mean.unwrap_or(0))
                    / 1_000;
                if expected == 0 {
                    (
                        observed,
                        expected,
                        SpatialCommunicationPairDisposition::NoExpectedSignal,
                        "the lineage-marginal null has zero expected signal; enrichment is not identified".into(),
                    )
                } else if observed >= request.min_signal_milli
                    && observed.saturating_mul(1_000) / expected >= request.min_enrichment_milli
                {
                    (
                        observed,
                        expected,
                        SpatialCommunicationPairDisposition::Enriched,
                        "observed sender-receiver signal exceeds the lineage-marginal random-mixing null".into(),
                    )
                } else {
                    (
                        observed,
                        expected,
                        SpatialCommunicationPairDisposition::NotEnriched,
                        "observed signal does not clear both the absolute and null-enrichment gates".into(),
                    )
                }
            };
            let enrichment = observed
                .saturating_mul(1_000)
                .checked_div(expected)
                .unwrap_or(0);
            results.push(SpatialCommunicationPair {
                pair_id: key,
                source_lineage: source_lineage.clone(),
                target_lineage: target_lineage.clone(),
                ligand_feature: pair.ligand_feature.clone(),
                receptor_feature: pair.receptor_feature.clone(),
                observed_signal_milli: observed,
                expected_signal_milli: expected,
                enrichment_milli: enrichment,
                neighbor_edges: edges.len() as u32,
                sender_cells: sender_cells as u32,
                receiver_cells: receiver_cells as u32,
                disposition,
                rationale,
            });
        }
    }
    results.sort_by(|left, right| {
        right
            .observed_signal_milli
            .cmp(&left.observed_signal_milli)
            .then_with(|| left.pair_id.cmp(&right.pair_id))
    });
    results.truncate(request.max_pairs.min(results.len()));
    let mut pair_order = results
        .iter()
        .map(|result| result.pair_id.clone())
        .collect::<Vec<_>>();
    pair_order.sort();
    let mut enriched = Vec::new();
    let mut not_enriched = Vec::new();
    let mut missing = Vec::new();
    let mut sparse = Vec::new();
    let mut no_expected = Vec::new();
    let mut negative = Vec::new();
    for result in &results {
        match result.disposition {
            SpatialCommunicationPairDisposition::Enriched => enriched.push(result.pair_id.clone()),
            SpatialCommunicationPairDisposition::NotEnriched => {
                not_enriched.push(result.pair_id.clone());
                negative.push(format!("not-enriched:{}", result.pair_id));
            }
            SpatialCommunicationPairDisposition::MissingFeature => {
                missing.push(result.pair_id.clone())
            }
            SpatialCommunicationPairDisposition::Sparse => sparse.push(result.pair_id.clone()),
            SpatialCommunicationPairDisposition::NoExpectedSignal => {
                no_expected.push(result.pair_id.clone());
                negative.push(format!("no-expected-signal:{}", result.pair_id));
            }
        }
    }
    for order in [
        &mut enriched,
        &mut not_enriched,
        &mut missing,
        &mut sparse,
        &mut no_expected,
        &mut negative,
    ] {
        order.sort();
    }
    let mut uncertainty = Vec::new();
    if total_neighbor_edges == 0 {
        uncertainty.push("no cross-lineage spatial neighbours were observed".into());
    }
    if results.len() < sorted_pairs.len() {
        uncertainty.push(
            "pair output was bounded by max_pairs; omitted pair strata remain unreported".into(),
        );
    }
    if results.iter().any(|result| {
        matches!(
            result.disposition,
            SpatialCommunicationPairDisposition::MissingFeature
                | SpatialCommunicationPairDisposition::Sparse
                | SpatialCommunicationPairDisposition::NoExpectedSignal
        )
    }) {
        uncertainty.push(
            "some pair/lineage strata lack complete feature coverage or an identifiable null"
                .into(),
        );
    }
    let disposition = if enriched.is_empty() {
        if results.is_empty() || total_neighbor_edges == 0 {
            SpatialCommunicationDisposition::Unresolved
        } else {
            SpatialCommunicationDisposition::Partial
        }
    } else if uncertainty.is_empty() {
        SpatialCommunicationDisposition::Qualified
    } else {
        SpatialCommunicationDisposition::Partial
    };
    let mut output = SpatialCommunicationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_order: samples.into_iter().collect(),
        lineage_order: lineages.into_iter().collect(),
        pair_order,
        pairs: results,
        enriched_order: enriched,
        not_enriched_order: not_enriched,
        missing_feature_order: missing,
        sparse_order: sparse,
        no_expected_signal_order: no_expected,
        total_neighbor_edges,
        negative_evidence: negative,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-spatial-communication"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SpatialCommunicationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> SpatialCommunicationRequest {
        SpatialCommunicationRequest {
            study_id: "communication-study".into(),
            model_system: GliomaModelSystem::Organoid,
            radius_milli: 1_500,
            min_neighbors: 2,
            min_lineage_cells: 1,
            min_signal_milli: 100,
            min_enrichment_milli: 900,
            max_pairs: 10,
        }
    }

    fn cells() -> Vec<SpatialCommunicationCell> {
        vec![
            SpatialCommunicationCell {
                cell_id: "tumour-1".into(),
                sample_id: "sample-1".into(),
                lineage: "tumour".into(),
                x_milli: 0,
                y_milli: 0,
                ligand_scores_milli: BTreeMap::from([("L1".into(), 900)]),
                receptor_scores_milli: BTreeMap::new(),
                artifact: artifact("tumour-1"),
            },
            SpatialCommunicationCell {
                cell_id: "tumour-2".into(),
                sample_id: "sample-1".into(),
                lineage: "tumour".into(),
                x_milli: 500,
                y_milli: 0,
                ligand_scores_milli: BTreeMap::from([("L1".into(), 800)]),
                receptor_scores_milli: BTreeMap::new(),
                artifact: artifact("tumour-2"),
            },
            SpatialCommunicationCell {
                cell_id: "myeloid-1".into(),
                sample_id: "sample-1".into(),
                lineage: "myeloid".into(),
                x_milli: 1_000,
                y_milli: 0,
                ligand_scores_milli: BTreeMap::new(),
                receptor_scores_milli: BTreeMap::from([("R1".into(), 900)]),
                artifact: artifact("myeloid-1"),
            },
            SpatialCommunicationCell {
                cell_id: "myeloid-2".into(),
                sample_id: "sample-1".into(),
                lineage: "myeloid".into(),
                x_milli: 1_000,
                y_milli: 500,
                ligand_scores_milli: BTreeMap::new(),
                receptor_scores_milli: BTreeMap::from([("R1".into(), 800)]),
                artifact: artifact("myeloid-2"),
            },
        ]
    }

    fn pairs() -> Vec<LigandReceptorPair> {
        vec![LigandReceptorPair {
            pair_id: "L1-R1".into(),
            ligand_feature: "L1".into(),
            receptor_feature: "R1".into(),
        }]
    }

    #[test]
    fn communication_is_enriched_against_lineage_null() {
        let output = analyze_glioma_spatial_communication(&request(), &cells(), &pairs()).unwrap();
        assert_eq!(output.disposition, SpatialCommunicationDisposition::Partial);
        assert_eq!(output.enriched_order.len(), 1);
        output.validate().unwrap();
    }

    #[test]
    fn missing_features_and_sparse_neighbours_are_explicit() {
        let mut sparse_request = request();
        sparse_request.min_neighbors = 100;
        let output =
            analyze_glioma_spatial_communication(&sparse_request, &cells(), &pairs()).unwrap();
        assert_eq!(output.disposition, SpatialCommunicationDisposition::Partial);
        assert!(!output.sparse_order.is_empty());
        let output = analyze_glioma_spatial_communication(
            &request(),
            &cells(),
            &[LigandReceptorPair {
                pair_id: "missing".into(),
                ligand_feature: "unknown".into(),
                receptor_feature: "R1".into(),
            }],
        )
        .unwrap();
        assert!(!output.missing_feature_order.is_empty());
    }

    #[test]
    fn input_permutation_replays_identically() {
        let first = analyze_glioma_spatial_communication(&request(), &cells(), &pairs()).unwrap();
        let mut reverse_cells = cells();
        reverse_cells.reverse();
        let mut reverse_pairs = pairs();
        reverse_pairs.reverse();
        let second =
            analyze_glioma_spatial_communication(&request(), &reverse_cells, &reverse_pairs)
                .unwrap();
        assert_eq!(first, second);
    }
}
