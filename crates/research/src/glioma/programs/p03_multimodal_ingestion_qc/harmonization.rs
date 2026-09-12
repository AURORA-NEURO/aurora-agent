//! Robust batch-effect harmonization for preclinical glioma modality vectors.
//!
//! This feature applies deterministic median-centering within each modality.  A declared
//! reference batch is preferred; otherwise the median over eligible batches is used.  It emits
//! corrected vectors and per-batch diagnostics, but never imputes a missing feature: absent
//! features, insufficient batch coverage, large corrections, and incomplete modality overlap
//! remain explicit gates for downstream consensus or causal analysis.

use super::concordance::FeatureValue;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalHarmonization1@1";
pub const MAX_VECTORS: usize = 16_384;
pub const MAX_FEATURES_PER_VECTOR: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizationRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub reference_batch: Option<String>,
    pub min_vectors_per_batch: usize,
    pub min_shared_features: usize,
    pub max_correction_milli: u64,
    pub max_post_harmonization_spread_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizationVector {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub artifact: LocalArtifactRef,
    pub features: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedFeature {
    pub feature_id: String,
    pub original_milli: i64,
    pub correction_milli: i64,
    pub corrected_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedVector {
    pub observation_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub batch_id: String,
    pub feature_order: Vec<String>,
    pub features: Vec<HarmonizedFeature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchHarmonizationDiagnostic {
    pub batch_id: String,
    pub modality: GliomaModality,
    pub vector_count: usize,
    pub feature_order: Vec<String>,
    pub max_correction_milli: u64,
    pub post_harmonization_spread_milli: u64,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarmonizationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalHarmonization {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub reference_batch: Option<String>,
    pub modality_order: Vec<GliomaModality>,
    pub shared_feature_order: Vec<String>,
    pub vector_order: Vec<String>,
    pub diagnostics: Vec<BatchHarmonizationDiagnostic>,
    pub corrected_vectors: Vec<HarmonizedVector>,
    pub excluded_batch_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_feature_order: Vec<String>,
    pub max_correction_milli: u64,
    pub max_post_harmonization_spread_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: HarmonizationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HarmonizationError {
    #[error("multimodal harmonization request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal harmonization vector is invalid: {0}")]
    InvalidVector(String),
    #[error("multimodal harmonization output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal harmonization digest failed: {0}")]
    Digest(String),
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    if values.is_empty() {
        0
    } else {
        values[(values.len() - 1) / 2]
    }
}

fn digest_input(output: &MultimodalHarmonization) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "reference_batch": output.reference_batch,
        "modality_order": output.modality_order,
        "shared_feature_order": output.shared_feature_order,
        "vector_order": output.vector_order,
        "diagnostics": output.diagnostics,
        "corrected_vectors": output.corrected_vectors,
        "excluded_batch_order": output.excluded_batch_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_feature_order": output.missing_feature_order,
        "max_correction_milli": output.max_correction_milli,
        "max_post_harmonization_spread_milli": output.max_post_harmonization_spread_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MultimodalHarmonization {
    pub fn validate(&self) -> Result<(), HarmonizationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || !ordered_unique(&self.modality_order)
            || !ordered_unique(&self.shared_feature_order)
            || !ordered_unique(&self.vector_order)
            || !ordered_unique(&self.excluded_batch_order)
            || !ordered_unique(&self.missing_modality_order)
            || !ordered_unique(&self.missing_feature_order)
            || !ordered_unique(&self.negative_evidence)
            || !ordered_unique(&self.uncertainty)
            || self.diagnostics.windows(2).any(|pair| {
                (pair[0].modality, &pair[0].batch_id) >= (pair[1].modality, &pair[1].batch_id)
            })
            || self.vector_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .corrected_vectors
                .windows(2)
                .any(|pair| pair[0].observation_id >= pair[1].observation_id)
            || self.diagnostics.iter().any(|diagnostic| {
                diagnostic.batch_id.trim().is_empty()
                    || diagnostic.vector_count == 0
                    || !ordered_unique(&diagnostic.feature_order)
                    || (!diagnostic.eligible && diagnostic.exclusion_reason.is_none())
            })
            || self.corrected_vectors.iter().any(|vector| {
                vector.observation_id.trim().is_empty()
                    || vector.sample_lineage.trim().is_empty()
                    || vector.batch_id.trim().is_empty()
                    || vector.feature_order
                        != vector
                            .features
                            .iter()
                            .map(|feature| feature.feature_id.clone())
                            .collect::<Vec<_>>()
                    || !ordered_unique(&vector.feature_order)
                    || vector.features.iter().any(|feature| {
                        feature.feature_id.trim().is_empty()
                            || feature.corrected_milli
                                != feature
                                    .original_milli
                                    .saturating_sub(feature.correction_milli)
                    })
            })
        {
            return Err(HarmonizationError::InvalidOutput(
                "identity, ordering, diagnostic, or corrected-vector invariants are invalid".into(),
            ));
        }
        let diagnostic_ids = self
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{:?}:{}", diagnostic.modality, diagnostic.batch_id))
            .collect::<BTreeSet<_>>();
        let expected_vector_order = self
            .corrected_vectors
            .iter()
            .map(|vector| vector.observation_id.clone())
            .collect::<Vec<_>>();
        if self.vector_order != expected_vector_order
            || diagnostic_ids.len() != self.diagnostics.len()
            || self.excluded_batch_order.iter().any(|batch| {
                !self
                    .diagnostics
                    .iter()
                    .any(|diagnostic| &diagnostic.batch_id == batch)
            })
        {
            return Err(HarmonizationError::InvalidOutput(
                "diagnostic, vector, or excluded-batch partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| HarmonizationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(HarmonizationError::InvalidOutput(
                "digest is not bound to multimodal harmonization".into(),
            ));
        }
        Ok(())
    }
}

pub fn harmonize_glioma_multimodal_batches(
    request: &HarmonizationRequest,
    vectors: &[HarmonizationVector],
) -> Result<MultimodalHarmonization, HarmonizationError> {
    if request.study_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.min_vectors_per_batch == 0
        || request.min_shared_features == 0
        || request.max_correction_milli > i64::MAX as u64
        || request.max_post_harmonization_spread_milli > i64::MAX as u64
        || vectors.is_empty()
        || vectors.len() > MAX_VECTORS
    {
        return Err(HarmonizationError::InvalidRequest(
            "study, modality coverage, positive batch/feature floors, bounds, and vector limit are required".into(),
        ));
    }
    if request
        .reference_batch
        .as_ref()
        .is_some_and(|batch| batch.trim().is_empty())
    {
        return Err(HarmonizationError::InvalidRequest(
            "reference batch cannot be empty".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut vector_keys = BTreeSet::new();
    for vector in vectors {
        if vector.observation_id.trim().is_empty()
            || vector.study_id != request.study_id
            || vector.sample_lineage.trim().is_empty()
            || vector.model_system != request.model_system
            || vector.batch_id.trim().is_empty()
            || vector.artifact.validate().is_err()
            || !vector.artifact.local_only
            || vector.artifact.contains_human_data
            || vector.artifact.contains_direct_identifiers
            || vector.features.is_empty()
            || vector.features.len() > MAX_FEATURES_PER_VECTOR
            || !observation_ids.insert(vector.observation_id.clone())
            || !vector_keys.insert((vector.sample_lineage.clone(), vector.modality))
        {
            return Err(HarmonizationError::InvalidVector(
                "vector identity, study/model binding, local privacy posture, feature bound, or sample/modality uniqueness is invalid".into(),
            ));
        }
        let mut feature_ids = BTreeSet::new();
        for feature in &vector.features {
            if feature.feature_id.trim().is_empty()
                || !feature_ids.insert(feature.feature_id.clone())
            {
                return Err(HarmonizationError::InvalidVector(
                    "feature identity must be non-empty and unique per vector".into(),
                ));
            }
        }
    }
    let modality_order = request
        .required_modalities
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let mut modality_feature_sets = BTreeMap::<GliomaModality, BTreeSet<String>>::new();
    for modality in &modality_order {
        let mut features = BTreeSet::new();
        for vector in vectors.iter().filter(|vector| vector.modality == *modality) {
            features.extend(
                vector
                    .features
                    .iter()
                    .map(|feature| feature.feature_id.clone()),
            );
        }
        modality_feature_sets.insert(*modality, features);
    }
    let shared_feature_order = if modality_feature_sets.is_empty() {
        Vec::new()
    } else {
        let mut iter = modality_feature_sets.values();
        let mut shared = iter.next().cloned().unwrap_or_default();
        for features in iter {
            shared = shared.intersection(features).cloned().collect();
        }
        shared.into_iter().collect()
    };
    let missing_modality_order = modality_order
        .iter()
        .copied()
        .filter(|modality| {
            vectors
                .iter()
                .filter(|vector| vector.modality == *modality)
                .count()
                < request.min_vectors_per_batch
        })
        .collect::<Vec<_>>();
    let missing_feature_order = if shared_feature_order.len() >= request.min_shared_features {
        Vec::new()
    } else {
        shared_feature_order.clone()
    };
    let mut grouped = BTreeMap::<(GliomaModality, String), Vec<&HarmonizationVector>>::new();
    for vector in vectors {
        grouped
            .entry((vector.modality, vector.batch_id.clone()))
            .or_default()
            .push(vector);
    }
    let mut eligible_keys = BTreeSet::new();
    let mut excluded_batch_order = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for ((modality, batch_id), batch_vectors) in &grouped {
        let mut feature_order = BTreeSet::new();
        for vector in batch_vectors {
            feature_order.extend(
                vector
                    .features
                    .iter()
                    .map(|feature| feature.feature_id.clone()),
            );
        }
        let eligible = batch_vectors.len() >= request.min_vectors_per_batch;
        if !eligible {
            excluded_batch_order.insert(batch_id.clone());
        } else {
            eligible_keys.insert((*modality, batch_id.clone()));
        }
        diagnostics.push(BatchHarmonizationDiagnostic {
            batch_id: batch_id.clone(),
            modality: *modality,
            vector_count: batch_vectors.len(),
            feature_order: feature_order.into_iter().collect(),
            max_correction_milli: 0,
            post_harmonization_spread_milli: 0,
            eligible,
            exclusion_reason: (!eligible).then(|| "batch-vector-floor-not-met".into()),
        });
    }
    diagnostics.sort_by(|left, right| {
        (left.modality, &left.batch_id).cmp(&(right.modality, &right.batch_id))
    });
    let mut reference_values = BTreeMap::<(GliomaModality, String), i64>::new();
    for modality in &modality_order {
        let reference_key = request
            .reference_batch
            .as_ref()
            .map(|batch| (*modality, batch.clone()))
            .filter(|key| eligible_keys.contains(key));
        let feature_ids = modality_feature_sets
            .get(modality)
            .cloned()
            .unwrap_or_default();
        for feature_id in feature_ids {
            let values = if let Some((_, batch_id)) = &reference_key {
                grouped
                    .get(&(*modality, batch_id.clone()))
                    .into_iter()
                    .flat_map(|batch_vectors| batch_vectors.iter())
                    .flat_map(|vector| vector.features.iter())
                    .filter(|feature| feature.feature_id == feature_id)
                    .map(|feature| feature.value_milli)
                    .collect::<Vec<_>>()
            } else {
                grouped
                    .iter()
                    .filter(|((candidate_modality, batch_id), _)| {
                        *candidate_modality == *modality
                            && eligible_keys.contains(&(*candidate_modality, batch_id.clone()))
                    })
                    .flat_map(|(_, batch_vectors)| batch_vectors.iter())
                    .flat_map(|vector| vector.features.iter())
                    .filter(|feature| feature.feature_id == feature_id)
                    .map(|feature| feature.value_milli)
                    .collect::<Vec<_>>()
            };
            if !values.is_empty() {
                let mut values = values;
                reference_values.insert((*modality, feature_id), median(&mut values));
            }
        }
    }
    let mut corrections = BTreeMap::<(GliomaModality, String, String), i64>::new();
    for ((modality, batch_id), batch_vectors) in &grouped {
        if !eligible_keys.contains(&(*modality, batch_id.clone())) {
            continue;
        }
        let mut feature_ids = BTreeSet::new();
        for vector in batch_vectors {
            feature_ids.extend(
                vector
                    .features
                    .iter()
                    .map(|feature| feature.feature_id.clone()),
            );
        }
        for feature_id in feature_ids {
            let mut values = batch_vectors
                .iter()
                .flat_map(|vector| vector.features.iter())
                .filter(|feature| feature.feature_id == feature_id)
                .map(|feature| feature.value_milli)
                .collect::<Vec<_>>();
            let Some(reference) = reference_values.get(&(*modality, feature_id.clone())) else {
                continue;
            };
            let correction = median(&mut values).saturating_sub(*reference);
            corrections.insert((*modality, batch_id.clone(), feature_id), correction);
        }
    }
    let mut corrected_vectors = Vec::with_capacity(vectors.len());
    let mut vector_order = Vec::new();
    for vector in vectors {
        vector_order.push(vector.observation_id.clone());
        let mut features = vector
            .features
            .iter()
            .map(|feature| {
                let correction = corrections
                    .get(&(
                        vector.modality,
                        vector.batch_id.clone(),
                        feature.feature_id.clone(),
                    ))
                    .copied()
                    .unwrap_or(0);
                HarmonizedFeature {
                    feature_id: feature.feature_id.clone(),
                    original_milli: feature.value_milli,
                    correction_milli: correction,
                    corrected_milli: feature.value_milli.saturating_sub(correction),
                }
            })
            .collect::<Vec<_>>();
        features.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
        corrected_vectors.push(HarmonizedVector {
            observation_id: vector.observation_id.clone(),
            sample_lineage: vector.sample_lineage.clone(),
            modality: vector.modality,
            batch_id: vector.batch_id.clone(),
            feature_order: features
                .iter()
                .map(|feature| feature.feature_id.clone())
                .collect(),
            features,
        });
    }
    corrected_vectors.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    vector_order = corrected_vectors
        .iter()
        .map(|vector| vector.observation_id.clone())
        .collect();
    let mut max_correction = 0_u64;
    let mut max_spread = 0_u64;
    for diagnostic in &mut diagnostics {
        let key = (diagnostic.modality, diagnostic.batch_id.clone());
        let feature_ids = corrections
            .keys()
            .filter(|(modality, batch_id, _)| *modality == key.0 && *batch_id == key.1)
            .map(|(_, _, feature_id)| feature_id.clone())
            .collect::<BTreeSet<_>>();
        diagnostic.max_correction_milli = feature_ids
            .iter()
            .filter_map(|feature_id| corrections.get(&(key.0, key.1.clone(), feature_id.clone())))
            .map(|correction| correction.unsigned_abs())
            .max()
            .unwrap_or(0);
        diagnostic.post_harmonization_spread_milli = feature_ids
            .iter()
            .filter_map(|feature_id| {
                reference_values
                    .get(&(key.0, feature_id.clone()))
                    .map(|reference| (feature_id, reference))
            })
            .map(|(feature_id, reference)| {
                corrected_vectors
                    .iter()
                    .filter(|vector| vector.modality == key.0 && vector.batch_id == key.1)
                    .flat_map(|vector| vector.features.iter())
                    .filter(|feature| feature.feature_id == *feature_id)
                    .map(|feature| (feature.corrected_milli - *reference).unsigned_abs())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);
        max_correction = max_correction.max(diagnostic.max_correction_milli);
        max_spread = max_spread.max(diagnostic.post_harmonization_spread_milli);
    }
    let mut negative_evidence = BTreeSet::new();
    if max_correction > request.max_correction_milli {
        negative_evidence.insert("batch-correction-exceeds-declared-bound".into());
    }
    if max_spread > request.max_post_harmonization_spread_milli {
        negative_evidence.insert("post-harmonization-spread-exceeds-declared-bound".into());
    }
    let mut uncertainty = BTreeSet::new();
    if !missing_modality_order.is_empty() {
        uncertainty.insert("required-modality-coverage-incomplete".into());
    }
    if shared_feature_order.len() < request.min_shared_features {
        uncertainty.insert("shared-feature-floor-not-met-without-imputation".into());
    }
    if !excluded_batch_order.is_empty() {
        uncertainty.insert("under-replicated-batches-excluded".into());
    }
    let disposition = if vectors.is_empty()
        || !missing_modality_order.is_empty()
        || shared_feature_order.len() < request.min_shared_features
        || eligible_keys.is_empty()
    {
        HarmonizationDisposition::Unresolved
    } else if !uncertainty.is_empty()
        || !negative_evidence.is_empty()
        || max_correction > request.max_correction_milli
        || max_spread > request.max_post_harmonization_spread_milli
    {
        HarmonizationDisposition::Partial
    } else {
        HarmonizationDisposition::Qualified
    };
    let mut output = MultimodalHarmonization {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        reference_batch: request.reference_batch.clone(),
        modality_order,
        shared_feature_order,
        vector_order,
        diagnostics,
        corrected_vectors,
        excluded_batch_order: excluded_batch_order.into_iter().collect(),
        missing_modality_order,
        missing_feature_order,
        max_correction_milli: max_correction,
        max_post_harmonization_spread_milli: max_spread,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| HarmonizationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| HarmonizationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-vector+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn vector(
        id: &str,
        sample: &str,
        modality: GliomaModality,
        batch: &str,
        offset: i64,
    ) -> HarmonizationVector {
        HarmonizationVector {
            observation_id: id.into(),
            study_id: "study".into(),
            sample_lineage: sample.into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            batch_id: batch.into(),
            artifact: artifact(id),
            features: vec![
                FeatureValue {
                    feature_id: "f1".into(),
                    value_milli: 100 + offset,
                },
                FeatureValue {
                    feature_id: "f2".into(),
                    value_milli: 200 + offset,
                },
            ],
        }
    }

    fn request() -> HarmonizationRequest {
        HarmonizationRequest {
            study_id: "study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            reference_batch: Some("b1".into()),
            min_vectors_per_batch: 2,
            min_shared_features: 2,
            max_correction_milli: 100,
            max_post_harmonization_spread_milli: 20,
        }
    }

    #[test]
    fn median_centering_removes_batch_shift_and_is_replay_stable() {
        let vectors = vec![
            vector("v1", "s1", GliomaModality::Genomics, "b1", 0),
            vector("v2", "s2", GliomaModality::Genomics, "b1", 10),
            vector("v3", "s3", GliomaModality::Genomics, "b2", 50),
            vector("v4", "s4", GliomaModality::Genomics, "b2", 60),
        ];
        let first = harmonize_glioma_multimodal_batches(&request(), &vectors).unwrap();
        let second = harmonize_glioma_multimodal_batches(&request(), &vectors).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, HarmonizationDisposition::Qualified);
        assert_eq!(first.max_correction_milli, 50);
        let b2 = first
            .corrected_vectors
            .iter()
            .find(|vector| vector.batch_id == "b2")
            .unwrap();
        assert_eq!(b2.features[0].corrected_milli, 100);
    }

    #[test]
    fn missing_modality_is_unresolved_without_imputation() {
        let mut request = request();
        request.required_modalities =
            BTreeSet::from([GliomaModality::Genomics, GliomaModality::Transcriptomics]);
        let vectors = vec![
            vector("v1", "s1", GliomaModality::Genomics, "b1", 0),
            vector("v2", "s2", GliomaModality::Genomics, "b1", 10),
        ];
        let output = harmonize_glioma_multimodal_batches(&request, &vectors).unwrap();
        assert_eq!(output.disposition, HarmonizationDisposition::Unresolved);
        assert!(output
            .uncertainty
            .contains(&"required-modality-coverage-incomplete".to_string()));
    }

    #[test]
    fn correction_cap_is_partial_and_explicit() {
        let mut request = request();
        request.max_correction_milli = 10;
        let vectors = vec![
            vector("v1", "s1", GliomaModality::Genomics, "b1", 0),
            vector("v2", "s2", GliomaModality::Genomics, "b1", 10),
            vector("v3", "s3", GliomaModality::Genomics, "b2", 50),
            vector("v4", "s4", GliomaModality::Genomics, "b2", 60),
        ];
        let output = harmonize_glioma_multimodal_batches(&request, &vectors).unwrap();
        assert_eq!(output.disposition, HarmonizationDisposition::Partial);
        assert!(output
            .negative_evidence
            .contains(&"batch-correction-exceeds-declared-bound".to_string()));
    }
}
