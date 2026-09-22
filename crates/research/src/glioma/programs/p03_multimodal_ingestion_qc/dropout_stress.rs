//! Modality-dropout stress analysis for preclinical glioma interpretation.
//!
//! A multimodal result is only useful if the conclusion survives realistic missingness and QC
//! failure. This feature computes a deterministic quality-weighted endpoint score under declared
//! dropout scenarios, never imputes a missing modality, and routes unstable or contradictory
//! scenarios back to acquisition/QC rather than silently promoting a fragile conclusion.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalDropoutStress1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_SCENARIOS: usize = 512;
pub const MAX_ENDPOINT_ABS_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropoutModalitySignal {
    pub modality: GliomaModality,
    pub signal_milli: i64,
    pub quality_milli: u16,
    pub feature_count: u32,
    pub required_for_endpoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropoutScenario {
    pub scenario_id: String,
    pub missing_modality_order: Vec<GliomaModality>,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropoutStressRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub signals: Vec<DropoutModalitySignal>,
    pub scenarios: Vec<DropoutScenario>,
    pub min_quality_milli: u16,
    pub max_allowed_shift_milli: u64,
    pub min_stability_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropoutScenarioDisposition {
    Stable,
    Degraded,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropoutScenarioResult {
    pub scenario_id: String,
    pub missing_modality_order: Vec<GliomaModality>,
    pub available_modality_order: Vec<GliomaModality>,
    pub aggregate_signal_milli: Option<i64>,
    pub delta_from_baseline_milli: Option<i64>,
    pub stability_milli: u16,
    pub disposition: DropoutScenarioDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropoutStressDisposition {
    Stable,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropoutStressAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub baseline_signal_milli: Option<i64>,
    pub scenario_order: Vec<String>,
    pub scenarios: Vec<DropoutScenarioResult>,
    pub stable_scenario_order: Vec<String>,
    pub degraded_scenario_order: Vec<String>,
    pub contradictory_scenario_order: Vec<String>,
    pub unresolved_scenario_order: Vec<String>,
    pub worst_case_stability_milli: u16,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: DropoutStressDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DropoutStressError {
    #[error("dropout stress request is invalid: {0}")]
    InvalidRequest(String),
    #[error("dropout stress input is invalid: {0}")]
    InvalidInput(String),
    #[error("dropout stress output is invalid: {0}")]
    InvalidOutput(String),
    #[error("dropout stress digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DropoutStressAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "baseline_signal_milli": output.baseline_signal_milli,
        "scenario_order": output.scenario_order,
        "scenarios": output.scenarios,
        "stable_scenario_order": output.stable_scenario_order,
        "degraded_scenario_order": output.degraded_scenario_order,
        "contradictory_scenario_order": output.contradictory_scenario_order,
        "unresolved_scenario_order": output.unresolved_scenario_order,
        "worst_case_stability_milli": output.worst_case_stability_milli,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &DropoutStressRequest) -> Result<(), DropoutStressError> {
    if request.objective.trim().is_empty()
        || request.endpoint_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.signals.is_empty()
        || request.signals.len() > MAX_MODALITIES
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || request.min_quality_milli > 1_000
        || request.max_allowed_shift_milli > MAX_ENDPOINT_ABS_MILLI
        || request.min_stability_milli > 1_000
    {
        return Err(DropoutStressError::InvalidRequest(
            "objective, endpoint/study identity, bounded signals/scenarios, quality, shift, and stability gates are required".into(),
        ));
    }
    let mut modalities = BTreeSet::new();
    for signal in &request.signals {
        if signal.quality_milli > 1_000
            || signal.feature_count == 0
            || signal.signal_milli.unsigned_abs() > MAX_ENDPOINT_ABS_MILLI
            || !modalities.insert(signal.modality)
        {
            return Err(DropoutStressError::InvalidInput(
                "signals require unique modalities, bounded values, positive feature counts, and finite quality".into(),
            ));
        }
    }
    let mut scenario_ids = BTreeSet::new();
    let mut weight_sum = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || scenario.weight_milli == 0
            || scenario.weight_milli > 1_000
            || !scenario_ids.insert(scenario.scenario_id.clone())
            || !canonical(&scenario.missing_modality_order)
            || scenario
                .missing_modality_order
                .iter()
                .any(|modality| !modalities.contains(modality))
        {
            return Err(DropoutStressError::InvalidInput(
                "scenario identity, weights, missing-modality ordering, and modality references are invalid".into(),
            ));
        }
        weight_sum = weight_sum.saturating_add(u32::from(scenario.weight_milli));
    }
    if weight_sum != 1_000 {
        return Err(DropoutStressError::InvalidInput(
            "dropout scenario weights must sum to exactly 1000 milli-units".into(),
        ));
    }
    Ok(())
}

fn aggregate_signal(signals: &[&DropoutModalitySignal], min_quality_milli: u16) -> Option<i64> {
    let eligible = signals
        .iter()
        .filter(|signal| signal.quality_milli >= min_quality_milli)
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return None;
    }
    let denominator = eligible
        .iter()
        .map(|signal| u64::from(signal.quality_milli) * u64::from(signal.feature_count))
        .sum::<u64>();
    if denominator == 0 {
        return None;
    }
    let numerator = eligible
        .iter()
        .map(|signal| {
            i128::from(signal.signal_milli)
                * i128::from(signal.quality_milli)
                * i128::from(signal.feature_count)
        })
        .sum::<i128>();
    Some(
        (numerator / i128::from(denominator)).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
            as i64,
    )
}

fn stability(baseline: Option<i64>, scenario: Option<i64>, max_shift: u64) -> u16 {
    let (Some(base), Some(value)) = (baseline, scenario) else {
        return 0;
    };
    let shift = base.saturating_sub(value).unsigned_abs();
    if shift >= max_shift {
        0
    } else {
        (1_000_u64.saturating_sub(shift.saturating_mul(1_000) / max_shift.max(1))) as u16
    }
}

fn validate_output(output: &DropoutStressAnalysis) -> Result<(), DropoutStressError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || !canonical(&output.scenario_order)
        || !canonical(&output.stable_scenario_order)
        || !canonical(&output.degraded_scenario_order)
        || !canonical(&output.contradictory_scenario_order)
        || !canonical(&output.unresolved_scenario_order)
        || !canonical(&output.acquisition_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.worst_case_stability_milli > 1_000
        || output.scenarios.len() != output.scenario_order.len()
        || output
            .scenarios
            .windows(2)
            .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
        || output.scenarios.iter().any(|scenario| {
            !canonical(&scenario.missing_modality_order)
                || !canonical(&scenario.available_modality_order)
                || scenario.stability_milli > 1_000
                || scenario.next_action.trim().is_empty()
                || !canonical(&scenario.negative_evidence)
                || !canonical(&scenario.uncertainty)
        })
    {
        return Err(DropoutStressError::InvalidOutput(
            "identity, ordering, scenario cardinality, stability, or next-action invariants are invalid".into(),
        ));
    }
    let scenario_ids = output
        .scenario_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let result_ids = output
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<BTreeSet<_>>();
    if scenario_ids != result_ids {
        return Err(DropoutStressError::InvalidOutput(
            "scenario order does not reconcile with scenario results".into(),
        ));
    }
    let mut partitions = BTreeSet::new();
    for order in [
        &output.stable_scenario_order,
        &output.degraded_scenario_order,
        &output.contradictory_scenario_order,
        &output.unresolved_scenario_order,
    ] {
        for scenario_id in order.iter() {
            if !scenario_ids.contains(scenario_id) || !partitions.insert(scenario_id) {
                return Err(DropoutStressError::InvalidOutput(
                    "scenario disposition orders do not partition the scenario set".into(),
                ));
            }
        }
    }
    if partitions.len() != scenario_ids.len() {
        return Err(DropoutStressError::InvalidOutput(
            "scenario disposition orders omit one or more scenarios".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| DropoutStressError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(DropoutStressError::InvalidOutput(
            "digest is not bound to dropout stress output".into(),
        ));
    }
    Ok(())
}

impl DropoutStressAnalysis {
    pub fn validate(&self) -> Result<(), DropoutStressError> {
        validate_output(self)
    }
}

/// Stress a multimodal endpoint against declared modality dropout and quality failure scenarios.
pub fn analyze_glioma_multimodal_dropout_stress(
    request: &DropoutStressRequest,
) -> Result<DropoutStressAnalysis, DropoutStressError> {
    validate_request(request)?;
    let signal_by_modality = request
        .signals
        .iter()
        .map(|signal| (signal.modality, signal))
        .collect::<BTreeMap<_, _>>();
    let all_signals = request.signals.iter().collect::<Vec<_>>();
    let baseline_signal_milli = aggregate_signal(&all_signals, request.min_quality_milli);
    let modality_order = signal_by_modality.keys().copied().collect::<Vec<_>>();
    let scenario_order = request
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut results = Vec::new();
    let mut stable = Vec::new();
    let mut degraded = Vec::new();
    let mut contradictory = Vec::new();
    let mut unresolved = Vec::new();
    let mut acquisition = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for scenario in &request.scenarios {
        let missing = scenario
            .missing_modality_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let available = modality_order
            .iter()
            .copied()
            .filter(|modality| !missing.contains(modality))
            .collect::<Vec<_>>();
        let available_signals = available
            .iter()
            .filter_map(|modality| signal_by_modality.get(modality).copied())
            .collect::<Vec<_>>();
        let aggregate = aggregate_signal(&available_signals, request.min_quality_milli);
        let required_missing = request
            .signals
            .iter()
            .filter(|signal| signal.required_for_endpoint && missing.contains(&signal.modality))
            .map(|signal| signal.modality)
            .collect::<Vec<_>>();
        let delta = baseline_signal_milli
            .zip(aggregate)
            .map(|(base, value)| value.saturating_sub(base));
        let stability_milli = stability(
            baseline_signal_milli,
            aggregate,
            request.max_allowed_shift_milli,
        );
        let sign_changed = baseline_signal_milli
            .zip(aggregate)
            .map(|(base, value)| base != 0 && value != 0 && base.signum() != value.signum())
            .unwrap_or(false);
        let (disposition, next_action) = if aggregate.is_none() || !required_missing.is_empty() {
            unresolved.push(scenario.scenario_id.clone());
            for modality in &required_missing {
                acquisition.insert(*modality);
            }
            uncertainty.insert(format!(
                "{}:required-modality-missing",
                scenario.scenario_id
            ));
            (
                DropoutScenarioDisposition::Unresolved,
                format!(
                    "restore required modalities for scenario {} before interpretation",
                    scenario.scenario_id
                ),
            )
        } else if sign_changed {
            contradictory.push(scenario.scenario_id.clone());
            negative_evidence.insert(format!(
                "{}:sign-reversal-under-dropout",
                scenario.scenario_id
            ));
            for modality in &scenario.missing_modality_order {
                acquisition.insert(*modality);
            }
            (
                DropoutScenarioDisposition::Contradictory,
                format!(
                    "reconcile sign reversal in scenario {} before downstream analysis",
                    scenario.scenario_id
                ),
            )
        } else if stability_milli < request.min_stability_milli
            || delta.map(|value| value.unsigned_abs()).unwrap_or(u64::MAX)
                > request.max_allowed_shift_milli
        {
            degraded.push(scenario.scenario_id.clone());
            for modality in &scenario.missing_modality_order {
                acquisition.insert(*modality);
            }
            uncertainty.insert(format!(
                "{}:endpoint-shift-exceeds-stability-gate",
                scenario.scenario_id
            ));
            (
                DropoutScenarioDisposition::Degraded,
                format!(
                    "acquire or QC missing modalities for scenario {}",
                    scenario.scenario_id
                ),
            )
        } else {
            stable.push(scenario.scenario_id.clone());
            (
                DropoutScenarioDisposition::Stable,
                format!(
                    "scenario {} remains within multimodal stability gate",
                    scenario.scenario_id
                ),
            )
        };
        results.push(DropoutScenarioResult {
            scenario_id: scenario.scenario_id.clone(),
            missing_modality_order: scenario.missing_modality_order.clone(),
            available_modality_order: available,
            aggregate_signal_milli: aggregate,
            delta_from_baseline_milli: delta,
            stability_milli,
            disposition,
            negative_evidence: if matches!(disposition, DropoutScenarioDisposition::Contradictory) {
                vec![format!(
                    "{}:sign-reversal-under-dropout",
                    scenario.scenario_id
                )]
            } else {
                Vec::new()
            },
            uncertainty: if matches!(disposition, DropoutScenarioDisposition::Unresolved) {
                vec![format!(
                    "{}:required-modality-missing",
                    scenario.scenario_id
                )]
            } else if matches!(disposition, DropoutScenarioDisposition::Degraded) {
                vec![format!(
                    "{}:endpoint-shift-exceeds-stability-gate",
                    scenario.scenario_id
                )]
            } else {
                Vec::new()
            },
            next_action,
        });
    }
    results.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    stable.sort();
    degraded.sort();
    contradictory.sort();
    unresolved.sort();
    let worst_case_stability_milli = results
        .iter()
        .map(|scenario| scenario.stability_milli)
        .min()
        .unwrap_or(0);
    let disposition = if !contradictory.is_empty() {
        DropoutStressDisposition::Blocked
    } else if !unresolved.is_empty() {
        DropoutStressDisposition::Unresolved
    } else if !degraded.is_empty() {
        DropoutStressDisposition::Conditional
    } else {
        DropoutStressDisposition::Stable
    };
    let mut output = DropoutStressAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        modality_order,
        baseline_signal_milli,
        scenario_order,
        scenarios: results,
        stable_scenario_order: stable,
        degraded_scenario_order: degraded,
        contradictory_scenario_order: contradictory,
        unresolved_scenario_order: unresolved,
        worst_case_stability_milli,
        acquisition_order: acquisition.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-dropout-stress"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DropoutStressError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(scenarios: Vec<DropoutScenario>) -> DropoutStressRequest {
        DropoutStressRequest {
            objective: "stress invasion endpoint under missing modalities".into(),
            endpoint_id: "invasion".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            signals: vec![
                DropoutModalitySignal {
                    modality: GliomaModality::Imaging,
                    signal_milli: 500,
                    quality_milli: 900,
                    feature_count: 100,
                    required_for_endpoint: false,
                },
                DropoutModalitySignal {
                    modality: GliomaModality::Spatial,
                    signal_milli: 520,
                    quality_milli: 900,
                    feature_count: 80,
                    required_for_endpoint: true,
                },
            ],
            scenarios,
            min_quality_milli: 700,
            max_allowed_shift_milli: 150,
            min_stability_milli: 700,
        }
    }

    #[test]
    fn dropout_stress_releases_stable_scenario() {
        let output = analyze_glioma_multimodal_dropout_stress(&request(vec![DropoutScenario {
            scenario_id: "complete".into(),
            missing_modality_order: Vec::new(),
            weight_milli: 1_000,
        }]))
        .unwrap();
        assert_eq!(output.disposition, DropoutStressDisposition::Stable);
        assert_eq!(output.stable_scenario_order, vec!["complete"]);
        output.validate().unwrap();
    }

    #[test]
    fn dropout_stress_preserves_required_missing_modality() {
        let output = analyze_glioma_multimodal_dropout_stress(&request(vec![DropoutScenario {
            scenario_id: "spatial-missing".into(),
            missing_modality_order: vec![GliomaModality::Spatial],
            weight_milli: 1_000,
        }]))
        .unwrap();
        assert_eq!(output.disposition, DropoutStressDisposition::Unresolved);
        assert_eq!(output.acquisition_order, vec![GliomaModality::Spatial]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("required")));
    }
}
