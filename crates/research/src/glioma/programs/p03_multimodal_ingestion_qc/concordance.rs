//! Feature-level concordance analysis for preclinical glioma modalities.
//!
//! Metadata QC answers whether modality objects are comparable; this feature answers whether
//! their measured feature vectors agree on the declared sample.  It aligns feature identifiers,
//! computes a fixed-point Pearson-style correlation with integer square roots, and reports
//! concordant, contradictory, or unresolved pairs.  Missing modalities, insufficient overlap,
//! and zero-variance vectors are never silently imputed.  The input boundary remains local,
//! de-identified, and preclinical.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalConcordance1@1";
pub const MAX_FEATURES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureValue {
    pub feature_id: String,
    pub value_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityVector {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub features: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcordanceRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub min_shared_features: usize,
    pub min_correlation_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairConcordanceDisposition {
    Concordant,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityConcordance {
    pub left_modality: GliomaModality,
    pub right_modality: GliomaModality,
    pub shared_feature_order: Vec<String>,
    pub correlation_milli: i64,
    pub absolute_correlation_milli: u16,
    pub disposition: PairConcordanceDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcordanceDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalConcordance {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub pairs: Vec<ModalityConcordance>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ConcordanceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConcordanceError {
    #[error("concordance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("concordance vector is invalid: {0}")]
    InvalidVector(String),
    #[error("concordance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("concordance digest failed: {0}")]
    Digest(String),
}

fn integer_sqrt(value: u128) -> u128 {
    let mut low = 0_u128;
    let mut high = value.saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid.max(1) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn digest_input(output: &MultimodalConcordance) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "sample_lineage": output.sample_lineage,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "pairs": output.pairs,
        "missing_modality_order": output.missing_modality_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MultimodalConcordance {
    pub fn validate(&self) -> Result<(), ConcordanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.sample_lineage.trim().is_empty()
            || self
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .missing_modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.pairs.windows(2).any(|pair| {
                (pair[0].left_modality, pair[0].right_modality)
                    >= (pair[1].left_modality, pair[1].right_modality)
            })
            || self.pairs.iter().any(|pair| {
                pair.left_modality >= pair.right_modality
                    || pair
                        .shared_feature_order
                        .windows(2)
                        .any(|items| items[0] >= items[1])
                    || pair.absolute_correlation_milli > 1_000
                    || pair.correlation_milli.abs() > 1_000
                    || pair
                        .negative_evidence
                        .windows(2)
                        .any(|items| items[0] >= items[1])
                    || pair
                        .uncertainty
                        .windows(2)
                        .any(|items| items[0] >= items[1])
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ConcordanceError::InvalidOutput(
                "identity, modality/pair ordering, correlation bounds, or ordering is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ConcordanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ConcordanceError::InvalidOutput(
                "digest is not bound to the multimodal concordance".into(),
            ));
        }
        Ok(())
    }
}

fn pair_concordance(
    left: &ModalityVector,
    right: &ModalityVector,
    request: &ConcordanceRequest,
) -> ModalityConcordance {
    let left_values = left
        .features
        .iter()
        .map(|feature| (feature.feature_id.as_str(), feature.value_milli))
        .collect::<BTreeMap<_, _>>();
    let right_values = right
        .features
        .iter()
        .map(|feature| (feature.feature_id.as_str(), feature.value_milli))
        .collect::<BTreeMap<_, _>>();
    let shared_feature_order = left_values
        .keys()
        .filter(|feature| right_values.contains_key(*feature))
        .map(|feature| (*feature).to_string())
        .collect::<Vec<_>>();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if shared_feature_order.len() < request.min_shared_features {
        uncertainty.insert("minimum-shared-feature-floor-not-met".into());
    }
    let left_mean = if shared_feature_order.is_empty() {
        0
    } else {
        shared_feature_order
            .iter()
            .map(|feature| left_values[feature.as_str()] as i128)
            .sum::<i128>()
            / shared_feature_order.len() as i128
    };
    let right_mean = if shared_feature_order.is_empty() {
        0
    } else {
        shared_feature_order
            .iter()
            .map(|feature| right_values[feature.as_str()] as i128)
            .sum::<i128>()
            / shared_feature_order.len() as i128
    };
    let mut numerator = 0_i128;
    let mut left_variance = 0_u128;
    let mut right_variance = 0_u128;
    for feature in &shared_feature_order {
        let left_delta = left_values[feature.as_str()] as i128 - left_mean;
        let right_delta = right_values[feature.as_str()] as i128 - right_mean;
        numerator = numerator.saturating_add(left_delta.saturating_mul(right_delta));
        left_variance = left_variance.saturating_add(
            left_delta
                .unsigned_abs()
                .saturating_mul(left_delta.unsigned_abs()),
        );
        right_variance = right_variance.saturating_add(
            right_delta
                .unsigned_abs()
                .saturating_mul(right_delta.unsigned_abs()),
        );
    }
    let denominator = integer_sqrt(left_variance).saturating_mul(integer_sqrt(right_variance));
    let correlation_milli = if denominator == 0 {
        uncertainty.insert("zero-variance-modality-vector".into());
        0
    } else {
        (numerator.saturating_mul(1_000) / denominator as i128).clamp(-1_000, 1_000) as i64
    };
    let absolute_correlation_milli = correlation_milli.unsigned_abs().min(1_000) as u16;
    if denominator != 0 && correlation_milli < request.min_correlation_milli as i64 {
        negative.insert("modality-pair-correlation-below-threshold".into());
    }
    let disposition =
        if shared_feature_order.len() < request.min_shared_features || denominator == 0 {
            PairConcordanceDisposition::Unresolved
        } else if correlation_milli < request.min_correlation_milli as i64 {
            PairConcordanceDisposition::Contradictory
        } else {
            PairConcordanceDisposition::Concordant
        };
    ModalityConcordance {
        left_modality: left.modality,
        right_modality: right.modality,
        shared_feature_order,
        correlation_milli,
        absolute_correlation_milli,
        disposition,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
    }
}

/// Compare all required modality vectors for one preclinical sample lineage.
pub fn analyze_multimodal_concordance(
    request: &ConcordanceRequest,
    vectors: &[ModalityVector],
) -> Result<MultimodalConcordance, ConcordanceError> {
    if request.study_id.trim().is_empty()
        || request.required_modalities.len() < 2
        || request.min_shared_features == 0
        || request.min_correlation_milli > 1_000
        || vectors.is_empty()
    {
        return Err(ConcordanceError::InvalidRequest(
            "study, at least two required modalities, overlap floor, correlation bound, and vectors are required".into(),
        ));
    }
    let mut by_modality = BTreeMap::<GliomaModality, &ModalityVector>::new();
    let mut sample_lineage = None;
    for vector in vectors {
        vector
            .artifact
            .validate()
            .map_err(|error| ConcordanceError::InvalidVector(error.to_string()))?;
        if vector.observation_id.trim().is_empty()
            || vector.study_id != request.study_id
            || vector.sample_lineage.trim().is_empty()
            || vector.model_system != request.model_system
            || vector.features.is_empty()
            || vector.features.len() > MAX_FEATURES
            || !vector.features.iter().all(|feature| {
                !feature.feature_id.trim().is_empty()
                    && feature.value_milli.unsigned_abs() <= 1_000_000_000
            })
            || vector
                .features
                .windows(2)
                .any(|pair| pair[0].feature_id >= pair[1].feature_id)
            || vector
                .features
                .windows(2)
                .any(|pair| pair[0].feature_id == pair[1].feature_id)
            || by_modality.insert(vector.modality, vector).is_some()
        {
            return Err(ConcordanceError::InvalidVector(
                "vector identity, study/model binding, feature ordering, value bound, or modality uniqueness is invalid".into(),
            ));
        }
        if let Some(existing) = &sample_lineage {
            if existing != &vector.sample_lineage {
                return Err(ConcordanceError::InvalidVector(
                    "all modality vectors must share one sample lineage".into(),
                ));
            }
        } else {
            sample_lineage = Some(vector.sample_lineage.clone());
        }
    }
    let modality_order = by_modality.keys().copied().collect::<Vec<_>>();
    let missing_modality_order = request
        .required_modalities
        .difference(&by_modality.keys().copied().collect())
        .copied()
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (index, left_modality) in modality_order.iter().enumerate() {
        for right_modality in modality_order.iter().skip(index + 1) {
            pairs.push(pair_concordance(
                by_modality[left_modality],
                by_modality[right_modality],
                request,
            ));
        }
    }
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &missing_modality_order {
        negative.insert(format!("required-modality-missing:{modality:?}"));
    }
    if !missing_modality_order.is_empty() {
        uncertainty.insert("required-modality-coverage-incomplete".into());
    }
    if pairs
        .iter()
        .any(|pair| pair.disposition == PairConcordanceDisposition::Contradictory)
    {
        negative.insert("one-or-more-modality-pairs-contradictory".into());
    }
    if pairs
        .iter()
        .any(|pair| pair.disposition == PairConcordanceDisposition::Unresolved)
    {
        uncertainty.insert("one-or-more-modality-pairs-unresolved".into());
    }
    let disposition = if !missing_modality_order.is_empty()
        || pairs
            .iter()
            .any(|pair| pair.disposition == PairConcordanceDisposition::Unresolved)
    {
        ConcordanceDisposition::Unresolved
    } else if pairs
        .iter()
        .any(|pair| pair.disposition == PairConcordanceDisposition::Contradictory)
    {
        ConcordanceDisposition::Partial
    } else {
        ConcordanceDisposition::Qualified
    };
    let mut output = MultimodalConcordance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        sample_lineage: sample_lineage.unwrap_or_default(),
        model_system: request.model_system,
        modality_order,
        pairs,
        missing_modality_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| ConcordanceError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ConcordanceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(modality: GliomaModality, values: &[i64]) -> ModalityVector {
        ModalityVector {
            observation_id: format!("{modality:?}"),
            study_id: "study-1".into(),
            sample_lineage: "sample-1".into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{modality:?}"),
                content_hash: ContentHash::of_value(
                    &serde_json::json!({"modality": format!("{modality:?}")}),
                )
                .unwrap(),
                content_type: "application/vnd.aurora.glioma-vector+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            features: values
                .iter()
                .enumerate()
                .map(|(index, value)| FeatureValue {
                    feature_id: format!("feature-{index:03}"),
                    value_milli: *value,
                })
                .collect(),
        }
    }

    fn request() -> ConcordanceRequest {
        ConcordanceRequest {
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: BTreeSet::from([
                GliomaModality::Genomics,
                GliomaModality::Transcriptomics,
            ]),
            min_shared_features: 3,
            min_correlation_milli: 900,
        }
    }

    #[test]
    fn concordant_vectors_are_qualified_and_replay_stable() {
        let vectors = vec![
            vector(GliomaModality::Genomics, &[1, 2, 3]),
            vector(GliomaModality::Transcriptomics, &[10, 20, 30]),
        ];
        let first = analyze_multimodal_concordance(&request(), &vectors).unwrap();
        let second = analyze_multimodal_concordance(&request(), &vectors).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ConcordanceDisposition::Qualified);
        assert_eq!(first.pairs[0].correlation_milli, 1_000);
        first.validate().unwrap();
    }

    #[test]
    fn contradictory_vectors_are_partial_not_silently_repaired() {
        let output = analyze_multimodal_concordance(
            &request(),
            &[
                vector(GliomaModality::Genomics, &[1, 2, 3]),
                vector(GliomaModality::Transcriptomics, &[30, 20, 10]),
            ],
        )
        .unwrap();
        assert_eq!(output.disposition, ConcordanceDisposition::Partial);
        assert_eq!(output.pairs[0].correlation_milli, -1_000);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradictory")));
    }

    #[test]
    fn missing_required_modality_is_unresolved() {
        let output = analyze_multimodal_concordance(
            &request(),
            &[vector(GliomaModality::Genomics, &[1, 2, 3])],
        )
        .unwrap();
        assert_eq!(output.disposition, ConcordanceDisposition::Unresolved);
        assert_eq!(
            output.missing_modality_order,
            vec![GliomaModality::Transcriptomics]
        );
    }
}
