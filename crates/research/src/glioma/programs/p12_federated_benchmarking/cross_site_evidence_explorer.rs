//! Aggregate-only cross-site evidence explorer for preclinical glioma benchmarks.
//!
//! This feature turns permitted site-level summaries into uncertainty-aware comparison cells. It
//! never receives raw traces or site identifiers, never reverses suppression, and keeps revoked,
//! non-comparable, and under-covered strata visible instead of silently dropping them.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossSiteEvidenceView1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_CELLS: usize = 1_024;
pub const MAX_LABEL_LENGTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCellDisposition {
    Available,
    Suppressed,
    NonComparable,
    Revoked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSiteEvidenceObservation {
    pub observation_id: String,
    pub cell_key: String,
    pub model_system: String,
    pub assay: String,
    pub method: String,
    pub window: String,
    pub value_milli: i64,
    pub uncertainty_milli: u32,
    pub aggregate_site_count: u32,
    pub suppressed: bool,
    pub comparable: bool,
    pub revoked: bool,
    pub mapping_confidence_milli: u16,
    pub provenance_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSiteEvidenceExplorerRequest {
    pub objective: String,
    pub observations: Vec<CrossSiteEvidenceObservation>,
    pub suppression_threshold_sites: u32,
    pub minimum_mapping_confidence_milli: u16,
    pub max_cells: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCell {
    pub cell_key: String,
    pub model_system: String,
    pub assay: String,
    pub method: String,
    pub window: String,
    pub observation_order: Vec<String>,
    pub provenance_digest_order: Vec<ContentHash>,
    pub effective_site_count: u32,
    pub observation_count: u32,
    pub aggregate_value_milli: Option<i64>,
    pub uncertainty_milli: Option<u32>,
    pub suppression_reason_order: Vec<String>,
    pub disposition: EvidenceCellDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossSiteEvidenceView {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub cell_order: Vec<String>,
    pub cells: Vec<EvidenceCell>,
    pub omitted_observation_order: Vec<String>,
    pub total_observation_count: u32,
    pub visible_observation_count: u32,
    pub revoked_observation_count: u32,
    pub suppressed_cell_order: Vec<String>,
    pub non_comparable_cell_order: Vec<String>,
    pub unresolved_cell_order: Vec<String>,
    pub indirect_query_protection: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossSiteEvidenceExplorerError {
    #[error("cross-site evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-site evidence view is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-site evidence digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_LABEL_LENGTH
        && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(view: &CrossSiteEvidenceView) -> serde_json::Value {
    serde_json::json!({
        "feature_id": view.feature_id,
        "output_schema": view.output_schema,
        "objective": view.objective,
        "cell_order": view.cell_order,
        "cells": view.cells,
        "omitted_observation_order": view.omitted_observation_order,
        "total_observation_count": view.total_observation_count,
        "visible_observation_count": view.visible_observation_count,
        "revoked_observation_count": view.revoked_observation_count,
        "suppressed_cell_order": view.suppressed_cell_order,
        "non_comparable_cell_order": view.non_comparable_cell_order,
        "unresolved_cell_order": view.unresolved_cell_order,
        "indirect_query_protection": view.indirect_query_protection,
    })
}

fn validate_request(
    request: &CrossSiteEvidenceExplorerRequest,
) -> Result<(), CrossSiteEvidenceExplorerError> {
    if !safe_text(&request.objective)
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.suppression_threshold_sites == 0
        || request.max_cells == 0
        || request.max_cells > MAX_CELLS
        || request.minimum_mapping_confidence_milli > 1_000
    {
        return Err(CrossSiteEvidenceExplorerError::InvalidRequest(
            "objective, bounded observations/cells, positive suppression threshold, and confidence bound are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if !safe_text(&observation.observation_id)
            || !ids.insert(observation.observation_id.clone())
            || !safe_text(&observation.cell_key)
            || !safe_text(&observation.model_system)
            || !safe_text(&observation.assay)
            || !safe_text(&observation.method)
            || !safe_text(&observation.window)
            || observation.mapping_confidence_milli > 1_000
            || observation.aggregate_site_count == 0
            || observation.provenance_digest.as_str().len() != 64
        {
            return Err(CrossSiteEvidenceExplorerError::InvalidRequest(format!(
                "observation {} is malformed or duplicated",
                observation.observation_id
            )));
        }
    }
    Ok(())
}

impl CrossSiteEvidenceView {
    pub fn validate(&self) -> Result<(), CrossSiteEvidenceExplorerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !canonical(&self.cell_order)
            || self.cells.len() != self.cell_order.len()
            || !canonical(&self.omitted_observation_order)
            || !canonical(&self.suppressed_cell_order)
            || !canonical(&self.non_comparable_cell_order)
            || !canonical(&self.unresolved_cell_order)
            || self.visible_observation_count > self.total_observation_count
            || self.revoked_observation_count > self.total_observation_count
            || self.digest.as_str().len() != 64
        {
            return Err(CrossSiteEvidenceExplorerError::InvalidOutput(
                "evidence-view identity, ordering, counts, or digest invariants are invalid".into(),
            ));
        }
        if self.cells.iter().any(|cell| {
            !safe_text(&cell.cell_key)
                || !safe_text(&cell.model_system)
                || !safe_text(&cell.assay)
                || !safe_text(&cell.method)
                || !safe_text(&cell.window)
                || !canonical(&cell.observation_order)
                || !canonical(&cell.provenance_digest_order)
                || !canonical(&cell.suppression_reason_order)
                || cell.observation_count == 0
                || (cell.effective_site_count == 0
                    && cell.disposition != EvidenceCellDisposition::Revoked)
                || cell
                    .aggregate_value_milli
                    .is_some_and(|_| cell.disposition != EvidenceCellDisposition::Available)
                || cell
                    .uncertainty_milli
                    .is_some_and(|_| cell.disposition != EvidenceCellDisposition::Available)
        }) {
            return Err(CrossSiteEvidenceExplorerError::InvalidOutput(
                "cell values must be aggregate-only, ordered, and disposition-bound".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| CrossSiteEvidenceExplorerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossSiteEvidenceExplorerError::InvalidOutput(
                "evidence view digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Build uncertainty-aware, aggregate-only cross-site evidence cells.
pub fn explore_glioma_cross_site_evidence(
    request: &CrossSiteEvidenceExplorerRequest,
) -> Result<CrossSiteEvidenceView, CrossSiteEvidenceExplorerError> {
    validate_request(request)?;
    let mut groups = BTreeMap::<String, Vec<CrossSiteEvidenceObservation>>::new();
    for observation in &request.observations {
        groups
            .entry(observation.cell_key.clone())
            .or_default()
            .push(observation.clone());
    }
    if groups.len() > request.max_cells {
        return Err(CrossSiteEvidenceExplorerError::InvalidRequest(
            "cell cardinality exceeds the query bound".into(),
        ));
    }
    let mut cells = Vec::with_capacity(groups.len());
    let mut omitted = BTreeSet::new();
    let mut revoked_count = 0u32;
    for (cell_key, mut observations) in groups {
        observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        let first = observations.first().expect("validated non-empty group");
        let observation_order = observations
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect::<Vec<_>>();
        let provenance_digest_order = observations
            .iter()
            .map(|observation| observation.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let revoked = observations
            .iter()
            .filter(|observation| observation.revoked)
            .count() as u32;
        revoked_count = revoked_count.saturating_add(revoked);
        if revoked > 0 {
            omitted.extend(
                observations
                    .iter()
                    .filter(|observation| observation.revoked)
                    .map(|observation| observation.observation_id.clone()),
            );
        }
        let effective = observations
            .iter()
            .filter(|observation| !observation.revoked)
            .collect::<Vec<_>>();
        let effective_site_count = effective
            .iter()
            .map(|observation| observation.aggregate_site_count)
            .sum::<u32>();
        let mut reasons = BTreeSet::new();
        let disposition = if revoked > 0 && effective.is_empty() {
            reasons.insert("revoked_contribution".into());
            EvidenceCellDisposition::Revoked
        } else if effective.iter().any(|observation| !observation.comparable) {
            reasons.insert("non_comparable_mapping".into());
            EvidenceCellDisposition::NonComparable
        } else if effective.iter().any(|observation| observation.suppressed)
            || effective_site_count < request.suppression_threshold_sites
        {
            reasons.insert("privacy_or_minimum_count_suppression".into());
            EvidenceCellDisposition::Suppressed
        } else if effective.iter().any(|observation| {
            observation.mapping_confidence_milli < request.minimum_mapping_confidence_milli
        }) {
            reasons.insert("mapping_confidence_below_threshold".into());
            EvidenceCellDisposition::Unresolved
        } else {
            EvidenceCellDisposition::Available
        };
        let (aggregate_value_milli, uncertainty_milli) = if disposition
            == EvidenceCellDisposition::Available
        {
            let weight_sum = effective
                .iter()
                .map(|observation| i128::from(observation.aggregate_site_count))
                .sum::<i128>();
            let weighted_sum = effective
                .iter()
                .map(|observation| {
                    i128::from(observation.value_milli)
                        * i128::from(observation.aggregate_site_count)
                })
                .sum::<i128>();
            let mean = (weighted_sum / weight_sum).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                as i64;
            let max_uncertainty = effective
                .iter()
                .map(|observation| observation.uncertainty_milli)
                .max()
                .unwrap_or(0);
            let min_value = effective
                .iter()
                .map(|observation| observation.value_milli)
                .min()
                .unwrap_or(0);
            let max_value = effective
                .iter()
                .map(|observation| observation.value_milli)
                .max()
                .unwrap_or(0);
            let heterogeneity = max_value.saturating_sub(min_value).unsigned_abs() as u32;
            (
                Some(mean),
                Some(max_uncertainty.saturating_add(heterogeneity)),
            )
        } else {
            (None, None)
        };
        cells.push(EvidenceCell {
            cell_key,
            model_system: first.model_system.clone(),
            assay: first.assay.clone(),
            method: first.method.clone(),
            window: first.window.clone(),
            observation_order,
            provenance_digest_order,
            effective_site_count,
            observation_count: observations.len() as u32,
            aggregate_value_milli,
            uncertainty_milli,
            suppression_reason_order: reasons.into_iter().collect(),
            disposition,
        });
    }
    cells.sort_by(|left, right| left.cell_key.cmp(&right.cell_key));
    let cell_order = cells
        .iter()
        .map(|cell| cell.cell_key.clone())
        .collect::<Vec<_>>();
    let suppressed_cell_order = cells
        .iter()
        .filter(|cell| cell.disposition == EvidenceCellDisposition::Suppressed)
        .map(|cell| cell.cell_key.clone())
        .collect::<Vec<_>>();
    let non_comparable_cell_order = cells
        .iter()
        .filter(|cell| cell.disposition == EvidenceCellDisposition::NonComparable)
        .map(|cell| cell.cell_key.clone())
        .collect::<Vec<_>>();
    let unresolved_cell_order = cells
        .iter()
        .filter(|cell| cell.disposition == EvidenceCellDisposition::Unresolved)
        .map(|cell| cell.cell_key.clone())
        .collect::<Vec<_>>();
    let visible = cells
        .iter()
        .filter(|cell| cell.disposition == EvidenceCellDisposition::Available)
        .map(|cell| cell.observation_count)
        .sum::<u32>();
    let mut view = CrossSiteEvidenceView {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        cell_order,
        cells,
        omitted_observation_order: omitted.into_iter().collect(),
        total_observation_count: request.observations.len() as u32,
        visible_observation_count: visible,
        revoked_observation_count: revoked_count,
        suppressed_cell_order,
        non_comparable_cell_order,
        unresolved_cell_order,
        indirect_query_protection: true,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-site-evidence"),
    };
    view.digest = ContentHash::of_value(&digest_body(&view))
        .map_err(|error| CrossSiteEvidenceExplorerError::Digest(error.to_string()))?;
    view.validate()?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn observation(id: &str, value_milli: i64) -> CrossSiteEvidenceObservation {
        CrossSiteEvidenceObservation {
            observation_id: id.into(),
            cell_key: "organoid|invasion|imaging|day7".into(),
            model_system: "organoid".into(),
            assay: "invasion".into(),
            method: "imaging".into(),
            window: "day7".into(),
            value_milli,
            uncertainty_milli: 50,
            aggregate_site_count: 3,
            suppressed: false,
            comparable: true,
            revoked: false,
            mapping_confidence_milli: 900,
            provenance_digest: digest(id),
        }
    }

    fn request(
        observations: Vec<CrossSiteEvidenceObservation>,
    ) -> CrossSiteEvidenceExplorerRequest {
        CrossSiteEvidenceExplorerRequest {
            objective: "compare glioma invasion across sites".into(),
            observations,
            suppression_threshold_sites: 2,
            minimum_mapping_confidence_milli: 700,
            max_cells: 8,
        }
    }

    #[test]
    fn available_cells_are_weighted_and_heterogeneity_is_visible() {
        let view = explore_glioma_cross_site_evidence(&request(vec![
            observation("obs-a", 100),
            observation("obs-b", 200),
        ]))
        .unwrap();
        assert_eq!(
            view.cells[0].disposition,
            EvidenceCellDisposition::Available
        );
        assert_eq!(view.cells[0].aggregate_value_milli, Some(150));
        assert_eq!(view.cells[0].uncertainty_milli, Some(150));
        assert!(view.validate().is_ok());
    }

    #[test]
    fn suppression_and_non_comparability_never_become_values() {
        let mut suppressed = observation("suppressed", 500);
        suppressed.aggregate_site_count = 1;
        let mut incomparable = observation("incomparable", 600);
        incomparable.cell_key = "organoid|invasion|legacy|day7".into();
        incomparable.comparable = false;
        let view =
            explore_glioma_cross_site_evidence(&request(vec![suppressed, incomparable])).unwrap();
        assert!(view
            .cells
            .iter()
            .all(|cell| cell.aggregate_value_milli.is_none()));
        assert_eq!(view.suppressed_cell_order.len(), 1);
        assert_eq!(view.non_comparable_cell_order.len(), 1);
    }

    #[test]
    fn revoked_contributions_are_omitted_from_values_but_remain_auditable() {
        let mut revoked = observation("revoked", 700);
        revoked.revoked = true;
        let view = explore_glioma_cross_site_evidence(&request(vec![revoked])).unwrap();
        assert_eq!(view.cells[0].disposition, EvidenceCellDisposition::Revoked);
        assert!(view.cells[0].aggregate_value_milli.is_none());
        assert_eq!(view.revoked_observation_count, 1);
        assert_eq!(view.omitted_observation_order, vec!["revoked"]);
    }

    #[test]
    fn low_mapping_confidence_is_unresolved_not_silently_pooled() {
        let mut uncertain = observation("uncertain", 800);
        uncertain.mapping_confidence_milli = 100;
        let view = explore_glioma_cross_site_evidence(&request(vec![uncertain])).unwrap();
        assert_eq!(
            view.cells[0].disposition,
            EvidenceCellDisposition::Unresolved
        );
        assert_eq!(view.cells[0].aggregate_value_milli, None);
        assert!(view.unresolved_cell_order.contains(&view.cells[0].cell_key));
    }

    #[test]
    fn duplicate_observation_ids_are_rejected() {
        let duplicate = observation("dup", 1);
        let error =
            explore_glioma_cross_site_evidence(&request(vec![duplicate.clone(), duplicate]))
                .unwrap_err();
        assert!(matches!(
            error,
            CrossSiteEvidenceExplorerError::InvalidRequest(_)
        ));
    }
}
