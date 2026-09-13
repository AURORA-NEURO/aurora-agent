//! Temporal-spatial state alignment for preclinical glioma research.
//!
//! This feature composes longitudinal multimodal fusion, robust spatial registration, and
//! lineage-aware spatial propagation into one researcher-facing product.  It does not pretend
//! that a spatial cell was measured at an unobserved timepoint: every sample/timepoint is joined
//! by a caller-declared mapping, coverage is quantified, and state mismatches become typed
//! follow-up actions.  The result is useful for experiment planning and mechanism research while
//! remaining local-only, deterministic, and outside clinical decision making.

use super::spatial_niche::SpatialCell;
use super::spatial_propagation::{
    analyze_glioma_spatial_state_propagation, SpatialPropagationAnalysis,
    SpatialPropagationDisposition,
};
use super::spatial_registration::{
    register_glioma_spatial_samples, RegisteredSpatialCell, SpatialRegistrationAnalysis,
    SpatialRegistrationCell, SpatialRegistrationDisposition,
};
use super::temporal_fusion::{
    analyze_glioma_temporal_multimodal_fusion, TemporalFusionAnalysis, TemporalFusionDisposition,
    TemporalFusionRequest, TemporalObservation,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaTemporalSpatialAlignment1@1";
pub const MAX_SAMPLE_TIMEPOINTS: usize = 512;
pub const MAX_STATE_GAP_MILLI: u64 = 2_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleTimepoint {
    pub sample_id: String,
    pub timepoint: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalSpatialAlignmentRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub temporal_request: TemporalFusionRequest,
    pub registration_request: super::spatial_registration::SpatialRegistrationRequest,
    pub propagation_request: super::spatial_propagation::SpatialPropagationRequest,
    /// A declared mapping from each spatial sample to the temporal timepoint at which it was
    /// collected.  The engine never infers this mapping from coordinates or sample names.
    pub sample_timepoints: Vec<SampleTimepoint>,
    pub min_spatial_coverage_milli: u16,
    pub max_state_gap_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalSpatialDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentGate {
    pub gate_id: String,
    pub disposition: TemporalSpatialDisposition,
    pub support_milli: u16,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignedSampleState {
    pub sample_id: String,
    pub timepoint: u32,
    pub temporal_state_id: Option<String>,
    pub registered_cell_order: Vec<String>,
    pub propagated_hotspot_order: Vec<String>,
    pub temporal_state_mean_milli: Option<i64>,
    pub spatial_state_mean_milli: Option<i64>,
    pub state_gap_milli: Option<u64>,
    pub spatial_coverage_milli: u16,
    pub disposition: TemporalSpatialDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalSpatialAction {
    AcquireTemporalState,
    RegisterSpatialSample,
    CollectSpatialAtTimepoint,
    InvestigateStateMismatch,
    ConfirmSpatialPropagation,
    PublishAlignedStateMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalSpatialAlignment {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_timepoint_order: Vec<String>,
    pub aligned_sample_order: Vec<String>,
    pub aligned_states: Vec<AlignedSampleState>,
    pub temporal_gate: AlignmentGate,
    pub registration_gate: AlignmentGate,
    pub propagation_gate: AlignmentGate,
    pub alignment_gate: AlignmentGate,
    pub priority_action_order: Vec<TemporalSpatialAction>,
    pub missing_sample_timepoint_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: TemporalSpatialDisposition,
    pub temporal: TemporalFusionAnalysis,
    pub registration: SpatialRegistrationAnalysis,
    pub propagation: SpatialPropagationAnalysis,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemporalSpatialAlignmentError {
    #[error("temporal-spatial alignment request is invalid: {0}")]
    InvalidRequest(String),
    #[error("temporal-spatial alignment input is invalid: {0}")]
    InvalidInput(String),
    #[error("temporal-spatial alignment output is invalid: {0}")]
    InvalidOutput(String),
    #[error("temporal-spatial alignment digest failed: {0}")]
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

fn disposition_from_temporal(value: TemporalFusionDisposition) -> TemporalSpatialDisposition {
    match value {
        TemporalFusionDisposition::Qualified => TemporalSpatialDisposition::Qualified,
        TemporalFusionDisposition::Partial => TemporalSpatialDisposition::Partial,
        TemporalFusionDisposition::Unresolved => TemporalSpatialDisposition::Unresolved,
    }
}

fn disposition_from_registration(
    value: SpatialRegistrationDisposition,
) -> TemporalSpatialDisposition {
    match value {
        SpatialRegistrationDisposition::Qualified => TemporalSpatialDisposition::Qualified,
        SpatialRegistrationDisposition::Partial => TemporalSpatialDisposition::Partial,
        SpatialRegistrationDisposition::Unresolved => TemporalSpatialDisposition::Unresolved,
    }
}

fn disposition_from_propagation(
    value: SpatialPropagationDisposition,
) -> TemporalSpatialDisposition {
    match value {
        SpatialPropagationDisposition::Qualified => TemporalSpatialDisposition::Qualified,
        SpatialPropagationDisposition::Partial => TemporalSpatialDisposition::Partial,
        SpatialPropagationDisposition::Unresolved => TemporalSpatialDisposition::Unresolved,
    }
}

fn support_for(disposition: TemporalSpatialDisposition) -> u16 {
    match disposition {
        TemporalSpatialDisposition::Qualified => 1_000,
        TemporalSpatialDisposition::Partial => 500,
        TemporalSpatialDisposition::Unresolved => 0,
    }
}

fn digest_input(output: &TemporalSpatialAlignment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_timepoint_order": output.sample_timepoint_order,
        "aligned_sample_order": output.aligned_sample_order,
        "aligned_states": output.aligned_states,
        "temporal_gate": output.temporal_gate,
        "registration_gate": output.registration_gate,
        "propagation_gate": output.propagation_gate,
        "alignment_gate": output.alignment_gate,
        "priority_action_order": output.priority_action_order,
        "missing_sample_timepoint_order": output.missing_sample_timepoint_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "temporal": output.temporal,
        "registration": output.registration,
        "propagation": output.propagation,
    })
}

fn validate_request(
    request: &TemporalSpatialAlignmentRequest,
    observations: &[TemporalObservation],
    cells: &[SpatialRegistrationCell],
) -> Result<(), TemporalSpatialAlignmentError> {
    if !valid_identifier(&request.study_id)
        || request.temporal_request.study_id != request.study_id
        || request.registration_request.study_id != request.study_id
        || request.propagation_request.study_id != request.study_id
        || request.temporal_request.model_system != request.model_system
        || request.registration_request.model_system != request.model_system
        || request.propagation_request.model_system != request.model_system
        || request.sample_timepoints.is_empty()
        || request.sample_timepoints.len() > MAX_SAMPLE_TIMEPOINTS
        || request.min_spatial_coverage_milli == 0
        || request.min_spatial_coverage_milli > 1_000
        || request.max_state_gap_milli > MAX_STATE_GAP_MILLI
        || observations.is_empty()
        || cells.is_empty()
    {
        return Err(TemporalSpatialAlignmentError::InvalidRequest(
            "study/model bindings, bounded non-empty observations/cells, coverage floor, and sample-timepoint mappings are required".into(),
        ));
    }
    let mut samples = BTreeSet::new();
    for mapping in &request.sample_timepoints {
        if !valid_identifier(&mapping.sample_id) || !samples.insert(mapping.sample_id.clone()) {
            return Err(TemporalSpatialAlignmentError::InvalidRequest(
                "sample-timepoint mappings must have unique valid sample identifiers".into(),
            ));
        }
    }
    if observations.iter().any(|observation| {
        observation.study_id != request.study_id || observation.model_system != request.model_system
    }) || cells.iter().any(|cell| cell.sample_id.is_empty())
    {
        return Err(TemporalSpatialAlignmentError::InvalidInput(
            "all inputs must remain bound to the declared study and model system".into(),
        ));
    }
    Ok(())
}

fn mean_i64(values: impl Iterator<Item = i64>) -> Option<i64> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(
            values
                .iter()
                .map(|value| i128::from(*value))
                .sum::<i128>()
                .checked_div(i128::try_from(values.len()).ok()?)?
                .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
        )
    }
}

fn abs_gap(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right))
        .unsigned_abs()
        .min(u128::from(u64::MAX)) as u64
}

fn registered_to_spatial(
    registered: &[RegisteredSpatialCell],
    source_cells: &[SpatialRegistrationCell],
) -> Result<Vec<SpatialCell>, TemporalSpatialAlignmentError> {
    let artifacts = source_cells
        .iter()
        .map(|cell| (cell.cell_id.clone(), cell.artifact.clone()))
        .collect::<BTreeMap<_, _>>();
    registered
        .iter()
        .map(|cell| {
            let artifact = artifacts.get(&cell.cell_id).cloned().ok_or_else(|| {
                TemporalSpatialAlignmentError::InvalidInput(
                    "registered cell lacks a source artifact reference".into(),
                )
            })?;
            Ok(SpatialCell {
                cell_id: cell.cell_id.clone(),
                sample_id: cell.sample_id.clone(),
                lineage: cell.lineage.clone(),
                x_milli: cell.aligned_x_milli,
                y_milli: cell.aligned_y_milli,
                state_milli: cell.state_milli,
                artifact,
            })
        })
        .collect()
}

fn gate(
    gate_id: &str,
    disposition: TemporalSpatialDisposition,
    blockers: impl IntoIterator<Item = String>,
) -> AlignmentGate {
    let mut blockers = blockers.into_iter().collect::<Vec<_>>();
    blockers.sort();
    blockers.dedup();
    AlignmentGate {
        gate_id: gate_id.into(),
        disposition,
        support_milli: support_for(disposition),
        blockers,
    }
}

impl TemporalSpatialAlignment {
    pub fn validate(&self) -> Result<(), TemporalSpatialAlignmentError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.sample_timepoint_order)
            || !canonical(&self.aligned_sample_order)
            || self.aligned_sample_order.len() != self.aligned_states.len()
            || self.aligned_states.windows(2).any(|pair| {
                (pair[0].sample_id.as_str(), pair[0].timepoint)
                    >= (pair[1].sample_id.as_str(), pair[1].timepoint)
            })
            || self.aligned_states.iter().any(|state| {
                !valid_identifier(&state.sample_id)
                    || !canonical(&state.registered_cell_order)
                    || !canonical(&state.propagated_hotspot_order)
                    || !canonical(&state.uncertainty)
                    || state.spatial_coverage_milli > 1_000
                    || state
                        .state_gap_milli
                        .is_some_and(|gap| gap > MAX_STATE_GAP_MILLI)
            })
            || !canonical(&self.priority_action_order)
            || !canonical(&self.missing_sample_timepoint_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(TemporalSpatialAlignmentError::InvalidOutput(
                "alignment identity, ordering, coverage, or uncertainty invariants are invalid"
                    .into(),
            ));
        }
        self.temporal
            .validate()
            .map_err(|error| TemporalSpatialAlignmentError::InvalidOutput(error.to_string()))?;
        self.registration
            .validate()
            .map_err(|error| TemporalSpatialAlignmentError::InvalidOutput(error.to_string()))?;
        self.propagation
            .validate()
            .map_err(|error| TemporalSpatialAlignmentError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| TemporalSpatialAlignmentError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(TemporalSpatialAlignmentError::InvalidOutput(
                "alignment digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Join longitudinal modality states to registered spatial propagation without inventing a
/// timepoint.  The output is an executable evidence map and a deterministic follow-up queue.
pub fn analyze_glioma_temporal_spatial_alignment(
    request: &TemporalSpatialAlignmentRequest,
    observations: &[TemporalObservation],
    cells: &[SpatialRegistrationCell],
) -> Result<TemporalSpatialAlignment, TemporalSpatialAlignmentError> {
    validate_request(request, observations, cells)?;
    let temporal =
        analyze_glioma_temporal_multimodal_fusion(&request.temporal_request, observations)
            .map_err(|error| TemporalSpatialAlignmentError::InvalidInput(error.to_string()))?;
    let registration = register_glioma_spatial_samples(&request.registration_request, cells)
        .map_err(|error| TemporalSpatialAlignmentError::InvalidInput(error.to_string()))?;
    let aligned_cells = registered_to_spatial(&registration.registered_cells, cells)?;
    let propagation =
        analyze_glioma_spatial_state_propagation(&request.propagation_request, &aligned_cells)
            .map_err(|error| TemporalSpatialAlignmentError::InvalidInput(error.to_string()))?;

    let temporal_states = temporal
        .states
        .iter()
        .map(|state| ((state.sample_id.clone(), state.timepoint), state))
        .collect::<BTreeMap<_, _>>();
    let source_counts = cells
        .iter()
        .fold(BTreeMap::<String, usize>::new(), |mut counts, cell| {
            *counts.entry(cell.sample_id.clone()).or_default() += 1;
            counts
        });
    let registered_by_sample = registration.registered_cells.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut cells, cell| {
            cells
                .entry(cell.sample_id.clone())
                .or_default()
                .push(cell.cell_id.clone());
            cells
        },
    );
    let trajectories_by_sample = propagation.trajectories.iter().fold(
        BTreeMap::<String, Vec<&_>>::new(),
        |mut values, trajectory| {
            values
                .entry(trajectory.sample_id.clone())
                .or_default()
                .push(trajectory);
            values
        },
    );
    let hotspots = propagation
        .hotspot_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut mappings = request.sample_timepoints.clone();
    mappings.sort_by(|left, right| {
        (left.sample_id.as_str(), left.timepoint).cmp(&(right.sample_id.as_str(), right.timepoint))
    });
    let mut aligned_states = Vec::new();
    let mut missing = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut actions = BTreeSet::new();
    for mapping in mappings {
        let key = (mapping.sample_id.clone(), mapping.timepoint);
        let temporal_state = temporal_states.get(&key).copied();
        let registered = registered_by_sample
            .get(&mapping.sample_id)
            .cloned()
            .unwrap_or_default();
        let trajectories = trajectories_by_sample
            .get(&mapping.sample_id)
            .cloned()
            .unwrap_or_default();
        let temporal_mean = temporal_state
            .and_then(|state| mean_i64(state.features.iter().map(|feature| feature.value_milli)));
        let spatial_mean = mean_i64(
            trajectories
                .iter()
                .filter_map(|trajectory| trajectory.state_order.last().copied()),
        );
        let coverage = source_counts
            .get(&mapping.sample_id)
            .copied()
            .filter(|total| *total > 0)
            .map(|total| {
                (u128::from(registered.len() as u64) * 1_000 / u128::from(total as u64)).min(1_000)
                    as u16
            })
            .unwrap_or(0);
        let gap = temporal_mean
            .zip(spatial_mean)
            .map(|(left, right)| abs_gap(left, right));
        let mut local_uncertainty = BTreeSet::new();
        if temporal_state.is_none() {
            missing.insert(format!(
                "{}@{}:temporal-state",
                mapping.sample_id, mapping.timepoint
            ));
            local_uncertainty.insert("temporal-state-not-observed-at-declared-timepoint".into());
            actions.insert(TemporalSpatialAction::AcquireTemporalState);
        }
        if registered.is_empty() {
            missing.insert(format!(
                "{}@{}:registered-spatial-cells",
                mapping.sample_id, mapping.timepoint
            ));
            local_uncertainty.insert("no-registered-spatial-cells-for-sample".into());
            actions.insert(TemporalSpatialAction::RegisterSpatialSample);
        }
        if coverage < request.min_spatial_coverage_milli {
            local_uncertainty.insert("spatial-coverage-below-declared-floor".into());
            actions.insert(TemporalSpatialAction::CollectSpatialAtTimepoint);
        }
        if let Some(gap) = gap {
            if gap > request.max_state_gap_milli {
                local_uncertainty
                    .insert("temporal-and-spatial-state-gap-exceeds-declared-bound".into());
                actions.insert(TemporalSpatialAction::InvestigateStateMismatch);
            }
        }
        if trajectories
            .iter()
            .any(|trajectory| trajectory.converged_step.is_none())
        {
            local_uncertainty.insert("one-or-more-spatial-trajectories-did-not-converge".into());
            actions.insert(TemporalSpatialAction::ConfirmSpatialPropagation);
        }
        if trajectories.is_empty() {
            negative.insert(format!(
                "{}:no-spatial-propagation-trajectory",
                mapping.sample_id
            ));
        }
        if temporal_state.is_some() && !registered.is_empty() {
            if let Some(gap) = gap {
                if gap <= request.max_state_gap_milli
                    && coverage >= request.min_spatial_coverage_milli
                {
                    negative.insert(format!(
                        "{}@{}:aligned-state-gap-within-bound",
                        mapping.sample_id, mapping.timepoint
                    ));
                }
            }
        }
        let disposition = if temporal_state.is_none() || registered.is_empty() {
            TemporalSpatialDisposition::Unresolved
        } else if coverage < request.min_spatial_coverage_milli
            || gap.is_some_and(|value| value > request.max_state_gap_milli)
            || !trajectories
                .iter()
                .all(|trajectory| trajectory.converged_step.is_some())
        {
            TemporalSpatialDisposition::Partial
        } else {
            TemporalSpatialDisposition::Qualified
        };
        if disposition != TemporalSpatialDisposition::Qualified {
            uncertainty.extend(local_uncertainty.iter().cloned());
        }
        let mut local_uncertainty = local_uncertainty.into_iter().collect::<Vec<_>>();
        local_uncertainty.sort();
        aligned_states.push(AlignedSampleState {
            sample_id: mapping.sample_id,
            timepoint: mapping.timepoint,
            temporal_state_id: temporal_state.map(|state| state.state_id.clone()),
            registered_cell_order: {
                let mut values = registered;
                values.sort();
                values
            },
            propagated_hotspot_order: trajectories
                .iter()
                .filter(|trajectory| hotspots.contains(&trajectory.cell_id))
                .map(|trajectory| trajectory.cell_id.clone())
                .collect(),
            temporal_state_mean_milli: temporal_mean,
            spatial_state_mean_milli: spatial_mean,
            state_gap_milli: gap,
            spatial_coverage_milli: coverage,
            disposition,
            uncertainty: local_uncertainty,
        });
    }
    if aligned_states
        .iter()
        .any(|state| state.disposition != TemporalSpatialDisposition::Qualified)
    {
        actions.remove(&TemporalSpatialAction::PublishAlignedStateMap);
    } else {
        actions.insert(TemporalSpatialAction::PublishAlignedStateMap);
    }
    let temporal_gate = gate(
        "temporal_state_gate",
        disposition_from_temporal(temporal.disposition),
        temporal.uncertainty.clone(),
    );
    let registration_gate = gate(
        "spatial_registration_gate",
        disposition_from_registration(registration.disposition),
        registration.uncertainty.clone(),
    );
    let propagation_gate = gate(
        "spatial_propagation_gate",
        disposition_from_propagation(propagation.disposition),
        propagation.uncertainty.clone(),
    );
    let qualified_count = aligned_states
        .iter()
        .filter(|state| state.disposition == TemporalSpatialDisposition::Qualified)
        .count();
    let alignment_disposition = if qualified_count == aligned_states.len() {
        TemporalSpatialDisposition::Qualified
    } else if qualified_count > 0 {
        TemporalSpatialDisposition::Partial
    } else {
        TemporalSpatialDisposition::Unresolved
    };
    let alignment_gate = gate(
        "temporal_spatial_alignment_gate",
        alignment_disposition,
        uncertainty.iter().cloned(),
    );
    let mut sample_timepoint_order = aligned_states
        .iter()
        .map(|state| format!("{}@{}", state.sample_id, state.timepoint))
        .collect::<Vec<_>>();
    sample_timepoint_order.sort();
    let aligned_sample_order = aligned_states
        .iter()
        .map(|state| state.sample_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut negative_evidence = negative.into_iter().collect::<Vec<_>>();
    negative_evidence.extend(temporal.negative_evidence.iter().cloned());
    negative_evidence.extend(registration.negative_evidence.iter().cloned());
    negative_evidence.extend(propagation.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty.extend(temporal.uncertainty.iter().cloned());
    uncertainty.extend(registration.uncertainty.iter().cloned());
    uncertainty.extend(propagation.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let output = TemporalSpatialAlignment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_timepoint_order,
        aligned_sample_order,
        aligned_states,
        temporal_gate,
        registration_gate,
        propagation_gate,
        alignment_gate,
        priority_action_order: actions.into_iter().collect(),
        missing_sample_timepoint_order: missing.into_iter().collect(),
        negative_evidence,
        uncertainty,
        disposition: alignment_disposition,
        temporal,
        registration,
        propagation,
        digest: ContentHash::of_value(&serde_json::json!({"pending": FEATURE_ID}))
            .map_err(|error| TemporalSpatialAlignmentError::Digest(error.to_string()))?,
    };
    let mut output = output;
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| TemporalSpatialAlignmentError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::{
        FeatureValue, SpatialRegistrationCell,
    };
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

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

    fn request() -> TemporalSpatialAlignmentRequest {
        TemporalSpatialAlignmentRequest {
            study_id: "alignment-study".into(),
            model_system: GliomaModelSystem::Organoid,
            temporal_request: TemporalFusionRequest {
                study_id: "alignment-study".into(),
                model_system: GliomaModelSystem::Organoid,
                required_modalities: [GliomaModality::Imaging].into_iter().collect(),
                min_timepoints_per_sample: 2,
                min_modalities_per_timepoint: 1,
                min_shared_features: 1,
                min_transition_support_milli: 500,
                max_state_change_milli: 500,
                require_complete_time_grid: false,
            },
            registration_request: super::super::spatial_registration::SpatialRegistrationRequest {
                study_id: "alignment-study".into(),
                model_system: GliomaModelSystem::Organoid,
                reference_sample_id: "sample-a".into(),
                min_cells_per_landmark: 1,
                min_shared_lineages: 1,
                max_residual_milli: 100,
                max_landmark_spread_milli: 100,
            },
            propagation_request: super::super::spatial_propagation::SpatialPropagationRequest {
                study_id: "alignment-study".into(),
                model_system: GliomaModelSystem::Organoid,
                radius_milli: 2_000,
                max_steps: 4,
                self_retention_milli: 800,
                neighbor_weight_milli: 200,
                cross_lineage_weight_milli: 1_000,
                convergence_tolerance_milli: 10,
                hotspot_threshold_milli: 100,
            },
            sample_timepoints: vec![SampleTimepoint {
                sample_id: "sample-a".into(),
                timepoint: 0,
            }],
            min_spatial_coverage_milli: 800,
            max_state_gap_milli: 1_000,
        }
    }

    #[test]
    fn aligns_declared_temporal_state_to_registered_spatial_cells() {
        let request = request();
        let observations = vec![
            TemporalObservation {
                observation_id: "obs-a".into(),
                study_id: "alignment-study".into(),
                sample_id: "sample-a".into(),
                timepoint: 0,
                modality: GliomaModality::Imaging,
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("obs-a"),
                features: vec![FeatureValue {
                    feature_id: "state".into(),
                    value_milli: 500,
                }],
            },
            TemporalObservation {
                observation_id: "obs-a-t1".into(),
                study_id: "alignment-study".into(),
                sample_id: "sample-a".into(),
                timepoint: 1,
                modality: GliomaModality::Imaging,
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("obs-a-t1"),
                features: vec![FeatureValue {
                    feature_id: "state".into(),
                    value_milli: 500,
                }],
            },
        ];
        let cells = vec![
            SpatialRegistrationCell {
                cell_id: "cell-a".into(),
                sample_id: "sample-a".into(),
                lineage: "tumour".into(),
                x_milli: 0,
                y_milli: 0,
                state_milli: 500,
                artifact: artifact("cell-a"),
            },
            SpatialRegistrationCell {
                cell_id: "cell-b".into(),
                sample_id: "sample-a".into(),
                lineage: "tumour".into(),
                x_milli: 1_000,
                y_milli: 0,
                state_milli: 500,
                artifact: artifact("cell-b"),
            },
        ];
        let output = analyze_glioma_temporal_spatial_alignment(&request, &observations, &cells)
            .expect("alignment");
        assert_eq!(output.disposition, TemporalSpatialDisposition::Qualified);
        assert_eq!(output.aligned_states[0].state_gap_milli, Some(0));
        assert!(output
            .priority_action_order
            .contains(&TemporalSpatialAction::PublishAlignedStateMap));
    }

    #[test]
    fn preserves_missing_temporal_state_as_unresolved() {
        let mut request = request();
        request.sample_timepoints[0].timepoint = 7;
        let observations = vec![
            TemporalObservation {
                observation_id: "obs-a".into(),
                study_id: "alignment-study".into(),
                sample_id: "sample-a".into(),
                timepoint: 0,
                modality: GliomaModality::Imaging,
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("obs-a"),
                features: vec![FeatureValue {
                    feature_id: "state".into(),
                    value_milli: 500,
                }],
            },
            TemporalObservation {
                observation_id: "obs-a-t1".into(),
                study_id: "alignment-study".into(),
                sample_id: "sample-a".into(),
                timepoint: 1,
                modality: GliomaModality::Imaging,
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("obs-a-t1"),
                features: vec![FeatureValue {
                    feature_id: "state".into(),
                    value_milli: 500,
                }],
            },
        ];
        let cells = vec![SpatialRegistrationCell {
            cell_id: "cell-a".into(),
            sample_id: "sample-a".into(),
            lineage: "tumour".into(),
            x_milli: 0,
            y_milli: 0,
            state_milli: 500,
            artifact: artifact("cell-a"),
        }];
        let output = analyze_glioma_temporal_spatial_alignment(&request, &observations, &cells)
            .expect("alignment remains explicit");
        assert_eq!(
            output.aligned_states[0].disposition,
            TemporalSpatialDisposition::Unresolved
        );
        assert!(output
            .priority_action_order
            .contains(&TemporalSpatialAction::AcquireTemporalState));
        assert!(output
            .missing_sample_timepoint_order
            .iter()
            .any(|value| value.contains("temporal-state")));
    }
}
