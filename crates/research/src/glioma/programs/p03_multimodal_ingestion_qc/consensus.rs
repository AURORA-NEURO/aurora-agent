//! Deterministic multimodal consensus clustering for preclinical glioma samples.
//!
//! Each sample lineage is represented by a per-feature median across its declared modalities,
//! which limits the influence of one noisy assay without inventing measurements.  Sample
//! distances are mean absolute differences over explicitly shared features and are then clustered
//! with bounded k-medoids.  Missing modality coverage, insufficient feature overlap, disconnected
//! profiles, and small clusters remain explicit rather than being imputed.

use super::concordance::ModalityVector;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalConsensus1@1";
pub const MAX_VECTORS: usize = 16_384;
pub const MAX_SAMPLES: usize = 2_048;
pub const MAX_ITERATIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub cluster_count: usize,
    pub min_modalities_per_sample: usize,
    pub min_modalities_per_feature: usize,
    pub min_shared_features: usize,
    pub max_iterations: usize,
    pub max_distance_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusAssignment {
    pub sample_lineage: String,
    pub cluster_id: String,
    pub distance_to_medoid_milli: u64,
    pub modality_order: Vec<GliomaModality>,
    pub feature_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusCluster {
    pub cluster_id: String,
    pub medoid_sample_lineage: String,
    pub sample_order: Vec<String>,
    pub mean_distance_milli: u64,
    pub max_distance_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsensusDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalConsensus {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_order: Vec<String>,
    pub assignments: Vec<ConsensusAssignment>,
    pub unresolved_sample_order: Vec<String>,
    pub unresolved_pair_order: Vec<String>,
    pub cluster_order: Vec<String>,
    pub clusters: Vec<ConsensusCluster>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ConsensusDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConsensusError {
    #[error("consensus request is invalid: {0}")]
    InvalidRequest(String),
    #[error("consensus vector is invalid: {0}")]
    InvalidVector(String),
    #[error("consensus output is invalid: {0}")]
    InvalidOutput(String),
    #[error("consensus digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct Profile {
    sample_lineage: String,
    modality_order: BTreeSet<GliomaModality>,
    features: BTreeMap<String, i64>,
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalConsensus) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_order": output.sample_order,
        "assignments": output.assignments,
        "unresolved_sample_order": output.unresolved_sample_order,
        "unresolved_pair_order": output.unresolved_pair_order,
        "cluster_order": output.cluster_order,
        "clusters": output.clusters,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MultimodalConsensus {
    pub fn validate(&self) -> Result<(), ConsensusError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || !ordered_unique(&self.sample_order)
            || !ordered_unique(&self.unresolved_sample_order)
            || !ordered_unique(&self.unresolved_pair_order)
            || !ordered_unique(&self.cluster_order)
            || self.assignments.iter().any(|assignment| {
                assignment.sample_lineage.trim().is_empty()
                    || assignment.cluster_id.trim().is_empty()
                    || assignment
                        .modality_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
            || self
                .assignments
                .windows(2)
                .any(|pair| pair[0].sample_lineage >= pair[1].sample_lineage)
            || self.clusters.iter().any(|cluster| {
                cluster.cluster_id.trim().is_empty()
                    || cluster.medoid_sample_lineage.trim().is_empty()
                    || cluster.sample_order.is_empty()
                    || !ordered_unique(&cluster.sample_order)
                    || cluster.mean_distance_milli > cluster.max_distance_milli
            })
            || self
                .clusters
                .windows(2)
                .any(|pair| pair[0].cluster_id >= pair[1].cluster_id)
        {
            return Err(ConsensusError::InvalidOutput(
                "identity, sample/cluster ordering, assignment, or distance bounds are invalid"
                    .into(),
            ));
        }
        let sample_ids = self.sample_order.iter().cloned().collect::<BTreeSet<_>>();
        let cluster_ids = self
            .clusters
            .iter()
            .map(|cluster| cluster.cluster_id.clone())
            .collect::<BTreeSet<_>>();
        let cluster_member_ids = self
            .clusters
            .iter()
            .flat_map(|cluster| cluster.sample_order.iter().cloned())
            .collect::<BTreeSet<_>>();
        let assigned_ids = self
            .assignments
            .iter()
            .map(|assignment| assignment.sample_lineage.clone())
            .collect::<BTreeSet<_>>();
        let unresolved_ids = self
            .unresolved_sample_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let declared_cluster_order = self
            .clusters
            .iter()
            .map(|cluster| cluster.cluster_id.clone())
            .collect::<Vec<_>>();
        if declared_cluster_order != self.cluster_order
            || cluster_ids.len() != self.clusters.len()
            || self.assignments.iter().any(|assignment| {
                !cluster_ids.contains(&assignment.cluster_id)
                    || !sample_ids.contains(&assignment.sample_lineage)
            })
            || self.clusters.iter().any(|cluster| {
                !cluster
                    .sample_order
                    .iter()
                    .all(|sample_id| sample_ids.contains(sample_id))
                    || !cluster
                        .sample_order
                        .contains(&cluster.medoid_sample_lineage)
            })
            || cluster_member_ids != assigned_ids
            || assigned_ids.intersection(&unresolved_ids).next().is_some()
            || assigned_ids
                .union(&unresolved_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != sample_ids
            || self.assignments.len() != assigned_ids.len()
        {
            return Err(ConsensusError::InvalidOutput(
                "assigned and unresolved samples do not partition sample_order".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ConsensusError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ConsensusError::InvalidOutput(
                "digest is not bound to multimodal consensus".into(),
            ));
        }
        Ok(())
    }
}

fn distance(left: &Profile, right: &Profile, min_shared_features: usize) -> Option<u64> {
    let mut shared = 0_usize;
    let mut total = 0_u128;
    for (feature, left_value) in &left.features {
        if let Some(right_value) = right.features.get(feature) {
            shared += 1;
            total += (*left_value as i128 - *right_value as i128).unsigned_abs();
        }
    }
    if shared < min_shared_features {
        None
    } else {
        Some((total / shared as u128).min(u64::MAX as u128) as u64)
    }
}

fn pair_key(left: &str, right: &str) -> String {
    if left < right {
        format!("{left}|{right}")
    } else {
        format!("{right}|{left}")
    }
}

pub fn analyze_multimodal_consensus(
    request: &ConsensusRequest,
    vectors: &[ModalityVector],
) -> Result<MultimodalConsensus, ConsensusError> {
    if request.study_id.trim().is_empty()
        || request.cluster_count == 0
        || request.min_modalities_per_sample == 0
        || request.min_modalities_per_feature == 0
        || request.min_shared_features == 0
        || request.max_iterations == 0
        || request.max_iterations > MAX_ITERATIONS
        || vectors.len() > MAX_VECTORS
    {
        return Err(ConsensusError::InvalidRequest(
            "study, cluster/modality/feature floors, iteration bound, or vector bound is invalid"
                .into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut grouped = BTreeMap::<String, Vec<&ModalityVector>>::new();
    for vector in vectors {
        vector
            .artifact
            .validate()
            .map_err(|error| ConsensusError::InvalidVector(error.to_string()))?;
        if vector.observation_id.trim().is_empty()
            || vector.study_id != request.study_id
            || vector.sample_lineage.trim().is_empty()
            || vector.model_system != request.model_system
            || vector.features.is_empty()
            || vector.features.len() > super::concordance::MAX_FEATURES
            || vector
                .features
                .windows(2)
                .any(|pair| pair[0].feature_id >= pair[1].feature_id)
            || vector.features.iter().any(|feature| {
                feature.feature_id.trim().is_empty() || feature.value_milli.abs() > 1_000_000_000
            })
            || !observation_ids.insert(vector.observation_id.clone())
        {
            return Err(ConsensusError::InvalidVector(
                "vector identity, study/model binding, feature ordering, value bounds, or uniqueness is invalid".into(),
            ));
        }
        grouped
            .entry(vector.sample_lineage.clone())
            .or_default()
            .push(vector);
    }
    if grouped.len() < request.cluster_count || grouped.len() > MAX_SAMPLES {
        return Err(ConsensusError::InvalidVector(
            "sample count cannot satisfy cluster count or sample bound".into(),
        ));
    }
    let sample_order = grouped.keys().cloned().collect::<Vec<_>>();
    let mut profiles = Vec::with_capacity(sample_order.len());
    let mut unresolved_sample_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for sample_id in &sample_order {
        let sample_vectors = &grouped[sample_id];
        let mut seen_modalities = BTreeSet::new();
        if sample_vectors
            .iter()
            .any(|vector| !seen_modalities.insert(vector.modality))
        {
            return Err(ConsensusError::InvalidVector(
                "each sample lineage may contribute at most one vector per modality".into(),
            ));
        }
        let modalities = sample_vectors
            .iter()
            .map(|vector| vector.modality)
            .collect::<BTreeSet<_>>();
        if modalities.len() < request.min_modalities_per_sample {
            unresolved_sample_order.insert(sample_id.clone());
            uncertainty.insert("sample-modality-floor-not-met".into());
        }
        let mut feature_values = BTreeMap::<String, Vec<i64>>::new();
        for vector in sample_vectors {
            for feature in &vector.features {
                feature_values
                    .entry(feature.feature_id.clone())
                    .or_default()
                    .push(feature.value_milli);
            }
        }
        let features = feature_values
            .into_iter()
            .filter_map(|(feature, mut values)| {
                if values.len() < request.min_modalities_per_feature {
                    return None;
                }
                values.sort_unstable();
                Some((feature, values[(values.len() - 1) / 2]))
            })
            .collect::<BTreeMap<_, _>>();
        if features.len() < request.min_shared_features {
            unresolved_sample_order.insert(sample_id.clone());
            uncertainty.insert("sample-feature-floor-not-met".into());
        }
        profiles.push(Profile {
            sample_lineage: sample_id.clone(),
            modality_order: modalities,
            features,
        });
    }
    let profile_map = profiles
        .iter()
        .map(|profile| (profile.sample_lineage.clone(), profile))
        .collect::<BTreeMap<_, _>>();
    let mut distances = BTreeMap::<String, u64>::new();
    let mut unresolved_pair_order = BTreeSet::new();
    for left_index in 0..sample_order.len() {
        for right_index in left_index + 1..sample_order.len() {
            let left = &profile_map[&sample_order[left_index]];
            let right = &profile_map[&sample_order[right_index]];
            let key = pair_key(&left.sample_lineage, &right.sample_lineage);
            if let Some(value) = distance(left, right, request.min_shared_features) {
                distances.insert(key, value);
            } else {
                unresolved_pair_order.insert(key);
            }
        }
    }
    if !unresolved_pair_order.is_empty() {
        uncertainty.insert("sample-pair-shared-feature-floor-not-met".into());
    }
    // Only profiles that satisfy the modality and feature floors can influence a cluster. An
    // unresolved profile is retained in the output partition, but it is never used as a medoid,
    // assignment target, or distance statistic.
    let eligible_order = sample_order
        .iter()
        .filter(|sample_id| !unresolved_sample_order.contains(*sample_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut medoids = eligible_order
        .iter()
        .take(request.cluster_count)
        .cloned()
        .collect::<Vec<_>>();
    let mut assignments_by_sample = BTreeMap::<String, usize>::new();
    for _ in 0..request.max_iterations {
        let mut changed = false;
        for sample_id in &eligible_order {
            if unresolved_sample_order.contains(sample_id) {
                continue;
            }
            let mut best = None::<(u64, String, usize)>;
            for (index, medoid) in medoids.iter().enumerate() {
                let distance_value = if sample_id == medoid {
                    0
                } else if let Some(value) = distances.get(&pair_key(sample_id, medoid)) {
                    *value
                } else {
                    continue;
                };
                let candidate = (distance_value, medoid.clone(), index);
                if best.as_ref().is_none_or(|current| candidate < *current) {
                    best = Some(candidate);
                }
            }
            if let Some((_, _, cluster_index)) = best {
                changed |= assignments_by_sample.insert(sample_id.clone(), cluster_index)
                    != Some(cluster_index);
            } else {
                assignments_by_sample.remove(sample_id);
                unresolved_sample_order.insert(sample_id.clone());
            }
        }
        let mut next_medoids = medoids.clone();
        for (cluster_index, next_medoid) in next_medoids.iter_mut().enumerate() {
            let members = eligible_order
                .iter()
                .filter(|sample_id| assignments_by_sample.get(*sample_id) == Some(&cluster_index))
                .cloned()
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let mut best = None::<(usize, u128, String)>;
            for candidate in &members {
                let mut missing = 0_usize;
                let mut total = 0_u128;
                for member in &members {
                    if let Some(value) = distances.get(&pair_key(candidate, member)) {
                        total = total.saturating_add(u128::from(*value));
                    } else {
                        missing += 1;
                    }
                }
                let value = (missing, total, candidate.clone());
                if best.as_ref().is_none_or(|current| value < *current) {
                    best = Some(value);
                }
            }
            if let Some((_, _, candidate)) = best {
                if *next_medoid != candidate {
                    *next_medoid = candidate;
                    changed = true;
                }
            }
        }
        medoids = next_medoids;
        if !changed {
            break;
        }
    }
    let mut cluster_keys = medoids
        .iter()
        .enumerate()
        .map(|(index, medoid)| (medoid.clone(), index))
        .collect::<Vec<_>>();
    cluster_keys.sort_by(|left, right| left.0.cmp(&right.0));
    let mut cluster_id_by_index = BTreeMap::new();
    for (rank, (_, original_index)) in cluster_keys.iter().enumerate() {
        cluster_id_by_index.insert(*original_index, format!("cluster-{:02}", rank + 1));
    }
    let mut assignments = Vec::new();
    let mut clusters = Vec::new();
    for (original_index, medoid) in medoids.iter().enumerate() {
        let cluster_id = cluster_id_by_index[&original_index].clone();
        let members = eligible_order
            .iter()
            .filter(|sample_id| assignments_by_sample.get(*sample_id) == Some(&original_index))
            .cloned()
            .collect::<Vec<_>>();
        if members.is_empty() {
            uncertainty.insert(format!("empty-cluster:{cluster_id}"));
            continue;
        }
        let member_distances = members
            .iter()
            .filter_map(|member| {
                if member == medoid {
                    Some(0)
                } else {
                    distances.get(&pair_key(member, medoid)).copied()
                }
            })
            .collect::<Vec<_>>();
        if member_distances.len() != members.len() {
            uncertainty.insert(format!("cluster-disconnected:{cluster_id}"));
        }
        let mean_distance_milli = if member_distances.is_empty() {
            0
        } else {
            member_distances.iter().sum::<u64>() / member_distances.len() as u64
        };
        let max_distance_milli = member_distances.iter().copied().max().unwrap_or(0);
        if max_distance_milli > request.max_distance_milli {
            uncertainty.insert(format!("cluster-distance-bound-exceeded:{cluster_id}"));
        }
        clusters.push(ConsensusCluster {
            cluster_id,
            medoid_sample_lineage: medoid.clone(),
            sample_order: members.clone(),
            mean_distance_milli,
            max_distance_milli,
        });
        for member in members {
            let profile = &profile_map[&member];
            if !unresolved_sample_order.contains(&member) {
                assignments.push(ConsensusAssignment {
                    sample_lineage: member,
                    cluster_id: cluster_id_by_index[&original_index].clone(),
                    distance_to_medoid_milli: distances
                        .get(&pair_key(profile.sample_lineage.as_str(), medoid))
                        .copied()
                        .unwrap_or(0),
                    modality_order: profile.modality_order.iter().copied().collect(),
                    feature_count: profile.features.len(),
                });
            }
        }
    }
    assignments.sort_by(|left, right| left.sample_lineage.cmp(&right.sample_lineage));
    clusters.sort_by(|left, right| left.cluster_id.cmp(&right.cluster_id));
    let mut negative_evidence = BTreeSet::new();
    if clusters.len() < request.cluster_count {
        negative_evidence.insert("requested-cluster-count-not-realized".into());
    }
    if !unresolved_pair_order.is_empty() {
        negative_evidence.insert("incomparable-sample-pairs-preserved".into());
    }
    let cluster_order = clusters
        .iter()
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let disposition = if assignments.is_empty() || clusters.len() < request.cluster_count {
        ConsensusDisposition::Unresolved
    } else if !unresolved_sample_order.is_empty() || !uncertainty.is_empty() {
        ConsensusDisposition::Partial
    } else {
        ConsensusDisposition::Qualified
    };
    let mut output = MultimodalConsensus {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_order,
        assignments,
        unresolved_sample_order: unresolved_sample_order.into_iter().collect(),
        unresolved_pair_order: unresolved_pair_order.into_iter().collect(),
        cluster_order,
        clusters,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| ConsensusError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ConsensusError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;
    use bioprism_ids::ContentHash;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn vector(sample: &str, modality: GliomaModality, values: &[i64]) -> ModalityVector {
        ModalityVector {
            observation_id: format!("{sample}-{modality:?}"),
            study_id: "study".into(),
            sample_lineage: sample.into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{sample}-{modality:?}"),
                content_hash: hash(sample),
                content_type: "application/vnd.aurora.glioma-vector+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            features: values
                .iter()
                .enumerate()
                .map(|(index, value)| super::super::concordance::FeatureValue {
                    feature_id: format!("feature-{index}"),
                    value_milli: *value,
                })
                .collect(),
        }
    }

    fn request() -> ConsensusRequest {
        ConsensusRequest {
            study_id: "study".into(),
            model_system: GliomaModelSystem::Organoid,
            cluster_count: 2,
            min_modalities_per_sample: 2,
            min_modalities_per_feature: 1,
            min_shared_features: 3,
            max_iterations: 8,
            max_distance_milli: 100,
        }
    }

    #[test]
    fn consensus_clustering_is_replay_stable() {
        let vectors = vec![
            vector("s1", GliomaModality::Genomics, &[1, 2, 3]),
            vector("s1", GliomaModality::Transcriptomics, &[1, 2, 3]),
            vector("s2", GliomaModality::Genomics, &[2, 3, 4]),
            vector("s2", GliomaModality::Transcriptomics, &[2, 3, 4]),
            vector("s3", GliomaModality::Genomics, &[900, 901, 902]),
            vector("s3", GliomaModality::Transcriptomics, &[900, 901, 902]),
            vector("s4", GliomaModality::Genomics, &[901, 902, 903]),
            vector("s4", GliomaModality::Transcriptomics, &[901, 902, 903]),
        ];
        let first = analyze_multimodal_consensus(&request(), &vectors).unwrap();
        let second = analyze_multimodal_consensus(&request(), &vectors).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ConsensusDisposition::Qualified);
        assert_eq!(first.assignments.len(), 4);
        first.validate().unwrap();
    }

    #[test]
    fn missing_modality_is_partial_and_not_imputed() {
        let mut vectors = vec![
            vector("s1", GliomaModality::Genomics, &[1, 2, 3]),
            vector("s1", GliomaModality::Transcriptomics, &[1, 2, 3]),
            vector("s2", GliomaModality::Genomics, &[2, 3, 4]),
            vector("s2", GliomaModality::Transcriptomics, &[2, 3, 4]),
            vector("s3", GliomaModality::Genomics, &[900, 901, 902]),
            vector("s3", GliomaModality::Transcriptomics, &[900, 901, 902]),
        ];
        vectors.push(vector("s4", GliomaModality::Genomics, &[901, 902, 903]));
        let output = analyze_multimodal_consensus(&request(), &vectors).unwrap();
        assert_eq!(output.disposition, ConsensusDisposition::Partial);
        assert_eq!(output.unresolved_sample_order, vec!["s4"]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("modality-floor")));
    }
}
