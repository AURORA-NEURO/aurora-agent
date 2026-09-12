//! Longitudinal discrete-state transition analysis for preclinical glioma models.
//!
//! The analyzer turns repeated observations of de-identified experimental units into transition
//! matrices and treatment-vs-control contrasts. State labels are caller-declared research states
//! (for example, invasion-score bands or experimentally defined phenotypes), not diagnoses. The
//! computation is descriptive and uncertainty-aware: irregular sampling, absent transitions,
//! sparse arms, and null or contradictory contrasts remain explicit.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaStateTransition1@1";
pub const MAX_STATES: usize = 128;
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_UNITS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    /// Ordered research states. The order is descriptive and supplied by the investigator; it
    /// is used only to label upward/downward transitions, never as a clinical severity scale.
    pub state_order: Vec<String>,
    pub min_units_per_arm: usize,
    pub min_transitions_per_arm: usize,
    pub max_timepoint_gap: u32,
    pub min_contrast_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub timepoint: u32,
    pub state_id: String,
    pub state_score_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionDirection {
    Upward,
    Downward,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionCellDisposition {
    Estimable,
    Absent,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionCell {
    pub arm_id: String,
    pub from_state: String,
    pub to_state: String,
    pub direction: TransitionDirection,
    pub transition_count: u32,
    pub source_transition_count: u32,
    pub source_unit_count: u32,
    pub probability_milli: u16,
    pub disposition: TransitionCellDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionContrastDisposition {
    EnrichedInTreatment,
    ReducedInTreatment,
    Null,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionContrast {
    pub contrast_id: String,
    pub from_state: String,
    pub to_state: String,
    pub direction: TransitionDirection,
    pub control_probability_milli: u16,
    pub treatment_probability_milli: u16,
    pub difference_milli: i32,
    pub absolute_difference_milli: u16,
    pub control_source_transition_count: u32,
    pub treatment_source_transition_count: u32,
    pub disposition: TransitionContrastDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateTransitionDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub control_arm: String,
    pub treatment_arm: String,
    pub state_order: Vec<String>,
    pub arm_order: Vec<String>,
    pub unit_order: Vec<String>,
    pub cells: Vec<StateTransitionCell>,
    pub cell_order: Vec<String>,
    pub contrasts: Vec<StateTransitionContrast>,
    pub contrast_order: Vec<String>,
    pub enriched_order: Vec<String>,
    pub reduced_order: Vec<String>,
    pub null_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub control_unit_count: u32,
    pub treatment_unit_count: u32,
    pub control_transition_count: u32,
    pub treatment_transition_count: u32,
    pub skipped_gap_count: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: StateTransitionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateTransitionError {
    #[error("state-transition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("state-transition input is invalid: {0}")]
    InvalidInput(String),
    #[error("state-transition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("state-transition digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &StateTransitionAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "state_order": output.state_order,
        "arm_order": output.arm_order,
        "unit_order": output.unit_order,
        "cells": output.cells,
        "cell_order": output.cell_order,
        "contrasts": output.contrasts,
        "contrast_order": output.contrast_order,
        "enriched_order": output.enriched_order,
        "reduced_order": output.reduced_order,
        "null_order": output.null_order,
        "unresolved_order": output.unresolved_order,
        "control_unit_count": output.control_unit_count,
        "treatment_unit_count": output.treatment_unit_count,
        "control_transition_count": output.control_transition_count,
        "treatment_transition_count": output.treatment_transition_count,
        "skipped_gap_count": output.skipped_gap_count,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn direction(from: usize, to: usize) -> TransitionDirection {
    match to.cmp(&from) {
        std::cmp::Ordering::Less => TransitionDirection::Downward,
        std::cmp::Ordering::Equal => TransitionDirection::Stable,
        std::cmp::Ordering::Greater => TransitionDirection::Upward,
    }
}

impl StateTransitionAnalysis {
    pub fn validate(&self) -> Result<(), StateTransitionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || self.state_order.len() < 2
            || self.state_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.arm_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.unit_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.cell_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .contrast_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.cells.len() != self.cell_order.len()
            || self.contrasts.len() != self.contrast_order.len()
            || self.cells.iter().any(|cell| {
                cell.arm_id.trim().is_empty()
                    || cell.from_state.trim().is_empty()
                    || cell.to_state.trim().is_empty()
                    || cell.source_transition_count == 0
                        && cell.disposition != TransitionCellDisposition::Unresolved
                    || cell.probability_milli > 1_000
                    || cell.rationale.trim().is_empty()
            })
            || self.contrasts.iter().any(|contrast| {
                contrast.contrast_id.trim().is_empty()
                    || contrast.from_state.trim().is_empty()
                    || contrast.to_state.trim().is_empty()
                    || contrast.absolute_difference_milli > 1_000
                    || contrast.rationale.trim().is_empty()
            })
            || self
                .enriched_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.reduced_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.null_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .unresolved_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .iter()
                .chain(self.uncertainty.iter())
                .any(|item| item.trim().is_empty())
            || self.digest.as_str().len() != 64
        {
            return Err(StateTransitionError::InvalidOutput(
                "identity, state/arm ordering, transition metrics, or digest is invalid".into(),
            ));
        }
        let cell_ids = self
            .cells
            .iter()
            .map(|cell| format!("{}:{}:{}", cell.arm_id, cell.from_state, cell.to_state))
            .collect::<BTreeSet<_>>();
        if cell_ids.len() != self.cells.len()
            || cell_ids != self.cell_order.iter().cloned().collect::<BTreeSet<_>>()
        {
            return Err(StateTransitionError::InvalidOutput(
                "transition cell identities do not reconcile".into(),
            ));
        }
        let contrast_ids = self
            .contrasts
            .iter()
            .map(|contrast| contrast.contrast_id.as_str())
            .collect::<BTreeSet<_>>();
        if contrast_ids.len() != self.contrasts.len()
            || contrast_ids
                != self
                    .contrast_order
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
        {
            return Err(StateTransitionError::InvalidOutput(
                "transition contrast identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| StateTransitionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(StateTransitionError::InvalidOutput(
                "state-transition digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_inputs(
    request: &StateTransitionRequest,
    observations: &[StateTransitionObservation],
) -> Result<(), StateTransitionError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.state_order.len() < 2
        || request.state_order.len() > MAX_STATES
        || request
            .state_order
            .iter()
            .any(|state| state.trim().is_empty())
        || request.state_order.iter().collect::<BTreeSet<_>>().len() != request.state_order.len()
        || request.min_units_per_arm == 0
        || request.min_transitions_per_arm == 0
        || request.max_timepoint_gap == 0
        || request.min_contrast_milli > 1_000
    {
        return Err(StateTransitionError::InvalidRequest(
            "objective, distinct arms, ordered states, unit/transition floors, gap, or contrast threshold is invalid".into(),
        ));
    }
    if observations.is_empty() || observations.len() > MAX_OBSERVATIONS {
        return Err(StateTransitionError::InvalidInput(
            "observation count is empty or exceeds the bounded longitudinal capacity".into(),
        ));
    }
    let state_ids = request.state_order.iter().cloned().collect::<BTreeSet<_>>();
    let arm_ids = BTreeSet::from([request.control_arm.as_str(), request.treatment_arm.as_str()]);
    let mut observation_ids = BTreeSet::new();
    let mut unit_timepoints = BTreeSet::new();
    let mut units = BTreeSet::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || !arm_ids.contains(observation.arm_id.as_str())
            || observation.model_system != request.model_system
            || !state_ids.contains(&observation.state_id)
            || observation.state_score_milli > 1_000
            || !observation_ids.insert(observation.observation_id.clone())
            || !unit_timepoints.insert((observation.unit_id.clone(), observation.timepoint))
        {
            return Err(StateTransitionError::InvalidInput(
                "observation identity, arm/model/state binding, score, or duplicate timepoint is invalid".into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| StateTransitionError::InvalidInput(error.to_string()))?;
        units.insert(observation.unit_id.clone());
    }
    if units.len() > MAX_UNITS {
        return Err(StateTransitionError::InvalidInput(
            "unit count exceeds the bounded longitudinal capacity".into(),
        ));
    }
    Ok(())
}

/// Estimate state transition matrices and a treatment-vs-control contrast for local longitudinal
/// preclinical glioma observations.
pub fn analyze_glioma_state_transitions(
    request: &StateTransitionRequest,
    observations: &[StateTransitionObservation],
) -> Result<StateTransitionAnalysis, StateTransitionError> {
    validate_inputs(request, observations)?;
    let state_index = request
        .state_order
        .iter()
        .enumerate()
        .map(|(index, state)| (state.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut sorted = observations.to_vec();
    sorted.sort_by(|left, right| {
        left.arm_id
            .cmp(&right.arm_id)
            .then_with(|| left.unit_id.cmp(&right.unit_id))
            .then_with(|| left.timepoint.cmp(&right.timepoint))
    });
    let mut by_unit = BTreeMap::<(String, String), Vec<&StateTransitionObservation>>::new();
    for observation in &sorted {
        by_unit
            .entry((observation.arm_id.clone(), observation.unit_id.clone()))
            .or_default()
            .push(observation);
    }
    let mut counts = BTreeMap::<(String, String, String), u32>::new();
    let mut source_counts = BTreeMap::<(String, String), u32>::new();
    let mut source_units = BTreeMap::<String, BTreeSet<String>>::new();
    let mut skipped_gap_count = 0_u32;
    for ((arm, unit), unit_observations) in by_unit {
        for window in unit_observations.windows(2) {
            let gap = window[1].timepoint.saturating_sub(window[0].timepoint);
            if gap == 0 || gap > request.max_timepoint_gap {
                skipped_gap_count = skipped_gap_count.saturating_add(1);
                continue;
            }
            let from = window[0].state_id.clone();
            let to = window[1].state_id.clone();
            *counts.entry((arm.clone(), from.clone(), to)).or_default() += 1;
            *source_counts.entry((arm.clone(), from)).or_default() += 1;
            source_units
                .entry(arm.clone())
                .or_default()
                .insert(unit.clone());
        }
    }
    let mut arm_order = vec![request.control_arm.clone(), request.treatment_arm.clone()];
    arm_order.sort();
    let mut unit_order = sorted
        .iter()
        .map(|observation| observation.unit_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unit_order.sort();
    let mut cells = Vec::new();
    for arm in &arm_order {
        for from in &request.state_order {
            for to in &request.state_order {
                let transition_count = counts
                    .get(&(arm.clone(), from.clone(), to.clone()))
                    .copied()
                    .unwrap_or(0);
                let source_transition_count = source_counts
                    .get(&(arm.clone(), from.clone()))
                    .copied()
                    .unwrap_or(0);
                let source_unit_count = source_units
                    .get(arm)
                    .map(|units| units.len() as u32)
                    .unwrap_or(0);
                let from_index = state_index[from.as_str()];
                let to_index = state_index[to.as_str()];
                let (probability, disposition, rationale) = if source_transition_count == 0 {
                    (
                        0,
                        TransitionCellDisposition::Unresolved,
                        "no observed outgoing transition from this state in the arm".into(),
                    )
                } else if transition_count == 0 {
                    (
                        0,
                        TransitionCellDisposition::Absent,
                        "no transition of this type was observed among eligible consecutive timepoints".into(),
                    )
                } else {
                    (
                        ((u64::from(transition_count) * 1_000)
                            / u64::from(source_transition_count))
                            .min(1_000) as u16,
                        TransitionCellDisposition::Estimable,
                        "transition probability is estimated from eligible consecutive observations".into(),
                    )
                };
                cells.push(StateTransitionCell {
                    arm_id: arm.clone(),
                    from_state: from.clone(),
                    to_state: to.clone(),
                    direction: direction(from_index, to_index),
                    transition_count,
                    source_transition_count,
                    source_unit_count,
                    probability_milli: probability,
                    disposition,
                    rationale,
                });
            }
        }
    }
    let mut contrasts = Vec::new();
    for from in &request.state_order {
        for to in &request.state_order {
            let control = cells.iter().find(|cell| {
                cell.arm_id == request.control_arm
                    && cell.from_state == *from
                    && cell.to_state == *to
            });
            let treatment = cells.iter().find(|cell| {
                cell.arm_id == request.treatment_arm
                    && cell.from_state == *from
                    && cell.to_state == *to
            });
            let (control, treatment) = (
                control.expect("control cell generated"),
                treatment.expect("treatment cell generated"),
            );
            let contrast_id = format!("{}:{}", from, to);
            let difference =
                i32::from(treatment.probability_milli) - i32::from(control.probability_milli);
            let absolute_difference = difference.unsigned_abs().min(1_000) as u16;
            let both_estimable = control.source_transition_count
                >= request.min_transitions_per_arm as u32
                && treatment.source_transition_count >= request.min_transitions_per_arm as u32;
            let (disposition, rationale) = if !both_estimable {
                (
                    TransitionContrastDisposition::Unresolved,
                    "one or both arms lack the declared transition support floor".into(),
                )
            } else if absolute_difference < request.min_contrast_milli {
                (
                    TransitionContrastDisposition::Null,
                    "treatment and control transition probabilities remain within the declared contrast gate".into(),
                )
            } else if difference > 0 {
                (
                    TransitionContrastDisposition::EnrichedInTreatment,
                    "transition probability is higher in treatment than control under the declared descriptive contrast".into(),
                )
            } else {
                (
                    TransitionContrastDisposition::ReducedInTreatment,
                    "transition probability is lower in treatment than control under the declared descriptive contrast".into(),
                )
            };
            contrasts.push(StateTransitionContrast {
                contrast_id,
                from_state: from.clone(),
                to_state: to.clone(),
                direction: direction(state_index[from.as_str()], state_index[to.as_str()]),
                control_probability_milli: control.probability_milli,
                treatment_probability_milli: treatment.probability_milli,
                difference_milli: difference,
                absolute_difference_milli: absolute_difference,
                control_source_transition_count: control.source_transition_count,
                treatment_source_transition_count: treatment.source_transition_count,
                disposition,
                rationale,
            });
        }
    }
    contrasts.sort_by(|left, right| {
        right
            .absolute_difference_milli
            .cmp(&left.absolute_difference_milli)
            .then_with(|| left.contrast_id.cmp(&right.contrast_id))
    });
    let cell_order = cells
        .iter()
        .map(|cell| format!("{}:{}:{}", cell.arm_id, cell.from_state, cell.to_state))
        .collect::<Vec<_>>();
    let mut sorted_cell_order = cell_order.clone();
    sorted_cell_order.sort();
    let mut contrast_order = contrasts
        .iter()
        .map(|contrast| contrast.contrast_id.clone())
        .collect::<Vec<_>>();
    contrast_order.sort();
    let mut enriched = Vec::new();
    let mut reduced = Vec::new();
    let mut null = Vec::new();
    let mut unresolved = Vec::new();
    let mut negative = Vec::new();
    for contrast in &contrasts {
        match contrast.disposition {
            TransitionContrastDisposition::EnrichedInTreatment => {
                enriched.push(contrast.contrast_id.clone())
            }
            TransitionContrastDisposition::ReducedInTreatment => {
                reduced.push(contrast.contrast_id.clone());
                negative.push(format!("reduced-in-treatment:{}", contrast.contrast_id));
            }
            TransitionContrastDisposition::Null => {
                null.push(contrast.contrast_id.clone());
                negative.push(format!("null-transition-contrast:{}", contrast.contrast_id));
            }
            TransitionContrastDisposition::Unresolved => {
                unresolved.push(contrast.contrast_id.clone())
            }
        }
    }
    for order in [
        &mut enriched,
        &mut reduced,
        &mut null,
        &mut unresolved,
        &mut negative,
    ] {
        order.sort();
    }
    let control_unit_count = sorted
        .iter()
        .filter(|observation| observation.arm_id == request.control_arm)
        .map(|observation| observation.unit_id.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u32;
    let treatment_unit_count = sorted
        .iter()
        .filter(|observation| observation.arm_id == request.treatment_arm)
        .map(|observation| observation.unit_id.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u32;
    let control_transition_count = source_counts
        .iter()
        .filter(|((arm, _), _)| arm == &request.control_arm)
        .map(|(_, count)| *count)
        .sum::<u32>();
    let treatment_transition_count = source_counts
        .iter()
        .filter(|((arm, _), _)| arm == &request.treatment_arm)
        .map(|(_, count)| *count)
        .sum::<u32>();
    let mut uncertainty = Vec::new();
    if control_unit_count < request.min_units_per_arm as u32
        || treatment_unit_count < request.min_units_per_arm as u32
    {
        uncertainty.push("one or both arms are below the declared longitudinal unit floor".into());
    }
    if control_transition_count < request.min_transitions_per_arm as u32
        || treatment_transition_count < request.min_transitions_per_arm as u32
    {
        uncertainty
            .push("one or both arms are below the declared eligible transition floor".into());
    }
    if skipped_gap_count > 0 {
        uncertainty.push(format!(
            "{skipped_gap_count} irregular or over-gap observation windows were excluded"
        ));
    }
    if contrasts.iter().any(|contrast| {
        contrast.disposition == TransitionContrastDisposition::Unresolved
            && (contrast.control_source_transition_count > 0
                || contrast.treatment_source_transition_count > 0)
    }) {
        uncertainty.push("some transition contrasts are not estimable in both arms".into());
    }
    let disposition = if control_unit_count < request.min_units_per_arm as u32
        || treatment_unit_count < request.min_units_per_arm as u32
        || control_transition_count == 0
        || treatment_transition_count == 0
    {
        StateTransitionDisposition::Unresolved
    } else if !enriched.is_empty() || !reduced.is_empty() {
        if uncertainty.is_empty() {
            StateTransitionDisposition::Qualified
        } else {
            StateTransitionDisposition::Partial
        }
    } else if !null.is_empty() {
        StateTransitionDisposition::Negative
    } else {
        StateTransitionDisposition::Partial
    };
    let mut output = StateTransitionAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        state_order: request.state_order.clone(),
        arm_order,
        unit_order,
        cells,
        cell_order: sorted_cell_order,
        contrasts,
        contrast_order,
        enriched_order: enriched,
        reduced_order: reduced,
        null_order: null,
        unresolved_order: unresolved,
        control_unit_count,
        treatment_unit_count,
        control_transition_count,
        treatment_transition_count,
        skipped_gap_count,
        negative_evidence: negative,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-state-transition"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| StateTransitionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> StateTransitionRequest {
        StateTransitionRequest {
            objective: "compare invasion-state transitions in glioma organoids".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            state_order: vec!["low".into(), "high".into()],
            min_units_per_arm: 2,
            min_transitions_per_arm: 2,
            max_timepoint_gap: 2,
            min_contrast_milli: 100,
        }
    }

    fn observations() -> Vec<StateTransitionObservation> {
        let mut result = Vec::new();
        for (arm, first, second) in [
            ("control", "low", "low"),
            ("control", "low", "low"),
            ("treated", "low", "high"),
            ("treated", "low", "high"),
        ] {
            let index = result.len();
            result.push(StateTransitionObservation {
                observation_id: format!("o-{index}-0"),
                unit_id: format!("{arm}-{index}"),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: format!("b-{index}-0"),
                timepoint: 0,
                state_id: first.into(),
                state_score_milli: if first == "low" { 100 } else { 900 },
                artifact: artifact(&format!("o-{index}-0")),
            });
            result.push(StateTransitionObservation {
                observation_id: format!("o-{index}-1"),
                unit_id: format!("{arm}-{index}"),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: format!("b-{index}-1"),
                timepoint: 1,
                state_id: second.into(),
                state_score_milli: if second == "low" { 100 } else { 900 },
                artifact: artifact(&format!("o-{index}-1")),
            });
        }
        result
    }

    #[test]
    fn transition_matrix_and_contrast_are_qualified() {
        let output = analyze_glioma_state_transitions(&request(), &observations()).unwrap();
        assert_eq!(output.disposition, StateTransitionDisposition::Qualified);
        assert!(output.enriched_order.contains(&"low:high".into()));
        output.validate().unwrap();
    }

    #[test]
    fn sparse_and_irregular_windows_remain_unresolved() {
        let mut sparse = observations();
        sparse.retain(|observation| observation.arm_id == "control");
        sparse[1].timepoint = 99;
        let output = analyze_glioma_state_transitions(&request(), &sparse).unwrap();
        assert_eq!(output.disposition, StateTransitionDisposition::Unresolved);
        assert!(output.skipped_gap_count > 0);
        assert!(!output.uncertainty.is_empty());
    }

    #[test]
    fn input_permutation_replays_identically() {
        let first = analyze_glioma_state_transitions(&request(), &observations()).unwrap();
        let mut reversed = observations();
        reversed.reverse();
        let second = analyze_glioma_state_transitions(&request(), &reversed).unwrap();
        assert_eq!(first, second);
    }
}
