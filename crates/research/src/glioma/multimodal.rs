//! Multimodal ingestion and quality control for glioma research objects.
//!
//! The QC contract is intentionally metadata-first.  Providers parse OME-NGFF, AnnData, VCF,
//! image, or assay payloads locally and pass this module de-identified observation metadata.  A
//! missing cell, incompatible coordinate system, batch defect, or high missingness is a visible
//! result instead of an imputation disguised as comparability.

use super::super::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQc1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalObservation {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub coordinate_system: String,
    pub unit_system: String,
    pub missing_fraction_milli: u16,
    pub feature_count: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRequest {
    pub study_id: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub expected_coordinate_system: String,
    pub expected_unit_system: String,
    pub max_missing_fraction_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QcCell {
    pub observation_id: String,
    pub comparable: bool,
    pub defect_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalQcReport {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub cells: Vec<QcCell>,
    pub comparable_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_order: Vec<GliomaModelSystem>,
    pub defect_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultimodalDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalError {
    #[error("multimodal request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("multimodal report is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal digest failed: {0}")]
    Digest(String),
}

fn digest_input(report: &MultimodalQcReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "study_id": report.study_id,
        "cells": report.cells,
        "comparable_order": report.comparable_order,
        "excluded_order": report.excluded_order,
        "missing_modality_order": report.missing_modality_order,
        "missing_model_order": report.missing_model_order,
        "defect_order": report.defect_order,
        "negative_evidence": report.negative_evidence,
        "uncertainty": report.uncertainty,
        "disposition": report.disposition,
    })
}

impl MultimodalQcReport {
    pub fn validate(&self) -> Result<(), MultimodalError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self
                .cells
                .iter()
                .any(|cell| cell.observation_id.trim().is_empty())
            || self
                .comparable_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.excluded_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.defect_order.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .missing_modality_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .missing_model_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
        {
            return Err(MultimodalError::InvalidOutput(
                "identity or canonical ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| MultimodalError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalError::InvalidOutput(
                "digest is not bound to the QC report".into(),
            ));
        }
        Ok(())
    }
}

pub fn harmonize_multimodal_inputs(
    request: &MultimodalRequest,
    observations: &[MultimodalObservation],
) -> Result<MultimodalQcReport, MultimodalError> {
    if request.study_id.trim().is_empty()
        || request.expected_coordinate_system.trim().is_empty()
        || request.expected_unit_system.trim().is_empty()
        || request.max_missing_fraction_milli > 1_000
    {
        return Err(MultimodalError::InvalidRequest(
            "study, coordinate/unit systems, or missingness bound is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut cells = Vec::with_capacity(observations.len());
    let mut comparable = Vec::new();
    let mut excluded = Vec::new();
    let mut all_defects = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut seen_modalities = BTreeSet::new();
    let mut seen_models = BTreeSet::new();
    for observation in observations {
        observation
            .artifact
            .validate()
            .map_err(|e| MultimodalError::InvalidObservation(e.to_string()))?;
        if observation.observation_id.trim().is_empty()
            || observation.study_id != request.study_id
            || observation.sample_lineage.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.coordinate_system.trim().is_empty()
            || observation.unit_system.trim().is_empty()
            || observation.feature_count == 0
            || observation.missing_fraction_milli > 1_000
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(MultimodalError::InvalidObservation(
                "identity, study binding, dimensions, missingness, or uniqueness is invalid".into(),
            ));
        }
        seen_modalities.insert(observation.modality);
        seen_models.insert(observation.model_system);
        let mut defects = BTreeSet::new();
        if observation.coordinate_system != request.expected_coordinate_system {
            defects.insert(format!(
                "{}:coordinate-system-mismatch",
                observation.observation_id
            ));
        }
        if observation.unit_system != request.expected_unit_system {
            defects.insert(format!(
                "{}:unit-system-mismatch",
                observation.observation_id
            ));
        }
        if observation.missing_fraction_milli > request.max_missing_fraction_milli {
            defects.insert(format!(
                "{}:missingness-over-bound",
                observation.observation_id
            ));
        }
        let defect_order = defects.iter().cloned().collect::<Vec<_>>();
        if defect_order.is_empty() {
            comparable.push(observation.observation_id.clone());
        } else {
            excluded.push(observation.observation_id.clone());
            all_defects.extend(defect_order.iter().cloned());
            uncertainty.extend(defect_order.iter().cloned());
        }
        cells.push(QcCell {
            observation_id: observation.observation_id.clone(),
            comparable: defect_order.is_empty(),
            defect_order,
        });
    }
    cells.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    comparable.sort();
    excluded.sort();
    let missing_modality_order = request
        .required_modalities
        .difference(&seen_modalities)
        .copied()
        .collect::<Vec<_>>();
    let missing_model_order = request
        .required_model_systems
        .difference(&seen_models)
        .copied()
        .collect::<Vec<_>>();
    for modality in &missing_modality_order {
        negative.insert(format!("required-modality-missing:{modality:?}"));
    }
    for model in &missing_model_order {
        negative.insert(format!("required-model-missing:{model:?}"));
    }
    if observations.is_empty() {
        negative.insert("no-observations-provided".into());
    }
    let disposition = if comparable.is_empty()
        || !missing_modality_order.is_empty()
        || !missing_model_order.is_empty()
    {
        MultimodalDisposition::Unresolved
    } else if !excluded.is_empty() || !all_defects.is_empty() {
        MultimodalDisposition::Partial
    } else {
        MultimodalDisposition::Qualified
    };
    let mut report = MultimodalQcReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        cells,
        comparable_order: comparable,
        excluded_order: excluded,
        missing_modality_order,
        missing_model_order,
        defect_order: all_defects.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| MultimodalError::Digest(e.to_string()))?,
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|e| MultimodalError::Digest(e.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn observation(id: &str, modality: GliomaModality) -> MultimodalObservation {
        MultimodalObservation {
            observation_id: id.into(),
            study_id: "study-1".into(),
            sample_lineage: "sample-organoid-1".into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            batch_id: "batch-a".into(),
            coordinate_system: "cellular".into(),
            unit_system: "log2-counts".into(),
            missing_fraction_milli: 50,
            feature_count: 100,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-multimodal+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> MultimodalRequest {
        MultimodalRequest {
            study_id: "study-1".into(),
            required_modalities: BTreeSet::from([
                GliomaModality::Genomics,
                GliomaModality::Imaging,
            ]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            expected_coordinate_system: "cellular".into(),
            expected_unit_system: "log2-counts".into(),
            max_missing_fraction_milli: 100,
        }
    }

    #[test]
    fn qc_requires_each_requested_modality_and_preserves_defects() {
        let report = harmonize_multimodal_inputs(
            &request(),
            &[observation("geno", GliomaModality::Genomics)],
        )
        .unwrap();
        assert_eq!(report.disposition, MultimodalDisposition::Unresolved);
        assert_eq!(report.missing_modality_order, vec![GliomaModality::Imaging]);
        assert!(report
            .negative_evidence
            .iter()
            .any(|item| item.contains("required-modality-missing")));
    }

    #[test]
    fn coordinate_and_missingness_defects_are_partial_not_imputed() {
        let mut bad = observation("bad", GliomaModality::Imaging);
        bad.coordinate_system = "voxel".into();
        bad.missing_fraction_milli = 500;
        let report = harmonize_multimodal_inputs(
            &request(),
            &[observation("geno", GliomaModality::Genomics), bad],
        )
        .unwrap();
        assert_eq!(report.disposition, MultimodalDisposition::Partial);
        assert_eq!(report.excluded_order, vec!["bad"]);
        assert_eq!(report.comparable_order, vec!["geno"]);
        report.validate().unwrap();
    }
}
