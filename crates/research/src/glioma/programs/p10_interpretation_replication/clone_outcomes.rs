//! Adjudication of clone-aware perturbation-panel outcomes.
//!
//! A panel selection is not evidence. This module reconciles a selected preclinical panel with
//! replicate-level, caller-supplied local observations and keeps supported, null, negative,
//! contradictory, and unresolved branch/candidate cells distinct. It is deliberately bounded and
//! deterministic so an autonomous campaign can choose the next measurement without promoting a
//! missing or contradictory branch into a biological conclusion.

use crate::glioma::programs::p06_experiment_design::{
    ClonePerturbationPanel, ClonePerturbationPanelDisposition,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaClonePanelOutcomeAnalysis1@1";
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_REPLICATES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonePanelMeasurementState {
    Measured,
    Null,
    Missing,
    Contradictory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonePanelCellDisposition {
    Supported,
    Negative,
    Null,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelObservation {
    pub observation_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub candidate_id: String,
    pub branch_id: String,
    pub replicate_id: String,
    pub state: ClonePanelMeasurementState,
    pub effect_milli: i32,
    pub uncertainty_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelOutcomeRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub min_replicates: usize,
    pub effect_threshold_milli: u16,
    pub max_uncertainty_milli: u16,
    pub require_all_selected: bool,
    pub require_all_branches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelCellAnalysis {
    pub cell_id: String,
    pub candidate_id: String,
    pub branch_id: String,
    pub observation_order: Vec<String>,
    pub replicate_count: usize,
    pub median_effect_milli: Option<i32>,
    pub max_uncertainty_milli: u16,
    pub disposition: ClonePanelCellDisposition,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelCandidateAnalysis {
    pub candidate_id: String,
    pub expected_branch_order: Vec<String>,
    pub supported_branch_order: Vec<String>,
    pub unresolved_branch_order: Vec<String>,
    pub median_effect_milli: Option<i32>,
    pub disposition: ClonePanelCellDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelBranchAnalysis {
    pub branch_id: String,
    pub selected_candidate_order: Vec<String>,
    pub supported_candidate_order: Vec<String>,
    pub unresolved_candidate_order: Vec<String>,
    pub disposition: ClonePanelCellDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClonePanelOutcomeDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePanelOutcomeAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub panel_digest: ContentHash,
    pub cell_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub branch_order: Vec<String>,
    pub cells: Vec<ClonePanelCellAnalysis>,
    pub candidates: Vec<ClonePanelCandidateAnalysis>,
    pub branches: Vec<ClonePanelBranchAnalysis>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ClonePanelOutcomeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClonePanelOutcomeError {
    #[error("clone panel outcome request is invalid: {0}")]
    InvalidRequest(String),
    #[error("clone panel observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("clone panel outcome output is invalid: {0}")]
    InvalidOutput(String),
    #[error("clone panel outcome digest failed: {0}")]
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

fn cell_id(candidate_id: &str, branch_id: &str) -> String {
    format!("{candidate_id}@{branch_id}")
}

fn digest_input(output: &ClonePanelOutcomeAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "panel_digest": output.panel_digest,
        "cell_order": output.cell_order,
        "candidate_order": output.candidate_order,
        "branch_order": output.branch_order,
        "cells": output.cells,
        "candidates": output.candidates,
        "branches": output.branches,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &ClonePanelOutcomeRequest) -> Result<(), ClonePanelOutcomeError> {
    if !valid_identifier(&request.study_id)
        || request.min_replicates == 0
        || request.min_replicates > MAX_REPLICATES
        || request.effect_threshold_milli == 0
        || request.max_uncertainty_milli > 1_000
    {
        return Err(ClonePanelOutcomeError::InvalidRequest(
            "study, replicate, effect, or uncertainty bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    request: &ClonePanelOutcomeRequest,
    observation: &ClonePanelObservation,
) -> Result<(), ClonePanelOutcomeError> {
    if !valid_identifier(&observation.observation_id)
        || !valid_identifier(&observation.candidate_id)
        || !valid_identifier(&observation.branch_id)
        || !valid_identifier(&observation.replicate_id)
        || observation.study_id != request.study_id
        || observation.model_system != request.model_system
        || observation.effect_milli.unsigned_abs() > 1_000_000
        || observation.uncertainty_milli > 1_000
    {
        return Err(ClonePanelOutcomeError::InvalidObservation(
            "observation identity, study/model binding, effect, or uncertainty is invalid".into(),
        ));
    }
    observation
        .artifact
        .validate()
        .map_err(|error| ClonePanelOutcomeError::InvalidObservation(error.to_string()))
}

fn median(values: &mut [i32]) -> Option<i32> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[(values.len() - 1) / 2])
}

fn classify_cell(
    observations: &[&ClonePanelObservation],
    request: &ClonePanelOutcomeRequest,
) -> (
    Option<i32>,
    usize,
    u16,
    ClonePanelCellDisposition,
    Vec<String>,
    Vec<String>,
) {
    let measured = observations
        .iter()
        .filter(|observation| observation.state == ClonePanelMeasurementState::Measured)
        .map(|observation| observation.effect_milli)
        .collect::<Vec<_>>();
    let mut effects = measured.clone();
    let median_effect = median(&mut effects);
    let max_uncertainty = observations
        .iter()
        .map(|observation| observation.uncertainty_milli)
        .max()
        .unwrap_or(0);
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if observations.len() < request.min_replicates {
        uncertainty.insert(format!(
            "replicate-floor:{}<{}",
            observations.len(),
            request.min_replicates
        ));
    }
    if max_uncertainty > request.max_uncertainty_milli {
        uncertainty.insert(format!(
            "uncertainty-floor:{}>{}",
            max_uncertainty, request.max_uncertainty_milli
        ));
    }
    if observations
        .iter()
        .any(|observation| observation.state == ClonePanelMeasurementState::Contradictory)
    {
        negative.insert("declared-contradictory-measurement".into());
    }
    let mixed_signs =
        measured.iter().any(|effect| *effect < 0) && measured.iter().any(|effect| *effect > 0);
    if mixed_signs {
        negative.insert("replicate-direction-conflict".into());
    }
    let disposition = if observations.is_empty()
        || measured.len() < request.min_replicates
        || max_uncertainty > request.max_uncertainty_milli
        || !negative.is_empty()
    {
        if !negative.is_empty() {
            ClonePanelCellDisposition::Contradictory
        } else {
            ClonePanelCellDisposition::Unresolved
        }
    } else if median_effect
        .map(|effect| effect.unsigned_abs() >= u32::from(request.effect_threshold_milli))
        .unwrap_or(false)
    {
        ClonePanelCellDisposition::Supported
    } else if measured.iter().all(|effect| *effect == 0)
        || observations
            .iter()
            .any(|observation| observation.state == ClonePanelMeasurementState::Null)
    {
        negative.insert("null-or-zero-effect".into());
        ClonePanelCellDisposition::Null
    } else {
        negative.insert("effect-below-declared-threshold".into());
        ClonePanelCellDisposition::Negative
    };
    (
        median_effect,
        measured.len(),
        max_uncertainty,
        disposition,
        uncertainty.into_iter().collect(),
        negative.into_iter().collect(),
    )
}

impl ClonePanelOutcomeAnalysis {
    pub fn validate(&self) -> Result<(), ClonePanelOutcomeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.cell_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.branch_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.cells.len() != self.cell_order.len()
            || self.candidates.len() != self.candidate_order.len()
            || self.branches.len() != self.branch_order.len()
            || self.cells.iter().any(|cell| {
                !valid_identifier(&cell.cell_id)
                    || !canonical(&cell.observation_order)
                    || !canonical(&cell.uncertainty)
                    || !canonical(&cell.negative_evidence)
                    || cell.max_uncertainty_milli > 1_000
            })
            || self.candidates.iter().any(|candidate| {
                !valid_identifier(&candidate.candidate_id)
                    || !canonical(&candidate.expected_branch_order)
                    || !canonical(&candidate.supported_branch_order)
                    || !canonical(&candidate.unresolved_branch_order)
            })
            || self.branches.iter().any(|branch| {
                !valid_identifier(&branch.branch_id)
                    || !canonical(&branch.selected_candidate_order)
                    || !canonical(&branch.supported_candidate_order)
                    || !canonical(&branch.unresolved_candidate_order)
            })
        {
            return Err(ClonePanelOutcomeError::InvalidOutput(
                "outcome identity, ordering, cardinality, or nested invariant failed".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClonePanelOutcomeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClonePanelOutcomeError::InvalidOutput(
                "outcome digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Reconcile replicate-level outcomes against a selected clone-aware panel. No observation is
/// imputed: missing cells remain unresolved and contradictory/null effects remain first-class.
pub fn analyze_glioma_clone_panel_outcomes(
    request: &ClonePanelOutcomeRequest,
    panel: &ClonePerturbationPanel,
    observations: &[ClonePanelObservation],
) -> Result<ClonePanelOutcomeAnalysis, ClonePanelOutcomeError> {
    validate_request(request)?;
    panel
        .validate()
        .map_err(|error| ClonePanelOutcomeError::InvalidOutput(error.to_string()))?;
    if panel.study_id != request.study_id || panel.model_system != request.model_system {
        return Err(ClonePanelOutcomeError::InvalidRequest(
            "panel study/model binding does not match outcome request".into(),
        ));
    }
    if observations.len() > MAX_OBSERVATIONS {
        return Err(ClonePanelOutcomeError::InvalidRequest(
            "observation bound exceeded".into(),
        ));
    }
    let selected = panel
        .selected_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let branches = panel.branch_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut by_cell = BTreeMap::<String, Vec<&ClonePanelObservation>>::new();
    for observation in observations {
        validate_observation(request, observation)?;
        if !selected.contains(&observation.candidate_id) {
            return Err(ClonePanelOutcomeError::InvalidObservation(
                "observation references a candidate that was not selected".into(),
            ));
        }
        if !branches.contains(&observation.branch_id) {
            return Err(ClonePanelOutcomeError::InvalidObservation(
                "observation references an unknown panel branch".into(),
            ));
        }
        let key = format!(
            "{}:{}:{}",
            observation.candidate_id, observation.branch_id, observation.replicate_id
        );
        if !seen.insert(key) {
            return Err(ClonePanelOutcomeError::InvalidObservation(
                "candidate, branch, and replicate identity must be unique".into(),
            ));
        }
        by_cell
            .entry(cell_id(&observation.candidate_id, &observation.branch_id))
            .or_default()
            .push(observation);
    }
    let expected_cells = panel
        .branch_coverage
        .iter()
        .flat_map(|branch| {
            branch
                .selected_candidate_order
                .iter()
                .map(move |candidate_id| (candidate_id.clone(), branch.branch_id.clone()))
        })
        .collect::<BTreeSet<_>>();
    let mut cells = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut missing_selected_cells = false;
    for (candidate_id, branch_id) in &expected_cells {
        let id = cell_id(candidate_id, branch_id);
        let mut rows = by_cell.remove(&id).unwrap_or_default();
        if rows.is_empty() {
            missing_selected_cells = true;
        }
        rows.sort_by(|left, right| left.replicate_id.cmp(&right.replicate_id));
        let (
            median_effect,
            replicate_count,
            max_uncertainty,
            disposition,
            cell_uncertainty,
            cell_negative,
        ) = classify_cell(&rows, request);
        let observation_order = rows
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect::<Vec<_>>();
        if disposition == ClonePanelCellDisposition::Unresolved {
            uncertainty.insert(format!("unresolved-cell:{id}"));
        }
        for item in &cell_uncertainty {
            uncertainty.insert(format!("cell:{id}:{item}"));
        }
        for item in &cell_negative {
            negative_evidence.insert(format!("cell:{id}:{item}"));
        }
        cells.push(ClonePanelCellAnalysis {
            cell_id: id,
            candidate_id: candidate_id.clone(),
            branch_id: branch_id.clone(),
            observation_order,
            replicate_count,
            median_effect_milli: median_effect,
            max_uncertainty_milli: max_uncertainty,
            disposition,
            uncertainty: cell_uncertainty,
            negative_evidence: cell_negative,
        });
    }
    cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let cell_lookup = cells
        .iter()
        .map(|cell| (cell.cell_id.clone(), cell))
        .collect::<BTreeMap<_, _>>();
    let candidate_order = panel.selected_order.clone();
    let branch_order = panel.branch_order.clone();
    let mut candidates = Vec::new();
    for candidate_id in &candidate_order {
        let expected_branch_order = expected_cells
            .iter()
            .filter(|(candidate, _)| candidate == candidate_id)
            .map(|(_, branch)| branch.clone())
            .collect::<Vec<_>>();
        let supported_branch_order = expected_branch_order
            .iter()
            .filter(|branch| {
                cell_lookup[&cell_id(candidate_id, branch)].disposition
                    == ClonePanelCellDisposition::Supported
            })
            .cloned()
            .collect::<Vec<_>>();
        let unresolved_branch_order = expected_branch_order
            .iter()
            .filter(|branch| {
                matches!(
                    cell_lookup[&cell_id(candidate_id, branch)].disposition,
                    ClonePanelCellDisposition::Unresolved
                        | ClonePanelCellDisposition::Contradictory
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let effects = expected_branch_order
            .iter()
            .filter_map(|branch| cell_lookup[&cell_id(candidate_id, branch)].median_effect_milli)
            .collect::<Vec<_>>();
        let mut effect_values = effects;
        let median_effect = median(&mut effect_values);
        let disposition = if expected_branch_order.is_empty() {
            ClonePanelCellDisposition::Unresolved
        } else if supported_branch_order.len() == expected_branch_order.len() {
            ClonePanelCellDisposition::Supported
        } else if unresolved_branch_order.len() == expected_branch_order.len() {
            ClonePanelCellDisposition::Unresolved
        } else {
            ClonePanelCellDisposition::Negative
        };
        candidates.push(ClonePanelCandidateAnalysis {
            candidate_id: candidate_id.clone(),
            expected_branch_order,
            supported_branch_order,
            unresolved_branch_order,
            median_effect_milli: median_effect,
            disposition,
        });
    }
    let mut branch_rows = Vec::new();
    for branch_id in &branch_order {
        let selected_candidate_order = panel
            .branch_coverage
            .iter()
            .find(|branch| branch.branch_id == *branch_id)
            .map(|branch| branch.selected_candidate_order.clone())
            .unwrap_or_default();
        let supported_candidate_order = selected_candidate_order
            .iter()
            .filter(|candidate| {
                cell_lookup[&cell_id(candidate, branch_id)].disposition
                    == ClonePanelCellDisposition::Supported
            })
            .cloned()
            .collect::<Vec<_>>();
        let unresolved_candidate_order = selected_candidate_order
            .iter()
            .filter(|candidate| {
                matches!(
                    cell_lookup[&cell_id(candidate, branch_id)].disposition,
                    ClonePanelCellDisposition::Unresolved
                        | ClonePanelCellDisposition::Contradictory
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let disposition = if supported_candidate_order.is_empty()
            && unresolved_candidate_order.len() == selected_candidate_order.len()
        {
            ClonePanelCellDisposition::Unresolved
        } else if !supported_candidate_order.is_empty() {
            ClonePanelCellDisposition::Supported
        } else {
            ClonePanelCellDisposition::Negative
        };
        if disposition != ClonePanelCellDisposition::Supported {
            uncertainty.insert(format!("branch-not-supported:{branch_id}"));
        }
        branch_rows.push(ClonePanelBranchAnalysis {
            branch_id: branch_id.clone(),
            selected_candidate_order,
            supported_candidate_order,
            unresolved_candidate_order,
            disposition,
        });
    }
    for leftover in by_cell.values() {
        for observation in leftover {
            uncertainty.insert(format!(
                "observation-not-in-selected-panel:{}",
                observation.observation_id
            ));
        }
    }
    let mut next_actions = BTreeSet::new();
    for cell in &cells {
        match cell.disposition {
            ClonePanelCellDisposition::Unresolved | ClonePanelCellDisposition::Contradictory => {
                next_actions.insert(format!("measure:{}", cell.cell_id));
            }
            ClonePanelCellDisposition::Null | ClonePanelCellDisposition::Negative => {
                next_actions.insert(format!("retest:{}", cell.cell_id));
            }
            ClonePanelCellDisposition::Supported => {}
        }
    }
    let all_supported = !cells.is_empty()
        && cells
            .iter()
            .all(|cell| cell.disposition == ClonePanelCellDisposition::Supported)
        && branch_rows
            .iter()
            .all(|branch| branch.disposition == ClonePanelCellDisposition::Supported);
    let all_complete_negative = !cells.is_empty()
        && cells.iter().all(|cell| {
            matches!(
                cell.disposition,
                ClonePanelCellDisposition::Negative | ClonePanelCellDisposition::Null
            )
        });
    let disposition = if all_supported
        && panel.disposition == ClonePerturbationPanelDisposition::Qualified
        && (!request.require_all_selected || !missing_selected_cells)
        && (!request.require_all_branches
            || branch_rows
                .iter()
                .all(|branch| !branch.selected_candidate_order.is_empty()))
    {
        ClonePanelOutcomeDisposition::Qualified
    } else if all_complete_negative {
        ClonePanelOutcomeDisposition::Negative
    } else if cells.iter().any(|cell| cell.replicate_count > 0) {
        ClonePanelOutcomeDisposition::Partial
    } else {
        ClonePanelOutcomeDisposition::Unresolved
    };
    if request.require_all_selected && missing_selected_cells {
        uncertainty.insert("selected-panel-cells-are-missing".into());
    }
    if request.require_all_branches
        && branch_rows
            .iter()
            .any(|branch| branch.selected_candidate_order.is_empty())
    {
        uncertainty.insert("branch-has-no-selected-candidate".into());
    }
    if panel.disposition != ClonePerturbationPanelDisposition::Qualified {
        uncertainty.insert("design-panel-was-not-qualified".into());
    }
    let cell_order = cells
        .iter()
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    let mut output = ClonePanelOutcomeAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        panel_digest: panel.digest.clone(),
        cell_order,
        candidate_order,
        branch_order,
        cells,
        candidates,
        branches: branch_rows,
        next_action_order: next_actions.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-clone-panel-outcomes"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClonePanelOutcomeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::{
        analyze_glioma_clonal_evolution, ClonalEvolutionRequest, CloneMarker, CloneMarkerState,
        CloneProfile,
    };
    use crate::glioma::programs::p06_experiment_design::{
        plan_glioma_clone_perturbation_panel, ClonePerturbationCandidate, ClonePerturbationKind,
        ClonePerturbationPanelRequest,
    };
    use bioprism_ids::ContentHash;

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

    fn panel() -> ClonePerturbationPanel {
        let profile = |id: &str, clone_id: &str, timepoint: u32, markers: &[&str]| CloneProfile {
            profile_id: id.into(),
            study_id: "outcome-study".into(),
            sample_lineage: "lineage-a".into(),
            clone_id: clone_id.into(),
            timepoint,
            model_system: GliomaModelSystem::Organoid,
            abundance_milli: if timepoint == 0 { 400 } else { 600 },
            artifact: artifact(id),
            markers: markers
                .iter()
                .map(|marker_id| CloneMarker {
                    marker_id: (*marker_id).into(),
                    state: CloneMarkerState::Present,
                    confidence_milli: 900,
                })
                .collect(),
        };
        let graph = analyze_glioma_clonal_evolution(
            &ClonalEvolutionRequest {
                study_id: "outcome-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_markers: 1,
                min_parent_score_milli: 500,
                max_time_gap: 5,
                min_abundance_milli: 1,
                allow_parallel_branches: true,
                max_parent_candidates: 2,
            },
            &[
                profile("root", "clone-a", 0, &["egfr", "tp53"]),
                profile("ec", "clone-b", 1, &["egfr", "tp53", "ecDNA"]),
                profile("pt", "clone-c", 1, &["egfr", "tp53", "pten"]),
            ],
        )
        .unwrap();
        plan_glioma_clone_perturbation_panel(
            &ClonePerturbationPanelRequest {
                study_id: "outcome-study".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_milli: 10,
                min_coverage_milli: 500,
                max_selected: 2,
                require_branch_coverage: true,
                allow_uncertain_targets: true,
            },
            &graph,
            &[
                ClonePerturbationCandidate {
                    candidate_id: "ec-panel".into(),
                    kind: ClonePerturbationKind::Inhibit,
                    target_marker_order: vec!["ecDNA".into()],
                    cost_milli: 5,
                    expected_effect_milli: 900,
                    purpose: "branch assay".into(),
                    artifact: artifact("ec-panel"),
                },
                ClonePerturbationCandidate {
                    candidate_id: "pt-panel".into(),
                    kind: ClonePerturbationKind::Inhibit,
                    target_marker_order: vec!["pten".into()],
                    cost_milli: 5,
                    expected_effect_milli: 900,
                    purpose: "branch assay".into(),
                    artifact: artifact("pt-panel"),
                },
            ],
        )
        .unwrap()
    }

    fn request() -> ClonePanelOutcomeRequest {
        ClonePanelOutcomeRequest {
            study_id: "outcome-study".into(),
            model_system: GliomaModelSystem::Organoid,
            min_replicates: 2,
            effect_threshold_milli: 500,
            max_uncertainty_milli: 200,
            require_all_selected: true,
            require_all_branches: true,
        }
    }

    fn observations(panel: &ClonePerturbationPanel) -> Vec<ClonePanelObservation> {
        panel
            .branch_coverage
            .iter()
            .flat_map(|branch| {
                branch
                    .selected_candidate_order
                    .iter()
                    .flat_map(move |candidate_id| {
                        (0..2).map(move |replicate| ClonePanelObservation {
                            observation_id: format!(
                                "{candidate_id}-{}-{replicate}",
                                branch.branch_id
                            ),
                            study_id: "outcome-study".into(),
                            model_system: GliomaModelSystem::Organoid,
                            candidate_id: candidate_id.clone(),
                            branch_id: branch.branch_id.clone(),
                            replicate_id: format!("r{replicate}"),
                            state: ClonePanelMeasurementState::Measured,
                            effect_milli: -800,
                            uncertainty_milli: 100,
                            artifact: artifact(&format!(
                                "{candidate_id}-{}-{replicate}",
                                branch.branch_id
                            )),
                        })
                    })
            })
            .collect()
    }

    #[test]
    fn supported_replicates_qualify_and_replay() {
        let panel = panel();
        let observations = observations(&panel);
        let first = analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations).unwrap();
        let second =
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ClonePanelOutcomeDisposition::Qualified);
        assert!(first.next_action_order.is_empty());
    }

    #[test]
    fn missing_branch_is_unresolved_and_requests_measurement() {
        let panel = panel();
        let mut observations = observations(&panel);
        observations.retain(|observation| !observation.branch_id.contains("pt"));
        let output =
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations).unwrap();
        assert_eq!(output.disposition, ClonePanelOutcomeDisposition::Partial);
        assert!(output
            .next_action_order
            .iter()
            .any(|item| item.starts_with("measure:")));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "selected-panel-cells-are-missing"));
    }

    #[test]
    fn contradictory_replicates_remain_negative_evidence() {
        let panel = panel();
        let mut observations = observations(&panel);
        observations[0].state = ClonePanelMeasurementState::Contradictory;
        let output =
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations).unwrap();
        assert_eq!(output.disposition, ClonePanelOutcomeDisposition::Partial);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("declared-contradictory-measurement")));
    }

    #[test]
    fn input_permutation_is_replay_stable() {
        let panel = panel();
        let observations = observations(&panel);
        let reversed = observations.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations).unwrap(),
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &reversed).unwrap()
        );
    }

    #[test]
    fn human_artifact_is_refused() {
        let panel = panel();
        let mut observations = observations(&panel);
        observations[0].artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_clone_panel_outcomes(&request(), &panel, &observations),
            Err(ClonePanelOutcomeError::InvalidObservation(_))
        ));
    }
}
