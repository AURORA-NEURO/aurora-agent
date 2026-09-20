//! Cross-study fusion of typed glioma protocol evidence surfaces.
//!
//! The protocol evidence surface is intentionally local and study-scoped. This feature provides
//! the next scientific layer: it compares those surfaces across preclinical model systems,
//! modalities, and institution-local study labels without moving raw data. Robust medians,
//! heterogeneity, sign consistency, and explicit missing/contradictory states determine whether
//! a finding can be transported to a broader research program.

use super::evidence_surface::{ProtocolEvidenceDisposition, ProtocolEvidenceSurface};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolMultiStudyFusion1@1";
pub const MAX_STUDIES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceStudySurface {
    pub study_id: String,
    pub site_id: String,
    pub model_system: GliomaModelSystem,
    pub surface: ProtocolEvidenceSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceFusionRequest {
    pub objective: String,
    pub studies: Vec<ProtocolEvidenceStudySurface>,
    pub min_studies: u16,
    pub min_quality_milli: u16,
    pub max_heterogeneity_milli: u32,
    pub contradiction_threshold_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFusionDisposition {
    Qualified,
    Negative,
    Partial,
    Heterogeneous,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolFusionCell {
    pub endpoint_id: String,
    pub study_order: Vec<String>,
    pub site_order: Vec<String>,
    pub model_order: Vec<GliomaModelSystem>,
    pub modality_order: Vec<String>,
    pub measured_study_count: u16,
    pub qualified_study_count: u16,
    pub negative_study_count: u16,
    pub partial_study_count: u16,
    pub unresolved_study_count: u16,
    pub robust_value_milli: Option<i32>,
    pub heterogeneity_milli: u32,
    pub sign_consistency_milli: u16,
    pub quality_milli: u16,
    pub information_milli: u32,
    pub disposition: ProtocolFusionDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEvidenceFusionDisposition {
    Qualified,
    Negative,
    Partial,
    Heterogeneous,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolEvidenceFusion {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub surface_digest_order: Vec<String>,
    pub cells: Vec<ProtocolFusionCell>,
    pub qualified_endpoint_order: Vec<String>,
    pub negative_endpoint_order: Vec<String>,
    pub partial_endpoint_order: Vec<String>,
    pub heterogeneous_endpoint_order: Vec<String>,
    pub contradictory_endpoint_order: Vec<String>,
    pub unresolved_endpoint_order: Vec<String>,
    pub overall_information_milli: u32,
    pub next_actions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProtocolEvidenceFusionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolEvidenceFusionError {
    #[error("protocol evidence fusion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol evidence fusion input is invalid: {0}")]
    InvalidInput(String),
    #[error("protocol evidence fusion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol evidence fusion digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(fusion: &ProtocolEvidenceFusion) -> serde_json::Value {
    serde_json::json!({
        "feature_id": fusion.feature_id,
        "output_schema": fusion.output_schema,
        "objective": fusion.objective,
        "surface_digest_order": fusion.surface_digest_order,
        "cells": fusion.cells,
        "qualified_endpoint_order": fusion.qualified_endpoint_order,
        "negative_endpoint_order": fusion.negative_endpoint_order,
        "partial_endpoint_order": fusion.partial_endpoint_order,
        "heterogeneous_endpoint_order": fusion.heterogeneous_endpoint_order,
        "contradictory_endpoint_order": fusion.contradictory_endpoint_order,
        "unresolved_endpoint_order": fusion.unresolved_endpoint_order,
        "overall_information_milli": fusion.overall_information_milli,
        "next_actions": fusion.next_actions,
        "negative_evidence": fusion.negative_evidence,
        "uncertainty": fusion.uncertainty,
        "disposition": fusion.disposition,
    })
}

fn median_i32(values: &mut [i32]) -> i32 {
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        ((i64::from(values[middle - 1]) + i64::from(values[middle])) / 2) as i32
    }
}

fn validate_request(
    request: &ProtocolEvidenceFusionRequest,
) -> Result<(), ProtocolEvidenceFusionError> {
    if request.objective.trim().is_empty()
        || request.studies.is_empty()
        || request.studies.len() > MAX_STUDIES
        || request.min_studies == 0
        || request.min_studies as usize > request.studies.len()
        || request.min_quality_milli > 1_000
        || request.contradiction_threshold_milli == 0
    {
        return Err(ProtocolEvidenceFusionError::InvalidRequest(
            "objective, study bounds, minimum study support, quality, and contradiction thresholds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_fusion(fusion: &ProtocolEvidenceFusion) -> Result<(), ProtocolEvidenceFusionError> {
    if fusion.feature_id != FEATURE_ID
        || fusion.output_schema != OUTPUT_SCHEMA
        || fusion.objective.trim().is_empty()
        || !canonical(&fusion.surface_digest_order)
        || fusion
            .surface_digest_order
            .iter()
            .any(|digest| digest.len() != 64)
        || fusion.cells.is_empty()
        || !canonical(&fusion.qualified_endpoint_order)
        || !canonical(&fusion.negative_endpoint_order)
        || !canonical(&fusion.partial_endpoint_order)
        || !canonical(&fusion.heterogeneous_endpoint_order)
        || !canonical(&fusion.contradictory_endpoint_order)
        || !canonical(&fusion.unresolved_endpoint_order)
        || !canonical(&fusion.next_actions)
        || !canonical(&fusion.negative_evidence)
        || !canonical(&fusion.uncertainty)
        || fusion
            .cells
            .windows(2)
            .any(|pair| pair[0].endpoint_id >= pair[1].endpoint_id)
        || fusion.cells.iter().any(|cell| {
            cell.endpoint_id.trim().is_empty()
                || !canonical(&cell.study_order)
                || !canonical(&cell.site_order)
                || !canonical(&cell.model_order)
                || !canonical(&cell.modality_order)
                || cell.measured_study_count == 0
                || cell.site_order.is_empty()
                || cell.quality_milli > 1_000
                || cell.sign_consistency_milli > 1_000
                || cell.information_milli > 1_000_000
                || cell.next_action.trim().is_empty()
                || !canonical(&cell.negative_evidence)
                || !canonical(&cell.uncertainty)
        })
    {
        return Err(ProtocolEvidenceFusionError::InvalidOutput(
            "identity, ordering, support, score, or limitation invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(fusion))
        .map_err(|error| ProtocolEvidenceFusionError::Digest(error.to_string()))?;
    if expected != fusion.digest {
        return Err(ProtocolEvidenceFusionError::InvalidOutput(
            "digest is not bound to the evidence fusion".into(),
        ));
    }
    Ok(())
}

impl ProtocolEvidenceFusion {
    pub fn validate(&self) -> Result<(), ProtocolEvidenceFusionError> {
        validate_fusion(self)
    }
}

/// Fuse study-local evidence surfaces without moving raw experimental data.
pub fn fuse_glioma_protocol_evidence(
    request: &ProtocolEvidenceFusionRequest,
) -> Result<ProtocolEvidenceFusion, ProtocolEvidenceFusionError> {
    validate_request(request)?;
    let mut study_ids = BTreeSet::new();
    let mut surface_digest_order = Vec::new();
    for study in &request.studies {
        if study.study_id.trim().is_empty()
            || study.site_id.trim().is_empty()
            || !study_ids.insert(study.study_id.clone())
        {
            return Err(ProtocolEvidenceFusionError::InvalidInput(
                "study identifiers and site labels must be non-empty and unique by study".into(),
            ));
        }
        study
            .surface
            .validate()
            .map_err(|error| ProtocolEvidenceFusionError::InvalidInput(error.to_string()))?;
        if study.surface.objective != request.objective {
            return Err(ProtocolEvidenceFusionError::InvalidInput(format!(
                "study {} surface objective does not match fusion objective",
                study.study_id
            )));
        }
        surface_digest_order.push(study.surface.digest.as_str().to_string());
    }
    surface_digest_order.sort();
    surface_digest_order.dedup();
    let mut grouped = BTreeMap::<
        String,
        Vec<(
            &ProtocolEvidenceStudySurface,
            &super::evidence_surface::ProtocolEvidenceCell,
        )>,
    >::new();
    for study in &request.studies {
        for cell in &study.surface.cells {
            grouped
                .entry(cell.endpoint_id.clone())
                .or_default()
                .push((study, cell));
        }
    }
    let mut cells = Vec::new();
    let mut qualified = Vec::new();
    let mut negative = Vec::new();
    let mut partial = Vec::new();
    let mut heterogeneous = Vec::new();
    let mut contradictory = Vec::new();
    let mut unresolved = Vec::new();
    let mut next_actions = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for (endpoint_id, entries) in grouped {
        let study_order = entries
            .iter()
            .map(|(study, _)| study.study_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let model_order = entries
            .iter()
            .map(|(study, _)| study.model_system)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let site_order = entries
            .iter()
            .map(|(study, _)| study.site_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let modality_order = entries
            .iter()
            .flat_map(|(_, cell)| cell.modality_order.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let measured = entries
            .iter()
            .filter_map(|(_, cell)| cell.robust_value_milli)
            .collect::<Vec<_>>();
        let measured_study_count = measured.len() as u16;
        let robust_value_milli = if measured.is_empty() {
            None
        } else {
            let mut values = measured.clone();
            Some(median_i32(&mut values))
        };
        let heterogeneity_milli = if measured.len() > 1 {
            let min = measured.iter().copied().min().unwrap_or_default();
            let max = measured.iter().copied().max().unwrap_or_default();
            i64::from(max)
                .saturating_sub(i64::from(min))
                .clamp(0, i64::from(u32::MAX)) as u32
        } else {
            0
        };
        let positive_count = measured
            .iter()
            .filter(|value| i64::from(**value) >= i64::from(request.contradiction_threshold_milli))
            .count();
        let negative_count = measured
            .iter()
            .filter(|value| i64::from(**value) <= -i64::from(request.contradiction_threshold_milli))
            .count();
        let sign_consistency_milli = if measured.is_empty() {
            0
        } else {
            (positive_count.max(negative_count) * 1_000 / measured.len()) as u16
        };
        let qualified_count = entries
            .iter()
            .filter(|(_, cell)| cell.disposition == ProtocolEvidenceDisposition::Qualified)
            .count() as u16;
        let negative_surface_count = entries
            .iter()
            .filter(|(_, cell)| cell.disposition == ProtocolEvidenceDisposition::Negative)
            .count() as u16;
        let partial_count = entries
            .iter()
            .filter(|(_, cell)| cell.disposition == ProtocolEvidenceDisposition::Partial)
            .count() as u16;
        let unresolved_count = entries
            .iter()
            .filter(|(_, cell)| matches!(cell.disposition, ProtocolEvidenceDisposition::Unresolved))
            .count() as u16;
        let quality_milli = if entries.is_empty() {
            0
        } else {
            (entries
                .iter()
                .map(|(_, cell)| u32::from(cell.quality_milli))
                .sum::<u32>()
                / entries.len() as u32) as u16
        };
        let (disposition, next_action) = if positive_count > 0 && negative_count > 0 {
            contradictory.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Contradictory,
                format!("reconcile contradictory endpoint {endpoint_id} across model systems before transport"),
            )
        } else if measured_study_count < request.min_studies {
            unresolved.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Unresolved,
                format!(
                    "acquire endpoint {endpoint_id} in additional independent preclinical studies"
                ),
            )
        } else if quality_milli < request.min_quality_milli || partial_count > 0 {
            partial.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Partial,
                format!("raise quality or resolve partial execution for endpoint {endpoint_id}"),
            )
        } else if heterogeneity_milli > request.max_heterogeneity_milli {
            heterogeneous.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Heterogeneous,
                format!("model and modality stratify endpoint {endpoint_id} before pooling"),
            )
        } else if robust_value_milli == Some(0) || negative_surface_count == measured_study_count {
            negative.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Negative,
                format!("publish the replicated null or negative endpoint {endpoint_id} and test alternatives"),
            )
        } else if qualified_count >= request.min_studies {
            qualified.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Qualified,
                format!("transport endpoint {endpoint_id} to downstream mechanism analysis with model context"),
            )
        } else {
            partial.push(endpoint_id.clone());
            (
                ProtocolFusionDisposition::Partial,
                format!("resolve study-level disposition disagreement for endpoint {endpoint_id}"),
            )
        };
        let information_milli = entries
            .iter()
            .map(|(_, cell)| cell.information_milli)
            .sum::<u32>()
            .min(1_000_000)
            / entries.len().max(1) as u32;
        let mut cell_negative = Vec::new();
        if negative_count > 0 || negative_surface_count > 0 {
            cell_negative.push(format!("{endpoint_id}:negative-study-signal"));
        }
        if positive_count > 0 && negative_count > 0 {
            cell_negative.push(format!("{endpoint_id}:cross-study-contradiction"));
        }
        cell_negative.sort();
        let mut cell_uncertainty = Vec::new();
        if measured_study_count < request.min_studies {
            cell_uncertainty.push(format!("{endpoint_id}:study-support-below-gate"));
        }
        if heterogeneity_milli > request.max_heterogeneity_milli {
            cell_uncertainty.push(format!("{endpoint_id}:heterogeneity-above-gate"));
        }
        if model_order.len() < 2 {
            cell_uncertainty.push(format!("{endpoint_id}:single-model-transport-boundary"));
        }
        cell_uncertainty.sort();
        negative_evidence.extend(cell_negative.clone());
        uncertainty.extend(cell_uncertainty.clone());
        next_actions.push(next_action.clone());
        cells.push(ProtocolFusionCell {
            endpoint_id,
            study_order,
            site_order,
            model_order,
            modality_order,
            measured_study_count,
            qualified_study_count: qualified_count,
            negative_study_count: negative_surface_count,
            partial_study_count: partial_count,
            unresolved_study_count: unresolved_count,
            robust_value_milli,
            heterogeneity_milli,
            sign_consistency_milli,
            quality_milli,
            information_milli,
            disposition,
            negative_evidence: cell_negative,
            uncertainty: cell_uncertainty,
            next_action,
        });
    }
    cells.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    qualified.sort();
    negative.sort();
    partial.sort();
    heterogeneous.sort();
    contradictory.sort();
    unresolved.sort();
    next_actions.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let overall_information_milli = cells
        .iter()
        .map(|cell| cell.information_milli)
        .sum::<u32>()
        .min(1_000_000);
    let disposition = if !contradictory.is_empty() {
        ProtocolEvidenceFusionDisposition::Contradictory
    } else if !qualified.is_empty()
        && negative.is_empty()
        && partial.is_empty()
        && heterogeneous.is_empty()
        && unresolved.is_empty()
    {
        ProtocolEvidenceFusionDisposition::Qualified
    } else if !negative.is_empty()
        && qualified.is_empty()
        && partial.is_empty()
        && heterogeneous.is_empty()
        && unresolved.is_empty()
    {
        ProtocolEvidenceFusionDisposition::Negative
    } else if !heterogeneous.is_empty() {
        ProtocolEvidenceFusionDisposition::Heterogeneous
    } else if qualified.is_empty()
        && negative.is_empty()
        && partial.is_empty()
        && heterogeneous.is_empty()
        && unresolved.is_empty()
    {
        ProtocolEvidenceFusionDisposition::Unresolved
    } else {
        ProtocolEvidenceFusionDisposition::Partial
    };
    let mut fusion = ProtocolEvidenceFusion {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        surface_digest_order,
        cells,
        qualified_endpoint_order: qualified,
        negative_endpoint_order: negative,
        partial_endpoint_order: partial,
        heterogeneous_endpoint_order: heterogeneous,
        contradictory_endpoint_order: contradictory,
        unresolved_endpoint_order: unresolved,
        overall_information_milli,
        next_actions,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-multistudy-fusion"),
    };
    fusion.digest = ContentHash::of_value(&digest_input(&fusion))
        .map_err(|error| ProtocolEvidenceFusionError::Digest(error.to_string()))?;
    validate_fusion(&fusion)?;
    Ok(fusion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::evidence_surface::{
        ProtocolEvidenceCell, ProtocolEvidenceSurface,
    };

    fn surface(
        study: &str,
        model_system: GliomaModelSystem,
        value: i32,
    ) -> ProtocolEvidenceSurface {
        let mut surface = ProtocolEvidenceSurface {
            feature_id: super::super::evidence_surface::FEATURE_ID.into(),
            output_schema: super::super::evidence_surface::OUTPUT_SCHEMA.into(),
            objective: "fuse invasion evidence".into(),
            protocol_digest: ContentHash::of_bytes(format!("protocol-{study}").as_bytes()),
            execution_digest: ContentHash::of_bytes(format!("execution-{study}").as_bytes()),
            cells: vec![ProtocolEvidenceCell {
                endpoint_id: "invasion".into(),
                measurement_order: vec![format!("{study}-m1"), format!("{study}-m2")],
                task_order: vec!["assay".into()],
                modality_order: vec!["imaging".into()],
                measurement_count: 2,
                replicate_count: 2,
                robust_value_milli: Some(value),
                spread_milli: 20,
                quality_milli: 900,
                max_uncertainty_milli: 50,
                disposition: ProtocolEvidenceDisposition::Qualified,
                information_milli: 800,
                negative_evidence: Vec::new(),
                uncertainty: Vec::new(),
                next_action: "handoff".into(),
            }],
            qualified_endpoint_order: vec!["invasion".into()],
            negative_endpoint_order: Vec::new(),
            partial_endpoint_order: Vec::new(),
            unresolved_endpoint_order: Vec::new(),
            contradictory_endpoint_order: Vec::new(),
            overall_information_milli: 800,
            next_actions: vec!["handoff".into()],
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition:
                super::super::evidence_surface::ProtocolEvidenceSurfaceDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        surface.digest =
            ContentHash::of_value(&super::super::evidence_surface::digest_input(&surface)).unwrap();
        let _ = model_system;
        surface
    }

    fn request(studies: Vec<ProtocolEvidenceStudySurface>) -> ProtocolEvidenceFusionRequest {
        ProtocolEvidenceFusionRequest {
            objective: "fuse invasion evidence".into(),
            studies,
            min_studies: 2,
            min_quality_milli: 700,
            max_heterogeneity_milli: 100,
            contradiction_threshold_milli: 100,
        }
    }

    #[test]
    fn fusion_qualifies_consistent_cross_model_evidence() {
        let fusion = fuse_glioma_protocol_evidence(&request(vec![
            ProtocolEvidenceStudySurface {
                study_id: "study-a".into(),
                site_id: "site-a".into(),
                model_system: GliomaModelSystem::Organoid,
                surface: surface("a", GliomaModelSystem::Organoid, 400),
            },
            ProtocolEvidenceStudySurface {
                study_id: "study-b".into(),
                site_id: "site-b".into(),
                model_system: GliomaModelSystem::MouseModel,
                surface: surface("b", GliomaModelSystem::MouseModel, 430),
            },
        ]))
        .unwrap();
        assert_eq!(
            fusion.disposition,
            ProtocolEvidenceFusionDisposition::Qualified
        );
        assert_eq!(fusion.qualified_endpoint_order, vec!["invasion"]);
        assert_eq!(fusion.cells[0].robust_value_milli, Some(415));
        assert_eq!(fusion.cells[0].site_order, vec!["site-a", "site-b"]);
        fusion.validate().unwrap();
    }

    #[test]
    fn fusion_routes_heterogeneity_and_contradiction_explicitly() {
        let fusion = fuse_glioma_protocol_evidence(&request(vec![
            ProtocolEvidenceStudySurface {
                study_id: "study-a".into(),
                site_id: "site-a".into(),
                model_system: GliomaModelSystem::Organoid,
                surface: surface("a", GliomaModelSystem::Organoid, 500),
            },
            ProtocolEvidenceStudySurface {
                study_id: "study-b".into(),
                site_id: "site-b".into(),
                model_system: GliomaModelSystem::MouseModel,
                surface: surface("b", GliomaModelSystem::MouseModel, -500),
            },
        ]))
        .unwrap();
        assert_eq!(
            fusion.disposition,
            ProtocolEvidenceFusionDisposition::Contradictory
        );
        assert_eq!(fusion.contradictory_endpoint_order, vec!["invasion"]);
        assert!(fusion
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradiction")));
    }

    #[test]
    fn fusion_preserves_replicated_negative_program_result() {
        let mut first = surface("a", GliomaModelSystem::Organoid, 0);
        let mut second = surface("b", GliomaModelSystem::MouseModel, 0);
        for current in [&mut first, &mut second] {
            current.cells[0].disposition = ProtocolEvidenceDisposition::Negative;
            current.qualified_endpoint_order.clear();
            current.negative_endpoint_order = vec!["invasion".into()];
            current.digest =
                ContentHash::of_value(&super::super::evidence_surface::digest_input(current))
                    .unwrap();
        }
        let fusion = fuse_glioma_protocol_evidence(&request(vec![
            ProtocolEvidenceStudySurface {
                study_id: "study-a".into(),
                site_id: "site-a".into(),
                model_system: GliomaModelSystem::Organoid,
                surface: first,
            },
            ProtocolEvidenceStudySurface {
                study_id: "study-b".into(),
                site_id: "site-b".into(),
                model_system: GliomaModelSystem::MouseModel,
                surface: second,
            },
        ]))
        .unwrap();
        assert_eq!(
            fusion.disposition,
            ProtocolEvidenceFusionDisposition::Negative
        );
        assert_eq!(fusion.negative_endpoint_order, vec!["invasion"]);
    }
}
