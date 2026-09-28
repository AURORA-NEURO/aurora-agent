//! Longitudinal discrete-state transition analysis for preclinical glioma models.
//!
//! The analyzer turns repeated observations of independent preclinical experimental units into
//! unit-balanced transition matrices and treatment-vs-control contrasts. State labels are
//! caller-declared research states (for example, invasion-score bands or experimentally defined
//! phenotypes), not diagnoses. This is a descriptive unit-level analysis: without lineage-resolved
//! cell observations it cannot establish single-cell plasticity or separate state switching from
//! differential clonal growth. Irregular sampling, absent transitions, sparse arms, and null or
//! contradictory contrasts remain explicit. Implements `GAF-GLIOMA-P10-F14`.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaStateTransition1@3";
pub const MAX_STATES: usize = 128;
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_UNITS: usize = 16_384;
pub const MAX_BATCH_SENSITIVITY_BATCHES: usize = 128;
const MAX_BATCH_SENSITIVITY_WORK: u128 = 8_388_608;

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
    /// Identifier for one independent experimental unit, held constant across that unit's
    /// repeated timepoints. Do not use a cell, sequencing read, image field, or transition window
    /// as the unit; repeated windows are clustered within this identifier by the estimator.
    pub unit_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub timepoint: u32,
    pub state_id: String,
    pub state_score_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionUnitDestinationCount {
    pub to_state: String,
    pub transition_count: u32,
}

/// Sparse transition row for one experimental unit and one observed source state. Keeping this
/// profile makes downstream estimates and resampling operate on biological units, not windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionUnitProfile {
    pub arm_id: String,
    pub unit_id: String,
    pub from_state: String,
    pub source_transition_count: u32,
    pub destination_counts: Vec<StateTransitionUnitDestinationCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateTransitionBatchSensitivityDisposition {
    Stable,
    Fragile,
    Partial,
    Unresolved,
}

/// Leave-one-batch-out sensitivity summary for all outgoing transitions from one state.
/// It is deliberately separate from the primary unit-balanced estimate: omitting a batch is a
/// robustness diagnostic, not a replacement estimand or a claim of causal identification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransitionBatchSensitivity {
    pub from_state: String,
    pub batch_count: u32,
    pub evaluated_batch_count: u32,
    pub maximum_leave_one_batch_out_deviation_milli: u16,
    pub sign_reversal_count: u32,
    pub unstable_contrast_order: Vec<String>,
    pub disposition: StateTransitionBatchSensitivityDisposition,
    pub rationale: String,
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
    /// Pooled-window transition probability retained for descriptive compatibility.
    pub probability_milli: u16,
    /// Equal-weight mean of each independent unit's within-row transition proportions.
    pub unit_balanced_probability_milli: u16,
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
    /// Equal-unit probabilities used by the primary contrast and disposition.
    pub control_probability_milli: u16,
    pub treatment_probability_milli: u16,
    pub difference_milli: i32,
    pub absolute_difference_milli: u16,
    /// Pooled-window estimates retained for sensitivity diagnostics.
    pub control_pooled_probability_milli: u16,
    pub treatment_pooled_probability_milli: u16,
    pub pooled_difference_milli: i32,
    pub pooled_absolute_difference_milli: u16,
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
    pub unit_profiles: Vec<StateTransitionUnitProfile>,
    pub cells: Vec<StateTransitionCell>,
    pub cell_order: Vec<String>,
    pub contrasts: Vec<StateTransitionContrast>,
    pub batch_sensitivity: Vec<StateTransitionBatchSensitivity>,
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
        "unit_profiles": output.unit_profiles,
        "cells": output.cells,
        "cell_order": output.cell_order,
        "contrasts": output.contrasts,
        "batch_sensitivity": output.batch_sensitivity,
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

fn unit_balanced_probabilities(
    profiles: &[&StateTransitionUnitProfile],
    state_order: &[String],
) -> Vec<u16> {
    if profiles.is_empty() {
        return vec![0; state_order.len()];
    }
    let mut summed_milli = vec![0_u64; state_order.len()];
    for profile in profiles {
        let mut probabilities = vec![0_u16; state_order.len()];
        let mut remainders = Vec::with_capacity(state_order.len());
        let mut assigned = 0_u64;
        for (index, state) in state_order.iter().enumerate() {
            let count = profile
                .destination_counts
                .iter()
                .find(|destination| destination.to_state == *state)
                .map_or(0_u64, |destination| u64::from(destination.transition_count));
            let scaled = count * 1_000;
            let share = scaled / u64::from(profile.source_transition_count);
            probabilities[index] = share as u16;
            assigned += share;
            remainders.push((index, scaled % u64::from(profile.source_transition_count)));
        }
        remainders.sort_by(
            |(left_index, left_remainder), (right_index, right_remainder)| {
                right_remainder
                    .cmp(left_remainder)
                    .then_with(|| left_index.cmp(right_index))
            },
        );
        for (index, _) in remainders
            .into_iter()
            .take(1_000_u64.saturating_sub(assigned) as usize)
        {
            probabilities[index] = probabilities[index].saturating_add(1);
        }
        for (sum, probability) in summed_milli.iter_mut().zip(probabilities) {
            *sum += u64::from(probability);
        }
    }

    let unit_count = profiles.len() as u64;
    let mut probabilities = summed_milli
        .iter()
        .map(|sum| (sum / unit_count) as u16)
        .collect::<Vec<_>>();
    let mut remainders = summed_milli
        .iter()
        .enumerate()
        .map(|(index, sum)| (index, sum % unit_count))
        .collect::<Vec<_>>();
    let assigned = probabilities
        .iter()
        .map(|value| u64::from(*value))
        .sum::<u64>();
    remainders.sort_by(
        |(left_index, left_remainder), (right_index, right_remainder)| {
            right_remainder
                .cmp(left_remainder)
                .then_with(|| left_index.cmp(right_index))
        },
    );
    for (index, _) in remainders
        .into_iter()
        .take(1_000_u64.saturating_sub(assigned) as usize)
    {
        probabilities[index] = probabilities[index].saturating_add(1);
    }
    probabilities
}

#[derive(Debug, Clone)]
struct StateTransitionEvent {
    arm_id: String,
    unit_id: String,
    from_state: String,
    to_state: String,
    source_batch_id: String,
    destination_batch_id: String,
}

fn batch_excluded_row_probabilities(
    events: &[&StateTransitionEvent],
    from_state: &str,
    omitted_batch: &str,
    request: &StateTransitionRequest,
) -> BTreeMap<String, (u32, u32, Vec<u16>)> {
    let mut counts = BTreeMap::<(String, String), BTreeMap<String, u32>>::new();
    for event in events.iter().filter(|event| {
        event.from_state == from_state
            && event.source_batch_id != omitted_batch
            && event.destination_batch_id != omitted_batch
    }) {
        *counts
            .entry((event.arm_id.clone(), event.unit_id.clone()))
            .or_default()
            .entry(event.to_state.clone())
            .or_default() += 1;
    }

    let mut profiles_by_arm = BTreeMap::<String, Vec<StateTransitionUnitProfile>>::new();
    for ((arm_id, unit_id), destinations) in counts {
        let source_transition_count = destinations.values().copied().sum();
        let destination_counts = destinations
            .into_iter()
            .map(
                |(to_state, transition_count)| StateTransitionUnitDestinationCount {
                    to_state,
                    transition_count,
                },
            )
            .collect();
        profiles_by_arm
            .entry(arm_id.clone())
            .or_default()
            .push(StateTransitionUnitProfile {
                arm_id,
                unit_id,
                from_state: from_state.into(),
                source_transition_count,
                destination_counts,
            });
    }

    profiles_by_arm
        .into_iter()
        .map(|(arm_id, profiles)| {
            let references = profiles.iter().collect::<Vec<_>>();
            let unit_count = references.len() as u32;
            let transition_count = references
                .iter()
                .map(|profile| profile.source_transition_count)
                .sum();
            let probabilities = unit_balanced_probabilities(&references, &request.state_order);
            (arm_id, (unit_count, transition_count, probabilities))
        })
        .collect()
}

fn transition_sign(difference_milli: i32, minimum_contrast_milli: u16) -> i8 {
    if difference_milli == 0 || difference_milli.unsigned_abs() < u32::from(minimum_contrast_milli)
    {
        0
    } else if difference_milli > 0 {
        1
    } else {
        -1
    }
}

fn batch_sensitivity(
    request: &StateTransitionRequest,
    events: &[StateTransitionEvent],
    contrasts: &[StateTransitionContrast],
) -> Vec<StateTransitionBatchSensitivity> {
    let mut output = Vec::with_capacity(request.state_order.len());
    let mut total_work = 0_u128;
    for from_state in &request.state_order {
        let source_events = events
            .iter()
            .filter(|event| event.from_state == *from_state)
            .collect::<Vec<_>>();
        let mut batch_ids = source_events
            .iter()
            .flat_map(|event| [&event.source_batch_id, &event.destination_batch_id])
            .cloned()
            .collect::<BTreeSet<_>>();
        let batch_count = batch_ids.len().min(u32::MAX as usize) as u32;
        let source_contrasts = contrasts
            .iter()
            .filter(|contrast| contrast.from_state == *from_state)
            .collect::<Vec<_>>();
        if batch_ids.len() < 2 || batch_ids.len() > MAX_BATCH_SENSITIVITY_BATCHES {
            output.push(StateTransitionBatchSensitivity {
                from_state: from_state.clone(),
                batch_count,
                evaluated_batch_count: 0,
                maximum_leave_one_batch_out_deviation_milli: 0,
                sign_reversal_count: 0,
                unstable_contrast_order: Vec::new(),
                disposition: StateTransitionBatchSensitivityDisposition::Unresolved,
                rationale: if batch_ids.len() < 2 {
                    "fewer than two distinct assay batches contribute to this source-state row".into()
                } else {
                    format!(
                        "batch count exceeds the bounded leave-one-batch-out limit of {MAX_BATCH_SENSITIVITY_BATCHES}"
                    )
                },
            });
            continue;
        }
        let requested_work = (source_events.len() as u128).saturating_mul(batch_ids.len() as u128);
        if total_work.saturating_add(requested_work) > MAX_BATCH_SENSITIVITY_WORK {
            output.push(StateTransitionBatchSensitivity {
                from_state: from_state.clone(),
                batch_count,
                evaluated_batch_count: 0,
                maximum_leave_one_batch_out_deviation_milli: 0,
                sign_reversal_count: 0,
                unstable_contrast_order: Vec::new(),
                disposition: StateTransitionBatchSensitivityDisposition::Unresolved,
                rationale: "leave-one-batch-out work exceeds the bounded sensitivity budget".into(),
            });
            continue;
        }
        total_work = total_work.saturating_add(requested_work);

        let mut evaluated_batch_count = 0_u32;
        let mut maximum_deviation = 0_u16;
        let mut sign_reversal_count = 0_u32;
        let mut unstable_contrasts = BTreeSet::new();
        for batch_id in std::mem::take(&mut batch_ids) {
            let probabilities =
                batch_excluded_row_probabilities(&source_events, from_state, &batch_id, request);
            let Some((control_units, control_transitions, control_probabilities)) =
                probabilities.get(&request.control_arm)
            else {
                continue;
            };
            let Some((treatment_units, treatment_transitions, treatment_probabilities)) =
                probabilities.get(&request.treatment_arm)
            else {
                continue;
            };
            if *control_units < request.min_units_per_arm as u32
                || *treatment_units < request.min_units_per_arm as u32
                || *control_transitions < request.min_transitions_per_arm as u32
                || *treatment_transitions < request.min_transitions_per_arm as u32
            {
                continue;
            }
            evaluated_batch_count = evaluated_batch_count.saturating_add(1);
            for contrast in &source_contrasts {
                let Some(destination_index) = request
                    .state_order
                    .iter()
                    .position(|state| state == &contrast.to_state)
                else {
                    continue;
                };
                let difference = i32::from(treatment_probabilities[destination_index])
                    - i32::from(control_probabilities[destination_index]);
                let deviation = difference.abs_diff(contrast.difference_milli).min(1_000) as u16;
                maximum_deviation = maximum_deviation.max(deviation);
                let leave_one_out_sign = transition_sign(difference, request.min_contrast_milli);
                let primary_sign =
                    transition_sign(contrast.difference_milli, request.min_contrast_milli);
                if primary_sign != 0
                    && leave_one_out_sign != 0
                    && leave_one_out_sign != primary_sign
                {
                    sign_reversal_count = sign_reversal_count.saturating_add(1);
                }
                if leave_one_out_sign != primary_sign
                    || deviation > 0
                        && u32::from(deviation) >= u32::from(request.min_contrast_milli)
                {
                    unstable_contrasts.insert(contrast.contrast_id.clone());
                }
            }
        }
        let disposition = if evaluated_batch_count == 0 {
            StateTransitionBatchSensitivityDisposition::Unresolved
        } else if evaluated_batch_count < batch_count {
            StateTransitionBatchSensitivityDisposition::Partial
        } else if sign_reversal_count > 0 || !unstable_contrasts.is_empty() {
            StateTransitionBatchSensitivityDisposition::Fragile
        } else {
            StateTransitionBatchSensitivityDisposition::Stable
        };
        let rationale = match disposition {
            StateTransitionBatchSensitivityDisposition::Stable => {
                "all batch omissions retained the primary transition-contrast directions".into()
            }
            StateTransitionBatchSensitivityDisposition::Fragile => {
                "one or more batch omissions materially changed or reversed a transition contrast"
                    .into()
            }
            StateTransitionBatchSensitivityDisposition::Partial => {
                "some batch omissions fell below the independent-unit support floor".into()
            }
            StateTransitionBatchSensitivityDisposition::Unresolved => {
                "no batch omission retained sufficient independent-unit support in both arms".into()
            }
        };
        output.push(StateTransitionBatchSensitivity {
            from_state: from_state.clone(),
            batch_count,
            evaluated_batch_count,
            maximum_leave_one_batch_out_deviation_milli: maximum_deviation,
            sign_reversal_count,
            unstable_contrast_order: unstable_contrasts.into_iter().collect(),
            disposition,
            rationale,
        });
    }
    output.sort_by(|left, right| left.from_state.cmp(&right.from_state));
    output
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
            || self.batch_sensitivity.len() != self.state_order.len()
            || self
                .batch_sensitivity
                .windows(2)
                .any(|pair| pair[0].from_state >= pair[1].from_state)
            || self.batch_sensitivity.iter().any(|sensitivity| {
                sensitivity.from_state.trim().is_empty()
                    || !self.state_order.contains(&sensitivity.from_state)
                    || sensitivity.evaluated_batch_count > sensitivity.batch_count
                    || sensitivity.batch_count > MAX_OBSERVATIONS as u32
                    || sensitivity.maximum_leave_one_batch_out_deviation_milli > 1_000
                    || sensitivity
                        .unstable_contrast_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || sensitivity
                        .unstable_contrast_order
                        .iter()
                        .any(|contrast_id| {
                            !self.contrast_order.contains(contrast_id)
                                || !contrast_id.starts_with(&format!("{}:", sensitivity.from_state))
                        })
                    || sensitivity.rationale.trim().is_empty()
                    || match sensitivity.disposition {
                        StateTransitionBatchSensitivityDisposition::Stable => {
                            sensitivity.batch_count < 2
                                || sensitivity.evaluated_batch_count != sensitivity.batch_count
                                || sensitivity.sign_reversal_count != 0
                                || !sensitivity.unstable_contrast_order.is_empty()
                        }
                        StateTransitionBatchSensitivityDisposition::Fragile => {
                            sensitivity.evaluated_batch_count == 0
                                || sensitivity.sign_reversal_count == 0
                                    && sensitivity.unstable_contrast_order.is_empty()
                        }
                        StateTransitionBatchSensitivityDisposition::Partial => {
                            sensitivity.evaluated_batch_count == 0
                                || sensitivity.evaluated_batch_count >= sensitivity.batch_count
                        }
                        StateTransitionBatchSensitivityDisposition::Unresolved => {
                            sensitivity.evaluated_batch_count != 0
                        }
                    }
            })
            || self.cells.iter().any(|cell| {
                cell.arm_id.trim().is_empty()
                    || cell.from_state.trim().is_empty()
                    || cell.to_state.trim().is_empty()
                    || cell.source_transition_count == 0
                        && cell.disposition != TransitionCellDisposition::Unresolved
                    || cell.source_unit_count > cell.source_transition_count
                    || (cell.source_transition_count == 0) != (cell.source_unit_count == 0)
                    || cell.probability_milli > 1_000
                    || cell.unit_balanced_probability_milli > 1_000
                    || cell.rationale.trim().is_empty()
            })
            || self.contrasts.iter().any(|contrast| {
                contrast.contrast_id.trim().is_empty()
                    || contrast.from_state.trim().is_empty()
                    || contrast.to_state.trim().is_empty()
                    || contrast.control_probability_milli > 1_000
                    || contrast.treatment_probability_milli > 1_000
                    || contrast.difference_milli
                        != i32::from(contrast.treatment_probability_milli)
                            - i32::from(contrast.control_probability_milli)
                    || contrast.absolute_difference_milli
                        != contrast.difference_milli.unsigned_abs() as u16
                    || contrast.control_pooled_probability_milli > 1_000
                    || contrast.treatment_pooled_probability_milli > 1_000
                    || contrast.pooled_difference_milli
                        != i32::from(contrast.treatment_pooled_probability_milli)
                            - i32::from(contrast.control_pooled_probability_milli)
                    || contrast.pooled_absolute_difference_milli
                        != contrast.pooled_difference_milli.unsigned_abs() as u16
                    || contrast.absolute_difference_milli > 1_000
                    || contrast.pooled_absolute_difference_milli > 1_000
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
        for arm in &self.arm_order {
            for from_state in &self.state_order {
                let row = self
                    .cells
                    .iter()
                    .filter(|cell| cell.arm_id == *arm && cell.from_state == *from_state)
                    .collect::<Vec<_>>();
                let unit_balanced_sum = row
                    .iter()
                    .map(|cell| u32::from(cell.unit_balanced_probability_milli))
                    .sum::<u32>();
                if row.len() != self.state_order.len()
                    || row.iter().any(|cell| {
                        cell.source_transition_count != row[0].source_transition_count
                            || cell.source_unit_count != row[0].source_unit_count
                    })
                    || (row[0].source_unit_count > 0 && unit_balanced_sum != 1_000)
                    || (row[0].source_unit_count == 0 && unit_balanced_sum != 0)
                {
                    return Err(StateTransitionError::InvalidOutput(
                        "destination cells in a transition row disagree on independent-unit support".into(),
                    ));
                }
            }
        }
        let mut profile_transition_counts = BTreeMap::<(String, String, String), u64>::new();
        let mut profile_source_counts = BTreeMap::<(String, String), u64>::new();
        let mut profile_source_units = BTreeMap::<(String, String), BTreeSet<String>>::new();
        if self.unit_profiles.windows(2).any(|pair| {
            (&pair[0].arm_id, &pair[0].unit_id, &pair[0].from_state)
                >= (&pair[1].arm_id, &pair[1].unit_id, &pair[1].from_state)
        }) || self.unit_profiles.iter().any(|profile| {
            let destination_sum = profile
                .destination_counts
                .iter()
                .map(|destination| u64::from(destination.transition_count))
                .sum::<u64>();
            profile.arm_id.trim().is_empty()
                || profile.unit_id.trim().is_empty()
                || profile.from_state.trim().is_empty()
                || !self.arm_order.contains(&profile.arm_id)
                || !self.unit_order.contains(&profile.unit_id)
                || !self.state_order.contains(&profile.from_state)
                || profile.source_transition_count == 0
                || destination_sum != u64::from(profile.source_transition_count)
                || profile.destination_counts.is_empty()
                || profile.destination_counts.iter().any(|destination| {
                    destination.transition_count == 0
                        || !self.state_order.contains(&destination.to_state)
                })
                || profile
                    .destination_counts
                    .windows(2)
                    .any(|pair| pair[0].to_state >= pair[1].to_state)
        }) {
            return Err(StateTransitionError::InvalidOutput(
                "unit transition profiles are invalid, unordered, or outside the declared state space".into(),
            ));
        }
        for profile in &self.unit_profiles {
            *profile_source_counts
                .entry((profile.arm_id.clone(), profile.from_state.clone()))
                .or_default() += u64::from(profile.source_transition_count);
            profile_source_units
                .entry((profile.arm_id.clone(), profile.from_state.clone()))
                .or_default()
                .insert(profile.unit_id.clone());
            for destination in &profile.destination_counts {
                *profile_transition_counts
                    .entry((
                        profile.arm_id.clone(),
                        profile.from_state.clone(),
                        destination.to_state.clone(),
                    ))
                    .or_default() += u64::from(destination.transition_count);
            }
        }
        if self.cells.iter().any(|cell| {
            profile_transition_counts
                .get(&(
                    cell.arm_id.clone(),
                    cell.from_state.clone(),
                    cell.to_state.clone(),
                ))
                .copied()
                .unwrap_or(0)
                != u64::from(cell.transition_count)
                || profile_source_counts
                    .get(&(cell.arm_id.clone(), cell.from_state.clone()))
                    .copied()
                    .unwrap_or(0)
                    != u64::from(cell.source_transition_count)
                || profile_source_units
                    .get(&(cell.arm_id.clone(), cell.from_state.clone()))
                    .map_or(0, |units| units.len() as u32)
                    != cell.source_unit_count
        }) {
            return Err(StateTransitionError::InvalidOutput(
                "unit transition profiles do not reconcile with aggregate transition cells".into(),
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
    let mut source_units = BTreeMap::<(String, String), BTreeSet<String>>::new();
    let mut unit_profile_counts =
        BTreeMap::<(String, String, String), BTreeMap<String, u32>>::new();
    let mut transition_events = Vec::new();
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
            transition_events.push(StateTransitionEvent {
                arm_id: arm.clone(),
                unit_id: unit.clone(),
                from_state: from.clone(),
                to_state: to.clone(),
                source_batch_id: window[0].batch_id.clone(),
                destination_batch_id: window[1].batch_id.clone(),
            });
            *counts.entry((arm.clone(), from.clone(), to)).or_default() += 1;
            let to_state = window[1].state_id.clone();
            *unit_profile_counts
                .entry((arm.clone(), unit.clone(), from.clone()))
                .or_default()
                .entry(to_state)
                .or_default() += 1;
            *source_counts
                .entry((arm.clone(), from.clone()))
                .or_default() += 1;
            source_units
                .entry((arm.clone(), from.clone()))
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
    let unit_profiles = unit_profile_counts
        .into_iter()
        .map(|((arm_id, unit_id, from_state), destinations)| {
            let source_transition_count = destinations.values().copied().sum();
            let destination_counts = destinations
                .into_iter()
                .map(
                    |(to_state, transition_count)| StateTransitionUnitDestinationCount {
                        to_state,
                        transition_count,
                    },
                )
                .collect();
            StateTransitionUnitProfile {
                arm_id,
                unit_id,
                from_state,
                source_transition_count,
                destination_counts,
            }
        })
        .collect::<Vec<_>>();
    let mut cells = Vec::new();
    for arm in &arm_order {
        for from in &request.state_order {
            let row_profiles = unit_profiles
                .iter()
                .filter(|profile| profile.arm_id == *arm && profile.from_state == *from)
                .collect::<Vec<_>>();
            let unit_balanced_row =
                unit_balanced_probabilities(&row_profiles, &request.state_order);
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
                    .get(&(arm.clone(), from.clone()))
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
                    unit_balanced_probability_milli: unit_balanced_row[to_index],
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
            let difference = i32::from(treatment.unit_balanced_probability_milli)
                - i32::from(control.unit_balanced_probability_milli);
            let absolute_difference = difference.unsigned_abs() as u16;
            let pooled_difference =
                i32::from(treatment.probability_milli) - i32::from(control.probability_milli);
            let pooled_absolute_difference = pooled_difference.unsigned_abs() as u16;
            let both_estimable = control.source_transition_count
                >= request.min_transitions_per_arm as u32
                && treatment.source_transition_count >= request.min_transitions_per_arm as u32
                && control.source_unit_count >= request.min_units_per_arm as u32
                && treatment.source_unit_count >= request.min_units_per_arm as u32;
            let (disposition, rationale) = if !both_estimable {
                (
                    TransitionContrastDisposition::Unresolved,
                    "one or both arms lack the declared independent-unit or transition support floor for this source state".into(),
                )
            } else if absolute_difference < request.min_contrast_milli {
                (
                    TransitionContrastDisposition::Null,
                    "equal-unit treatment and control transition probabilities remain within the declared contrast gate".into(),
                )
            } else if difference > 0 {
                (
                    TransitionContrastDisposition::EnrichedInTreatment,
                    "equal-unit transition probability is higher in treatment than control under the declared descriptive contrast".into(),
                )
            } else {
                (
                    TransitionContrastDisposition::ReducedInTreatment,
                    "equal-unit transition probability is lower in treatment than control under the declared descriptive contrast".into(),
                )
            };
            contrasts.push(StateTransitionContrast {
                contrast_id,
                from_state: from.clone(),
                to_state: to.clone(),
                direction: direction(state_index[from.as_str()], state_index[to.as_str()]),
                control_probability_milli: control.unit_balanced_probability_milli,
                treatment_probability_milli: treatment.unit_balanced_probability_milli,
                difference_milli: difference,
                absolute_difference_milli: absolute_difference,
                control_pooled_probability_milli: control.probability_milli,
                treatment_pooled_probability_milli: treatment.probability_milli,
                pooled_difference_milli: pooled_difference,
                pooled_absolute_difference_milli: pooled_absolute_difference,
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
    let batch_sensitivity = batch_sensitivity(request, &transition_events, &contrasts);
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
    if batch_sensitivity.iter().any(|sensitivity| {
        sensitivity.disposition == StateTransitionBatchSensitivityDisposition::Fragile
    }) {
        uncertainty.push(
            "one or more state-transition rows are sensitive to omission of an assay batch".into(),
        );
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
        unit_profiles,
        cells,
        cell_order: sorted_cell_order,
        contrasts,
        batch_sensitivity,
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
    fn zero_contrast_is_null_even_when_the_declared_gate_is_zero() {
        assert_eq!(transition_sign(0, 0), 0);
        assert_eq!(transition_sign(1, 0), 1);
        assert_eq!(transition_sign(-1, 0), -1);
    }

    #[test]
    fn leave_one_batch_out_finds_a_reversed_transition_signal_and_is_order_invariant() {
        let mut data = Vec::new();
        // Two small batches suggest reduced switching, while a larger third batch suggests
        // increased switching. The pooled unit-balanced effect is positive, but removing the
        // third batch reverses its direction. Each batch remains above the unit floor.
        for (batch, units, control_switches, treatment_switches) in [
            ("batch-a", 2, true, false),
            ("batch-b", 2, true, false),
            ("batch-c", 10, false, true),
        ] {
            for arm in ["control", "treated"] {
                let switches = if arm == "control" {
                    control_switches
                } else {
                    treatment_switches
                };
                for index in 0..units {
                    let unit_id = format!("{batch}-{arm}-{index}");
                    let to_state = if switches { "high" } else { "low" };
                    for (timepoint, state_id) in [(0, "low"), (1, to_state)] {
                        let observation_id = format!("{unit_id}-{timepoint}");
                        data.push(StateTransitionObservation {
                            observation_id: observation_id.clone(),
                            unit_id: unit_id.clone(),
                            arm_id: arm.into(),
                            model_system: GliomaModelSystem::Organoid,
                            batch_id: batch.into(),
                            timepoint,
                            state_id: state_id.into(),
                            state_score_milli: if state_id == "low" { 100 } else { 900 },
                            artifact: artifact(&observation_id),
                        });
                    }
                }
            }
        }

        let mut analysis_request = request();
        analysis_request.min_transitions_per_arm = 1;
        let output = analyze_glioma_state_transitions(&analysis_request, &data).unwrap();
        let sensitivity = output
            .batch_sensitivity
            .iter()
            .find(|item| item.from_state == "low")
            .unwrap();
        assert_eq!(sensitivity.batch_count, 3);
        assert_eq!(sensitivity.evaluated_batch_count, 3);
        assert_eq!(
            sensitivity.disposition,
            StateTransitionBatchSensitivityDisposition::Fragile
        );
        assert!(sensitivity.sign_reversal_count > 0);
        assert!(sensitivity
            .unstable_contrast_order
            .contains(&"low:high".into()));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("assay batch")));

        data.reverse();
        let reordered = analyze_glioma_state_transitions(&analysis_request, &data).unwrap();
        assert_eq!(output.batch_sensitivity, reordered.batch_sensitivity);
        assert_eq!(output.digest, reordered.digest);
    }

    #[test]
    fn source_unit_support_is_counted_per_arm_and_source_state() {
        let mut data = observations();
        let mut repeated = data
            .iter()
            .find(|item| {
                item.arm_id == "control" && item.unit_id == "control-0" && item.timepoint == 1
            })
            .unwrap()
            .clone();
        repeated.observation_id = "control-0-2".into();
        repeated.timepoint = 2;
        repeated.state_id = "high".into();
        repeated.state_score_milli = 900;
        repeated.artifact = artifact("control-0-2");
        data.push(repeated);
        let mut high_followup = data
            .iter()
            .find(|item| {
                item.arm_id == "control" && item.unit_id == "control-0" && item.timepoint == 2
            })
            .unwrap()
            .clone();
        high_followup.observation_id = "control-0-3".into();
        high_followup.timepoint = 3;
        high_followup.artifact = artifact("control-0-3");
        data.push(high_followup);

        let output = analyze_glioma_state_transitions(&request(), &data).unwrap();
        let low_row = output
            .cells
            .iter()
            .find(|cell| cell.arm_id == "control" && cell.from_state == "low")
            .unwrap();
        let high_row = output
            .cells
            .iter()
            .find(|cell| cell.arm_id == "control" && cell.from_state == "high")
            .unwrap();
        assert_eq!(low_row.source_unit_count, 2);
        assert_eq!(high_row.source_unit_count, 1);
        assert!(output
            .cells
            .iter()
            .filter(|cell| cell.arm_id == "control" && cell.from_state == "high")
            .all(|cell| cell.source_unit_count == 1));
        output.validate().unwrap();
    }

    #[test]
    fn unit_balanced_rows_do_not_let_one_long_trajectory_dominate() {
        let mut data = Vec::new();
        for (arm, unit_id, states) in [
            ("control", "control-long", vec!["low", "low", "low", "low"]),
            ("control", "control-switch", vec!["low", "high"]),
            ("treated", "treated-stable", vec!["low", "low"]),
            ("treated", "treated-switch", vec!["low", "high"]),
        ] {
            for (timepoint, state_id) in states.into_iter().enumerate() {
                let observation_id = format!("{unit_id}-{timepoint}");
                data.push(StateTransitionObservation {
                    observation_id: observation_id.clone(),
                    unit_id: unit_id.into(),
                    arm_id: arm.into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: format!("{unit_id}-batch"),
                    timepoint: timepoint as u32,
                    state_id: state_id.into(),
                    state_score_milli: if state_id == "low" { 100 } else { 900 },
                    artifact: artifact(&observation_id),
                });
            }
        }

        let output = analyze_glioma_state_transitions(&request(), &data).unwrap();
        let control_stable = output
            .cells
            .iter()
            .find(|cell| {
                cell.arm_id == "control" && cell.from_state == "low" && cell.to_state == "low"
            })
            .unwrap();
        let control_switch = output
            .cells
            .iter()
            .find(|cell| {
                cell.arm_id == "control" && cell.from_state == "low" && cell.to_state == "high"
            })
            .unwrap();
        assert_eq!(control_stable.probability_milli, 750);
        assert_eq!(control_switch.probability_milli, 250);
        assert_eq!(control_stable.unit_balanced_probability_milli, 500);
        assert_eq!(control_switch.unit_balanced_probability_milli, 500);
        assert_eq!(control_stable.source_unit_count, 2);
        assert_eq!(control_stable.source_transition_count, 4);
        let stable_contrast = output
            .contrasts
            .iter()
            .find(|contrast| contrast.contrast_id == "low:low")
            .unwrap();
        assert_eq!(stable_contrast.control_probability_milli, 500);
        assert_eq!(stable_contrast.treatment_probability_milli, 500);
        assert_eq!(stable_contrast.difference_milli, 0);
        assert_eq!(
            stable_contrast.disposition,
            TransitionContrastDisposition::Null
        );
        assert_eq!(stable_contrast.control_pooled_probability_milli, 750);
        assert_eq!(stable_contrast.pooled_difference_milli, -250);
        assert_eq!(
            output
                .unit_profiles
                .iter()
                .filter(|profile| profile.arm_id == "control" && profile.from_state == "low")
                .count(),
            2
        );
        output.validate().unwrap();
    }

    #[test]
    fn repeated_windows_from_one_unit_do_not_satisfy_the_source_state_unit_floor() {
        let mut data = Vec::new();
        for (arm, states) in [
            ("control", ["low", "low", "high"]),
            ("treated", ["low", "high", "high"]),
        ] {
            for (unit_id, sequence) in [
                (format!("{arm}-active"), states),
                (format!("{arm}-other"), ["high", "high", "high"]),
            ] {
                for (timepoint, state_id) in sequence
                    .into_iter()
                    .enumerate()
                    .take(if unit_id.ends_with("active") { 3 } else { 1 })
                {
                    let observation_id = format!("{unit_id}-{timepoint}");
                    data.push(StateTransitionObservation {
                        observation_id: observation_id.clone(),
                        unit_id: unit_id.clone(),
                        arm_id: arm.into(),
                        model_system: GliomaModelSystem::Organoid,
                        batch_id: format!("{unit_id}-batch"),
                        timepoint: timepoint as u32,
                        state_id: state_id.into(),
                        state_score_milli: if state_id == "low" { 100 } else { 900 },
                        artifact: artifact(&observation_id),
                    });
                }
            }
        }
        let mut request = request();
        request.min_transitions_per_arm = 1;
        let output = analyze_glioma_state_transitions(&request, &data).unwrap();
        let low_to_high = output
            .contrasts
            .iter()
            .find(|contrast| contrast.contrast_id == "low:high")
            .unwrap();
        assert_eq!(low_to_high.control_source_transition_count, 2);
        assert_eq!(low_to_high.treatment_source_transition_count, 1);
        let control_low_to_high = output
            .cells
            .iter()
            .find(|cell| {
                cell.arm_id == "control" && cell.from_state == "low" && cell.to_state == "high"
            })
            .unwrap();
        assert_eq!(control_low_to_high.source_transition_count, 2);
        assert_eq!(control_low_to_high.source_unit_count, 1);
        assert_eq!(
            low_to_high.disposition,
            TransitionContrastDisposition::Unresolved
        );
        assert_eq!(output.control_unit_count, 2);
        assert_eq!(output.treatment_unit_count, 2);
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
