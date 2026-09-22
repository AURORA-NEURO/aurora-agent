//! Robust landmark registration for multi-sample preclinical glioma spatial assays.
//!
//! Spatial niches and communication are only comparable across samples when coordinates share a
//! declared frame.  This feature estimates a bounded translation/isotropic-scale transform from
//! repeated lineage landmarks, applies it to institution-local cells, and preserves landmark
//! residuals, missing-lineage coverage, and the fact that rotation/affine structure was not
//! modeled.  It is a registration product for downstream research analysis, not a clinical image
//! interpretation or a claim that geometry establishes mechanism.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaSpatialRegistration1@1";
pub const MAX_CELLS: usize = 65_536;
pub const MAX_SAMPLES: usize = 512;
pub const MAX_LINEAGES: usize = 4_096;
pub const MAX_COORDINATE_MILLI: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialRegistrationRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub reference_sample_id: String,
    pub min_cells_per_landmark: usize,
    pub min_shared_lineages: usize,
    pub max_residual_milli: u64,
    pub max_landmark_spread_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialRegistrationCell {
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
pub enum SampleRegistrationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationLandmark {
    pub sample_id: String,
    pub lineage: String,
    pub observed_x_milli: i64,
    pub observed_y_milli: i64,
    pub reference_x_milli: i64,
    pub reference_y_milli: i64,
    pub aligned_x_milli: i64,
    pub aligned_y_milli: i64,
    pub cell_count: usize,
    pub residual_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleRegistration {
    pub sample_id: String,
    pub landmark_order: Vec<String>,
    pub shared_lineage_order: Vec<String>,
    pub missing_reference_lineage_order: Vec<String>,
    pub translation_x_milli: i64,
    pub translation_y_milli: i64,
    pub scale_milli: u32,
    pub max_residual_milli: u64,
    pub residual_mad_milli: u64,
    pub disposition: SampleRegistrationDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredSpatialCell {
    pub cell_id: String,
    pub sample_id: String,
    pub lineage: String,
    pub original_x_milli: i64,
    pub original_y_milli: i64,
    pub aligned_x_milli: i64,
    pub aligned_y_milli: i64,
    pub state_milli: i64,
    pub landmark_residual_milli: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialRegistrationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialRegistrationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub reference_sample_id: String,
    pub sample_order: Vec<String>,
    pub registered_cell_order: Vec<String>,
    pub unregistered_cell_order: Vec<String>,
    pub landmarks: Vec<RegistrationLandmark>,
    pub samples: Vec<SampleRegistration>,
    pub registered_cells: Vec<RegisteredSpatialCell>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SpatialRegistrationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpatialRegistrationError {
    #[error("spatial registration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("spatial registration cell is invalid: {0}")]
    InvalidCell(String),
    #[error("spatial registration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("spatial registration digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median_i64(values: &[i64]) -> i64 {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

fn median_u64(values: &[u64]) -> u64 {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
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

fn distance(left_x: i64, left_y: i64, right_x: i64, right_y: i64) -> u64 {
    let dx = i128::from(left_x) - i128::from(right_x);
    let dy = i128::from(left_y) - i128::from(right_y);
    integer_sqrt(
        dx.unsigned_abs()
            .saturating_mul(dx.unsigned_abs())
            .saturating_add(dy.unsigned_abs().saturating_mul(dy.unsigned_abs())),
    )
    .min(u128::from(u64::MAX)) as u64
}

fn landmark_key(sample_id: &str, lineage: &str) -> String {
    format!("{sample_id}:{lineage}")
}

fn apply_transform(x: i64, y: i64, scale_milli: u32, tx: i64, ty: i64) -> (i64, i64) {
    let x = (i128::from(x) * i128::from(scale_milli) / 1_000_i128)
        .saturating_add(i128::from(tx))
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    let y = (i128::from(y) * i128::from(scale_milli) / 1_000_i128)
        .saturating_add(i128::from(ty))
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    (x, y)
}

fn digest_input(output: &SpatialRegistrationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "reference_sample_id": output.reference_sample_id,
        "sample_order": output.sample_order,
        "registered_cell_order": output.registered_cell_order,
        "unregistered_cell_order": output.unregistered_cell_order,
        "landmarks": output.landmarks,
        "samples": output.samples,
        "registered_cells": output.registered_cells,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &SpatialRegistrationRequest,
    cells: &[SpatialRegistrationCell],
) -> Result<(), SpatialRegistrationError> {
    if request.study_id.trim().is_empty()
        || request.reference_sample_id.trim().is_empty()
        || request.min_cells_per_landmark == 0
        || request.min_shared_lineages == 0
        || request.max_residual_milli == 0
        || request.max_landmark_spread_milli == 0
        || cells.is_empty()
        || cells.len() > MAX_CELLS
    {
        return Err(SpatialRegistrationError::InvalidRequest(
            "study/reference identity, positive landmark floors, residual gates, and bounded cells are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut samples = BTreeSet::new();
    let mut lineages = BTreeSet::new();
    for cell in cells {
        cell.artifact
            .validate()
            .map_err(|error| SpatialRegistrationError::InvalidCell(error.to_string()))?;
        if cell.cell_id.trim().is_empty()
            || cell.sample_id.trim().is_empty()
            || cell.lineage.trim().is_empty()
            || cell.x_milli.unsigned_abs() > MAX_COORDINATE_MILLI as u64
            || cell.y_milli.unsigned_abs() > MAX_COORDINATE_MILLI as u64
            || !ids.insert(cell.cell_id.clone())
        {
            return Err(SpatialRegistrationError::InvalidCell(
                "cell identity, lineage, coordinate bound, artifact, or uniqueness is invalid"
                    .into(),
            ));
        }
        samples.insert(cell.sample_id.clone());
        lineages.insert(cell.lineage.clone());
    }
    if samples.len() > MAX_SAMPLES || lineages.len() > MAX_LINEAGES {
        return Err(SpatialRegistrationError::InvalidCell(
            "sample or lineage bound exceeded".into(),
        ));
    }
    if !samples.contains(&request.reference_sample_id) {
        return Err(SpatialRegistrationError::InvalidRequest(
            "reference sample is absent from the supplied cells".into(),
        ));
    }
    Ok(())
}

impl SpatialRegistrationAnalysis {
    pub fn validate(&self) -> Result<(), SpatialRegistrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.reference_sample_id.trim().is_empty()
            || !canonical(&self.sample_order)
            || !canonical(&self.registered_cell_order)
            || !canonical(&self.unregistered_cell_order)
            || self
                .registered_cell_order
                .iter()
                .any(|id| self.unregistered_cell_order.binary_search(id).is_ok())
            || self.landmarks.windows(2).any(|pair| {
                landmark_key(&pair[0].sample_id, &pair[0].lineage)
                    >= landmark_key(&pair[1].sample_id, &pair[1].lineage)
            })
            || self
                .samples
                .windows(2)
                .any(|pair| pair[0].sample_id >= pair[1].sample_id)
            || self
                .registered_cells
                .windows(2)
                .any(|pair| pair[0].cell_id >= pair[1].cell_id)
            || self.registered_cells.len() != self.registered_cell_order.len()
            || self
                .registered_cells
                .iter()
                .map(|cell| cell.cell_id.clone())
                .collect::<Vec<_>>()
                != self.registered_cell_order
            || self.landmarks.iter().any(|landmark| {
                landmark.sample_id.trim().is_empty()
                    || landmark.lineage.trim().is_empty()
                    || landmark.cell_count == 0
                    || landmark.residual_milli > MAX_COORDINATE_MILLI as u64 * 3
            })
            || self.samples.iter().any(|sample| {
                sample.sample_id.trim().is_empty()
                    || sample
                        .landmark_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || sample
                        .shared_lineage_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || sample
                        .missing_reference_lineage_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || sample.scale_milli == 0
                    || sample.uncertainty.iter().any(|item| item.trim().is_empty())
            })
        {
            return Err(SpatialRegistrationError::InvalidOutput(
                "identity, ordering, partition, transform, residual, or landmark bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SpatialRegistrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SpatialRegistrationError::InvalidOutput(
                "spatial registration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Register multiple local spatial samples to one declared reference frame.
pub fn register_glioma_spatial_samples(
    request: &SpatialRegistrationRequest,
    cells: &[SpatialRegistrationCell],
) -> Result<SpatialRegistrationAnalysis, SpatialRegistrationError> {
    validate_request(request, cells)?;
    let mut groups = BTreeMap::<(String, String), Vec<&SpatialRegistrationCell>>::new();
    for cell in cells {
        groups
            .entry((cell.sample_id.clone(), cell.lineage.clone()))
            .or_default()
            .push(cell);
    }
    let reference_lineages = groups
        .keys()
        .filter(|(sample_id, _)| sample_id == &request.reference_sample_id)
        .map(|(_, lineage)| lineage.clone())
        .collect::<BTreeSet<_>>();
    let mut reference_centroids = BTreeMap::<String, (i64, i64)>::new();
    for lineage in &reference_lineages {
        if let Some(values) = groups.get(&(request.reference_sample_id.clone(), lineage.clone())) {
            reference_centroids.insert(
                lineage.clone(),
                (
                    median_i64(&values.iter().map(|cell| cell.x_milli).collect::<Vec<_>>()),
                    median_i64(&values.iter().map(|cell| cell.y_milli).collect::<Vec<_>>()),
                ),
            );
        }
    }
    let sample_order = groups
        .keys()
        .map(|(sample_id, _)| sample_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut landmarks = Vec::new();
    let mut registrations = Vec::new();
    let mut transforms = BTreeMap::<String, (u32, i64, i64, BTreeMap<String, u64>)>::new();
    let mut global_uncertainty = BTreeSet::new();
    let mut global_negative = BTreeSet::new();
    for sample_id in &sample_order {
        let sample_lineages = groups
            .keys()
            .filter(|(candidate_sample, _)| candidate_sample == sample_id)
            .map(|(_, lineage)| lineage.clone())
            .collect::<BTreeSet<_>>();
        let shared = sample_lineages
            .intersection(&reference_lineages)
            .filter(|lineage| {
                groups
                    .get(&(sample_id.clone(), (*lineage).clone()))
                    .is_some_and(|values| values.len() >= request.min_cells_per_landmark)
                    && groups
                        .get(&(request.reference_sample_id.clone(), (*lineage).clone()))
                        .is_some_and(|values| values.len() >= request.min_cells_per_landmark)
            })
            .cloned()
            .collect::<Vec<_>>();
        let missing = reference_lineages
            .difference(&sample_lineages)
            .cloned()
            .collect::<Vec<_>>();
        let mut uncertainty = BTreeSet::new();
        if sample_id != &request.reference_sample_id {
            uncertainty.insert("rotation-and-shear-not-modeled".into());
        }
        if shared.len() < request.min_shared_lineages {
            uncertainty.insert("shared-lineage-landmark-floor-not-met".into());
            global_uncertainty.insert(format!("{sample_id}:insufficient-shared-lineages"));
        }
        if !missing.is_empty() {
            uncertainty.insert("one-or-more-reference-lineages-absent".into());
        }
        let (scale_milli, tx, ty) = if sample_id == &request.reference_sample_id
            || shared.is_empty()
        {
            (1_000, 0, 0)
        } else {
            let sample_anchor_x = median_i64(
                &shared
                    .iter()
                    .filter_map(|lineage| {
                        groups
                            .get(&(sample_id.clone(), lineage.clone()))
                            .map(|values| {
                                median_i64(
                                    &values.iter().map(|cell| cell.x_milli).collect::<Vec<_>>(),
                                )
                            })
                    })
                    .collect::<Vec<_>>(),
            );
            let sample_anchor_y = median_i64(
                &shared
                    .iter()
                    .filter_map(|lineage| {
                        groups
                            .get(&(sample_id.clone(), lineage.clone()))
                            .map(|values| {
                                median_i64(
                                    &values.iter().map(|cell| cell.y_milli).collect::<Vec<_>>(),
                                )
                            })
                    })
                    .collect::<Vec<_>>(),
            );
            let reference_anchor_x = median_i64(
                &shared
                    .iter()
                    .filter_map(|lineage| reference_centroids.get(lineage).map(|value| value.0))
                    .collect::<Vec<_>>(),
            );
            let reference_anchor_y = median_i64(
                &shared
                    .iter()
                    .filter_map(|lineage| reference_centroids.get(lineage).map(|value| value.1))
                    .collect::<Vec<_>>(),
            );
            let mut ratios = Vec::new();
            for lineage in &shared {
                let Some(values) = groups.get(&(sample_id.clone(), lineage.clone())) else {
                    continue;
                };
                let Some((reference_x, reference_y)) = reference_centroids.get(lineage) else {
                    continue;
                };
                let observed_x =
                    median_i64(&values.iter().map(|cell| cell.x_milli).collect::<Vec<_>>());
                let observed_y =
                    median_i64(&values.iter().map(|cell| cell.y_milli).collect::<Vec<_>>());
                let observed_radius =
                    distance(observed_x, observed_y, sample_anchor_x, sample_anchor_y);
                let reference_radius = distance(
                    *reference_x,
                    *reference_y,
                    reference_anchor_x,
                    reference_anchor_y,
                );
                if observed_radius > 0 && reference_radius > 0 {
                    ratios.push(
                        (u128::from(reference_radius) * 1_000 / u128::from(observed_radius)) as u64,
                    );
                }
            }
            let scale = median_u64(&ratios).clamp(250, 4_000) as u32;
            let (scaled_x, scaled_y) =
                apply_transform(sample_anchor_x, sample_anchor_y, scale, 0, 0);
            (
                scale,
                reference_anchor_x.saturating_sub(scaled_x),
                reference_anchor_y.saturating_sub(scaled_y),
            )
        };
        let mut residual_by_lineage = BTreeMap::new();
        let mut sample_landmark_order = Vec::new();
        let mut sample_landmarks = Vec::new();
        for lineage in &shared {
            let Some(values) = groups.get(&(sample_id.clone(), lineage.clone())) else {
                continue;
            };
            let Some((reference_x, reference_y)) = reference_centroids.get(lineage) else {
                continue;
            };
            let observed_x =
                median_i64(&values.iter().map(|cell| cell.x_milli).collect::<Vec<_>>());
            let observed_y =
                median_i64(&values.iter().map(|cell| cell.y_milli).collect::<Vec<_>>());
            let (aligned_x, aligned_y) =
                apply_transform(observed_x, observed_y, scale_milli, tx, ty);
            let residual = distance(aligned_x, aligned_y, *reference_x, *reference_y);
            residual_by_lineage.insert(lineage.clone(), residual);
            let key = landmark_key(sample_id, lineage);
            sample_landmark_order.push(key);
            sample_landmarks.push(RegistrationLandmark {
                sample_id: sample_id.clone(),
                lineage: lineage.clone(),
                observed_x_milli: observed_x,
                observed_y_milli: observed_y,
                reference_x_milli: *reference_x,
                reference_y_milli: *reference_y,
                aligned_x_milli: aligned_x,
                aligned_y_milli: aligned_y,
                cell_count: values.len(),
                residual_milli: residual,
            });
        }
        let residuals = sample_landmarks
            .iter()
            .map(|landmark| landmark.residual_milli)
            .collect::<Vec<_>>();
        let max_residual = residuals.iter().copied().max().unwrap_or(0);
        let residual_mad = if residuals.is_empty() {
            0
        } else {
            let center = median_u64(&residuals);
            median_u64(
                &residuals
                    .iter()
                    .map(|value| value.abs_diff(center))
                    .collect::<Vec<_>>(),
            )
        };
        let spread = max_residual.saturating_sub(residuals.iter().copied().min().unwrap_or(0));
        if max_residual > request.max_residual_milli {
            uncertainty.insert("landmark-residual-exceeds-gate".into());
            global_negative.insert(format!("{sample_id}:landmark-residual-exceeds-gate"));
        }
        if spread > request.max_landmark_spread_milli {
            uncertainty.insert("landmark-residual-spread-exceeds-gate".into());
        }
        let disposition = if shared.len() < request.min_shared_lineages {
            SampleRegistrationDisposition::Unresolved
        } else if max_residual > request.max_residual_milli
            || spread > request.max_landmark_spread_milli
        {
            SampleRegistrationDisposition::Partial
        } else {
            SampleRegistrationDisposition::Qualified
        };
        if disposition == SampleRegistrationDisposition::Unresolved {
            global_uncertainty.insert(format!("{sample_id}:registration-unresolved"));
        }
        transforms.insert(
            sample_id.clone(),
            (scale_milli, tx, ty, residual_by_lineage),
        );
        registrations.push(SampleRegistration {
            sample_id: sample_id.clone(),
            landmark_order: sample_landmark_order,
            shared_lineage_order: shared,
            missing_reference_lineage_order: missing,
            translation_x_milli: tx,
            translation_y_milli: ty,
            scale_milli,
            max_residual_milli: max_residual,
            residual_mad_milli: residual_mad,
            disposition,
            uncertainty: uncertainty.into_iter().collect(),
        });
        landmarks.extend(sample_landmarks);
    }
    landmarks.sort_by(|left, right| {
        landmark_key(&left.sample_id, &left.lineage)
            .cmp(&landmark_key(&right.sample_id, &right.lineage))
    });
    registrations.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    let mut registered_cells = Vec::new();
    let mut unregistered_cell_order = Vec::new();
    for cell in cells {
        let Some((scale, tx, ty, residual_by_lineage)) = transforms.get(&cell.sample_id) else {
            unregistered_cell_order.push(cell.cell_id.clone());
            continue;
        };
        let sample_disposition = registrations
            .iter()
            .find(|sample| sample.sample_id == cell.sample_id)
            .map(|sample| sample.disposition)
            .unwrap_or(SampleRegistrationDisposition::Unresolved);
        if sample_disposition == SampleRegistrationDisposition::Unresolved {
            unregistered_cell_order.push(cell.cell_id.clone());
            continue;
        }
        let (aligned_x, aligned_y) = apply_transform(cell.x_milli, cell.y_milli, *scale, *tx, *ty);
        registered_cells.push(RegisteredSpatialCell {
            cell_id: cell.cell_id.clone(),
            sample_id: cell.sample_id.clone(),
            lineage: cell.lineage.clone(),
            original_x_milli: cell.x_milli,
            original_y_milli: cell.y_milli,
            aligned_x_milli: aligned_x,
            aligned_y_milli: aligned_y,
            state_milli: cell.state_milli,
            landmark_residual_milli: residual_by_lineage.get(&cell.lineage).copied(),
        });
    }
    registered_cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    unregistered_cell_order.sort();
    let registered_cell_order = registered_cells
        .iter()
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    let disposition = if registrations
        .iter()
        .any(|sample| sample.disposition == SampleRegistrationDisposition::Unresolved)
    {
        SpatialRegistrationDisposition::Unresolved
    } else if registrations
        .iter()
        .any(|sample| sample.disposition == SampleRegistrationDisposition::Partial)
    {
        SpatialRegistrationDisposition::Partial
    } else {
        SpatialRegistrationDisposition::Qualified
    };
    let mut output = SpatialRegistrationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        reference_sample_id: request.reference_sample_id.clone(),
        sample_order,
        registered_cell_order,
        unregistered_cell_order,
        landmarks,
        samples: registrations,
        registered_cells,
        negative_evidence: global_negative.into_iter().collect(),
        uncertainty: global_uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-spatial-registration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SpatialRegistrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-spatial-registration+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn cell(id: &str, sample: &str, lineage: &str, x: i64, y: i64) -> SpatialRegistrationCell {
        SpatialRegistrationCell {
            cell_id: id.into(),
            sample_id: sample.into(),
            lineage: lineage.into(),
            x_milli: x,
            y_milli: y,
            state_milli: 500,
            artifact: artifact(id),
        }
    }

    fn request() -> SpatialRegistrationRequest {
        SpatialRegistrationRequest {
            study_id: "spatial-registration-study".into(),
            model_system: GliomaModelSystem::Organoid,
            reference_sample_id: "reference".into(),
            min_cells_per_landmark: 1,
            min_shared_lineages: 2,
            max_residual_milli: 20,
            max_landmark_spread_milli: 20,
        }
    }

    fn cells() -> Vec<SpatialRegistrationCell> {
        vec![
            cell("r-a", "reference", "tumour", 0, 0),
            cell("r-b", "reference", "myeloid", 1000, 0),
            cell("r-c", "reference", "astro", 0, 1000),
            cell("s-a", "shifted", "tumour", 100, 200),
            cell("s-b", "shifted", "myeloid", 1100, 200),
            cell("s-c", "shifted", "astro", 100, 1200),
        ]
    }

    #[test]
    fn registration_recovers_translation_and_replays() {
        let first = register_glioma_spatial_samples(&request(), &cells()).unwrap();
        let second = register_glioma_spatial_samples(&request(), &cells()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, SpatialRegistrationDisposition::Qualified);
        assert!(first
            .registered_cells
            .iter()
            .any(|cell| cell.cell_id == "s-a"
                && cell.aligned_x_milli == 0
                && cell.aligned_y_milli == 0));
        assert!(first.unregistered_cell_order.is_empty());
        first.validate().unwrap();
    }

    #[test]
    fn registration_preserves_missing_landmark_uncertainty() {
        let mut cells = cells();
        cells.retain(|cell| !(cell.sample_id == "shifted" && cell.lineage == "astro"));
        let mut request = request();
        request.min_shared_lineages = 3;
        let output = register_glioma_spatial_samples(&request, &cells).unwrap();
        assert_eq!(
            output.disposition,
            SpatialRegistrationDisposition::Unresolved
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("insufficient-shared-lineages")));
        assert!(output.unregistered_cell_order.iter().any(|id| id == "s-a"));
    }

    #[test]
    fn registration_refuses_human_artifacts() {
        let mut cells = cells();
        cells[0].artifact.contains_human_data = true;
        assert!(matches!(
            register_glioma_spatial_samples(&request(), &cells),
            Err(SpatialRegistrationError::InvalidCell(_))
        ));
    }
}
