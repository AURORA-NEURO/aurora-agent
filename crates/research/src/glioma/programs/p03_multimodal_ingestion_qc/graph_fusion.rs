//! Reliability-weighted multimodal graph fusion for preclinical glioma studies.
//!
//! This feature turns value-only modality vectors into a bounded, reproducible sample graph.
//! Modality-specific neighbours are kept visible, then fused with reliability weights before a
//! small deterministic diffusion. Missing modalities, sparse overlap, and contradictory modality
//! evidence are emitted as first-class negative evidence; no value is imputed and no clinical or
//! human-subject data is accepted.

use super::concordance::FeatureValue;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalGraphFusion1@1";
const SCALE: u64 = 1_000;
const MAX_VECTORS: usize = 16_384;
const MAX_FEATURES_PER_VECTOR: usize = 16_384;
const MAX_MODALITIES: usize = 16;
const MAX_NEIGHBOURS: usize = 128;
const MAX_STEPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFusionRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub min_samples: usize,
    pub min_modalities_per_sample: usize,
    pub min_shared_features: usize,
    pub neighbours: usize,
    pub diffusion_steps: usize,
    pub max_distance_milli: u64,
    pub min_consensus_support_milli: u16,
    pub max_disagreement_milli: u16,
    pub require_all_modalities: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFusionVector {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub reliability_milli: u16,
    pub features: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFusionNeighbour {
    pub source_sample: String,
    pub target_sample: String,
    pub modality: GliomaModality,
    pub distance_milli: u64,
    pub similarity_milli: u16,
    pub reliability_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFusionState {
    pub sample_lineage: String,
    pub modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub modality_support_milli: u16,
    pub fused_neighbour_order: Vec<String>,
    pub fused_similarity_milli: Vec<u16>,
    pub cross_modal_agreement_milli: u16,
    pub diffusion_score_milli: u16,
    pub structural_rank: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphFusionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphFusionAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub neighbour_order: Vec<(String, String)>,
    pub neighbours: Vec<GraphFusionNeighbour>,
    pub state_order: Vec<String>,
    pub states: Vec<GraphFusionState>,
    pub missing_sample_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub contradictory_pair_order: Vec<(String, String)>,
    pub sparse_pair_order: Vec<(String, String)>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GraphFusionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphFusionError {
    #[error("graph fusion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("graph fusion vector is invalid: {0}")]
    InvalidVector(String),
    #[error("graph fusion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("graph fusion digest failed: {0}")]
    Digest(String),
}

fn modality_label(modality: GliomaModality) -> &'static str {
    match modality {
        GliomaModality::Literature => "literature",
        GliomaModality::Histopathology => "histopathology",
        GliomaModality::Genomics => "genomics",
        GliomaModality::Transcriptomics => "transcriptomics",
        GliomaModality::Epigenomics => "epigenomics",
        GliomaModality::Proteomics => "proteomics",
        GliomaModality::Imaging => "imaging",
        GliomaModality::SingleCell => "single_cell",
        GliomaModality::Spatial => "spatial",
        GliomaModality::FunctionalPerturbation => "functional_perturbation",
        GliomaModality::OrganoidAssay => "organoid_assay",
        GliomaModality::AnimalModel => "animal_model",
        GliomaModality::Computational => "computational",
        GliomaModality::Instrument => "instrument",
        GliomaModality::Replication => "replication",
    }
}

fn ordered<T: Ord>(items: &[T]) -> bool {
    items.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique<T: Ord>(items: &[T]) -> bool {
    items.windows(2).all(|pair| pair[0] != pair[1])
}

fn digest_input(output: &GraphFusionAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_order": output.sample_order,
        "modality_order": output.modality_order,
        "neighbour_order": output.neighbour_order,
        "neighbours": output.neighbours,
        "state_order": output.state_order,
        "states": output.states,
        "missing_sample_order": output.missing_sample_order,
        "missing_modality_order": output.missing_modality_order,
        "contradictory_pair_order": output.contradictory_pair_order,
        "sparse_pair_order": output.sparse_pair_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl GraphFusionAnalysis {
    pub fn validate(&self) -> Result<(), GraphFusionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || !ordered(&self.sample_order)
            || !ordered(&self.modality_order)
            || !unique(&self.state_order)
            || !ordered(&self.missing_sample_order)
            || !ordered(&self.missing_modality_order)
            || !ordered(&self.neighbour_order)
            || !ordered(&self.contradictory_pair_order)
            || !ordered(&self.sparse_pair_order)
            || !ordered(&self.negative_evidence)
            || !ordered(&self.uncertainty)
            || self.neighbours.iter().any(|edge| {
                edge.source_sample == edge.target_sample
                    || edge.similarity_milli > SCALE as u16
                    || edge.reliability_milli > SCALE as u16
            })
            || self.states.iter().any(|state| {
                !ordered(&state.modality_order)
                    || !ordered(&state.missing_modality_order)
                    || !ordered(&state.fused_neighbour_order)
                    || state.fused_neighbour_order.len() != state.fused_similarity_milli.len()
                    || state.modality_support_milli > SCALE as u16
                    || state.cross_modal_agreement_milli > SCALE as u16
                    || state.diffusion_score_milli > SCALE as u16
            })
        {
            return Err(GraphFusionError::InvalidOutput(
                "identity, ordering, bounds, or state partitions are invalid".into(),
            ));
        }
        if self.sample_order.len() != self.states.len()
            || self.sample_order.iter().any(|sample| {
                !self
                    .states
                    .iter()
                    .any(|state| &state.sample_lineage == sample)
            })
            || self
                .state_order
                .iter()
                .any(|sample| !self.sample_order.contains(sample))
        {
            return Err(GraphFusionError::InvalidOutput(
                "sample and state partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| GraphFusionError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(GraphFusionError::InvalidOutput(
                "digest is not bound to graph fusion analysis".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &GraphFusionRequest) -> Result<(), GraphFusionError> {
    if request.study_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || request.min_samples < 2
        || request.min_modalities_per_sample == 0
        || request.min_modalities_per_sample > request.required_modalities.len()
        || request.min_shared_features == 0
        || request.neighbours == 0
        || request.neighbours > MAX_NEIGHBOURS
        || request.diffusion_steps == 0
        || request.diffusion_steps > MAX_STEPS
        || request.max_distance_milli == 0
        || request.min_consensus_support_milli > SCALE as u16
        || request.max_disagreement_milli > SCALE as u16
    {
        return Err(GraphFusionError::InvalidRequest(
            "study, modality, sample, neighbourhood, diffusion, or threshold bounds are invalid"
                .into(),
        ));
    }
    Ok(())
}

fn validate_vector(
    vector: &GraphFusionVector,
    request: &GraphFusionRequest,
) -> Result<(), GraphFusionError> {
    if vector.observation_id.trim().is_empty()
        || vector.study_id != request.study_id
        || vector.sample_lineage.trim().is_empty()
        || vector.model_system != request.model_system
        || !request.required_modalities.contains(&vector.modality)
        || vector.reliability_milli > SCALE as u16
        || vector.features.is_empty()
        || vector.features.len() > MAX_FEATURES_PER_VECTOR
    {
        return Err(GraphFusionError::InvalidVector(format!(
            "invalid identity, modality, reliability, or feature bounds for {}",
            vector.observation_id
        )));
    }
    vector
        .artifact
        .validate()
        .map_err(|e| GraphFusionError::InvalidVector(e.to_string()))?;
    let mut ids = BTreeSet::new();
    if vector.features.iter().any(|feature| {
        feature.feature_id.trim().is_empty() || !ids.insert(feature.feature_id.clone())
    }) {
        return Err(GraphFusionError::InvalidVector(format!(
            "feature identifiers are empty or duplicated for {}",
            vector.observation_id
        )));
    }
    Ok(())
}

#[derive(Clone)]
struct Profile {
    reliability: u16,
    features: BTreeMap<String, i64>,
}

type PairKey = (String, String);
type ModalityScore = (GliomaModality, u16, u64, u16);

fn pair_score(
    left: &Profile,
    right: &Profile,
    minimum_shared: usize,
    max_distance: u64,
) -> Option<(u16, u64, u16)> {
    let mut shared = 0usize;
    let mut distance = 0u128;
    for (feature, value) in &left.features {
        if let Some(other) = right.features.get(feature) {
            shared += 1;
            distance += (*value).abs_diff(*other) as u128;
        }
    }
    if shared < minimum_shared {
        return None;
    }
    let mean_distance = (distance / shared as u128).min(u128::from(max_distance)) as u64;
    let similarity = ((max_distance.saturating_sub(mean_distance) as u128 * u128::from(SCALE))
        / u128::from(max_distance)) as u16;
    let reliability = left.reliability.min(right.reliability);
    let weighted = ((u32::from(similarity) * u32::from(reliability)) / SCALE as u32) as u16;
    Some((weighted, mean_distance, reliability))
}

/// Analyze modality-specific neighbourhoods, fuse them with reliability weights, and run bounded
/// diffusion without moving raw artifacts across the local boundary.
pub fn analyze_glioma_multimodal_graph_fusion(
    request: &GraphFusionRequest,
    vectors: &[GraphFusionVector],
) -> Result<GraphFusionAnalysis, GraphFusionError> {
    validate_request(request)?;
    if vectors.len() > MAX_VECTORS {
        return Err(GraphFusionError::InvalidRequest(
            "vector count exceeds bounded graph budget".into(),
        ));
    }
    let mut seen_observations = BTreeSet::new();
    let mut by_sample: BTreeMap<String, BTreeMap<GliomaModality, Profile>> = BTreeMap::new();
    for vector in vectors {
        validate_vector(vector, request)?;
        if !seen_observations.insert(vector.observation_id.clone()) {
            return Err(GraphFusionError::InvalidVector(
                "observation identifiers must be unique".into(),
            ));
        }
        let features = vector
            .features
            .iter()
            .map(|f| (f.feature_id.clone(), f.value_milli))
            .collect::<BTreeMap<_, _>>();
        let modalities = by_sample.entry(vector.sample_lineage.clone()).or_default();
        if modalities
            .insert(
                vector.modality,
                Profile {
                    reliability: vector.reliability_milli,
                    features,
                },
            )
            .is_some()
        {
            return Err(GraphFusionError::InvalidVector(format!(
                "duplicate modality {} for sample {}",
                modality_label(vector.modality),
                vector.sample_lineage
            )));
        }
    }
    if by_sample.len() < request.min_samples {
        return Err(GraphFusionError::InvalidRequest(
            "insufficient distinct samples".into(),
        ));
    }
    let sample_order = by_sample.keys().cloned().collect::<Vec<_>>();
    let modality_order = request
        .required_modalities
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let mut missing_samples = Vec::new();
    let mut missing_modalities = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (sample, modalities) in &by_sample {
        if modalities.len() < request.min_modalities_per_sample {
            missing_samples.push(sample.clone());
        }
        for modality in &modality_order {
            if !modalities.contains_key(modality) {
                missing_modalities.insert(*modality);
            }
        }
    }
    if !missing_samples.is_empty() {
        negative.insert(format!(
            "{} samples do not meet the required modality count",
            missing_samples.len()
        ));
    }
    if !missing_modalities.is_empty() {
        negative.insert(
            "required modalities are missing for at least one sample; no modality was imputed"
                .into(),
        );
    }

    let mut neighbours = Vec::new();
    let mut pair_scores: BTreeMap<PairKey, Vec<ModalityScore>> = BTreeMap::new();
    let mut contradictory = BTreeSet::new();
    let mut sparse = BTreeSet::new();
    for left_index in 0..sample_order.len() {
        for right_index in (left_index + 1)..sample_order.len() {
            let left_name = &sample_order[left_index];
            let right_name = &sample_order[right_index];
            let left = &by_sample[left_name];
            let right = &by_sample[right_name];
            let key = (left_name.clone(), right_name.clone());
            let mut scores = Vec::new();
            for modality in &modality_order {
                if let (Some(a), Some(b)) = (left.get(modality), right.get(modality)) {
                    if let Some((similarity, distance, reliability)) = pair_score(
                        a,
                        b,
                        request.min_shared_features,
                        request.max_distance_milli,
                    ) {
                        scores.push((*modality, similarity, distance, reliability));
                    }
                }
            }
            if scores.is_empty() {
                sparse.insert(key.clone());
                continue;
            }
            let min = scores
                .iter()
                .map(|(_, score, _, _)| *score)
                .min()
                .unwrap_or(0);
            let max = scores
                .iter()
                .map(|(_, score, _, _)| *score)
                .max()
                .unwrap_or(0);
            if max - min > request.max_disagreement_milli {
                contradictory.insert(key.clone());
            }
            pair_scores.insert(key, scores);
        }
    }
    if !contradictory.is_empty() {
        negative.insert(
            "cross-modal disagreement exceeded the declared threshold for one or more pairs".into(),
        );
    }
    if !sparse.is_empty() {
        uncertainty.insert(
            "some sample pairs lacked sufficient shared features in every observed modality".into(),
        );
    }

    for sample in &sample_order {
        let mut candidates = Vec::new();
        for ((left, right), scores) in &pair_scores {
            let target = if left == sample {
                Some(right)
            } else if right == sample {
                Some(left)
            } else {
                None
            };
            if let Some(target) = target {
                let total: u64 = scores
                    .iter()
                    .map(|(_, similarity, _, _)| u64::from(*similarity))
                    .sum();
                let fused = (total / scores.len() as u64) as u16;
                for (modality, similarity, distance, reliability) in scores {
                    candidates.push((
                        target.clone(),
                        *modality,
                        *similarity,
                        *distance,
                        *reliability,
                        fused,
                    ));
                }
            }
        }
        candidates.sort_by(|a, b| (b.5, a.0.clone(), a.1).cmp(&(a.5, b.0.clone(), b.1)));
        let mut retained = BTreeSet::new();
        for (target, modality, _similarity, distance, reliability, fused) in candidates {
            if retained.len() >= request.neighbours {
                break;
            }
            if retained.insert(target.clone()) {
                neighbours.push(GraphFusionNeighbour {
                    source_sample: sample.clone(),
                    target_sample: target,
                    modality,
                    distance_milli: distance,
                    similarity_milli: fused,
                    reliability_milli: reliability,
                });
            }
        }
    }
    neighbours.sort_by(|a, b| {
        (a.source_sample.clone(), a.target_sample.clone(), a.modality).cmp(&(
            b.source_sample.clone(),
            b.target_sample.clone(),
            b.modality,
        ))
    });
    let mut state_scores = sample_order
        .iter()
        .map(|sample| (sample.clone(), SCALE))
        .collect::<BTreeMap<_, _>>();
    for _ in 0..request.diffusion_steps {
        let mut next = BTreeMap::new();
        for sample in &sample_order {
            let outgoing = neighbours
                .iter()
                .filter(|edge| &edge.source_sample == sample)
                .collect::<Vec<_>>();
            let neighbour_sum: u64 = outgoing
                .iter()
                .map(|edge| {
                    state_scores.get(&edge.target_sample).copied().unwrap_or(0)
                        * u64::from(edge.similarity_milli)
                        / SCALE
                })
                .sum();
            let neighbour_avg = if outgoing.is_empty() {
                0
            } else {
                neighbour_sum / outgoing.len() as u64
            };
            next.insert(
                sample.clone(),
                (250 * state_scores[sample] + 750 * neighbour_avg) / SCALE,
            );
        }
        state_scores = next;
    }
    let mut ordered_scores = state_scores
        .iter()
        .map(|(sample, score)| (sample.clone(), *score))
        .collect::<Vec<_>>();
    ordered_scores.sort_by(|a, b| (b.1, a.0.clone()).cmp(&(a.1, b.0.clone())));
    let state_order = ordered_scores
        .iter()
        .map(|(sample, _)| sample.clone())
        .collect::<Vec<_>>();
    let mut states = Vec::new();
    for (rank, sample) in state_order.iter().enumerate() {
        let modalities = &by_sample[sample];
        let present = modalities.keys().copied().collect::<Vec<_>>();
        let missing = modality_order
            .iter()
            .copied()
            .filter(|m| !modalities.contains_key(m))
            .collect::<Vec<_>>();
        let edges = neighbours
            .iter()
            .filter(|edge| &edge.source_sample == sample)
            .collect::<Vec<_>>();
        let fused_order = edges
            .iter()
            .map(|edge| edge.target_sample.clone())
            .collect::<Vec<_>>();
        let fused_similarity = edges
            .iter()
            .map(|edge| edge.similarity_milli)
            .collect::<Vec<_>>();
        let support =
            ((present.len() as u64 * SCALE) / modality_order.len() as u64).min(SCALE) as u16;
        let agreement = pair_scores
            .iter()
            .filter(|((a, b), _)| a == sample || b == sample)
            .flat_map(|(_, values)| values.iter().map(|(_, score, _, _)| u64::from(*score)))
            .sum::<u64>();
        let agreement_count = pair_scores
            .iter()
            .filter(|((a, b), _)| a == sample || b == sample)
            .flat_map(|(_, values)| values.iter())
            .count();
        let agreement = if agreement_count == 0 {
            0
        } else {
            (agreement / agreement_count as u64) as u16
        };
        states.push(GraphFusionState {
            sample_lineage: sample.clone(),
            modality_order: present,
            missing_modality_order: missing,
            modality_support_milli: support,
            fused_neighbour_order: fused_order,
            fused_similarity_milli: fused_similarity,
            cross_modal_agreement_milli: agreement,
            diffusion_score_milli: state_scores[sample].min(SCALE) as u16,
            structural_rank: rank,
        });
    }
    states.sort_by(|a, b| a.sample_lineage.cmp(&b.sample_lineage));
    let neighbour_order = neighbours
        .iter()
        .map(|edge| (edge.source_sample.clone(), edge.target_sample.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut disposition = if missing_samples.is_empty()
        && missing_modalities.is_empty()
        && contradictory.is_empty()
        && sparse.is_empty()
    {
        GraphFusionDisposition::Qualified
    } else {
        GraphFusionDisposition::Partial
    };
    if neighbours.is_empty()
        || states
            .iter()
            .all(|state| state.modality_support_milli < request.min_consensus_support_milli)
    {
        disposition = GraphFusionDisposition::Unresolved;
    }
    if request.require_all_modalities
        && (!missing_samples.is_empty() || !missing_modalities.is_empty())
    {
        disposition = GraphFusionDisposition::Unresolved;
    }
    let mut output = GraphFusionAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_order,
        modality_order,
        neighbour_order,
        neighbours,
        state_order,
        states,
        missing_sample_order: missing_samples,
        missing_modality_order: missing_modalities.into_iter().collect(),
        contradictory_pair_order: contradictory.into_iter().collect(),
        sparse_pair_order: sparse.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| GraphFusionError::Digest(e.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|e| GraphFusionError::Digest(e.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::ContentHash;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::parse("0".repeat(64)).expect("hash"),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }
    fn vector(sample: &str, modality: GliomaModality, values: &[(&str, i64)]) -> GraphFusionVector {
        GraphFusionVector {
            observation_id: format!("{sample}-{}", modality_label(modality)),
            study_id: "study-1".into(),
            sample_lineage: sample.into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: artifact(sample),
            reliability_milli: 900,
            features: values
                .iter()
                .map(|(feature_id, value_milli)| FeatureValue {
                    feature_id: (*feature_id).into(),
                    value_milli: *value_milli,
                })
                .collect(),
        }
    }
    fn request() -> GraphFusionRequest {
        GraphFusionRequest {
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: [GliomaModality::Genomics, GliomaModality::Transcriptomics]
                .into_iter()
                .collect(),
            min_samples: 3,
            min_modalities_per_sample: 2,
            min_shared_features: 2,
            neighbours: 2,
            diffusion_steps: 3,
            max_distance_milli: 1_000,
            min_consensus_support_milli: 500,
            max_disagreement_milli: 200,
            require_all_modalities: false,
        }
    }
    #[test]
    fn fuses_modalities_and_is_permutation_invariant() {
        let vectors = vec![
            vector("a", GliomaModality::Genomics, &[("x", 10), ("y", 20)]),
            vector(
                "a",
                GliomaModality::Transcriptomics,
                &[("x", 12), ("y", 18)],
            ),
            vector("b", GliomaModality::Genomics, &[("x", 11), ("y", 19)]),
            vector(
                "b",
                GliomaModality::Transcriptomics,
                &[("x", 13), ("y", 17)],
            ),
            vector("c", GliomaModality::Genomics, &[("x", 90), ("y", 80)]),
            vector(
                "c",
                GliomaModality::Transcriptomics,
                &[("x", 88), ("y", 82)],
            ),
        ];
        let first = analyze_glioma_multimodal_graph_fusion(&request(), &vectors).expect("analysis");
        let mut reversed = vectors.clone();
        reversed.reverse();
        let second =
            analyze_glioma_multimodal_graph_fusion(&request(), &reversed).expect("analysis");
        assert_eq!(first, second);
        assert_eq!(first.disposition, GraphFusionDisposition::Qualified);
        assert!(!first.neighbours.is_empty());
    }
    #[test]
    fn preserves_dropout_and_contradiction_as_partial_evidence() {
        let vectors = vec![
            vector("a", GliomaModality::Genomics, &[("x", 0), ("y", 0)]),
            vector(
                "a",
                GliomaModality::Transcriptomics,
                &[("x", 1_000), ("y", 1_000)],
            ),
            vector("b", GliomaModality::Genomics, &[("x", 0), ("y", 0)]),
            vector("b", GliomaModality::Transcriptomics, &[("x", 0), ("y", 0)]),
            vector("c", GliomaModality::Genomics, &[("x", 0), ("y", 0)]),
        ];
        let result =
            analyze_glioma_multimodal_graph_fusion(&request(), &vectors).expect("analysis");
        assert_eq!(result.disposition, GraphFusionDisposition::Partial);
        assert_eq!(result.missing_sample_order, vec!["c"]);
        assert!(!result.contradictory_pair_order.is_empty());
        assert!(!result.negative_evidence.is_empty());
    }
}
