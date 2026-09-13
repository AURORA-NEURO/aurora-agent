//! Longitudinal multimodal state-transition inference for preclinical glioma studies.
//!
//! This feature joins local, de-identified modality summaries on `(sample, timepoint)` and
//! computes a bounded state trajectory without pretending that missing timepoints or modalities
//! were observed.  It is deliberately not a clinical progression model.  The output is a typed
//! research artifact for mechanism exploration, experiment design, and replication: each state is
//! content-addressed, each transition reports its comparable-feature support, and stable/null
//! transitions are published as negative evidence rather than discarded.

use super::concordance::FeatureValue;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaTemporalMultimodalFusion1@1";
pub const MAX_OBSERVATIONS: usize = 8_192;
pub const MAX_FEATURES_PER_OBSERVATION: usize = 4_096;
pub const MAX_STATES: usize = 8_192;
pub const MAX_TRANSITIONS: usize = 8_192;
pub const MAX_TIMEPOINTS: usize = 256;
pub const MAX_SAMPLES: usize = 4_096;
pub const MAX_VALUE_MILLI: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalFusionRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub min_timepoints_per_sample: usize,
    pub min_modalities_per_timepoint: usize,
    pub min_shared_features: usize,
    pub min_transition_support_milli: u16,
    pub max_state_change_milli: u64,
    pub require_complete_time_grid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalObservation {
    pub observation_id: String,
    pub study_id: String,
    pub sample_id: String,
    pub timepoint: u32,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub features: Vec<FeatureValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalStateFeature {
    pub feature_id: String,
    pub value_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalState {
    pub state_id: String,
    pub sample_id: String,
    pub timepoint: u32,
    pub modality_order: Vec<GliomaModality>,
    pub feature_order: Vec<String>,
    pub features: Vec<TemporalStateFeature>,
    pub completeness_milli: u16,
    pub signature: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalTransitionDirection {
    Emerging,
    Contracting,
    Stable,
    Mixed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalTransition {
    pub transition_id: String,
    pub sample_id: String,
    pub from_timepoint: u32,
    pub to_timepoint: u32,
    pub from_state_id: String,
    pub to_state_id: String,
    pub shared_feature_order: Vec<String>,
    pub changed_feature_order: Vec<String>,
    pub conserved_feature_order: Vec<String>,
    pub distance_milli: u64,
    pub mean_delta_milli: i64,
    pub transition_support_milli: u16,
    pub direction: TemporalTransitionDirection,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalFusionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalFusionAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modality_order: Vec<GliomaModality>,
    pub sample_order: Vec<String>,
    pub timepoint_order: Vec<u32>,
    pub state_order: Vec<String>,
    pub transition_order: Vec<String>,
    pub priority_transition_order: Vec<String>,
    pub states: Vec<TemporalState>,
    pub transitions: Vec<TemporalTransition>,
    pub emerging_transition_order: Vec<String>,
    pub contracting_transition_order: Vec<String>,
    pub stable_transition_order: Vec<String>,
    pub missing_sample_order: Vec<String>,
    pub missing_timepoint_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: TemporalFusionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemporalFusionError {
    #[error("temporal fusion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("temporal fusion observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("temporal fusion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("temporal fusion digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@>".contains(&byte))
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

fn qualified_feature_id(modality: GliomaModality, feature_id: &str) -> String {
    format!("{}::{feature_id}", modality_label(modality))
}

fn state_id(sample_id: &str, timepoint: u32) -> String {
    format!("{sample_id}@{timepoint}")
}

fn transition_id(sample_id: &str, from_timepoint: u32, to_timepoint: u32) -> String {
    format!("{sample_id}@{from_timepoint}->{to_timepoint}")
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right))
        .unsigned_abs()
        .min(u128::from(u64::MAX)) as u64
}

fn digest_input(output: &TemporalFusionAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "required_modality_order": output.required_modality_order,
        "sample_order": output.sample_order,
        "timepoint_order": output.timepoint_order,
        "state_order": output.state_order,
        "transition_order": output.transition_order,
        "priority_transition_order": output.priority_transition_order,
        "states": output.states,
        "transitions": output.transitions,
        "emerging_transition_order": output.emerging_transition_order,
        "contracting_transition_order": output.contracting_transition_order,
        "stable_transition_order": output.stable_transition_order,
        "missing_sample_order": output.missing_sample_order,
        "missing_timepoint_order": output.missing_timepoint_order,
        "missing_modality_order": output.missing_modality_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl TemporalState {
    fn validate(&self) -> Result<(), TemporalFusionError> {
        if !valid_identifier(&self.state_id)
            || !valid_identifier(&self.sample_id)
            || self.feature_order.len() != self.features.len()
            || !canonical(&self.modality_order)
            || !canonical(&self.feature_order)
            || self
                .features
                .windows(2)
                .any(|pair| pair[0].feature_id >= pair[1].feature_id)
            || self.features.iter().any(|feature| {
                !valid_identifier(&feature.feature_id)
                    || feature.value_milli.unsigned_abs() > MAX_VALUE_MILLI as u64
            })
            || self.completeness_milli > 1_000
            || self.signature.as_str().len() != 64
        {
            return Err(TemporalFusionError::InvalidOutput(
                "state identity, ordering, value, completeness, or signature invariant failed"
                    .into(),
            ));
        }
        Ok(())
    }
}

impl TemporalTransition {
    fn validate(&self) -> Result<(), TemporalFusionError> {
        if !valid_identifier(&self.transition_id)
            || !valid_identifier(&self.sample_id)
            || self.from_timepoint >= self.to_timepoint
            || self.from_state_id.trim().is_empty()
            || self.to_state_id.trim().is_empty()
            || !canonical(&self.shared_feature_order)
            || !canonical(&self.changed_feature_order)
            || !canonical(&self.conserved_feature_order)
            || self
                .shared_feature_order
                .iter()
                .any(|feature| !valid_identifier(feature))
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self.transition_support_milli > 1_000
        {
            return Err(TemporalFusionError::InvalidOutput(
                "transition identity, order, partition, or support invariant failed".into(),
            ));
        }
        let shared = self
            .shared_feature_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let changed = self
            .changed_feature_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let conserved = self
            .conserved_feature_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if changed.intersection(&conserved).next().is_some()
            || changed.union(&conserved).cloned().collect::<BTreeSet<_>>() != shared
        {
            return Err(TemporalFusionError::InvalidOutput(
                "changed and conserved features must partition shared features".into(),
            ));
        }
        Ok(())
    }
}

impl TemporalFusionAnalysis {
    pub fn validate(&self) -> Result<(), TemporalFusionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.required_modality_order)
            || !canonical(&self.sample_order)
            || !canonical(&self.timepoint_order)
            || !canonical(&self.state_order)
            || !canonical(&self.transition_order)
            || !canonical(&self.emerging_transition_order)
            || !canonical(&self.contracting_transition_order)
            || !canonical(&self.stable_transition_order)
            || !canonical(&self.missing_sample_order)
            || !canonical(&self.missing_timepoint_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.states.len() != self.state_order.len()
            || self.transitions.len() != self.transition_order.len()
            || self.states.len() > MAX_STATES
            || self.transitions.len() > MAX_TRANSITIONS
        {
            return Err(TemporalFusionError::InvalidOutput(
                "analysis identity, ordering, cardinality, or limitation invariant failed".into(),
            ));
        }
        let states = self
            .states
            .iter()
            .map(|state| state.state_id.clone())
            .collect::<BTreeSet<_>>();
        let transitions = self
            .transitions
            .iter()
            .map(|transition| transition.transition_id.clone())
            .collect::<BTreeSet<_>>();
        if states != self.state_order.iter().cloned().collect::<BTreeSet<_>>()
            || transitions
                != self
                    .transition_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || !self
                .priority_transition_order
                .iter()
                .all(|id| transitions.contains(id))
            || !self
                .emerging_transition_order
                .iter()
                .chain(self.contracting_transition_order.iter())
                .chain(self.stable_transition_order.iter())
                .all(|id| transitions.contains(id))
            || self.states.iter().any(|state| state.validate().is_err())
            || self
                .transitions
                .iter()
                .any(|transition| transition.validate().is_err())
            || self.transitions.windows(2).any(|pair| {
                pair[0].transition_id >= pair[1].transition_id
                    || pair[0].from_timepoint >= pair[0].to_timepoint
            })
        {
            return Err(TemporalFusionError::InvalidOutput(
                "state/transition partitions or nested invariants do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| TemporalFusionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(TemporalFusionError::InvalidOutput(
                "temporal fusion digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &TemporalFusionRequest) -> Result<(), TemporalFusionError> {
    if !valid_identifier(&request.study_id)
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > 16
        || request.min_timepoints_per_sample < 2
        || request.min_timepoints_per_sample > MAX_TIMEPOINTS
        || request.min_modalities_per_timepoint == 0
        || request.min_modalities_per_timepoint > request.required_modalities.len()
        || request.min_shared_features == 0
        || request.min_shared_features > MAX_FEATURES_PER_OBSERVATION
        || request.min_transition_support_milli > 1_000
        || request.max_state_change_milli > MAX_VALUE_MILLI as u64 * 2
    {
        return Err(TemporalFusionError::InvalidRequest(
            "study, modality, timepoint, feature, support, or change bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    request: &TemporalFusionRequest,
    observation: &TemporalObservation,
    seen_ids: &mut BTreeSet<String>,
) -> Result<(), TemporalFusionError> {
    if !seen_ids.insert(observation.observation_id.clone())
        || !valid_identifier(&observation.observation_id)
        || !valid_identifier(&observation.sample_id)
        || observation.study_id != request.study_id
        || observation.model_system != request.model_system
        || !request.required_modalities.contains(&observation.modality)
        || observation.features.is_empty()
        || observation.features.len() > MAX_FEATURES_PER_OBSERVATION
    {
        return Err(TemporalFusionError::InvalidObservation(
            "observation identity, study/model binding, modality, or feature cardinality is invalid"
                .into(),
        ));
    }
    observation
        .artifact
        .validate()
        .map_err(|error| TemporalFusionError::InvalidObservation(error.to_string()))?;
    let mut features = BTreeSet::new();
    for feature in &observation.features {
        if !valid_identifier(&feature.feature_id)
            || feature.value_milli.unsigned_abs() > MAX_VALUE_MILLI as u64
            || !features.insert(feature.feature_id.clone())
        {
            return Err(TemporalFusionError::InvalidObservation(
                "features must be unique, bounded, and identifier-safe".into(),
            ));
        }
    }
    Ok(())
}

fn build_state(
    sample_id: &str,
    timepoint: u32,
    modalities: &BTreeMap<GliomaModality, &TemporalObservation>,
    required_modalities: &BTreeSet<GliomaModality>,
) -> Result<TemporalState, TemporalFusionError> {
    let mut feature_map = BTreeMap::<String, i64>::new();
    for (modality, observation) in modalities {
        for feature in &observation.features {
            feature_map.insert(
                qualified_feature_id(*modality, &feature.feature_id),
                feature.value_milli,
            );
        }
    }
    let feature_order = feature_map.keys().cloned().collect::<Vec<_>>();
    let features = feature_order
        .iter()
        .map(|feature_id| TemporalStateFeature {
            feature_id: feature_id.clone(),
            value_milli: feature_map[feature_id],
        })
        .collect::<Vec<_>>();
    let modality_order = modalities.keys().copied().collect::<Vec<_>>();
    let completeness_milli = (modalities.len() as u32)
        .saturating_mul(1_000)
        .checked_div(required_modalities.len() as u32)
        .unwrap_or_default()
        .min(1_000) as u16;
    let state_id = state_id(sample_id, timepoint);
    let signature = ContentHash::of_value(&serde_json::json!({
        "state_id": state_id,
        "sample_id": sample_id,
        "timepoint": timepoint,
        "modality_order": modality_order,
        "feature_order": feature_order,
        "features": features,
    }))
    .map_err(|error| TemporalFusionError::Digest(error.to_string()))?;
    Ok(TemporalState {
        state_id,
        sample_id: sample_id.into(),
        timepoint,
        modality_order,
        feature_order,
        features,
        completeness_milli,
        signature,
    })
}

fn map_features(state: &TemporalState) -> BTreeMap<&str, i64> {
    state
        .features
        .iter()
        .map(|feature| (feature.feature_id.as_str(), feature.value_milli))
        .collect()
}

fn build_transition(
    from: &TemporalState,
    to: &TemporalState,
    request: &TemporalFusionRequest,
) -> TemporalTransition {
    let from_features = map_features(from);
    let to_features = map_features(to);
    let shared = from_features
        .keys()
        .filter(|feature_id| to_features.contains_key(*feature_id))
        .map(|feature_id| (*feature_id).to_string())
        .collect::<Vec<_>>();
    let union_count = from_features
        .keys()
        .chain(to_features.keys())
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    let support = if union_count == 0 {
        0
    } else {
        ((shared.len() as u32) * 1_000 / union_count as u32).min(1_000) as u16
    };
    let comparable = shared.len() >= request.min_shared_features
        && support >= request.min_transition_support_milli;
    let mut changed = Vec::new();
    let mut conserved = Vec::new();
    let mut total_distance = 0_u128;
    let mut total_delta = 0_i128;
    let mut positive = false;
    let mut negative = false;
    for feature_id in &shared {
        let left = from_features[feature_id.as_str()];
        let right = to_features[feature_id.as_str()];
        let delta = i128::from(right) - i128::from(left);
        let distance = abs_difference(left, right);
        total_distance = total_distance.saturating_add(u128::from(distance));
        total_delta = total_delta.saturating_add(delta);
        if distance > request.max_state_change_milli {
            changed.push(feature_id.clone());
            positive |= delta > 0;
            negative |= delta < 0;
        } else {
            conserved.push(feature_id.clone());
        }
    }
    let distance = if shared.is_empty() {
        0
    } else {
        (total_distance / shared.len() as u128).min(u128::from(u64::MAX)) as u64
    };
    let mean_delta = if shared.is_empty() {
        0
    } else {
        (total_delta / shared.len() as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
            as i64
    };
    let (direction, uncertainty) = if !comparable {
        (
            TemporalTransitionDirection::Unresolved,
            vec![format!(
                "insufficient-comparable-features:{}-of-{}",
                shared.len(),
                request.min_shared_features
            )],
        )
    } else if changed.is_empty() {
        (TemporalTransitionDirection::Stable, Vec::new())
    } else if positive && negative {
        (
            TemporalTransitionDirection::Mixed,
            vec!["opposing-feature-deltas".into()],
        )
    } else if positive {
        (TemporalTransitionDirection::Emerging, Vec::new())
    } else if negative {
        (TemporalTransitionDirection::Contracting, Vec::new())
    } else {
        (TemporalTransitionDirection::Stable, Vec::new())
    };
    TemporalTransition {
        transition_id: transition_id(&from.sample_id, from.timepoint, to.timepoint),
        sample_id: from.sample_id.clone(),
        from_timepoint: from.timepoint,
        to_timepoint: to.timepoint,
        from_state_id: from.state_id.clone(),
        to_state_id: to.state_id.clone(),
        shared_feature_order: shared,
        changed_feature_order: changed,
        conserved_feature_order: conserved,
        distance_milli: distance,
        mean_delta_milli: mean_delta,
        transition_support_milli: support,
        direction,
        uncertainty,
    }
}

/// Infer a bounded longitudinal multimodal state graph for local preclinical glioma data.
pub fn analyze_glioma_temporal_multimodal_fusion(
    request: &TemporalFusionRequest,
    observations: &[TemporalObservation],
) -> Result<TemporalFusionAnalysis, TemporalFusionError> {
    validate_request(request)?;
    if observations.is_empty() || observations.len() > MAX_OBSERVATIONS {
        return Err(TemporalFusionError::InvalidRequest(
            "temporal fusion requires a bounded non-empty observation set".into(),
        ));
    }
    let mut seen_ids = BTreeSet::new();
    let mut grouped = BTreeMap::<(String, u32, GliomaModality), &TemporalObservation>::new();
    for observation in observations {
        validate_observation(request, observation, &mut seen_ids)?;
        let key = (
            observation.sample_id.clone(),
            observation.timepoint,
            observation.modality,
        );
        if grouped.insert(key, observation).is_some() {
            return Err(TemporalFusionError::InvalidObservation(
                "one sample/timepoint/modality may have only one observation".into(),
            ));
        }
    }
    let samples = grouped
        .keys()
        .map(|(sample_id, _, _)| sample_id.clone())
        .collect::<BTreeSet<_>>();
    let timepoints = grouped
        .keys()
        .map(|(_, timepoint, _)| *timepoint)
        .collect::<BTreeSet<_>>();
    if samples.len() > MAX_SAMPLES || timepoints.len() > MAX_TIMEPOINTS {
        return Err(TemporalFusionError::InvalidRequest(
            "sample or timepoint cardinality exceeds the bounded planner".into(),
        ));
    }
    let mut states = Vec::new();
    let mut states_by_sample = BTreeMap::<String, Vec<TemporalState>>::new();
    let sample_timepoints = samples
        .iter()
        .map(|sample_id| {
            let mut times = grouped
                .keys()
                .filter(|(sample, _, _)| sample == sample_id)
                .map(|(_, timepoint, _)| *timepoint)
                .collect::<BTreeSet<_>>();
            (sample_id.clone(), std::mem::take(&mut times))
        })
        .collect::<BTreeMap<_, _>>();
    for (sample_id, times) in &sample_timepoints {
        for timepoint in times {
            let modalities = grouped
                .iter()
                .filter(|((sample, point, _), _)| sample == sample_id && point == timepoint)
                .map(|((_, _, modality), observation)| (*modality, *observation))
                .collect::<BTreeMap<_, _>>();
            let state = build_state(
                sample_id,
                *timepoint,
                &modalities,
                &request.required_modalities,
            )?;
            states.push(state.clone());
            states_by_sample
                .entry(sample_id.clone())
                .or_default()
                .push(state);
        }
    }
    states.sort_by(|left, right| left.state_id.cmp(&right.state_id));
    for sample_states in states_by_sample.values_mut() {
        sample_states.sort_by_key(|state| state.timepoint);
    }
    let mut transitions = Vec::new();
    for sample_states in states_by_sample.values() {
        for pair in sample_states.windows(2) {
            transitions.push(build_transition(&pair[0], &pair[1], request));
        }
    }
    transitions.sort_by(|left, right| left.transition_id.cmp(&right.transition_id));
    if states.len() > MAX_STATES || transitions.len() > MAX_TRANSITIONS {
        return Err(TemporalFusionError::InvalidRequest(
            "state or transition cardinality exceeds the bounded planner".into(),
        ));
    }
    let sample_order = samples.into_iter().collect::<Vec<_>>();
    let timepoint_order = timepoints.into_iter().collect::<Vec<_>>();
    let state_order = states
        .iter()
        .map(|state| state.state_id.clone())
        .collect::<Vec<_>>();
    let transition_order = transitions
        .iter()
        .map(|transition| transition.transition_id.clone())
        .collect::<Vec<_>>();
    let mut priority = transitions.clone();
    priority.sort_by(|left, right| {
        right
            .distance_milli
            .cmp(&left.distance_milli)
            .then_with(|| {
                right
                    .transition_support_milli
                    .cmp(&left.transition_support_milli)
            })
            .then_with(|| left.transition_id.cmp(&right.transition_id))
    });
    let priority_transition_order = priority
        .iter()
        .map(|transition| transition.transition_id.clone())
        .collect::<Vec<_>>();
    let emerging_transition_order = transitions
        .iter()
        .filter(|transition| transition.direction == TemporalTransitionDirection::Emerging)
        .map(|transition| transition.transition_id.clone())
        .collect::<Vec<_>>();
    let contracting_transition_order = transitions
        .iter()
        .filter(|transition| transition.direction == TemporalTransitionDirection::Contracting)
        .map(|transition| transition.transition_id.clone())
        .collect::<Vec<_>>();
    let stable_transition_order = transitions
        .iter()
        .filter(|transition| transition.direction == TemporalTransitionDirection::Stable)
        .map(|transition| transition.transition_id.clone())
        .collect::<Vec<_>>();
    let mut missing_sample_order = Vec::new();
    let mut missing_timepoint_order = BTreeSet::new();
    let mut missing_modality_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (sample_id, sample_states) in &states_by_sample {
        if sample_states.len() < request.min_timepoints_per_sample {
            missing_sample_order.push(sample_id.clone());
            uncertainty.insert(format!(
                "sample-below-timepoint-floor:{}:{}-of-{}",
                sample_id,
                sample_states.len(),
                request.min_timepoints_per_sample
            ));
        }
        if request.require_complete_time_grid {
            for timepoint in &timepoint_order {
                if !sample_states
                    .iter()
                    .any(|state| state.timepoint == *timepoint)
                {
                    missing_timepoint_order.insert(format!("{sample_id}@{timepoint}"));
                }
            }
        }
        for state in sample_states {
            if state.modality_order.len() < request.min_modalities_per_timepoint {
                uncertainty.insert(format!("timepoint-below-modality-floor:{}", state.state_id));
            }
            for modality in &request.required_modalities {
                if !state.modality_order.contains(modality) {
                    missing_modality_order.insert(*modality);
                }
            }
        }
    }
    missing_sample_order.sort();
    for item in &missing_timepoint_order {
        uncertainty.insert(format!("missing-timepoint:{item}"));
    }
    for modality in &missing_modality_order {
        uncertainty.insert(format!("missing-modality:{}", modality_label(*modality)));
    }
    for transition in &transitions {
        uncertainty.extend(
            transition
                .uncertainty
                .iter()
                .map(|item| format!("transition:{}:{item}", transition.transition_id)),
        );
    }
    let mut negative_evidence = BTreeSet::new();
    for transition in &transitions {
        if transition.direction == TemporalTransitionDirection::Stable {
            negative_evidence.insert(format!(
                "stable-state-transition:{}:no-feature-change-above-threshold",
                transition.transition_id
            ));
        }
    }
    if transitions.is_empty() {
        uncertainty.insert("no-adjacent-timepoint-transition-observed".into());
    }
    let valid_transitions = transitions
        .iter()
        .filter(|transition| transition.direction != TemporalTransitionDirection::Unresolved)
        .count();
    let complete = !states.is_empty()
        && !transitions.is_empty()
        && valid_transitions == transitions.len()
        && missing_sample_order.is_empty()
        && missing_timepoint_order.is_empty()
        && missing_modality_order.is_empty()
        && states.iter().all(|state| {
            usize::from(state.completeness_milli)
                >= request.min_modalities_per_timepoint * 1_000 / request.required_modalities.len()
        });
    let disposition = if states.is_empty() || transitions.is_empty() {
        TemporalFusionDisposition::Unresolved
    } else if complete {
        TemporalFusionDisposition::Qualified
    } else {
        TemporalFusionDisposition::Partial
    };
    let mut output = TemporalFusionAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        required_modality_order: request.required_modalities.iter().copied().collect(),
        sample_order,
        timepoint_order,
        state_order,
        transition_order,
        priority_transition_order,
        states,
        transitions,
        emerging_transition_order,
        contracting_transition_order,
        stable_transition_order,
        missing_sample_order,
        missing_timepoint_order: missing_timepoint_order.into_iter().collect(),
        missing_modality_order: missing_modality_order.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-temporal-fusion"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| TemporalFusionError::Digest(error.to_string()))?;
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
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn observation(
        id: &str,
        sample: &str,
        timepoint: u32,
        modality: GliomaModality,
        x: i64,
        y: i64,
    ) -> TemporalObservation {
        TemporalObservation {
            observation_id: id.into(),
            study_id: "temporal-study".into(),
            sample_id: sample.into(),
            timepoint,
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: artifact(id),
            features: vec![
                FeatureValue {
                    feature_id: "state_x".into(),
                    value_milli: x,
                },
                FeatureValue {
                    feature_id: "state_y".into(),
                    value_milli: y,
                },
            ],
        }
    }

    fn request() -> TemporalFusionRequest {
        TemporalFusionRequest {
            study_id: "temporal-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: [GliomaModality::Transcriptomics, GliomaModality::Imaging]
                .into_iter()
                .collect(),
            min_timepoints_per_sample: 2,
            min_modalities_per_timepoint: 2,
            min_shared_features: 2,
            min_transition_support_milli: 800,
            max_state_change_milli: 50,
            require_complete_time_grid: true,
        }
    }

    #[test]
    fn longitudinal_fusion_identifies_emerging_transition_and_is_replay_stable() {
        let observations = vec![
            observation(
                "s1-t0-rna",
                "s1",
                0,
                GliomaModality::Transcriptomics,
                100,
                100,
            ),
            observation("s1-t0-img", "s1", 0, GliomaModality::Imaging, 110, 90),
            observation(
                "s1-t1-rna",
                "s1",
                1,
                GliomaModality::Transcriptomics,
                300,
                100,
            ),
            observation("s1-t1-img", "s1", 1, GliomaModality::Imaging, 290, 110),
        ];
        let first = analyze_glioma_temporal_multimodal_fusion(&request(), &observations).unwrap();
        let second = analyze_glioma_temporal_multimodal_fusion(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, TemporalFusionDisposition::Qualified);
        assert_eq!(first.emerging_transition_order.len(), 1);
        assert_eq!(first.transitions[0].changed_feature_order.len(), 2);
    }

    #[test]
    fn input_permutation_does_not_change_states_or_transitions() {
        let observations = vec![
            observation(
                "s1-t0-rna",
                "s1",
                0,
                GliomaModality::Transcriptomics,
                100,
                100,
            ),
            observation("s1-t0-img", "s1", 0, GliomaModality::Imaging, 110, 90),
            observation(
                "s1-t1-rna",
                "s1",
                1,
                GliomaModality::Transcriptomics,
                300,
                100,
            ),
            observation("s1-t1-img", "s1", 1, GliomaModality::Imaging, 290, 110),
        ];
        let reversed = observations.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            analyze_glioma_temporal_multimodal_fusion(&request(), &observations).unwrap(),
            analyze_glioma_temporal_multimodal_fusion(&request(), &reversed).unwrap()
        );
    }

    #[test]
    fn missing_modality_is_partial_and_never_imputed() {
        let observations = vec![
            observation(
                "s1-t0-rna",
                "s1",
                0,
                GliomaModality::Transcriptomics,
                100,
                100,
            ),
            observation(
                "s1-t1-rna",
                "s1",
                1,
                GliomaModality::Transcriptomics,
                300,
                100,
            ),
        ];
        let output = analyze_glioma_temporal_multimodal_fusion(&request(), &observations).unwrap();
        assert_eq!(output.disposition, TemporalFusionDisposition::Partial);
        assert_eq!(output.missing_modality_order, vec![GliomaModality::Imaging]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("missing-modality")));
    }

    #[test]
    fn stable_transition_is_published_as_negative_evidence() {
        let observations = vec![
            observation(
                "s1-t0-rna",
                "s1",
                0,
                GliomaModality::Transcriptomics,
                100,
                100,
            ),
            observation("s1-t0-img", "s1", 0, GliomaModality::Imaging, 100, 100),
            observation(
                "s1-t1-rna",
                "s1",
                1,
                GliomaModality::Transcriptomics,
                110,
                100,
            ),
            observation("s1-t1-img", "s1", 1, GliomaModality::Imaging, 100, 110),
        ];
        let output = analyze_glioma_temporal_multimodal_fusion(&request(), &observations).unwrap();
        assert_eq!(output.stable_transition_order.len(), 1);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("stable-state-transition")));
    }

    #[test]
    fn duplicate_observation_identity_is_rejected() {
        let mut observations = vec![observation(
            "duplicate",
            "s1",
            0,
            GliomaModality::Transcriptomics,
            100,
            100,
        )];
        observations.push(observations[0].clone());
        assert!(matches!(
            analyze_glioma_temporal_multimodal_fusion(&request(), &observations),
            Err(TemporalFusionError::InvalidObservation(_))
        ));
    }

    #[test]
    fn human_data_artifact_is_refused() {
        let mut item = observation("human", "s1", 0, GliomaModality::Transcriptomics, 100, 100);
        item.artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_temporal_multimodal_fusion(&request(), &[item]),
            Err(TemporalFusionError::InvalidObservation(_))
        ));
    }
}
