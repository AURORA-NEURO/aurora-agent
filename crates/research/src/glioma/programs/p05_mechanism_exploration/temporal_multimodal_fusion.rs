//! Temporal and multimodal mechanism fusion for preclinical glioma research.
//!
//! This composition capability reconciles mechanism predictions with local, de-identified
//! observations across modality and time. It is deliberately stricter than a one-shot fit:
//! quality, source independence, missingness, temporal coverage, and contradiction are scored
//! separately, and the output includes a bounded next-measurement frontier. It never invents an
//! observation, converts association into a clinical claim, or moves raw artifact bytes.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-temporal-multimodal-mechanism-fusion";
pub const OUTPUT_SCHEMA: &str = "GliomaTemporalMultimodalMechanismFusion1@1";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_PREDICTIONS_PER_MECHANISM: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_NEXT_ACTIONS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMechanismPrediction {
    pub feature_id: String,
    pub modality: GliomaModality,
    pub timepoint: u32,
    pub expected_milli: i64,
    pub uncertainty_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMechanismHypothesis {
    pub mechanism_id: String,
    pub statement: String,
    pub prior_milli: u16,
    pub predictions: Vec<TemporalMechanismPrediction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMechanismObservation {
    pub feature_id: String,
    pub modality: GliomaModality,
    pub timepoint: u32,
    pub observed_milli: i64,
    pub measurement_uncertainty_milli: u32,
    pub qc_milli: u16,
    pub source_id: String,
    pub independence_group: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMultimodalMechanismFusionRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<TemporalMechanismHypothesis>,
    pub observations: Vec<TemporalMechanismObservation>,
    pub max_next_actions: usize,
    pub min_support_milli: u16,
    pub contradiction_threshold_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalMechanismDisposition {
    Supported,
    Contradicted,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalMultimodalFusionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMechanismAssessment {
    pub mechanism_id: String,
    pub matched_prediction_order: Vec<String>,
    pub missing_prediction_order: Vec<String>,
    pub source_group_count: u16,
    pub modality_coverage: Vec<GliomaModality>,
    pub timepoint_order: Vec<u32>,
    pub coverage_milli: u16,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub posterior_milli: u16,
    pub disposition: TemporalMechanismDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalAcquisitionCandidate {
    pub action_id: String,
    pub feature_id: String,
    pub modality: GliomaModality,
    pub timepoint: u32,
    pub mechanism_order: Vec<String>,
    pub expected_separation_milli: u64,
    pub expected_information_milli: u64,
    pub priority_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalMultimodalMechanismFusion {
    pub composition_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub assessments: Vec<TemporalMechanismAssessment>,
    pub acquisition_order: Vec<String>,
    pub acquisitions: Vec<TemporalAcquisitionCandidate>,
    pub modality_coverage: Vec<GliomaModality>,
    pub timepoint_order: Vec<u32>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: TemporalMultimodalFusionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemporalMultimodalMechanismFusionError {
    #[error("temporal multimodal fusion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("temporal multimodal fusion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("temporal multimodal fusion digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PredictionKey {
    feature_id: String,
    modality: GliomaModality,
    timepoint: u32,
}

fn key(feature_id: &str, modality: GliomaModality, timepoint: u32) -> PredictionKey {
    PredictionKey {
        feature_id: feature_id.into(),
        modality,
        timepoint,
    }
}

fn key_label(value: &PredictionKey) -> String {
    format!(
        "{}:{}:{}",
        value.feature_id,
        serde_json::to_string(&value.modality).unwrap_or_else(|_| "unknown".into()),
        value.timepoint
    )
}

fn modality_slug(value: GliomaModality) -> &'static str {
    match value {
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

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn digest_input(output: &TemporalMultimodalMechanismFusion) -> serde_json::Value {
    serde_json::json!({
        "composition_id": output.composition_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "assessments": output.assessments,
        "acquisition_order": output.acquisition_order,
        "acquisitions": output.acquisitions,
        "modality_coverage": output.modality_coverage,
        "timepoint_order": output.timepoint_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &TemporalMultimodalMechanismFusionRequest,
) -> Result<(), TemporalMultimodalMechanismFusionError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.len() < 2
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.observations.len() > MAX_OBSERVATIONS
        || request.max_next_actions == 0
        || request.max_next_actions > MAX_NEXT_ACTIONS
        || request.min_support_milli > 1_000
        || request.contradiction_threshold_milli > 1_000
    {
        return Err(TemporalMultimodalMechanismFusionError::InvalidRequest(
            "objective, at least two mechanisms, bounded observations/actions, and 0..1000 thresholds are required".into(),
        ));
    }
    let mut mechanisms = BTreeSet::new();
    for mechanism in &request.mechanisms {
        if mechanism.mechanism_id.trim().is_empty()
            || mechanism.statement.trim().is_empty()
            || mechanism.prior_milli == 0
            || !mechanisms.insert(mechanism.mechanism_id.clone())
            || mechanism.predictions.is_empty()
            || mechanism.predictions.len() > MAX_PREDICTIONS_PER_MECHANISM
        {
            return Err(TemporalMultimodalMechanismFusionError::InvalidRequest(
                "mechanisms require unique ids, statements, positive priors, and bounded predictions".into(),
            ));
        }
        let mut predictions = BTreeSet::new();
        for prediction in &mechanism.predictions {
            if prediction.feature_id.trim().is_empty()
                || prediction.uncertainty_milli == 0
                || !predictions.insert(key(
                    &prediction.feature_id,
                    prediction.modality,
                    prediction.timepoint,
                ))
            {
                return Err(TemporalMultimodalMechanismFusionError::InvalidRequest(
                    "prediction keys must be non-empty, unique, and have positive uncertainty"
                        .into(),
                ));
            }
        }
    }
    let mut observations = BTreeSet::new();
    for observation in &request.observations {
        observation.artifact.validate().map_err(|error| {
            TemporalMultimodalMechanismFusionError::InvalidRequest(error.to_string())
        })?;
        if observation.feature_id.trim().is_empty()
            || observation.measurement_uncertainty_milli == 0
            || observation.qc_milli > 1_000
            || observation.source_id.trim().is_empty()
            || observation.independence_group.trim().is_empty()
            || !observations.insert((
                key(
                    &observation.feature_id,
                    observation.modality,
                    observation.timepoint,
                ),
                observation.source_id.clone(),
            ))
        {
            return Err(TemporalMultimodalMechanismFusionError::InvalidRequest(
                "observations require local artifacts, positive uncertainty, bounded QC, and unique source keys".into(),
            ));
        }
    }
    Ok(())
}

fn weighted_support(
    prediction: &TemporalMechanismPrediction,
    observation: &TemporalMechanismObservation,
) -> (u64, u64, u64) {
    let residual = prediction
        .expected_milli
        .saturating_sub(observation.observed_milli)
        .unsigned_abs();
    let scale = u64::from(prediction.uncertainty_milli)
        .saturating_add(u64::from(observation.measurement_uncertainty_milli));
    let agreement = 1_000_u64.saturating_sub(
        residual
            .saturating_mul(1_000)
            .saturating_div(scale.max(1))
            .min(1_000),
    );
    let weight = u64::from(observation.qc_milli.max(1));
    let contradiction = if agreement < 500 { weight } else { 0 };
    (agreement.saturating_mul(weight), weight, contradiction)
}

fn validate_output(
    output: &TemporalMultimodalMechanismFusion,
) -> Result<(), TemporalMultimodalMechanismFusionError> {
    if output.composition_id != COMPOSITION_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.mechanism_order.len() < 2
        || !canonical(&output.mechanism_order)
        || output.assessments.len() != output.mechanism_order.len()
        || output.assessments.windows(2).any(|pair| {
            pair[0].posterior_milli < pair[1].posterior_milli
                || (pair[0].posterior_milli == pair[1].posterior_milli
                    && pair[0].mechanism_id > pair[1].mechanism_id)
        })
        || output.assessments.iter().any(|assessment| {
            assessment.coverage_milli > 1_000
                || assessment.support_milli > 1_000
                || assessment.contradiction_milli > 1_000
                || !canonical(&assessment.matched_prediction_order)
                || !canonical(&assessment.missing_prediction_order)
                || !canonical(&assessment.modality_coverage)
                || !canonical(&assessment.timepoint_order)
        })
        || output.acquisitions.len() != output.acquisition_order.len()
        || output.acquisitions.windows(2).any(|pair| {
            pair[0].priority_milli < pair[1].priority_milli
                || (pair[0].priority_milli == pair[1].priority_milli
                    && pair[0].action_id > pair[1].action_id)
        })
        || output
            .modality_coverage
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || output
            .timepoint_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || output
            .negative_evidence
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || output.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(TemporalMultimodalMechanismFusionError::InvalidOutput(
            "identity, ordering, bounds, or cardinality is invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| TemporalMultimodalMechanismFusionError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(TemporalMultimodalMechanismFusionError::InvalidOutput(
            "fusion output digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl TemporalMultimodalMechanismFusion {
    pub fn validate(&self) -> Result<(), TemporalMultimodalMechanismFusionError> {
        validate_output(self)
    }
}

/// Reconcile temporal/multimodal observations with competing mechanism predictions and compile a
/// next-measurement frontier. All arithmetic is integer and replay-stable.
pub fn execute_glioma_temporal_multimodal_mechanism_fusion(
    request: &TemporalMultimodalMechanismFusionRequest,
) -> Result<TemporalMultimodalMechanismFusion, TemporalMultimodalMechanismFusionError> {
    validate_request(request)?;
    let mut observations: BTreeMap<PredictionKey, Vec<&TemporalMechanismObservation>> =
        BTreeMap::new();
    let mut modality_coverage = BTreeSet::new();
    let mut timepoints = BTreeSet::new();
    for observation in &request.observations {
        let observation_key = key(
            &observation.feature_id,
            observation.modality,
            observation.timepoint,
        );
        observations
            .entry(observation_key)
            .or_default()
            .push(observation);
        modality_coverage.insert(observation.modality);
        timepoints.insert(observation.timepoint);
    }

    let mut assessments = Vec::new();
    let mut raw_scores = BTreeMap::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for mechanism in &request.mechanisms {
        let mut matched = Vec::new();
        let mut missing = Vec::new();
        let mut modality_set = BTreeSet::new();
        let mut timepoint_set = BTreeSet::new();
        let mut source_groups = BTreeSet::new();
        let mut support_numerator = 0_u64;
        let mut support_denominator = 0_u64;
        let mut contradiction_weight = 0_u64;
        let mut prediction_keys = BTreeSet::new();
        for prediction in &mechanism.predictions {
            let prediction_key = key(
                &prediction.feature_id,
                prediction.modality,
                prediction.timepoint,
            );
            prediction_keys.insert(prediction_key.clone());
            if let Some(candidates) = observations.get(&prediction_key) {
                let mut best_by_group: BTreeMap<&str, &TemporalMechanismObservation> =
                    BTreeMap::new();
                for observation in candidates {
                    source_groups.insert(observation.independence_group.clone());
                    best_by_group
                        .entry(observation.independence_group.as_str())
                        .and_modify(|current| {
                            if observation.qc_milli > current.qc_milli {
                                *current = observation;
                            }
                        })
                        .or_insert(observation);
                }
                if best_by_group.values().next().is_some() {
                    for independent_observation in best_by_group.values() {
                        let (weighted, weight, contradiction) =
                            weighted_support(prediction, independent_observation);
                        support_numerator = support_numerator.saturating_add(weighted);
                        support_denominator = support_denominator.saturating_add(weight);
                        contradiction_weight = contradiction_weight.saturating_add(contradiction);
                    }
                    modality_set.insert(prediction.modality);
                    timepoint_set.insert(prediction.timepoint);
                    matched.push(key_label(&prediction_key));
                }
            } else {
                missing.push(key_label(&prediction_key));
            }
        }
        matched.sort();
        missing.sort();
        let coverage = (matched.len() as u64)
            .saturating_mul(1_000)
            .saturating_div(prediction_keys.len().max(1) as u64) as u16;
        let support = support_numerator
            .saturating_div(support_denominator.max(1))
            .min(1_000) as u16;
        let contradiction = contradiction_weight
            .saturating_mul(1_000)
            .saturating_div(support_denominator.max(1)) as u16;
        let independence_factor =
            1_000_u64.saturating_add((source_groups.len() as u64).saturating_mul(125).min(500));
        let score = u64::from(mechanism.prior_milli)
            .saturating_mul(u64::from(support))
            .saturating_mul(u64::from(1_000_u16.saturating_sub(contradiction)))
            .saturating_mul(u64::from(coverage))
            .saturating_mul(independence_factor)
            .saturating_div(1_000);
        raw_scores.insert(mechanism.mechanism_id.clone(), score);
        if !missing.is_empty() {
            uncertainty.push(format!(
                "{} missing {} predicted observation(s)",
                mechanism.mechanism_id,
                missing.len()
            ));
        }
        if contradiction >= request.contradiction_threshold_milli && contradiction > 0 {
            negative_evidence.push(format!(
                "{} has contradiction burden {} milli",
                mechanism.mechanism_id, contradiction
            ));
        }
        let disposition = if matched.is_empty() {
            TemporalMechanismDisposition::Insufficient
        } else if contradiction >= request.contradiction_threshold_milli {
            TemporalMechanismDisposition::Contradicted
        } else if support >= request.min_support_milli {
            TemporalMechanismDisposition::Supported
        } else {
            TemporalMechanismDisposition::Insufficient
        };
        assessments.push(TemporalMechanismAssessment {
            mechanism_id: mechanism.mechanism_id.clone(),
            matched_prediction_order: matched,
            missing_prediction_order: missing,
            source_group_count: source_groups.len().min(u16::MAX as usize) as u16,
            modality_coverage: modality_set.into_iter().collect(),
            timepoint_order: timepoint_set.into_iter().collect(),
            coverage_milli: coverage,
            support_milli: support,
            contradiction_milli: contradiction,
            posterior_milli: 0,
            disposition,
        });
    }

    let total_score = raw_scores.values().copied().sum::<u64>();
    if total_score == 0 {
        let prior_total = request
            .mechanisms
            .iter()
            .map(|mechanism| u64::from(mechanism.prior_milli))
            .sum::<u64>();
        for assessment in &mut assessments {
            let prior = request
                .mechanisms
                .iter()
                .find(|mechanism| mechanism.mechanism_id == assessment.mechanism_id)
                .map(|mechanism| u64::from(mechanism.prior_milli))
                .unwrap_or(0);
            assessment.posterior_milli = prior
                .saturating_mul(1_000)
                .saturating_div(prior_total.max(1)) as u16;
        }
    } else {
        for assessment in &mut assessments {
            assessment.posterior_milli = raw_scores
                .get(&assessment.mechanism_id)
                .copied()
                .unwrap_or(0)
                .saturating_mul(1_000)
                .saturating_div(total_score)
                .min(1_000) as u16;
        }
    }
    assessments.sort_by(|left, right| {
        right
            .posterior_milli
            .cmp(&left.posterior_milli)
            .then_with(|| left.mechanism_id.cmp(&right.mechanism_id))
    });

    let mut acquisition_by_key: BTreeMap<PredictionKey, BTreeSet<String>> = BTreeMap::new();
    let mut expected_by_key: BTreeMap<PredictionKey, Vec<i64>> = BTreeMap::new();
    for mechanism in &request.mechanisms {
        for prediction in &mechanism.predictions {
            let prediction_key = key(
                &prediction.feature_id,
                prediction.modality,
                prediction.timepoint,
            );
            if !observations.contains_key(&prediction_key) {
                acquisition_by_key
                    .entry(prediction_key.clone())
                    .or_default()
                    .insert(mechanism.mechanism_id.clone());
                expected_by_key
                    .entry(prediction_key)
                    .or_default()
                    .push(prediction.expected_milli);
            }
        }
    }
    let mut acquisitions = Vec::new();
    for (prediction_key, mechanism_ids) in acquisition_by_key {
        let values = expected_by_key
            .get(&prediction_key)
            .cloned()
            .unwrap_or_default();
        if mechanism_ids.len() < 2 || values.len() < 2 {
            continue;
        }
        let min_value = values.iter().copied().min().unwrap_or(0);
        let max_value = values.iter().copied().max().unwrap_or(0);
        let separation = max_value.saturating_sub(min_value).unsigned_abs();
        let expected_information = separation
            .saturating_mul(mechanism_ids.len() as u64)
            .min(1_000_000);
        let priority = expected_information
            .saturating_mul((mechanism_ids.len() as u64).min(16))
            .saturating_mul(1_000)
            .saturating_div(values.len().max(1) as u64);
        let action_id = format!(
            "fusion:{}:{}:{}",
            prediction_key.feature_id,
            modality_slug(prediction_key.modality),
            prediction_key.timepoint
        );
        acquisitions.push(TemporalAcquisitionCandidate {
            action_id,
            feature_id: prediction_key.feature_id,
            modality: prediction_key.modality,
            timepoint: prediction_key.timepoint,
            mechanism_order: mechanism_ids.into_iter().collect(),
            expected_separation_milli: separation,
            expected_information_milli: expected_information,
            priority_milli: priority,
            rationale: "unmeasured time/modality key separates multiple declared mechanisms".into(),
        });
    }
    acquisitions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    acquisitions.truncate(request.max_next_actions);
    let acquisition_order = acquisitions
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<Vec<_>>();
    let mechanism_order = request
        .mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if acquisitions.is_empty() {
        uncertainty.push("no unmeasured cross-mechanism acquisition candidate remains".into());
    }
    if request.observations.is_empty() {
        uncertainty
            .push("no local observations were supplied; posterior remains prior-driven".into());
    }
    let supported_count = assessments
        .iter()
        .filter(|assessment| assessment.disposition == TemporalMechanismDisposition::Supported)
        .count();
    let contradiction_count = assessments
        .iter()
        .filter(|assessment| assessment.disposition == TemporalMechanismDisposition::Contradicted)
        .count();
    let disposition = if contradiction_count > 0 && supported_count == 0 {
        TemporalMultimodalFusionDisposition::Unresolved
    } else if assessments
        .iter()
        .any(|assessment| assessment.disposition == TemporalMechanismDisposition::Insufficient)
        || !acquisitions.is_empty()
    {
        TemporalMultimodalFusionDisposition::Partial
    } else {
        TemporalMultimodalFusionDisposition::Qualified
    };
    let mut output = TemporalMultimodalMechanismFusion {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        assessments,
        acquisition_order,
        acquisitions,
        modality_coverage: modality_coverage.into_iter().collect(),
        timepoint_order: timepoints.into_iter().collect(),
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-temporal-multimodal-fusion"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| TemporalMultimodalMechanismFusionError::Digest(error.to_string()))?;
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
            content_type: "application/vnd.aurora.glioma-observation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn prediction(feature_id: &str, expected_milli: i64) -> TemporalMechanismPrediction {
        TemporalMechanismPrediction {
            feature_id: feature_id.into(),
            modality: GliomaModality::Transcriptomics,
            timepoint: 1,
            expected_milli,
            uncertainty_milli: 50,
        }
    }

    fn observation(feature_id: &str, observed_milli: i64) -> TemporalMechanismObservation {
        TemporalMechanismObservation {
            feature_id: feature_id.into(),
            modality: GliomaModality::Transcriptomics,
            timepoint: 1,
            observed_milli,
            measurement_uncertainty_milli: 50,
            qc_milli: 950,
            source_id: format!("source-{feature_id}"),
            independence_group: format!("site-{feature_id}"),
            artifact: artifact(&format!("artifact-{feature_id}")),
        }
    }

    fn request(
        observations: Vec<TemporalMechanismObservation>,
    ) -> TemporalMultimodalMechanismFusionRequest {
        TemporalMultimodalMechanismFusionRequest {
            objective: "rank invasion-state mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![
                TemporalMechanismHypothesis {
                    mechanism_id: "integrin".into(),
                    statement: "integrin-dependent invasion state".into(),
                    prior_milli: 500,
                    predictions: vec![prediction("invasion", 100), prediction("stress", 200)],
                },
                TemporalMechanismHypothesis {
                    mechanism_id: "hypoxia".into(),
                    statement: "hypoxia-driven invasion state".into(),
                    prior_milli: 500,
                    predictions: vec![prediction("invasion", 900), prediction("stress", 800)],
                },
            ],
            observations,
            max_next_actions: 4,
            min_support_milli: 500,
            contradiction_threshold_milli: 600,
        }
    }

    #[test]
    fn fusion_prefers_supported_mechanism_and_emits_next_measurement() {
        let output =
            execute_glioma_temporal_multimodal_mechanism_fusion(&request(vec![observation(
                "invasion", 110,
            )]))
            .expect("fusion should compile");
        assert_eq!(output.assessments[0].mechanism_id, "integrin");
        assert_eq!(
            output.assessments[0].disposition,
            TemporalMechanismDisposition::Supported
        );
        assert_eq!(
            output.disposition,
            TemporalMultimodalFusionDisposition::Partial
        );
        assert_eq!(output.acquisitions.len(), 1);
        assert_eq!(output.acquisitions[0].feature_id, "stress");
        assert!(output.validate().is_ok());
    }

    #[test]
    fn contradictory_observations_remain_unresolved() {
        let output = execute_glioma_temporal_multimodal_mechanism_fusion(&request(vec![
            observation("invasion", 500),
            observation("stress", 500),
        ]))
        .expect("contradictory fusion should still produce an artifact");
        assert_eq!(
            output.disposition,
            TemporalMultimodalFusionDisposition::Unresolved
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradiction burden")));
        assert!(
            output
                .assessments
                .iter()
                .all(|assessment| assessment.disposition
                    == TemporalMechanismDisposition::Contradicted)
        );
    }
}
