//! Deterministic multimodal latent-state factorization for preclinical glioma studies.
//!
//! The factorizer is intentionally complete-case and value-only. It robustly median-centers and
//! MAD-scales modality features, extracts orthogonal components with a deterministic fixed-point
//! power iteration, and reports explained variance, reconstruction error, convergence, and the
//! coverage it had to omit. Missing values are never imputed and the result is suitable as a
//! typed input to mechanism, experiment-design, or replication workflows.

use super::concordance::FeatureValue;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaLatentFactorization1@1";
pub const MAX_VECTORS: usize = 4_096;
pub const MAX_FEATURES_PER_VECTOR: usize = 16_384;
pub const MAX_COLUMNS: usize = 512;
pub const MAX_COMPONENTS: usize = 16;
const VECTOR_SCALE: i128 = 1_000_000;
const Z_LIMIT: i128 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentFactorRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub min_complete_samples: usize,
    pub min_shared_features: usize,
    pub components: usize,
    pub max_iterations: usize,
    pub convergence_tolerance_milli: u64,
    pub min_explained_variance_milli: u64,
    pub max_reconstruction_error_milli: u64,
    pub require_all_modalities: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentFactorVector {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub features: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentLoading {
    pub column_id: String,
    pub loading_milli: i64,
    pub absolute_loading_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentScore {
    pub sample_lineage: String,
    pub score_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentFactorComponent {
    pub component_index: usize,
    pub loading_order: Vec<String>,
    pub loadings: Vec<LatentLoading>,
    pub score_order: Vec<String>,
    pub scores: Vec<LatentScore>,
    pub explained_variance_milli: u64,
    pub reconstruction_error_milli: u64,
    pub convergence_iterations: usize,
    pub converged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatentFactorDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentFactorAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modality_order: Vec<GliomaModality>,
    pub sample_order: Vec<String>,
    pub column_order: Vec<String>,
    pub components: Vec<LatentFactorComponent>,
    pub total_explained_variance_milli: u64,
    pub reconstruction_error_milli: u64,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_feature_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: LatentFactorDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LatentFactorError {
    #[error("latent factor request is invalid: {0}")]
    InvalidRequest(String),
    #[error("latent factor vector is invalid: {0}")]
    InvalidVector(String),
    #[error("latent factor output is invalid: {0}")]
    InvalidOutput(String),
    #[error("latent factor digest failed: {0}")]
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

fn column_id(modality: GliomaModality, feature_id: &str) -> String {
    format!("{}::{feature_id}", modality_label(modality))
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

fn clamp_i128(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn abs_u64(value: i128) -> u64 {
    value.unsigned_abs().min(u128::from(u64::MAX)) as u64
}

fn digest_input(output: &LatentFactorAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "required_modality_order": output.required_modality_order,
        "sample_order": output.sample_order,
        "column_order": output.column_order,
        "components": output.components,
        "total_explained_variance_milli": output.total_explained_variance_milli,
        "reconstruction_error_milli": output.reconstruction_error_milli,
        "missing_modality_order": output.missing_modality_order,
        "missing_feature_order": output.missing_feature_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl LatentFactorAnalysis {
    pub fn validate(&self) -> Result<(), LatentFactorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || !ordered_unique(&self.required_modality_order)
            || !ordered_unique(&self.sample_order)
            || !ordered_unique(&self.column_order)
            || !ordered_unique(&self.missing_modality_order)
            || !ordered_unique(&self.missing_feature_order)
            || !ordered_unique(&self.negative_evidence)
            || !ordered_unique(&self.uncertainty)
            || self.total_explained_variance_milli > 1_000
            || self
                .components
                .windows(2)
                .any(|pair| pair[0].component_index >= pair[1].component_index)
            || self.components.iter().any(|component| {
                component.loading_order != self.column_order
                    || component.score_order != self.sample_order
                    || !ordered_unique(&component.loading_order)
                    || !ordered_unique(&component.score_order)
                    || component.loadings.len() != component.loading_order.len()
                    || component.scores.len() != component.score_order.len()
                    || component
                        .loadings
                        .iter()
                        .any(|loading| loading.absolute_loading_milli > 1_000)
                    || component
                        .loadings
                        .windows(2)
                        .any(|pair| pair[0].column_id >= pair[1].column_id)
                    || component
                        .scores
                        .windows(2)
                        .any(|pair| pair[0].sample_lineage >= pair[1].sample_lineage)
            })
        {
            return Err(LatentFactorError::InvalidOutput(
                "identity, ordering, component bounds, or latent partitions are invalid".into(),
            ));
        }
        if self
            .components
            .iter()
            .enumerate()
            .any(|(index, component)| component.component_index != index)
        {
            return Err(LatentFactorError::InvalidOutput(
                "component indices are not contiguous".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LatentFactorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LatentFactorError::InvalidOutput(
                "digest is not bound to latent factor analysis".into(),
            ));
        }
        Ok(())
    }
}

fn build_covariance(matrix: &[Vec<i128>], columns: usize) -> Vec<Vec<i128>> {
    let mut covariance = vec![vec![0_i128; columns]; columns];
    for row in matrix {
        for (left, left_value) in row.iter().enumerate().take(columns) {
            if *left_value == 0 {
                continue;
            }
            for (right, right_value) in row.iter().enumerate().skip(left).take(columns - left) {
                let contribution = left_value.saturating_mul(*right_value);
                covariance[left][right] = covariance[left][right].saturating_add(contribution);
                if right != left {
                    covariance[right][left] = covariance[right][left].saturating_add(contribution);
                }
            }
        }
    }
    covariance
}

fn power_component(
    covariance: &[Vec<i128>],
    max_iterations: usize,
    tolerance_milli: u64,
) -> (Vec<i128>, usize, bool) {
    let columns = covariance.len();
    let seed = (0..columns)
        .max_by_key(|index| covariance[*index][*index])
        .unwrap_or(0);
    if columns == 0 || covariance[seed][seed] <= 0 {
        return (vec![0; columns], 0, false);
    }
    let mut vector = vec![0_i128; columns];
    vector[seed] = VECTOR_SCALE;
    let mut converged = false;
    let mut iterations = 0;
    for iteration in 1..=max_iterations {
        let mut projected = vec![0_i128; columns];
        for left in 0..columns {
            projected[left] = covariance[left]
                .iter()
                .zip(&vector)
                .map(|(covariance, weight)| covariance.saturating_mul(*weight) / VECTOR_SCALE)
                .fold(0_i128, i128::saturating_add);
        }
        let norm = projected
            .iter()
            .map(|value| value.unsigned_abs())
            .fold(0_u128, u128::saturating_add);
        if norm == 0 {
            iterations = iteration;
            break;
        }
        let mut next = projected
            .iter()
            .map(|value| {
                value
                    .saturating_mul(VECTOR_SCALE)
                    .checked_div(i128::try_from(norm).unwrap_or(i128::MAX))
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        let pivot = next
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| value.unsigned_abs())
            .map(|(index, _)| index)
            .unwrap_or(0);
        if next[pivot] < 0 {
            for value in &mut next {
                *value = value.saturating_neg();
            }
        }
        let delta = next
            .iter()
            .zip(&vector)
            .map(|(next, current)| next.saturating_sub(*current).unsigned_abs())
            .max()
            .unwrap_or(0)
            .saturating_mul(1_000)
            / u128::try_from(VECTOR_SCALE).unwrap_or(u128::MAX);
        vector = next;
        iterations = iteration;
        if delta <= u128::from(tolerance_milli) {
            converged = true;
            break;
        }
    }
    (vector, iterations, converged)
}

fn mean_absolute_residual(matrix: &[Vec<i128>], columns: usize) -> u64 {
    if matrix.is_empty() || columns == 0 {
        return 0;
    }
    let total = matrix
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value.unsigned_abs())
        .fold(0_u128, u128::saturating_add);
    (total / u128::try_from(matrix.len().saturating_mul(columns)).unwrap_or(1))
        .min(u128::from(u64::MAX)) as u64
}

pub fn analyze_glioma_latent_factors(
    request: &LatentFactorRequest,
    vectors: &[LatentFactorVector],
) -> Result<LatentFactorAnalysis, LatentFactorError> {
    if request.study_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.min_complete_samples < 2
        || request.min_shared_features == 0
        || request.components == 0
        || request.components > MAX_COMPONENTS
        || request.max_iterations == 0
        || request.convergence_tolerance_milli > 1_000
        || request.min_explained_variance_milli > 1_000
        || request.max_reconstruction_error_milli > i64::MAX as u64
        || vectors.is_empty()
        || vectors.len() > MAX_VECTORS
    {
        return Err(LatentFactorError::InvalidRequest(
            "study, modality coverage, positive sample/feature/component floors, iteration and metric bounds are required".into(),
        ));
    }
    let mut observations = BTreeSet::new();
    let mut sample_modalities = BTreeSet::new();
    let mut by_sample_modality = BTreeMap::<(String, GliomaModality), BTreeMap<String, i64>>::new();
    let mut sample_to_modalities = BTreeMap::<String, BTreeSet<GliomaModality>>::new();
    for vector in vectors {
        if vector.observation_id.trim().is_empty()
            || vector.study_id != request.study_id
            || vector.sample_lineage.trim().is_empty()
            || vector.model_system != request.model_system
            || vector.artifact.validate().is_err()
            || !vector.artifact.local_only
            || vector.artifact.contains_human_data
            || vector.artifact.contains_direct_identifiers
            || vector.features.is_empty()
            || vector.features.len() > MAX_FEATURES_PER_VECTOR
            || !observations.insert(vector.observation_id.clone())
            || !sample_modalities.insert((vector.sample_lineage.clone(), vector.modality))
        {
            return Err(LatentFactorError::InvalidVector(
                "vector identity, study/model binding, local privacy posture, feature bound, or sample/modality uniqueness is invalid".into(),
            ));
        }
        let mut feature_map = BTreeMap::new();
        for feature in &vector.features {
            if feature.feature_id.trim().is_empty()
                || feature_map
                    .insert(feature.feature_id.clone(), feature.value_milli)
                    .is_some()
            {
                return Err(LatentFactorError::InvalidVector(
                    "feature identity must be non-empty and unique per vector".into(),
                ));
            }
        }
        by_sample_modality.insert(
            (vector.sample_lineage.clone(), vector.modality),
            feature_map,
        );
        sample_to_modalities
            .entry(vector.sample_lineage.clone())
            .or_default()
            .insert(vector.modality);
    }
    let required_modality_order = request
        .required_modalities
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let missing_modality_order = required_modality_order
        .iter()
        .copied()
        .filter(|modality| {
            sample_to_modalities
                .values()
                .filter(|modalities| modalities.contains(modality))
                .count()
                < request.min_complete_samples
        })
        .collect::<Vec<_>>();
    let complete_samples = sample_to_modalities
        .iter()
        .filter(|(_, modalities)| {
            required_modality_order
                .iter()
                .all(|modality| modalities.contains(modality))
        })
        .map(|(sample, _)| sample.clone())
        .collect::<Vec<_>>();
    let mut all_columns = BTreeSet::<(GliomaModality, String)>::new();
    for sample in &complete_samples {
        for modality in &required_modality_order {
            if let Some(features) = by_sample_modality.get(&(sample.clone(), *modality)) {
                all_columns.extend(features.keys().cloned().map(|feature| (*modality, feature)));
            }
        }
    }
    let shared_columns = all_columns
        .iter()
        .filter(|(modality, feature)| {
            complete_samples.iter().all(|sample| {
                by_sample_modality
                    .get(&(sample.clone(), *modality))
                    .is_some_and(|features| features.contains_key(feature))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_feature_order = all_columns
        .iter()
        .filter(|column| !shared_columns.contains(column))
        .map(|(modality, feature)| column_id(*modality, feature))
        .collect::<Vec<_>>();
    let column_order = shared_columns
        .iter()
        .map(|(modality, feature)| column_id(*modality, feature))
        .collect::<Vec<_>>();
    let mut matrix = Vec::<Vec<i128>>::new();
    for sample in &complete_samples {
        let row = shared_columns
            .iter()
            .map(|(modality, feature)| {
                by_sample_modality
                    .get(&(sample.clone(), *modality))
                    .and_then(|features| features.get(feature))
                    .copied()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        matrix.push(row.into_iter().map(i128::from).collect());
    }
    for column in 0..shared_columns.len() {
        let mut values = matrix
            .iter()
            .map(|row| clamp_i128(row[column]))
            .collect::<Vec<_>>();
        let center = median(&mut values);
        let mut deviations = matrix
            .iter()
            .map(|row| {
                (row[column]
                    .saturating_sub(i128::from(center))
                    .unsigned_abs()
                    .min(i128::from(i64::MAX) as u128)) as i64
            })
            .collect::<Vec<_>>();
        let mad = i128::from(median(&mut deviations));
        let scale = (mad.saturating_mul(1_483) / 1_000).max(1);
        for row in &mut matrix {
            row[column] = (row[column]
                .saturating_sub(i128::from(center))
                .saturating_mul(1_000)
                .checked_div(scale)
                .unwrap_or(0))
            .clamp(-Z_LIMIT, Z_LIMIT);
        }
    }
    let initial_energy = matrix
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value.saturating_mul(*value))
        .fold(0_i128, i128::saturating_add);
    let component_count = request.components.min(shared_columns.len());
    let mut residual = matrix.clone();
    let mut components = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for component_index in 0..component_count {
        let covariance = build_covariance(&residual, shared_columns.len());
        let (weights, iterations, converged) = power_component(
            &covariance,
            request.max_iterations,
            request.convergence_tolerance_milli,
        );
        if weights.iter().all(|weight| *weight == 0) {
            negative_evidence.insert("zero-residual-variance-blocked-component".into());
            break;
        }
        if !converged {
            negative_evidence.insert("latent-component-did-not-converge".into());
        }
        let weight_norm2 = weights
            .iter()
            .map(|weight| weight.saturating_mul(*weight))
            .fold(0_i128, i128::saturating_add)
            .max(1);
        let raw_scores = residual
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&weights)
                    .map(|(value, weight)| value.saturating_mul(*weight) / VECTOR_SCALE)
                    .fold(0_i128, i128::saturating_add)
            })
            .collect::<Vec<_>>();
        let scores = raw_scores
            .iter()
            .map(|score| {
                score
                    .saturating_mul(VECTOR_SCALE)
                    .saturating_mul(VECTOR_SCALE)
                    .checked_div(weight_norm2)
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        let component_energy = raw_scores
            .iter()
            .map(|score| {
                score
                    .saturating_mul(*score)
                    .saturating_mul(VECTOR_SCALE)
                    .saturating_mul(VECTOR_SCALE)
                    .checked_div(weight_norm2)
                    .unwrap_or(0)
            })
            .fold(0_i128, i128::saturating_add);
        let explained = if initial_energy <= 0 {
            0
        } else {
            (component_energy
                .saturating_mul(1_000)
                .checked_div(initial_energy)
                .unwrap_or(0))
            .clamp(0, 1_000) as u64
        };
        for (row, score) in residual.iter_mut().zip(&scores) {
            for (value, weight) in row.iter_mut().zip(&weights) {
                *value = value.saturating_sub(score.saturating_mul(*weight) / VECTOR_SCALE);
            }
        }
        let reconstruction_error = mean_absolute_residual(&residual, shared_columns.len());
        let loadings = column_order
            .iter()
            .zip(&weights)
            .map(|(column, weight)| {
                let loading = clamp_i128(weight.saturating_mul(1_000) / VECTOR_SCALE);
                LatentLoading {
                    column_id: column.clone(),
                    loading_milli: loading,
                    absolute_loading_milli: abs_u64(i128::from(loading)),
                }
            })
            .collect::<Vec<_>>();
        let latent_scores = complete_samples
            .iter()
            .zip(&scores)
            .map(|(sample, score)| LatentScore {
                sample_lineage: sample.clone(),
                score_milli: clamp_i128(*score),
            })
            .collect::<Vec<_>>();
        components.push(LatentFactorComponent {
            component_index,
            loading_order: column_order.clone(),
            loadings,
            score_order: complete_samples.clone(),
            scores: latent_scores,
            explained_variance_milli: explained,
            reconstruction_error_milli: reconstruction_error,
            convergence_iterations: iterations,
            converged,
        });
    }
    if complete_samples.len() < request.min_complete_samples {
        uncertainty.insert("complete-sample-floor-not-met-without-imputation".into());
    }
    if shared_columns.len() < request.min_shared_features {
        uncertainty.insert("shared-feature-floor-not-met-without-imputation".into());
    }
    if !missing_modality_order.is_empty() {
        uncertainty.insert("required-modality-coverage-incomplete".into());
    }
    if !missing_feature_order.is_empty() {
        uncertainty.insert("features-omitted-from-complete-case-matrix".into());
    }
    if initial_energy == 0 {
        negative_evidence.insert("zero-total-variance-in-complete-case-matrix".into());
    }
    let total_explained = components
        .iter()
        .map(|component| component.explained_variance_milli)
        .sum::<u64>()
        .min(1_000);
    let reconstruction_error = mean_absolute_residual(&residual, shared_columns.len());
    if total_explained < request.min_explained_variance_milli {
        uncertainty.insert("explained-variance-floor-not-met".into());
    }
    if reconstruction_error > request.max_reconstruction_error_milli {
        negative_evidence.insert("reconstruction-error-exceeds-declared-bound".into());
    }
    let disposition = if complete_samples.len() < request.min_complete_samples
        || shared_columns.len() < request.min_shared_features
        || components.is_empty()
        || (request.require_all_modalities && !missing_modality_order.is_empty())
    {
        LatentFactorDisposition::Unresolved
    } else if !negative_evidence.is_empty()
        || !uncertainty.is_empty()
        || total_explained < request.min_explained_variance_milli
        || reconstruction_error > request.max_reconstruction_error_milli
    {
        LatentFactorDisposition::Partial
    } else {
        LatentFactorDisposition::Qualified
    };
    let mut output = LatentFactorAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        required_modality_order,
        sample_order: complete_samples,
        column_order,
        components,
        total_explained_variance_milli: total_explained,
        reconstruction_error_milli: reconstruction_error,
        missing_modality_order,
        missing_feature_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| LatentFactorError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LatentFactorError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("latent-artifact-{id}"),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-latent-vector+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn vector(
        observation_id: &str,
        sample: &str,
        modality: GliomaModality,
        values: &[(&str, i64)],
    ) -> LatentFactorVector {
        LatentFactorVector {
            observation_id: observation_id.into(),
            study_id: "study".into(),
            sample_lineage: sample.into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: artifact(observation_id),
            features: values
                .iter()
                .map(|(feature_id, value_milli)| FeatureValue {
                    feature_id: (*feature_id).into(),
                    value_milli: *value_milli,
                })
                .collect(),
        }
    }

    fn request() -> LatentFactorRequest {
        LatentFactorRequest {
            study_id: "study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: BTreeSet::from([
                GliomaModality::Genomics,
                GliomaModality::Imaging,
            ]),
            min_complete_samples: 3,
            min_shared_features: 2,
            components: 1,
            max_iterations: 100,
            convergence_tolerance_milli: 1,
            min_explained_variance_milli: 500,
            max_reconstruction_error_milli: 500,
            require_all_modalities: true,
        }
    }

    #[test]
    fn extracts_replay_stable_shared_latent_state() {
        let vectors = vec![
            vector(
                "g1",
                "s1",
                GliomaModality::Genomics,
                &[("x", 100), ("y", 10)],
            ),
            vector("i1", "s1", GliomaModality::Imaging, &[("x", 50), ("y", 5)]),
            vector(
                "g2",
                "s2",
                GliomaModality::Genomics,
                &[("x", 200), ("y", 20)],
            ),
            vector(
                "i2",
                "s2",
                GliomaModality::Imaging,
                &[("x", 100), ("y", 10)],
            ),
            vector(
                "g3",
                "s3",
                GliomaModality::Genomics,
                &[("x", 300), ("y", 30)],
            ),
            vector(
                "i3",
                "s3",
                GliomaModality::Imaging,
                &[("x", 150), ("y", 15)],
            ),
        ];
        let first = analyze_glioma_latent_factors(&request(), &vectors).unwrap();
        let second = analyze_glioma_latent_factors(&request(), &vectors).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, LatentFactorDisposition::Qualified);
        assert_eq!(first.components.len(), 1);
        assert!(first.total_explained_variance_milli >= 900);
        assert!(first.reconstruction_error_milli <= 500);
    }

    #[test]
    fn missing_modality_is_unresolved_without_imputation() {
        let mut vectors = vec![
            vector(
                "g1",
                "s1",
                GliomaModality::Genomics,
                &[("x", 100), ("y", 10)],
            ),
            vector(
                "g2",
                "s2",
                GliomaModality::Genomics,
                &[("x", 200), ("y", 20)],
            ),
            vector(
                "g3",
                "s3",
                GliomaModality::Genomics,
                &[("x", 300), ("y", 30)],
            ),
        ];
        vectors.push(vector(
            "i1",
            "s1",
            GliomaModality::Imaging,
            &[("x", 50), ("y", 5)],
        ));
        let output = analyze_glioma_latent_factors(&request(), &vectors).unwrap();
        assert_eq!(output.disposition, LatentFactorDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|reason| reason == "required-modality-coverage-incomplete"));
    }

    #[test]
    fn feature_omission_is_visible_and_replay_stable() {
        let mut vectors = vec![
            vector(
                "g1",
                "s1",
                GliomaModality::Genomics,
                &[("x", 100), ("y", 10)],
            ),
            vector("i1", "s1", GliomaModality::Imaging, &[("x", 50), ("y", 5)]),
            vector(
                "g2",
                "s2",
                GliomaModality::Genomics,
                &[("x", 200), ("y", 20)],
            ),
            vector("i2", "s2", GliomaModality::Imaging, &[("x", 100)]),
            vector(
                "g3",
                "s3",
                GliomaModality::Genomics,
                &[("x", 300), ("y", 30)],
            ),
            vector(
                "i3",
                "s3",
                GliomaModality::Imaging,
                &[("x", 150), ("y", 15)],
            ),
        ];
        let mut request = request();
        request.min_shared_features = 2;
        request.require_all_modalities = false;
        let output = analyze_glioma_latent_factors(&request, &vectors).unwrap();
        assert_eq!(output.disposition, LatentFactorDisposition::Partial);
        assert!(output
            .missing_feature_order
            .iter()
            .any(|feature| feature == "imaging::y"));
        vectors.reverse();
        let replay = analyze_glioma_latent_factors(&request, &vectors).unwrap();
        assert_eq!(output.digest, replay.digest);
    }
}
