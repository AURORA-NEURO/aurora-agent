//! Cross-study quality-policy transport calibration for preclinical glioma workflows.
//!
//! A quality threshold learned in one model system or institution is not portable by default.
//! This feature compares bounded, de-identified QC summaries between a source and target study,
//! estimates modality-specific transport confidence, and emits a local-confirmation gate. It
//! transfers QC policy only; it never transports raw data, biology, diagnoses, or clinical rules.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityTransport1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_CELLS: usize = 65_536;
pub const MAX_SAMPLES_PER_CELL: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityTransportCell {
    pub cell_id: String,
    pub study_id: String,
    pub cohort_id: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub quality_milli: u16,
    pub coverage_milli: u16,
    pub alignment_milli: u16,
    pub drift_milli: u32,
    pub sample_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityTransportRequest {
    pub objective: String,
    pub source_study_id: String,
    pub target_study_id: String,
    pub source_model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub source_cells: Vec<QualityTransportCell>,
    pub target_cells: Vec<QualityTransportCell>,
    pub min_cells_per_modality: usize,
    pub min_quality_milli: u16,
    pub min_coverage_milli: u16,
    pub min_alignment_milli: u16,
    pub max_quality_gap_milli: u16,
    pub max_coverage_gap_milli: u16,
    pub max_drift_milli: u32,
    pub min_transfer_confidence_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTransportModalityDisposition {
    Transferable,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTransportDisposition {
    Qualified,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityTransportModalitySummary {
    pub modality: GliomaModality,
    pub source_cell_count: u32,
    pub target_cell_count: u32,
    pub source_quality_milli: Option<u16>,
    pub target_quality_milli: Option<u16>,
    pub quality_gap_milli: Option<u16>,
    pub source_coverage_milli: Option<u16>,
    pub target_coverage_milli: Option<u16>,
    pub coverage_gap_milli: Option<u16>,
    pub target_alignment_milli: Option<u16>,
    pub target_drift_milli: Option<u32>,
    pub transfer_confidence_milli: u16,
    pub disposition: QualityTransportModalityDisposition,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityTransportCalibration {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_study_id: String,
    pub target_study_id: String,
    pub source_model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub summaries: Vec<QualityTransportModalitySummary>,
    pub transfer_order: Vec<GliomaModality>,
    pub blocked_order: Vec<GliomaModality>,
    pub conditional_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QualityTransportDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityTransportError {
    #[error("quality transport request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality transport cell is invalid: {0}")]
    InvalidCell(String),
    #[error("quality transport output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality transport digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median_u16(values: &mut [u16]) -> u16 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

fn median_u32(values: &mut [u32]) -> u32 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

fn gap(left: u16, right: u16) -> u16 {
    left.abs_diff(right)
}

fn digest_input(output: &QualityTransportCalibration) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_study_id": output.source_study_id,
        "target_study_id": output.target_study_id,
        "source_model_system": output.source_model_system,
        "target_model_system": output.target_model_system,
        "modality_order": output.modality_order,
        "summaries": output.summaries,
        "transfer_order": output.transfer_order,
        "blocked_order": output.blocked_order,
        "conditional_order": output.conditional_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_cells(
    cells: &[QualityTransportCell],
    expected_study: &str,
    expected_model: GliomaModelSystem,
    modalities: &BTreeSet<GliomaModality>,
) -> Result<(), QualityTransportError> {
    if cells.len() > MAX_CELLS {
        return Err(QualityTransportError::InvalidRequest(
            "quality transport cell bound exceeded".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for cell in cells {
        if cell.cell_id.trim().is_empty()
            || cell.study_id != expected_study
            || cell.cohort_id.trim().is_empty()
            || cell.model_system != expected_model
            || !modalities.contains(&cell.modality)
            || cell.quality_milli > 1_000
            || cell.coverage_milli > 1_000
            || cell.alignment_milli > 1_000
            || cell.drift_milli > 1_000
            || cell.sample_count == 0
            || cell.sample_count > MAX_SAMPLES_PER_CELL
            || !ids.insert(cell.cell_id.clone())
        {
            return Err(QualityTransportError::InvalidCell(
                "cells require unique IDs, matching study/model bindings, bounded QC metrics, and positive sample counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_request(request: &QualityTransportRequest) -> Result<(), QualityTransportError> {
    if request.objective.trim().is_empty()
        || request.source_study_id.trim().is_empty()
        || request.target_study_id.trim().is_empty()
        || request.source_study_id == request.target_study_id
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || !canonical(&request.required_modalities)
        || request.min_cells_per_modality == 0
        || request.min_cells_per_modality > MAX_CELLS
        || request.min_quality_milli > 1_000
        || request.min_coverage_milli > 1_000
        || request.min_alignment_milli > 1_000
        || request.max_quality_gap_milli > 1_000
        || request.max_coverage_gap_milli > 1_000
        || request.max_drift_milli > 1_000
        || request.min_transfer_confidence_milli > 1_000
    {
        return Err(QualityTransportError::InvalidRequest(
            "distinct source/target studies, canonical modalities, bounded cells, and transport gates are required".into(),
        ));
    }
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    validate_cells(
        &request.source_cells,
        &request.source_study_id,
        request.source_model_system,
        &modalities,
    )?;
    validate_cells(
        &request.target_cells,
        &request.target_study_id,
        request.target_model_system,
        &modalities,
    )?;
    Ok(())
}

fn validate_output(output: &QualityTransportCalibration) -> Result<(), QualityTransportError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.source_study_id.trim().is_empty()
        || output.target_study_id.trim().is_empty()
        || output.source_study_id == output.target_study_id
        || !canonical(&output.modality_order)
        || output.summaries.len() != output.modality_order.len()
        || output
            .summaries
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || !canonical(&output.transfer_order)
        || !canonical(&output.blocked_order)
        || !canonical(&output.conditional_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.summaries.iter().any(|summary| {
            summary.transfer_confidence_milli > 1_000 || summary.next_action.trim().is_empty()
        })
    {
        return Err(QualityTransportError::InvalidOutput(
            "identity, modality ordering, summary cardinality, bounded metrics, or action invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| QualityTransportError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(QualityTransportError::InvalidOutput(
            "quality transport digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityTransportCalibration {
    pub fn validate(&self) -> Result<(), QualityTransportError> {
        validate_output(self)
    }
}

/// Compare source and target QC summaries and decide whether a modality's quality policy can be
/// transferred with local confirmation.
pub fn calibrate_glioma_multimodal_quality_transport(
    request: &QualityTransportRequest,
) -> Result<QualityTransportCalibration, QualityTransportError> {
    validate_request(request)?;
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut source = BTreeMap::<GliomaModality, Vec<&QualityTransportCell>>::new();
    let mut target = BTreeMap::<GliomaModality, Vec<&QualityTransportCell>>::new();
    for cell in &request.source_cells {
        source.entry(cell.modality).or_default().push(cell);
    }
    for cell in &request.target_cells {
        target.entry(cell.modality).or_default().push(cell);
    }
    let mut summaries = Vec::with_capacity(modalities.len());
    let mut transfer = Vec::new();
    let mut blocked = Vec::new();
    let mut conditional = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &modalities {
        let source_cells = source.get(modality).cloned().unwrap_or_default();
        let target_cells = target.get(modality).cloned().unwrap_or_default();
        let source_quality = if source_cells.is_empty() {
            None
        } else {
            Some(median_u16(
                &mut source_cells
                    .iter()
                    .map(|cell| cell.quality_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let target_quality = if target_cells.is_empty() {
            None
        } else {
            Some(median_u16(
                &mut target_cells
                    .iter()
                    .map(|cell| cell.quality_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let source_coverage = if source_cells.is_empty() {
            None
        } else {
            Some(median_u16(
                &mut source_cells
                    .iter()
                    .map(|cell| cell.coverage_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let target_coverage = if target_cells.is_empty() {
            None
        } else {
            Some(median_u16(
                &mut target_cells
                    .iter()
                    .map(|cell| cell.coverage_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let target_alignment = if target_cells.is_empty() {
            None
        } else {
            Some(median_u16(
                &mut target_cells
                    .iter()
                    .map(|cell| cell.alignment_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let target_drift = if target_cells.is_empty() {
            None
        } else {
            Some(median_u32(
                &mut target_cells
                    .iter()
                    .map(|cell| cell.drift_milli)
                    .collect::<Vec<_>>(),
            ))
        };
        let quality_gap = source_quality
            .zip(target_quality)
            .map(|(left, right)| gap(left, right));
        let coverage_gap = source_coverage
            .zip(target_coverage)
            .map(|(left, right)| gap(left, right));
        let sufficient_cells = source_cells.len() >= request.min_cells_per_modality
            && target_cells.len() >= request.min_cells_per_modality;
        let target_quality_ok =
            target_quality.is_some_and(|value| value >= request.min_quality_milli);
        let target_coverage_ok =
            target_coverage.is_some_and(|value| value >= request.min_coverage_milli);
        let alignment_ok =
            target_alignment.is_some_and(|value| value >= request.min_alignment_milli);
        let gaps_ok = quality_gap.is_some_and(|value| value <= request.max_quality_gap_milli)
            && coverage_gap.is_some_and(|value| value <= request.max_coverage_gap_milli);
        let drift_ok = target_drift.is_some_and(|value| value <= request.max_drift_milli);
        let confidence = if !sufficient_cells {
            0
        } else {
            let quality_penalty = quality_gap.unwrap_or(1_000) as u32 * 350 / 1_000;
            let coverage_penalty = coverage_gap.unwrap_or(1_000) as u32 * 250 / 1_000;
            let alignment_penalty =
                target_alignment.map_or(1_000, |value| 1_000 - value) as u32 * 250 / 1_000;
            let drift_penalty = target_drift.unwrap_or(1_000).min(1_000) * 150 / 1_000;
            1_000u32.saturating_sub(
                quality_penalty + coverage_penalty + alignment_penalty + drift_penalty,
            ) as u16
        };
        let disposition = if !sufficient_cells || !target_quality_ok || !target_coverage_ok {
            negative.insert(format!("insufficient-target-calibration:{modality:?}"));
            QualityTransportModalityDisposition::Blocked
        } else if !alignment_ok
            || !gaps_ok
            || !drift_ok
            || confidence < request.min_transfer_confidence_milli
        {
            uncertainty.insert(format!(
                "transport-requires-local-confirmation:{modality:?}"
            ));
            QualityTransportModalityDisposition::Conditional
        } else {
            transfer.push(*modality);
            QualityTransportModalityDisposition::Transferable
        };
        if matches!(disposition, QualityTransportModalityDisposition::Blocked) {
            blocked.push(*modality);
        } else if matches!(
            disposition,
            QualityTransportModalityDisposition::Conditional
        ) {
            conditional.push(*modality);
        }
        let next_action = match disposition {
            QualityTransportModalityDisposition::Transferable => {
                "apply source QC policy with target-local confirmation"
            }
            QualityTransportModalityDisposition::Conditional => {
                "collect target calibration and hold automatic policy transfer"
            }
            QualityTransportModalityDisposition::Blocked => {
                "acquire target QC cells before any policy transfer"
            }
            QualityTransportModalityDisposition::Unresolved => {
                "adjudicate source/target quality provenance before transfer"
            }
        }
        .into();
        summaries.push(QualityTransportModalitySummary {
            modality: *modality,
            source_cell_count: source_cells.len() as u32,
            target_cell_count: target_cells.len() as u32,
            source_quality_milli: source_quality,
            target_quality_milli: target_quality,
            quality_gap_milli: quality_gap,
            source_coverage_milli: source_coverage,
            target_coverage_milli: target_coverage,
            coverage_gap_milli: coverage_gap,
            target_alignment_milli: target_alignment,
            target_drift_milli: target_drift,
            transfer_confidence_milli: confidence,
            disposition,
            next_action,
        });
    }
    summaries.sort_by_key(|summary| summary.modality);
    transfer.sort();
    blocked.sort();
    conditional.sort();
    let disposition = if !blocked.is_empty() {
        QualityTransportDisposition::Blocked
    } else if !conditional.is_empty() {
        QualityTransportDisposition::Conditional
    } else if !transfer.is_empty() {
        QualityTransportDisposition::Qualified
    } else {
        QualityTransportDisposition::Unresolved
    };
    let next_action = match disposition {
        QualityTransportDisposition::Qualified => {
            "release QC policy as a target-local confirmation candidate"
        }
        QualityTransportDisposition::Conditional => {
            "run target-local calibration before enabling transferred QC thresholds"
        }
        QualityTransportDisposition::Blocked => {
            "block policy transfer and acquire missing or failed target calibration"
        }
        QualityTransportDisposition::Unresolved => {
            "hold transport for source/target evidence adjudication"
        }
    }
    .into();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let mut output = QualityTransportCalibration {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_study_id: request.source_study_id.clone(),
        target_study_id: request.target_study_id.clone(),
        source_model_system: request.source_model_system,
        target_model_system: request.target_model_system,
        modality_order,
        summaries,
        transfer_order: transfer,
        blocked_order: blocked,
        conditional_order: conditional,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-transport"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| QualityTransportError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(
        study: &str,
        modality: GliomaModality,
        suffix: &str,
        quality: u16,
    ) -> QualityTransportCell {
        QualityTransportCell {
            cell_id: format!("{study}-{suffix}"),
            study_id: study.into(),
            cohort_id: format!("cohort-{suffix}"),
            model_system: GliomaModelSystem::Organoid,
            modality,
            quality_milli: quality,
            coverage_milli: 900,
            alignment_milli: 950,
            drift_milli: 20,
            sample_count: 10,
        }
    }

    fn request(
        source: Vec<QualityTransportCell>,
        target: Vec<QualityTransportCell>,
    ) -> QualityTransportRequest {
        QualityTransportRequest {
            objective: "calibrate QC policy transport between glioma studies".into(),
            source_study_id: "source-study".into(),
            target_study_id: "target-study".into(),
            source_model_system: GliomaModelSystem::Organoid,
            target_model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            source_cells: source,
            target_cells: target,
            min_cells_per_modality: 2,
            min_quality_milli: 700,
            min_coverage_milli: 700,
            min_alignment_milli: 800,
            max_quality_gap_milli: 100,
            max_coverage_gap_milli: 150,
            max_drift_milli: 100,
            min_transfer_confidence_milli: 700,
        }
    }

    #[test]
    fn qualifies_stable_source_target_quality_transport() {
        let output = calibrate_glioma_multimodal_quality_transport(&request(
            vec![
                cell("source-study", GliomaModality::Genomics, "g1", 900),
                cell("source-study", GliomaModality::Genomics, "g2", 900),
                cell("source-study", GliomaModality::Imaging, "i1", 850),
                cell("source-study", GliomaModality::Imaging, "i2", 850),
            ],
            vec![
                cell("target-study", GliomaModality::Genomics, "g1", 880),
                cell("target-study", GliomaModality::Genomics, "g2", 880),
                cell("target-study", GliomaModality::Imaging, "i1", 830),
                cell("target-study", GliomaModality::Imaging, "i2", 830),
            ],
        ))
        .expect("transport calibration");
        assert_eq!(output.disposition, QualityTransportDisposition::Qualified);
        assert_eq!(output.transfer_order.len(), 2);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_missing_target_modality_without_zero_imputation() {
        let output = calibrate_glioma_multimodal_quality_transport(&request(
            vec![
                cell("source-study", GliomaModality::Genomics, "g1", 900),
                cell("source-study", GliomaModality::Genomics, "g2", 900),
                cell("source-study", GliomaModality::Imaging, "i1", 850),
                cell("source-study", GliomaModality::Imaging, "i2", 850),
            ],
            vec![
                cell("target-study", GliomaModality::Genomics, "g1", 880),
                cell("target-study", GliomaModality::Genomics, "g2", 880),
            ],
        ))
        .expect("blocked transport");
        assert_eq!(output.disposition, QualityTransportDisposition::Blocked);
        assert_eq!(output.blocked_order, vec![GliomaModality::Imaging]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("insufficient-target-calibration")));
    }

    #[test]
    fn flags_alignment_or_drift_as_conditional_local_confirmation() {
        let mut target = vec![
            cell("target-study", GliomaModality::Genomics, "g1", 880),
            cell("target-study", GliomaModality::Genomics, "g2", 880),
            cell("target-study", GliomaModality::Imaging, "i1", 830),
            cell("target-study", GliomaModality::Imaging, "i2", 830),
        ];
        target[0].alignment_milli = 500;
        let output = calibrate_glioma_multimodal_quality_transport(&request(
            vec![
                cell("source-study", GliomaModality::Genomics, "g1", 900),
                cell("source-study", GliomaModality::Genomics, "g2", 900),
                cell("source-study", GliomaModality::Imaging, "i1", 850),
                cell("source-study", GliomaModality::Imaging, "i2", 850),
            ],
            target,
        ))
        .expect("conditional transport");
        assert_eq!(output.disposition, QualityTransportDisposition::Conditional);
        assert!(output.conditional_order.contains(&GliomaModality::Genomics));
    }
}
