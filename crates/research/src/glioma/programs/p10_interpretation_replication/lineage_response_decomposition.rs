//! Decompose preclinical glioma lineage-propagation contrasts into net-yield and state-composition components.
//!
//! The analysis standardizes both arms to one investigator-declared pretreatment state mixture,
//! then applies an exact symmetric decomposition to paired P10-F02 bootstrap operators. It is a
//! descriptive assay-planning result: net yield combines proliferation and death, while state
//! composition is not an individual-cell switching probability. Implements `GAF-GLIOMA-P10-F04`.

use super::lineage_propagation::{
    EffectDisposition, LineagePropagationAnalysis, LineagePropagationDisposition, PpmInterval,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaLineageResponseDecomposition1@1";
const SCALE: i128 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageResponseDecompositionRequest {
    pub baseline_source_composition_ppm: Vec<u32>,
    /// Minimum absolute component effect considered practically material, in ppm descendants
    /// per million baseline cells.
    pub minimum_component_ppm: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageResponseDecompositionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageResponseFollowUpFocus {
    ValidateNetYield,
    ValidateStateComposition,
    ValidateBothComponents,
    IncreaseIndependentReplication,
    ResolveLineageIdentifiability,
    NoTargetedFollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageResponsePairDecomposition {
    pub from_state: String,
    pub to_state: String,
    pub baseline_share_ppm: u32,
    pub control_yield_ppm: u64,
    pub treatment_yield_ppm: u64,
    pub control_destination_share_ppm: Option<u32>,
    pub treatment_destination_share_ppm: Option<u32>,
    /// Standardized treatment-minus-control descendants per million baseline cells.
    pub total_effect_ppm: Option<i64>,
    /// Symmetric Kitagawa-style contribution attributable to changes in total yield.
    pub net_yield_component_ppm: Option<i64>,
    /// Symmetric Kitagawa-style contribution attributable to destination-state composition.
    pub state_composition_component_ppm: Option<i64>,
    /// Total effect minus the two rounded components; bounded fixed-point residue is retained.
    pub reconstruction_residual_ppm: Option<i64>,
    pub net_yield_interval_ppm: Option<PpmInterval>,
    pub state_composition_interval_ppm: Option<PpmInterval>,
    pub valid_bootstrap_draws: u16,
    pub net_yield_disposition: EffectDisposition,
    pub state_composition_disposition: EffectDisposition,
    pub follow_up_focus: LineageResponseFollowUpFocus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageResponseDecomposition {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: String,
    pub state_order: Vec<String>,
    pub control_arm: String,
    pub treatment_arm: String,
    pub interval_days: u32,
    pub baseline_source_composition_ppm: Vec<u32>,
    pub minimum_component_ppm: u64,
    pub confidence_level_milli: u16,
    pub input_disposition: LineagePropagationDisposition,
    pub decomposition_order: Vec<(String, String)>,
    pub decompositions: Vec<LineageResponsePairDecomposition>,
    pub disposition: LineageResponseDecompositionDisposition,
    pub limitations: Vec<String>,
    pub input_digest: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineageResponseDecompositionError {
    #[error("lineage response decomposition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage response decomposition input analysis is invalid: {0}")]
    InvalidAnalysis(String),
    #[error("lineage response decomposition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage response decomposition digest failed: {0}")]
    Digest(String),
}

fn rounded_ratio(numerator: i128, denominator: i128) -> i64 {
    debug_assert!(denominator > 0);
    let magnitude = numerator.unsigned_abs();
    let denominator = denominator as u128;
    let rounded = (magnitude + denominator / 2) / denominator;
    let signed = i128::try_from(rounded).unwrap_or(i128::MAX);
    if numerator < 0 {
        (-signed).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    } else {
        signed.clamp(0, i128::from(i64::MAX)) as i64
    }
}

fn matrix_yield(matrix: &[u64], states: usize, from: usize) -> u64 {
    (0..states).map(|to| matrix[to * states + from]).sum()
}

fn component_values(
    control: &[u64],
    treatment: &[u64],
    states: usize,
    from: usize,
    to: usize,
    baseline_share_ppm: u32,
) -> Option<(i64, i64, i64, i64, u64, u64)> {
    let control_yield = matrix_yield(control, states, from);
    let treatment_yield = matrix_yield(treatment, states, from);
    let control_value = control[to * states + from];
    let treatment_value = treatment[to * states + from];
    if baseline_share_ppm == 0 {
        return Some((0, 0, 0, 0, control_yield, treatment_yield));
    }
    if control_yield == 0 || treatment_yield == 0 {
        return None;
    }

    // Exact-rational symmetric decomposition:
    // delta(y*q) = delta(y)*(q_t + q_c)/2 + (y_t + y_c)*delta(q)/2.
    // Coefficients and yields are ppm; the shared 1e6 scales are canceled explicitly here.
    let b = i128::from(baseline_share_ppm);
    let yc = i128::from(control_yield);
    let yt = i128::from(treatment_yield);
    let cc = i128::from(control_value);
    let ct = i128::from(treatment_value);
    let delta_y = yt - yc;
    let total = rounded_ratio(b * (ct - cc), SCALE);
    let denominator = 2 * SCALE * yc * yt;
    let net_yield = rounded_ratio(b * delta_y * (ct * yc + cc * yt), denominator);
    let state_composition = rounded_ratio(b * (yt + yc) * (ct * yc - cc * yt), denominator);
    let residual = total - net_yield - state_composition;
    Some((
        total,
        net_yield,
        state_composition,
        residual,
        control_yield,
        treatment_yield,
    ))
}

fn percentile_interval(mut values: Vec<i64>, confidence_level_milli: u16) -> Option<PpmInterval> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let span = values.len() - 1;
    let tail_milli = (1_000 - u64::from(confidence_level_milli)) / 2;
    let lower = (span as u128 * u128::from(tail_milli) / 1_000) as usize;
    let upper = (span as u128 * u128::from(1_000 - tail_milli) / 1_000) as usize;
    Some(PpmInterval {
        lower: values[lower],
        upper: values[upper],
    })
}

fn component_disposition(interval: Option<PpmInterval>, margin_ppm: u64) -> EffectDisposition {
    let Some(interval) = interval else {
        return EffectDisposition::Unresolved;
    };
    let margin = margin_ppm.min(i64::MAX as u64) as i64;
    if interval.lower > margin || interval.upper < -margin {
        EffectDisposition::BeyondPracticalMargin
    } else if interval.lower >= -margin && interval.upper <= margin {
        EffectDisposition::WithinPracticalMargin
    } else {
        EffectDisposition::Inconclusive
    }
}

fn follow_up_focus(
    net_yield: EffectDisposition,
    composition: EffectDisposition,
) -> LineageResponseFollowUpFocus {
    use EffectDisposition::{BeyondPracticalMargin, WithinPracticalMargin};
    match (net_yield, composition) {
        (BeyondPracticalMargin, BeyondPracticalMargin) => {
            LineageResponseFollowUpFocus::ValidateBothComponents
        }
        (BeyondPracticalMargin, WithinPracticalMargin) => {
            LineageResponseFollowUpFocus::ValidateNetYield
        }
        (WithinPracticalMargin, BeyondPracticalMargin) => {
            LineageResponseFollowUpFocus::ValidateStateComposition
        }
        (WithinPracticalMargin, WithinPracticalMargin) => {
            LineageResponseFollowUpFocus::NoTargetedFollowUp
        }
        (EffectDisposition::Unresolved, _) | (_, EffectDisposition::Unresolved) => {
            LineageResponseFollowUpFocus::ResolveLineageIdentifiability
        }
        _ => LineageResponseFollowUpFocus::IncreaseIndependentReplication,
    }
}

fn digest_input(output: &LineageResponseDecomposition) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "state_order": output.state_order,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "interval_days": output.interval_days,
        "baseline_source_composition_ppm": output.baseline_source_composition_ppm,
        "minimum_component_ppm": output.minimum_component_ppm,
        "confidence_level_milli": output.confidence_level_milli,
        "input_disposition": output.input_disposition,
        "decomposition_order": output.decomposition_order,
        "decompositions": output.decompositions,
        "disposition": output.disposition,
        "limitations": output.limitations,
        "input_digest": output.input_digest,
    })
}

fn coefficient_matrix(analysis: &LineagePropagationAnalysis, treatment: bool) -> Vec<u64> {
    let states = analysis.state_order.len();
    let operator = if treatment {
        &analysis.treatment
    } else {
        &analysis.control
    };
    let mut matrix = vec![0_u64; states * states];
    for coefficient in &operator.coefficients {
        let Some(from) = analysis
            .state_order
            .iter()
            .position(|state| state == &coefficient.from_state)
        else {
            continue;
        };
        let Some(to) = analysis
            .state_order
            .iter()
            .position(|state| state == &coefficient.to_state)
        else {
            continue;
        };
        matrix[to * states + from] = coefficient.coefficient_ppm;
    }
    matrix
}

/// Standardize both arm operators to the same pretreatment state mixture and decompose the
/// predicted lineage-propagation contrast into net descendant yield and destination composition.
pub fn analyze_glioma_lineage_response_decomposition(
    request: &LineageResponseDecompositionRequest,
    analysis: &LineagePropagationAnalysis,
) -> Result<LineageResponseDecomposition, LineageResponseDecompositionError> {
    analysis
        .validate()
        .map_err(|error| LineageResponseDecompositionError::InvalidAnalysis(error.to_string()))?;
    let states = analysis.state_order.len();
    if request.baseline_source_composition_ppm.len() != states
        || request
            .baseline_source_composition_ppm
            .iter()
            .map(|share| u64::from(*share))
            .sum::<u64>()
            != SCALE as u64
    {
        return Err(LineageResponseDecompositionError::InvalidRequest(
            "baseline source composition must match the declared states and sum exactly to 1,000,000 ppm".into(),
        ));
    }

    let control = coefficient_matrix(analysis, false);
    let treatment = coefficient_matrix(analysis, true);
    let model_estimable = usize::from(analysis.control.source_design_rank) == states
        && usize::from(analysis.treatment.source_design_rank) == states
        && !matches!(
            analysis.disposition,
            LineagePropagationDisposition::PredictionFailure
                | LineagePropagationDisposition::Unresolved
        );
    let mut decomposition_order = Vec::with_capacity(states * states);
    let mut decompositions = Vec::with_capacity(states * states);

    for (from, from_state) in analysis.state_order.iter().enumerate() {
        for (to, to_state) in analysis.state_order.iter().enumerate() {
            let baseline_share = request.baseline_source_composition_ppm[from];
            decomposition_order.push((from_state.clone(), to_state.clone()));
            let point = if model_estimable {
                component_values(&control, &treatment, states, from, to, baseline_share)
            } else {
                None
            };
            let (total, net_yield, state_composition, residual, control_yield, treatment_yield) =
                point.unwrap_or((
                    0,
                    0,
                    0,
                    0,
                    matrix_yield(&control, states, from),
                    matrix_yield(&treatment, states, from),
                ));

            let mut net_yield_draws = Vec::with_capacity(analysis.bootstrap_draws.len());
            let mut composition_draws = Vec::with_capacity(analysis.bootstrap_draws.len());
            for draw in &analysis.bootstrap_draws {
                let Some((_, net, composition, _, _, _)) = component_values(
                    &draw.control_coefficients_ppm,
                    &draw.treatment_coefficients_ppm,
                    states,
                    from,
                    to,
                    baseline_share,
                ) else {
                    continue;
                };
                net_yield_draws.push(net);
                composition_draws.push(composition);
            }
            let valid_draw_count = net_yield_draws.len();
            let complete_bootstrap = model_estimable
                && point.is_some()
                && valid_draw_count == analysis.bootstrap_draws.len()
                && !analysis.bootstrap_draws.is_empty();
            let zero_interval = Some(PpmInterval { lower: 0, upper: 0 });
            let net_interval = if baseline_share == 0 {
                zero_interval
            } else if complete_bootstrap {
                percentile_interval(net_yield_draws, analysis.confidence_level_milli)
            } else {
                None
            };
            let composition_interval = if baseline_share == 0 {
                zero_interval
            } else if complete_bootstrap {
                percentile_interval(composition_draws, analysis.confidence_level_milli)
            } else {
                None
            };
            let valid_draws = valid_draw_count.min(u16::MAX as usize) as u16;
            let (net_disposition, composition_disposition) =
                if model_estimable && (baseline_share == 0 || point.is_some()) {
                    (
                        component_disposition(net_interval, request.minimum_component_ppm),
                        component_disposition(composition_interval, request.minimum_component_ppm),
                    )
                } else {
                    (EffectDisposition::Unresolved, EffectDisposition::Unresolved)
                };
            let follow_up = follow_up_focus(net_disposition, composition_disposition);
            decompositions.push(LineageResponsePairDecomposition {
                from_state: from_state.clone(),
                to_state: to_state.clone(),
                baseline_share_ppm: baseline_share,
                control_yield_ppm: control_yield,
                treatment_yield_ppm: treatment_yield,
                control_destination_share_ppm: (control_yield > 0).then(|| {
                    ((u128::from(control[to * states + from]) * SCALE as u128)
                        / u128::from(control_yield)) as u32
                }),
                treatment_destination_share_ppm: (treatment_yield > 0).then(|| {
                    ((u128::from(treatment[to * states + from]) * SCALE as u128)
                        / u128::from(treatment_yield)) as u32
                }),
                total_effect_ppm: (model_estimable && point.is_some()).then_some(total),
                net_yield_component_ppm: (model_estimable && point.is_some()).then_some(net_yield),
                state_composition_component_ppm: (model_estimable && point.is_some())
                    .then_some(state_composition),
                reconstruction_residual_ppm: (model_estimable && point.is_some())
                    .then_some(residual),
                net_yield_interval_ppm: net_interval,
                state_composition_interval_ppm: composition_interval,
                valid_bootstrap_draws: valid_draws,
                net_yield_disposition: net_disposition,
                state_composition_disposition: composition_disposition,
                follow_up_focus: follow_up,
            });
        }
    }

    let any_unresolved = !model_estimable
        || decompositions.iter().any(|row| {
            row.baseline_share_ppm > 0
                && (row.net_yield_disposition == EffectDisposition::Unresolved
                    || row.state_composition_disposition == EffectDisposition::Unresolved)
        });
    let partial = analysis.disposition == LineagePropagationDisposition::PartiallyIdentified
        || decompositions.iter().any(|row| {
            row.baseline_share_ppm > 0
                && usize::from(row.valid_bootstrap_draws) != analysis.bootstrap_draws.len()
        });
    let disposition = if any_unresolved {
        LineageResponseDecompositionDisposition::Unresolved
    } else if partial {
        LineageResponseDecompositionDisposition::Partial
    } else {
        LineageResponseDecompositionDisposition::Qualified
    };
    let input_digest = ContentHash::of_value(&serde_json::json!({
        "analysis_digest": analysis.digest,
        "request": request,
    }))
    .map_err(|error| LineageResponseDecompositionError::Digest(error.to_string()))?;
    let mut output = LineageResponseDecomposition {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: analysis.objective.clone(),
        model_system: format!("{:?}", analysis.model_system).to_lowercase(),
        state_order: analysis.state_order.clone(),
        control_arm: analysis.control_arm.clone(),
        treatment_arm: analysis.treatment_arm.clone(),
        interval_days: analysis.interval_days,
        baseline_source_composition_ppm: request.baseline_source_composition_ppm.clone(),
        minimum_component_ppm: request.minimum_component_ppm,
        confidence_level_milli: analysis.confidence_level_milli,
        input_disposition: analysis.disposition,
        decomposition_order,
        decompositions,
        disposition,
        limitations: vec![
            "net descendant yield combines proliferation, death, capture, and lineage-assignment effects; it is not a pure selection or viability estimate".into(),
            "destination-state composition is an aggregate lineage-level composition and is not a direct individual-cell switching probability".into(),
            "the decomposition is standardized to the investigator-supplied baseline state mixture; another mixture can produce different marginal effects".into(),
            "component intervals reuse the paired experimental-unit bootstrap operators from P10-F02; they do not add uncertainty in baseline-mixture measurement".into(),
            "if any bootstrap draw has a zero-yield source state with positive baseline share, component intervals are withheld rather than conditioning on estimable draws".into(),
        ],
        input_digest,
        digest: ContentHash::of_bytes(b"unsealed-glioma-lineage-response-decomposition"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LineageResponseDecompositionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl LineageResponseDecomposition {
    pub fn validate(&self) -> Result<(), LineageResponseDecompositionError> {
        let states = self.state_order.len();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !(2..=8).contains(&states)
            || self.objective.trim().is_empty()
            || self.model_system.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || self.interval_days == 0
            || self.baseline_source_composition_ppm.len() != states
            || self
                .baseline_source_composition_ppm
                .iter()
                .map(|share| u64::from(*share))
                .sum::<u64>()
                != SCALE as u64
            || self.confidence_level_milli < 500
            || self.confidence_level_milli >= 1_000
            || self.decomposition_order.len() != states * states
            || self.decompositions.len() != states * states
            || self
                .decompositions
                .iter()
                .zip(&self.decomposition_order)
                .any(|(row, order)| {
                    let baseline_matches = self
                        .state_order
                        .iter()
                        .position(|state| state == &row.from_state)
                        .is_some_and(|index| {
                            row.baseline_share_ppm == self.baseline_source_composition_ppm[index]
                        });
                    row.from_state != order.0
                        || row.to_state != order.1
                        || !baseline_matches
                        || row.control_yield_ppm > 8 * 50_000_000
                        || row.treatment_yield_ppm > 8 * 50_000_000
                        || row.valid_bootstrap_draws > 499
                        || row
                            .net_yield_interval_ppm
                            .is_some_and(|interval| interval.lower > interval.upper)
                        || row
                            .state_composition_interval_ppm
                            .is_some_and(|interval| interval.lower > interval.upper)
                        || match (
                            row.total_effect_ppm,
                            row.net_yield_component_ppm,
                            row.state_composition_component_ppm,
                            row.reconstruction_residual_ppm,
                        ) {
                            (
                                Some(total),
                                Some(yield_component),
                                Some(composition),
                                Some(residual),
                            ) => {
                                total - yield_component - composition != residual
                                    || residual.abs() > 2
                            }
                            (None, None, None, None) => false,
                            _ => true,
                        }
                })
            || self.limitations.len() != 5
            || self.limitations.iter().any(|item| item.trim().is_empty())
        {
            return Err(LineageResponseDecompositionError::InvalidOutput(
                "feature/schema, baseline mixture, pair ordering, component identity, or limitation invariants are invalid".into(),
            ));
        }
        if ContentHash::of_value(&digest_input(self))
            .map_err(|error| LineageResponseDecompositionError::Digest(error.to_string()))?
            != self.digest
        {
            return Err(LineageResponseDecompositionError::InvalidOutput(
                "lineage response decomposition digest does not match output".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
        analyze_glioma_lineage_propagation, LineagePropagationRequest, LineagePropagationSnapshot,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

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

    fn analysis(treatment_matrix: [[u32; 2]; 2]) -> LineagePropagationAnalysis {
        let request = LineagePropagationRequest {
            objective: "measure preclinical glioma state response".into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["npc_like".into(), "mes_like".into()],
            min_units_per_arm: 3,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 0,
            max_coefficient_ppm: 20_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"decomposition-test-seed"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        };
        let mut snapshots = Vec::new();
        for arm in ["control", "perturbation"] {
            for unit in 0..3 {
                let matrix = if arm == "control" {
                    [[8, 2], [1, 7]]
                } else {
                    treatment_matrix
                };
                for source in 0..2 {
                    let lineage = format!("{arm}-u{unit}-l{source}");
                    let mut values = vec![0_u32; 2];
                    values[source] = 100;
                    for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                        snapshots.push(LineagePropagationSnapshot {
                            observation_id: format!("{lineage}-t{day}"),
                            experimental_unit_id: format!("{arm}-u{unit}"),
                            lineage_id: lineage.clone(),
                            arm_id: arm.into(),
                            model_system: GliomaModelSystem::Organoid,
                            assay_batch_id: format!("batch-{unit}"),
                            timepoint_day: day,
                            state_counts: values.clone(),
                            capture_fraction_ppm: 1_000_000,
                            artifact: artifact(&format!("{lineage}-{day}")),
                        });
                        if step < 2 {
                            values = (0..2)
                                .map(|to| {
                                    (0..2)
                                        .map(|from| matrix[to][from] * values[from])
                                        .sum::<u32>()
                                        / 10
                                })
                                .collect();
                        }
                    }
                }
            }
        }
        analyze_glioma_lineage_propagation(&request, &snapshots).unwrap()
    }

    fn decomposition(analysis: &LineagePropagationAnalysis) -> LineageResponseDecomposition {
        analyze_glioma_lineage_response_decomposition(
            &LineageResponseDecompositionRequest {
                baseline_source_composition_ppm: vec![1_000_000, 0],
                minimum_component_ppm: 10_000,
            },
            analysis,
        )
        .unwrap()
    }

    #[test]
    fn isolates_state_composition_shift_when_source_specific_yield_is_preserved() {
        let output = decomposition(&analysis([[6, 2], [3, 7]]));
        let npc_to_npc = output
            .decompositions
            .iter()
            .find(|row| row.from_state == "npc_like" && row.to_state == "npc_like")
            .unwrap();
        let npc_to_mes = output
            .decompositions
            .iter()
            .find(|row| row.from_state == "npc_like" && row.to_state == "mes_like")
            .unwrap();
        assert_eq!(npc_to_npc.control_yield_ppm, npc_to_npc.treatment_yield_ppm);
        assert_eq!(npc_to_npc.net_yield_component_ppm, Some(0));
        assert_eq!(npc_to_mes.net_yield_component_ppm, Some(0));
        assert!(npc_to_npc.state_composition_component_ppm.unwrap() < 0);
        assert!(npc_to_mes.state_composition_component_ppm.unwrap() > 0);
        assert!(output.decompositions.iter().all(|row| {
            row.reconstruction_residual_ppm
                .is_none_or(|residual| residual.abs() <= 2)
        }));
        output.validate().unwrap();
    }

    #[test]
    fn isolates_net_yield_change_when_destination_composition_is_preserved() {
        let output = decomposition(&analysis([[16, 4], [2, 14]]));
        let npc_to_npc = output
            .decompositions
            .iter()
            .find(|row| row.from_state == "npc_like" && row.to_state == "npc_like")
            .unwrap();
        assert_eq!(
            npc_to_npc.control_destination_share_ppm,
            npc_to_npc.treatment_destination_share_ppm
        );
        assert_eq!(npc_to_npc.state_composition_component_ppm, Some(0));
        assert!(npc_to_npc.net_yield_component_ppm.unwrap() > 0);
        assert_eq!(
            npc_to_npc.follow_up_focus,
            LineageResponseFollowUpFocus::ValidateNetYield
        );
        assert_eq!(
            output.disposition,
            LineageResponseDecompositionDisposition::Qualified
        );
    }

    #[test]
    fn refuses_invalid_mixtures_and_unresolved_input_and_detects_zero_yield_draws() {
        let analysis = analysis([[6, 4], [3, 5]]);
        let invalid = LineageResponseDecompositionRequest {
            baseline_source_composition_ppm: vec![800_000, 100_000],
            minimum_component_ppm: 10_000,
        };
        assert!(matches!(
            analyze_glioma_lineage_response_decomposition(&invalid, &analysis),
            Err(LineageResponseDecompositionError::InvalidRequest(_))
        ));

        let zero_yield = vec![0_u64; 4];
        let observed = vec![300_000_u64, 0, 200_000, 400_000];
        assert!(component_values(&observed, &zero_yield, 2, 0, 0, 1_000_000).is_none());
        assert_eq!(
            component_values(&observed, &zero_yield, 2, 0, 0, 0),
            Some((0, 0, 0, 0, 500_000, 0))
        );

        let mut analysis = analysis;
        analysis.bootstrap_draws[0].treatment_coefficients_ppm[0..2].copy_from_slice(&[0, 0]);
        analysis.digest = ContentHash::of_bytes(b"deliberately-invalid-analysis");
        let error = analyze_glioma_lineage_response_decomposition(
            &LineageResponseDecompositionRequest {
                baseline_source_composition_ppm: vec![1_000_000, 0],
                minimum_component_ppm: 10_000,
            },
            &analysis,
        );
        assert!(matches!(
            error,
            Err(LineageResponseDecompositionError::InvalidAnalysis(_))
        ));
    }
}
