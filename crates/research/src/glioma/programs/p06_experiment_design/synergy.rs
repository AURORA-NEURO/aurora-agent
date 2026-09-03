//! Fixed-point combination-response and Bliss synergy analysis for preclinical glioma assays.
//!
//! The analyzer treats `response_milli` as a bounded inhibition/effect fraction (0..=1_000),
//! pools replicate observations by dose pair, and compares each combination with the declared
//! Bliss independence expectation from its two single-agent controls.  Missing singles,
//! under-replicated cells, noisy cells, antagonistic effects, and null results remain explicit;
//! this is not a clinical dose recommendation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaCombinationSynergy1@1";
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_DOSE_PAIRS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DosePair {
    pub dose_a_milli: u32,
    pub dose_b_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinationObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub dose_a_milli: u32,
    pub dose_b_milli: u32,
    pub response_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinationSynergyRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_replicates_per_cell: usize,
    pub min_combination_cells: usize,
    pub synergy_threshold_milli: u64,
    pub max_residual_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombinationCellDisposition {
    Synergistic,
    Additive,
    Antagonistic,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinationCell {
    pub dose_pair: DosePair,
    pub observation_order: Vec<String>,
    pub observation_count: usize,
    pub observed_response_milli: u16,
    pub single_a_response_milli: Option<u16>,
    pub single_b_response_milli: Option<u16>,
    pub bliss_expected_milli: Option<u16>,
    pub synergy_milli: Option<i64>,
    pub residual_mad_milli: u64,
    pub disposition: CombinationCellDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombinationSynergyDisposition {
    Qualified,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinationSynergyAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_pair: DosePair,
    pub cell_order: Vec<DosePair>,
    pub cells: Vec<CombinationCell>,
    pub top_synergy_order: Vec<DosePair>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CombinationSynergyDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CombinationSynergyError {
    #[error("combination-synergy request is invalid: {0}")]
    InvalidRequest(String),
    #[error("combination-synergy observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("combination-synergy output is invalid: {0}")]
    InvalidOutput(String),
    #[error("combination-synergy digest failed: {0}")]
    Digest(String),
}

fn mean(values: &[u16]) -> u16 {
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| *value as u64).sum::<u64>() / values.len() as u64) as u16
    }
}

fn residual_mad(values: &[u16], center: u16) -> u64 {
    let mut residuals = values
        .iter()
        .map(|value| (*value as i64 - center as i64).unsigned_abs())
        .collect::<Vec<_>>();
    residuals.sort_unstable();
    residuals.get(residuals.len() / 2).copied().unwrap_or(0)
}

fn pair_label(pair: DosePair) -> String {
    format!("{}:{}", pair.dose_a_milli, pair.dose_b_milli)
}

fn digest_input(output: &CombinationSynergyAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "control_pair": output.control_pair,
        "cell_order": output.cell_order,
        "cells": output.cells,
        "top_synergy_order": output.top_synergy_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl CombinationSynergyAnalysis {
    pub fn validate(&self) -> Result<(), CombinationSynergyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_pair
                != (DosePair {
                    dose_a_milli: 0,
                    dose_b_milli: 0,
                })
            || self.cell_order.len() != self.cells.len()
            || self
                .cells
                .iter()
                .map(|cell| cell.dose_pair)
                .collect::<Vec<_>>()
                != self.cell_order
            || self.cell_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .top_synergy_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.cells.iter().any(|cell| {
                cell.dose_pair.dose_a_milli == 0
                    || cell.dose_pair.dose_b_milli == 0
                    || cell.observation_order.is_empty()
                    || cell.observation_count != cell.observation_order.len()
                    || cell
                        .observation_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || cell.observed_response_milli > 1_000
                    || cell
                        .single_a_response_milli
                        .is_some_and(|value| value > 1_000)
                    || cell
                        .single_b_response_milli
                        .is_some_and(|value| value > 1_000)
                    || cell.bliss_expected_milli.is_some_and(|value| value > 1_000)
                    || cell.synergy_milli.is_some_and(|value| value.abs() > 1_000)
                    || cell.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            })
        {
            return Err(CombinationSynergyError::InvalidOutput(
                "identity, dose-pair ordering, replicate accounting, or bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CombinationSynergyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CombinationSynergyError::InvalidOutput(
                "digest is not bound to combination synergy".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_glioma_combination_synergy(
    request: &CombinationSynergyRequest,
    observations: &[CombinationObservation],
) -> Result<CombinationSynergyAnalysis, CombinationSynergyError> {
    if request.objective.trim().is_empty()
        || request.min_replicates_per_cell == 0
        || request.min_combination_cells == 0
        || request.synergy_threshold_milli > 1_000
        || request.max_residual_milli > 1_000
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(CombinationSynergyError::InvalidRequest(
            "objective, replicate/cell floors, response thresholds, or observation bound is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut groups = BTreeMap::<DosePair, Vec<&CombinationObservation>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.model_system != request.model_system
            || observation.response_milli > 1_000
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(CombinationSynergyError::InvalidObservation(
                "observation identity, model binding, response bound, or uniqueness is invalid"
                    .into(),
            ));
        }
        groups
            .entry(DosePair {
                dose_a_milli: observation.dose_a_milli,
                dose_b_milli: observation.dose_b_milli,
            })
            .or_default()
            .push(observation);
    }
    if groups.len() > MAX_DOSE_PAIRS {
        return Err(CombinationSynergyError::InvalidObservation(
            "dose-pair bound exceeded".into(),
        ));
    }
    let control_pair = DosePair {
        dose_a_milli: 0,
        dose_b_milli: 0,
    };
    let control_values = groups.get(&control_pair).map(|values| {
        values
            .iter()
            .map(|observation| observation.response_milli)
            .collect::<Vec<_>>()
    });
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    if control_values
        .as_ref()
        .is_none_or(|values| values.len() < request.min_replicates_per_cell)
    {
        uncertainty.insert("vehicle-control-replicate-floor-not-met".into());
    }
    let mut cells = Vec::new();
    for (pair, values) in groups
        .iter()
        .filter(|(pair, _)| pair.dose_a_milli > 0 && pair.dose_b_milli > 0)
    {
        let response_values = values
            .iter()
            .map(|observation| observation.response_milli)
            .collect::<Vec<_>>();
        let observed_response_milli = mean(&response_values);
        let residual_mad_milli = residual_mad(&response_values, observed_response_milli);
        let mut cell_uncertainty = BTreeSet::new();
        if values.len() < request.min_replicates_per_cell {
            cell_uncertainty.insert("combination-replicate-floor-not-met".into());
        }
        if residual_mad_milli > request.max_residual_milli {
            cell_uncertainty.insert("combination-residual-tolerance-exceeded".into());
        }
        let single_a_pair = DosePair {
            dose_a_milli: pair.dose_a_milli,
            dose_b_milli: 0,
        };
        let single_b_pair = DosePair {
            dose_a_milli: 0,
            dose_b_milli: pair.dose_b_milli,
        };
        let single_a_response_milli = groups.get(&single_a_pair).and_then(|values| {
            (values.len() >= request.min_replicates_per_cell).then(|| {
                mean(
                    &values
                        .iter()
                        .map(|observation| observation.response_milli)
                        .collect::<Vec<_>>(),
                )
            })
        });
        let single_b_response_milli = groups.get(&single_b_pair).and_then(|values| {
            (values.len() >= request.min_replicates_per_cell).then(|| {
                mean(
                    &values
                        .iter()
                        .map(|observation| observation.response_milli)
                        .collect::<Vec<_>>(),
                )
            })
        });
        if single_a_response_milli.is_none() {
            cell_uncertainty.insert("single-agent-a-control-missing".into());
        }
        if single_b_response_milli.is_none() {
            cell_uncertainty.insert("single-agent-b-control-missing".into());
        }
        if control_values
            .as_ref()
            .is_none_or(|values| values.len() < request.min_replicates_per_cell)
        {
            cell_uncertainty.insert("vehicle-control-missing".into());
        }
        let bliss_expected_milli =
            single_a_response_milli
                .zip(single_b_response_milli)
                .map(|(a, b)| {
                    (a as u32 + b as u32 - ((a as u32 * b as u32) / 1_000)).min(1_000) as u16
                });
        let synergy_milli =
            bliss_expected_milli.map(|expected| observed_response_milli as i64 - expected as i64);
        let disposition = if !cell_uncertainty.is_empty() || synergy_milli.is_none() {
            CombinationCellDisposition::Unresolved
        } else if synergy_milli.unwrap_or(0) as u64 >= request.synergy_threshold_milli {
            CombinationCellDisposition::Synergistic
        } else if synergy_milli.unwrap_or(0) < 0 {
            CombinationCellDisposition::Antagonistic
        } else {
            CombinationCellDisposition::Additive
        };
        if disposition == CombinationCellDisposition::Antagonistic {
            negative_evidence.insert(format!("antagonistic-combination:{}", pair_label(*pair)));
        }
        cells.push(CombinationCell {
            dose_pair: *pair,
            observation_order: values
                .iter()
                .map(|observation| observation.observation_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            observation_count: values.len(),
            observed_response_milli,
            single_a_response_milli,
            single_b_response_milli,
            bliss_expected_milli,
            synergy_milli,
            residual_mad_milli,
            disposition,
            uncertainty: cell_uncertainty.into_iter().collect(),
        });
    }
    cells.sort_by_key(|cell| cell.dose_pair);
    if cells.is_empty() {
        uncertainty.insert("no-combination-cells-provided".into());
    }
    if cells.len() < request.min_combination_cells {
        uncertainty.insert("combination-cell-floor-not-met".into());
    }
    let cell_order = cells.iter().map(|cell| cell.dose_pair).collect::<Vec<_>>();
    let mut top = cells
        .iter()
        .filter_map(|cell| cell.synergy_milli.map(|synergy| (synergy, cell.dose_pair)))
        .collect::<Vec<_>>();
    top.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let top_synergy_order = top.iter().map(|(_, pair)| *pair).collect::<Vec<_>>();
    let eligible_cells = cells
        .iter()
        .filter(|cell| cell.disposition != CombinationCellDisposition::Unresolved)
        .count();
    let disposition = if cells.len() < request.min_combination_cells
        || eligible_cells < request.min_combination_cells
        || !uncertainty.is_empty()
    {
        CombinationSynergyDisposition::Unresolved
    } else if top
        .first()
        .is_some_and(|(synergy, _)| *synergy >= request.synergy_threshold_milli as i64)
    {
        CombinationSynergyDisposition::Qualified
    } else {
        negative_evidence.insert("no-combination-exceeds-synergy-threshold".into());
        CombinationSynergyDisposition::Negative
    };
    let mut output = CombinationSynergyAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        control_pair,
        cell_order,
        cells,
        top_synergy_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| CombinationSynergyError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CombinationSynergyError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, a: u32, b: u32, response: u16) -> CombinationObservation {
        CombinationObservation {
            observation_id: id.into(),
            unit_id: format!("unit-{id}"),
            model_system: GliomaModelSystem::Organoid,
            batch_id: format!("batch-{id}"),
            dose_a_milli: a,
            dose_b_milli: b,
            response_milli: response,
        }
    }

    fn request() -> CombinationSynergyRequest {
        CombinationSynergyRequest {
            objective: "map glioma combination suppression".into(),
            model_system: GliomaModelSystem::Organoid,
            min_replicates_per_cell: 1,
            min_combination_cells: 1,
            synergy_threshold_milli: 50,
            max_residual_milli: 0,
        }
    }

    #[test]
    fn bliss_synergy_is_qualified_and_replay_stable() {
        let observations = vec![
            observation("v", 0, 0, 0),
            observation("a", 10, 0, 400),
            observation("b", 0, 10, 400),
            observation("ab", 10, 10, 900),
        ];
        let first = analyze_glioma_combination_synergy(&request(), &observations).unwrap();
        let second = analyze_glioma_combination_synergy(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cells[0].bliss_expected_milli, Some(640));
        assert_eq!(first.cells[0].synergy_milli, Some(260));
        assert_eq!(first.disposition, CombinationSynergyDisposition::Qualified);
    }

    #[test]
    fn missing_single_agent_is_unresolved() {
        let output = analyze_glioma_combination_synergy(
            &request(),
            &[
                observation("v", 0, 0, 0),
                observation("a", 10, 0, 400),
                observation("ab", 10, 10, 900),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            CombinationSynergyDisposition::Unresolved
        );
        assert!(output.cells[0]
            .uncertainty
            .iter()
            .any(|item| item.contains("single-agent-b")));
    }
}
